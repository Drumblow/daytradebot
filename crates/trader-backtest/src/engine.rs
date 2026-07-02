//! Engine de backtest determinístico.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use tracing::{debug, info, warn};

use trader_adapters::simulated::{SimulatedBroker, SimulatedBrokerConfig};
use trader_core::{
    context::MarketContextAnalyzer,
    execution::{ExecutionEngine, ExecutionResult},
    risk::{RiskConfig, RiskManager, RiskState},
};
use trader_domain::{Broker, Candle, SignalResult, Strategy, TradingMode};

/// Configuração de uma execução de backtest.
#[derive(Debug, Clone)]
pub struct BacktestConfig {
    /// Símbolo do ativo.
    pub symbol: String,
    /// Capital inicial.
    pub initial_capital: Decimal,
    /// Comissão por trade (entrada + saída).
    pub commission_per_trade: Decimal,
    /// Slippage percentual aplicado no preço de execução.
    pub slippage_pct: Decimal,
}

impl Default for BacktestConfig {
    fn default() -> Self {
        Self {
            symbol: "SPY".to_string(),
            initial_capital: Decimal::from(100_000),
            commission_per_trade: Decimal::from(35) / Decimal::from(100),
            slippage_pct: Decimal::from(1) / Decimal::from(1000), // 0.1%
        }
    }
}

/// Resultado bruto de uma execução de backtest.
#[derive(Debug, Clone)]
pub struct BacktestRun {
    pub symbol: String,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub initial_capital: Decimal,
    pub final_equity: Decimal,
    pub total_trades: usize,
    pub closed_trades: Vec<trader_domain::Trade>,
    pub daily_pnl_series: Vec<(DateTime<Utc>, Decimal)>,
}

/// Engine de backtest.
#[derive(Debug, Clone)]
pub struct BacktestEngine {
    config: BacktestConfig,
    broker: SimulatedBroker,
    execution_engine: ExecutionEngine,
    risk_state: RiskState,
    closed_trades: Vec<trader_domain::Trade>,
    daily_equity: Vec<(DateTime<Utc>, Decimal)>,
}

impl BacktestEngine {
    /// Cria uma nova engine de backtest.
    pub fn new(config: BacktestConfig, risk_config: RiskConfig) -> Self {
        let broker = SimulatedBroker::new(SimulatedBrokerConfig {
            account_id: Some("BACKTEST".to_string()),
            initial_cash: config.initial_capital,
            commission_per_trade: config.commission_per_trade,
            slippage_pct: config.slippage_pct,
        });

        let risk_manager = RiskManager::new(risk_config);
        let execution_engine = ExecutionEngine::new(risk_manager);

        Self {
            config,
            broker,
            execution_engine,
            risk_state: RiskState::default(),
            closed_trades: Vec::new(),
            daily_equity: Vec::new(),
        }
    }

    /// Executa o backtest sobre uma série de candles.
    ///
    /// A estratégia recebe apenas candles até o índice atual, evitando
    /// lookahead bias.
    pub async fn run<S: Strategy>(
        &mut self,
        strategy: &S,
        candles: &[Candle],
    ) -> anyhow::Result<BacktestRun> {
        if candles.is_empty() {
            anyhow::bail!("série de candles vazia");
        }

        let symbol = self.config.symbol.clone();
        let analyzer =
            MarketContextAnalyzer::new(trader_core::context::ContextAnalyzerConfig::default());

        let start_time = candles
            .first()
            .map(|c| c.timestamp)
            .unwrap_or_else(Utc::now);
        let end_time = candles.last().map(|c| c.timestamp).unwrap_or_else(Utc::now);

        for (idx, candle) in candles.iter().enumerate() {
            // Atualiza preço de mercado para possíveis execuções de stop/alvo.
            self.broker.set_market_price(&symbol, candle.close);

            // Registra equity no fechamento de cada candle para série diária.
            if let Ok(summary) = self.broker.get_account_summary().await {
                self.daily_equity.push((candle.timestamp, summary.equity));
            }

            // Sincroniza trades fechados antes de avaliar novo sinal.
            let newly_closed = self.sync_closed_trades();
            self.closed_trades.extend(newly_closed);

            // Se já houver posição aberta, não busca novo sinal.
            let has_position = self
                .broker
                .get_position(&symbol)
                .await
                .map(|p| p.is_some())
                .unwrap_or(false);

            if has_position {
                debug!(idx, "posição aberta; pulando análise de sinal");
                continue;
            }

            // Série histórica até o candle atual (inclusive).
            let history = &candles[..=idx];

            let ctx = match analyzer.analyze(&symbol, candle.timeframe, history) {
                Some(ctx) => ctx,
                None => continue,
            };

            match strategy.analyze(&ctx, &Default::default(), history) {
                SignalResult::Signal(signal) => {
                    let capital = self
                        .broker
                        .get_account_summary()
                        .await
                        .map(|s| s.equity)
                        .unwrap_or(self.config.initial_capital);

                    let result = self
                        .execution_engine
                        .process_signal(
                            &self.broker,
                            &signal,
                            &ctx,
                            None,
                            &self.risk_state,
                            capital,
                        )
                        .await;

                    match result {
                        ExecutionResult::Executed {
                            order_id,
                            position_size,
                            ..
                        } => {
                            info!(
                                idx,
                                %order_id,
                                %position_size,
                                entry = ?signal.entry_price,
                                stop = ?signal.stop_price,
                                target = ?signal.target_price,
                                "entrada executada no backtest"
                            );
                        }
                        ExecutionResult::RejectedByRisk { reason, detail } => {
                            debug!(?reason, %detail, "sinal rejeitado pelo risk manager no backtest");
                        }
                        ExecutionResult::RejectedByBroker { error } => {
                            warn!(%error, "broker simulado rejeitou ordem no backtest");
                        }
                    }
                }
                SignalResult::Rejected { reason, details } => {
                    debug!(?reason, ?details, "setup rejeitado no backtest");
                }
                _ => {}
            }
        }

        // Sincroniza trades fechados no último candle.
        let newly_closed = self.sync_closed_trades();
        self.closed_trades.extend(newly_closed);

        let summary = self.broker.get_account_summary().await?;
        let mut closed_trades = self.closed_trades.clone();
        closed_trades.extend(self.broker.get_closed_trades());

        Ok(BacktestRun {
            symbol,
            start_time,
            end_time,
            initial_capital: self.config.initial_capital,
            final_equity: summary.equity,
            total_trades: closed_trades.len(),
            closed_trades,
            daily_pnl_series: self.daily_equity.clone(),
        })
    }

