//! Motor de execução de sinais.
//!
//! A `ExecutionEngine` orquestra a fase crítica de entrada:
//! validação de risco, construção da ordem (incluindo bracket) e envio ao broker.
//!
//! Ela não gerencia o ciclo de vida da posição após a entrada — isso é
//! responsabilidade do worker de paper trading ou da engine de backtest,
//! que devem chamar `RiskManager::update_state` com o P&L real quando a
//! posição for fechada.

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
    #[allow(clippy::too_many_arguments)]
    pub async fn process_signal<B: Broker>(
        &self,
        broker: &B,
        signal: &Signal,
        ctx: &MarketContext,
        quote: Option<&Quote>,
        risk_state: &RiskState,
        capital: Decimal,
    ) -> ExecutionResult {
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
    order.price = signal.entry_price;
    order.stop_price = signal.stop_price;
    order.target_price = signal.target_price;
    order.time_in_force = trader_domain::TimeInForce::Day;

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
        let ctx = make_context(Utc::now());
        let signal = make_signal();
        let risk_state = RiskState::default();

        let result = engine
            .process_signal(
                &broker,
                &signal,
                &ctx,
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
        let ctx = make_context(Utc::now());
        let mut signal = make_signal();
        signal.target_price = Some(Decimal::from(501)); // risco/retorno ruim
        let risk_state = RiskState::default();

        let result = engine
            .process_signal(
                &broker,
                &signal,
                &ctx,
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
}
