//! Motor de execução de sinais.
//!
//! A `ExecutionEngine` orquestra a fase crítica de entrada:
//! validação de risco, construção da ordem (incluindo bracket) e envio ao broker.
//!
//! Ela não gerencia o ciclo de vida da posição após a entrada — isso é
//! responsabilidade do worker de paper trading ou da engine de backtest,
//! que devem chamar `RiskManager::update_state` com o P&L real quando a
//! posição for fechada.

pub mod time_exit;
pub mod trade_tracker;

use rust_decimal::Decimal;
use tracing::{debug, info, warn};

use crate::risk::{RiskCheck, RiskManager, RiskState};
use trader_domain::{
    Broker, BrokerError, MarketContext, Order, OrderSide, OrderType, Quote, RejectionReason, Signal,
};

/// Resultado da tentativa de execução de um sinal.
#[derive(Debug, Clone)]
pub enum ExecutionResult {
    /// Ordem enviada com sucesso.
    Executed {
        order_id: String,
        position_size: Decimal,
        risk_amount: Decimal,
    },
    /// Sinal rejeitado pelo risk manager.
    RejectedByRisk {
        reason: RejectionReason,
        detail: String,
    },
    /// Ordem rejeitada pelo broker.
    RejectedByBroker { error: String },
}

/// Motor de execução de sinais de trading.
#[derive(Debug, Clone)]
pub struct ExecutionEngine {
    risk_manager: RiskManager,
}

impl ExecutionEngine {
    /// Cria uma nova engine de execução.
    pub fn new(risk_manager: RiskManager) -> Self {
        Self { risk_manager }
    }

    /// Processa um sinal validado: aplica regras de risco e envia ordem ao broker.
    ///
    /// `reference_price` é o preço mais fresco que o chamador conhece no
    /// momento do envio (live: close da barra em formação do último fetch;
    /// backtest/replay: close da barra do sinal). Alimenta a guarda de
    /// overshoot (ADR-015).
    #[allow(clippy::too_many_arguments)]
    pub async fn process_signal<B: Broker>(
        &self,
        broker: &B,
        signal: &Signal,
        ctx: &MarketContext,
        quote: Option<&Quote>,
        reference_price: Option<Decimal>,
        risk_state: &RiskState,
        capital: Decimal,
    ) -> ExecutionResult {
        // Invariante de segurança: nunca abrir posição se já houver uma no
        // mesmo ativo — garantido na engine, não depende do caller.
        match broker.get_position(&signal.symbol).await {
            Ok(Some(_)) => {
                return ExecutionResult::RejectedByRisk {
                    reason: RejectionReason::PositionAlreadyOpen,
                    detail: "já existe posição aberta no ativo".to_string(),
                };
            }
            Ok(None) => {}
            Err(e) => {
                // Falha ao confirmar ausência de posição: fail closed.
                return ExecutionResult::RejectedByBroker {
                    error: format!("falha ao consultar posições: {e}"),
                };
            }
        }

        // Guarda de overshoot (ADR-015): se o preço já correu além do gatilho
        // de uma entrada stop mais do que a tolerância (fração da distância do
        // stop), a ordem não é enviada — o fill viria a mercado, o risco real
        // não seria o desenhado (trade 12 do live: overshoot 0.38 num stop de
        // 0.35 dobrou o risco). Só vale para entrada stop; um limit nunca
        // enche pior que o próprio preço.
        if signal.entry_order_type == trader_domain::EntryOrderType::Stop {
            if let (Some(reference), Some(entry), Some(stop)) =
                (reference_price, signal.entry_price, signal.stop_price)
            {
                let stop_distance = (entry - stop).abs();
                let overshoot = match signal.direction {
                    trader_domain::Direction::Long => reference - entry,
                    trader_domain::Direction::Short => entry - reference,
                };
                let tolerance = self.risk_manager.config().entry_overshoot_tolerance;
                if stop_distance > Decimal::ZERO && overshoot > tolerance * stop_distance {
                    warn!(
                        %reference, %entry, %stop, %overshoot, %tolerance,
                        "entrada stop invalidada: preço já passou do gatilho além da tolerância"
                    );
                    return ExecutionResult::RejectedByRisk {
                        reason: RejectionReason::SetupInvalidated,
                        detail: format!(
                            "overshoot {overshoot} além do gatilho {entry} (referência {reference}) \
                             excede {tolerance} × distância do stop {stop_distance}"
                        ),
                    };
                }
            }
        }

        match self
            .risk_manager
            .validate(signal, ctx, quote, risk_state, capital)
        {
            RiskCheck::Approved {
                position_size,
                risk_amount,
            } => {
                let order = match build_bracket_order(signal, position_size) {
                    Ok(order) => order,
                    Err(e) => {
                        return ExecutionResult::RejectedByBroker {
                            error: e.to_string(),
                        };
                    }
                };

                match broker.place_order(order).await {
                    Ok(id) => {
                        info!(%id, %position_size, "ordem de entrada enviada");
                        ExecutionResult::Executed {
                            order_id: id.to_string(),
                            position_size,
                            risk_amount,
                        }
                    }
                    Err(e) => {
                        warn!(error = %e, "broker rejeitou ordem de entrada");
                        ExecutionResult::RejectedByBroker {
                            error: e.to_string(),
                        }
                    }
                }
            }
            RiskCheck::Rejected(reason, detail) => {
                debug!(?reason, %detail, "sinal rejeitado pelo risk manager");
                ExecutionResult::RejectedByRisk { reason, detail }
            }
        }
    }