    fn sync_closed_trades(&mut self) -> Vec<trader_domain::Trade> {
        let trades = self.broker.get_closed_trades();
        if trades.is_empty() {
            return Vec::new();
        }

        let pnls: Vec<Decimal> = trades.iter().map(|t| t.net_pnl).collect();
        self.execution_engine
            .sync_risk_state(&mut self.risk_state, &pnls);
        self.broker.clear_closed_trades();
        trades
    }
}

/// Constrói uma configuração de risco padrão para backtest.
pub fn default_backtest_risk_config() -> RiskConfig {
    RiskConfig {
        trading_mode: TradingMode::Paper,
        risk_per_trade_pct: Decimal::from(1),
        max_daily_loss_pct: Decimal::from(2),
        max_trades_per_day: 100, // ilimitado para backtest
        max_consecutive_losses: 100,
        min_risk_reward: Decimal::from(2),
        max_spread_pct: Decimal::from(5) / Decimal::from(10000),
        max_atr_pct: Decimal::from(15) / Decimal::from(10),
        trading_start_time_utc: (0, 0, 0),
        trading_end_time_utc: (23, 59, 59),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};
    use rust_decimal::Decimal;
    use trader_core::strategies::pullback_trend_v1::PullbackTrendV1;
    use trader_domain::{Candle, TimeFrame};

    fn candle(
        symbol: &str,
        timestamp: chrono::DateTime<Utc>,
        open: Decimal,
        high: Decimal,
        low: Decimal,
        close: Decimal,
    ) -> Candle {
        Candle::new(
            symbol,
            TimeFrame::M5,
            timestamp,
            open,
            high,
            low,
            close,
            Decimal::from(1000),
        )
        .expect("candle válido")
    }

    fn generate_test_series(symbol: &str) -> Vec<Candle> {
        let base = Utc
            .with_ymd_and_hms(2026, 7, 2, 14, 30, 0)
            .single()
            .unwrap();
        let mut candles = Vec::new();

        // Tendência de alta.
        for i in 0..60 {
            let close = Decimal::from(400 + i);
            candles.push(candle(
                symbol,
                base + chrono::Duration::minutes(i as i64 * 5),
                close - Decimal::ONE,
                close + Decimal::ONE,
                close - Decimal::ONE,
                close,
            ));
        }

        // Nova máxima + pullback + barra de sinal.
        let last = candles.last().unwrap().timestamp;
        candles.push(candle(
            symbol,
            last + chrono::Duration::minutes(5),
            Decimal::from(459),
            Decimal::from(461),
            Decimal::from(458),
            Decimal::from(460),
        ));
        candles.push(candle(
            symbol,
            last + chrono::Duration::minutes(10),
            Decimal::from(460),
            Decimal::from(460),
            Decimal::from(456),
            Decimal::from(456),
        ));
        candles.push(candle(
            symbol,
            last + chrono::Duration::minutes(15),
            Decimal::from(456),
            Decimal::from(457),
            Decimal::from(455),
            Decimal::from(455),
        ));
        candles.push(candle(
            symbol,
            last + chrono::Duration::minutes(20),
            Decimal::from(457),
            Decimal::from(460),
            Decimal::from(456),
            Decimal::from(459),
        ));

        // Continuação até atingir alvo.
        let mut last_close = Decimal::from(459);
        for i in 1..=20 {
            last_close += Decimal::ONE;
            candles.push(candle(
                symbol,
                last + chrono::Duration::minutes((20 + i * 5) as i64),
                last_close - Decimal::ONE,
                last_close + Decimal::ONE,
                last_close - Decimal::ONE,
                last_close,
            ));
        }

        candles
    }

    #[tokio::test]
    async fn backtest_closes_trade_on_take_profit() {
        let candles = generate_test_series("SPY");
        let strategy = PullbackTrendV1::new(
            trader_core::strategies::pullback_trend_v1::config::PullbackTrendV1Config::default(),
        );
        let mut engine =
            BacktestEngine::new(BacktestConfig::default(), default_backtest_risk_config());

        let run = engine.run(&strategy, &candles).await.unwrap();

        assert!(
            !run.closed_trades.is_empty(),
            "esperado pelo menos um trade fechado"
        );
        assert_eq!(
            run.closed_trades[0].exit_reason,
            trader_domain::ExitReason::Target
        );
    }
}