    /// Atualiza o estado de risco com base em trades fechados.
    ///
    /// O chamador é responsável por fornecer o P&L líquido de cada trade fechado
    /// desde a última sincronização. Isso mantém a engine independente de como o
    /// broker armazena ou expõe o histórico.
    pub fn sync_risk_state(&self, risk_state: &mut RiskState, closed_pnls: &[Decimal]) {
        for pnl in closed_pnls {
            self.risk_manager.update_state(risk_state, *pnl);
        }
    }
}

/// Constrói uma ordem bracket (entrada + stop + alvo) a partir de um sinal.
fn build_bracket_order(signal: &Signal, position_size: Decimal) -> Result<Order, BrokerError> {
    let side = match signal.direction {
        trader_domain::Direction::Long => OrderSide::Buy,
        trader_domain::Direction::Short => OrderSide::Sell,
    };

    let mut order = Order::new(
        &signal.symbol,
        side,
        OrderType::Bracket,
        position_size,
        "simulated",
    )
    .map_err(|e| BrokerError::OrderRejected(e.to_string()))?;

    order.signal_id = None;
    order.entry_order_type = signal.entry_order_type;
    order.price = signal.entry_price;
    order.stop_price = signal.stop_price;
    order.target_price = signal.target_price;
    order.time_in_force = trader_domain::TimeInForce::Day;
    order.metadata = serde_json::json!({
        "entry_order_type": signal.entry_order_type,
    });

    Ok(order)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::risk::RiskConfig;
    use async_trait::async_trait;
    use chrono::Utc;
    use rust_decimal::Decimal;
    use tokio::sync::mpsc::Sender;
    use trader_domain::{
        AccountSummary, BrokerError, Direction, MarketContext, MarketPhase, OrderEvent, OrderId,
        OrderStatus, Position, SignalStatus, SubscriptionHandle, TimeFrame, TrendState,
        VolatilityRegime,
    };

    /// Broker mock que aceita qualquer ordem e retorna IDs fixos.
    #[derive(Debug, Clone, Default)]
    struct MockBroker {
        accepted: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    }

    #[async_trait]
    impl Broker for MockBroker {
        async fn place_order(&self, _order: Order) -> Result<OrderId, BrokerError> {
            let n = self
                .accepted
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(OrderId::from(format!("mock-{n}")))
        }

        async fn cancel_order(&self, _id: &OrderId) -> Result<(), BrokerError> {
            Ok(())
        }

        async fn get_order_status(&self, _id: &OrderId) -> Result<OrderStatus, BrokerError> {
            Ok(OrderStatus::Filled)
        }

        async fn get_open_orders(&self) -> Result<Vec<Order>, BrokerError> {
            Ok(Vec::new())
        }

        async fn get_position(&self, _symbol: &str) -> Result<Option<Position>, BrokerError> {
            Ok(None)
        }

        async fn get_positions(&self) -> Result<Vec<Position>, BrokerError> {
            Ok(Vec::new())
        }

        async fn get_account_summary(&self) -> Result<AccountSummary, BrokerError> {
            Ok(AccountSummary {
                broker: "mock".to_string(),
                account_id: None,
                cash: Decimal::from(100_000),
                equity: Decimal::from(100_000),
                buying_power: Decimal::from(100_000),
                daily_pnl: Decimal::ZERO,
                timestamp: Utc::now(),
            })
        }

        async fn subscribe_order_events(
            &self,
            _tx: Sender<OrderEvent>,
        ) -> Result<SubscriptionHandle, BrokerError> {
            Ok(SubscriptionHandle {
                id: "mock".to_string(),
            })
        }
    }

    fn make_context(timestamp: chrono::DateTime<Utc>) -> MarketContext {
        MarketContext {
            symbol: "SPY".to_string(),
            timeframe: TimeFrame::M15,
            timestamp,
            candle_timestamp: Some(timestamp),
            trend_state: TrendState::Uptrend,
            volatility_regime: VolatilityRegime::Normal,
            market_phase: MarketPhase::Regular,
            ema_20: Some(Decimal::from(100)),
            ema_50: None,
            sma_200: None,
            atr_14: Some(Decimal::from(1)),
            atr_percent_14: Some(Decimal::from(1)),
            volume_relative: None,
            hh_hl_count: None,
            lh_ll_count: None,
            range_percent: None,
            is_tradeable: true,
            raw_values: serde_json::Value::Object(Default::default()),
        }
    }

    fn make_signal() -> Signal {
        Signal {
            symbol: "SPY".to_string(),
            strategy_id: "pullback-trend-v1".to_string(),
            strategy_version: "1.0.0".to_string(),
            config_hash: "abc".to_string(),
            timeframe: TimeFrame::M15,
            timestamp: Utc::now(),
            direction: Direction::Long,
            status: SignalStatus::Accepted,
            entry_order_type: trader_domain::EntryOrderType::Stop,
            entry_price: Some(Decimal::from(500)),
            stop_price: Some(Decimal::from(495)),
            target_price: Some(Decimal::from(510)),
            risk_reward_ratio: Some(Decimal::from(2)),
            risk_amount: None,
            risk_percent: None,
            position_size: None,
            entry_reason: None,
            rejection_reason: None,
            rejection_details: None,
            market_snapshot: serde_json::Value::Object(Default::default()),
            correlation_id: "corr".to_string(),
        }
    }

    #[tokio::test]
    async fn executes_valid_signal() {
        let broker = MockBroker::default();
        let engine = ExecutionEngine::new(RiskManager::new(RiskConfig::default()));
        let ctx = make_context(
            Utc::now()
                .date_naive()
                .and_hms_opt(15, 0, 0)
                .unwrap()
                .and_utc(),
        );
        let signal = make_signal();
        let risk_state = RiskState::default();

        let result = engine
            .process_signal(
                &broker,
                &signal,
                &ctx,
                None,
                None,
                &risk_state,
                Decimal::from(100_000),
            )
            .await;

        match result {
            ExecutionResult::Executed { .. } => {}
            other => panic!("esperado execução, obtido {:?}", other),
        }
    }

    #[tokio::test]
    async fn rejects_signal_with_poor_risk_reward() {
        let broker = MockBroker::default();
        let engine = ExecutionEngine::new(RiskManager::new(RiskConfig::default()));
        let ctx = make_context(
            Utc::now()
                .date_naive()
                .and_hms_opt(15, 0, 0)
                .unwrap()
                .and_utc(),
        );
        let mut signal = make_signal();
        signal.target_price = Some(Decimal::from(501)); // risco/retorno ruim
        let risk_state = RiskState::default();

        let result = engine
            .process_signal(
                &broker,
                &signal,
                &ctx,
                None,
                None,
                &risk_state,
                Decimal::from(100_000),
            )
            .await;

        match result {
            ExecutionResult::RejectedByRisk {
                reason: RejectionReason::PoorRiskReward,
                ..
            } => {}
            other => panic!("esperado rejeição por risco/retorno, obtido {:?}", other),
        }
    }

    /// Cenário do trade 12 do live (ADR-015): sinal com entry 500 / stop 495
    /// (distância 5). Tolerância default 0.25 ⇒ overshoot máximo 1.25.
    #[tokio::test]
    async fn rejects_stop_entry_when_price_ran_beyond_tolerance() {
        let broker = MockBroker::default();
        let engine = ExecutionEngine::new(RiskManager::new(RiskConfig::default()));
        let ctx = make_context(
            Utc::now()
                .date_naive()
                .and_hms_opt(15, 0, 0)
                .unwrap()
                .and_utc(),
        );
        let signal = make_signal();
        let risk_state = RiskState::default();

        let result = engine
            .process_signal(
                &broker,
                &signal,
                &ctx,
                None,
                Some(Decimal::from(502)), // 2.0 além do gatilho > 1.25 tolerado
                &risk_state,
                Decimal::from(100_000),
            )
            .await;

        match result {
            ExecutionResult::RejectedByRisk {
                reason: RejectionReason::SetupInvalidated,
                ..
            } => {}
            other => panic!(
                "esperado SetupInvalidated por overshoot, obtido {:?}",
                other
            ),
        }
    }

    #[tokio::test]
    async fn allows_stop_entry_within_overshoot_tolerance() {
        let broker = MockBroker::default();
        let engine = ExecutionEngine::new(RiskManager::new(RiskConfig::default()));
        let ctx = make_context(
            Utc::now()
                .date_naive()
                .and_hms_opt(15, 0, 0)
                .unwrap()
                .and_utc(),
        );
        let signal = make_signal();
        let risk_state = RiskState::default();

        let result = engine
            .process_signal(
                &broker,
                &signal,
                &ctx,
                None,
                Some(Decimal::from(501)), // 1.0 além do gatilho ≤ 1.25 tolerado
                &risk_state,
                Decimal::from(100_000),
            )
            .await;

        match result {
            ExecutionResult::Executed { .. } => {}
            other => panic!("esperado execução dentro da tolerância, obtido {:?}", other),
        }
    }

    /// Direção short: overshoot é o preço ABAIXO do gatilho de venda.
    #[tokio::test]
    async fn rejects_short_stop_entry_on_overshoot_below_trigger() {
        let broker = MockBroker::default();
        let engine = ExecutionEngine::new(RiskManager::new(RiskConfig::default()));
        let ctx = make_context(
            Utc::now()
                .date_naive()
                .and_hms_opt(15, 0, 0)
                .unwrap()
                .and_utc(),
        );
        let mut signal = make_signal();
        signal.direction = Direction::Short;
        signal.entry_price = Some(Decimal::from(500));
        signal.stop_price = Some(Decimal::from(505));
        signal.target_price = Some(Decimal::from(490));
        let risk_state = RiskState::default();

        let result = engine
            .process_signal(
                &broker,
                &signal,
                &ctx,
                None,
                Some(Decimal::from(498)), // 2.0 abaixo do gatilho > 1.25 tolerado
                &risk_state,
                Decimal::from(100_000),
            )
            .await;

        match result {
            ExecutionResult::RejectedByRisk {
                reason: RejectionReason::SetupInvalidated,
                ..
            } => {}
            other => panic!("esperado SetupInvalidated no short, obtido {:?}", other),
        }
    }
}
