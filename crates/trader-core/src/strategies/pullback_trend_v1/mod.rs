//! Estratégia: Pullback em Tendência de Alta v1 (High 2).
//!
//! Baseada em *Trading Price Action Trends*, de Al Brooks.
//! Busca entradas de compra em tendência de alta, após pullback de duas pernas
//! e barra de sinal bullish.

pub mod config;
pub mod context;
pub mod entry;
pub mod setup;

use rust_decimal::Decimal;
use tracing::{debug, info};

use crate::context::MarketContextAnalyzer;
use crate::strategies::pullback_trend_v1::config::PullbackTrendV1Config;
use crate::strategies::pullback_trend_v1::context::check_context;
use crate::strategies::pullback_trend_v1::entry::build_signal;
use crate::strategies::pullback_trend_v1::setup::SetupResult;
use trader_domain::{
    Candle, MarketContext, RejectionReason, SignalResult, Strategy as StrategyTrait, StrategyId,
    StrategyState, TimeFrame,
};

/// Estratégia pullback em tendência de alta.
#[derive(Debug, Clone)]
pub struct PullbackTrendV1 {
    config: PullbackTrendV1Config,
    analyzer: MarketContextAnalyzer,
}

impl PullbackTrendV1 {
    pub fn new(config: PullbackTrendV1Config) -> Self {
        let params = &config.strategy.parameters;
        let analyzer_config = crate::context::ContextAnalyzerConfig {
            ema_period: params.ema_context_period,
            sma_period: params.sma_context_period,
            atr_period: 14,
            volume_period: 20,
            high_volatility_threshold_pct: params.max_atr_pct,
            low_volatility_threshold_pct: Decimal::from(3) / Decimal::from(10),
        };
        let analyzer = MarketContextAnalyzer::new(analyzer_config);
        Self { config, analyzer }
    }

    /// Carrega a configuração a partir de uma string TOML.
    pub fn from_toml(toml_str: &str) -> Result<Self, toml::de::Error> {
        let config: PullbackTrendV1Config = toml::from_str(toml_str)?;
        Ok(Self::new(config))
    }

    /// Retorna referência para os parâmetros da estratégia.
    pub fn parameters(&self) -> &config::StrategyParameters {
        &self.config.strategy.parameters
    }

    /// Analisa uma série de candles e retorna sinal ou rejeição.
    pub fn analyze_candles(&self, symbol: &str, candles: &[Candle]) -> SignalResult {
        let timeframe = match self
            .config
            .strategy
            .parameters
            .operational_timeframe
            .parse::<TimeFrame>()
        {
            Ok(tf) => tf,
            Err(_) => {
                return SignalResult::Rejected {
                    reason: RejectionReason::IncompleteSetup,
                    details: Some(serde_json::json!({ "reason": "invalid operational timeframe" })),
                }
            }
        };

        let ctx = match self.analyzer.analyze(symbol, timeframe, candles) {
            Some(ctx) => ctx,
            None => {
                return SignalResult::Rejected {
                    reason: RejectionReason::IncompleteSetup,
                    details: Some(
                        serde_json::json!({ "reason": "unable to compute market context" }),
                    ),
                }
            }
        };

        match check_context(&ctx, &self.config.strategy.parameters) {
            context::ContextCheck::Rejected(reason, details) => {
                debug!(?reason, "contexto rejeitado");
                return SignalResult::Rejected {
                    reason,
                    details: Some(details),
                };
            }
            context::ContextCheck::Approved => {}
        }

        match setup::detect_setup(candles, &self.config.strategy.parameters) {
            SetupResult::Found(setup) => {
                info!(
                    entry = %setup.entry_price,
                    stop = %setup.stop_price,
                    target = %setup.target_price,
                    "setup de pullback detectado"
                );

                let signal = build_signal(
                    symbol,
                    timeframe,
                    &setup,
                    &ctx,
                    &self.config.strategy.id,
                    &self.config.strategy.version,
                    self.config.config_hash(),
                );

                SignalResult::Signal(signal)
            }
            SetupResult::NotFound(reason, details) => SignalResult::Rejected {
                reason,
                details: Some(details),
            },
        }
    }
}

impl StrategyTrait for PullbackTrendV1 {
    fn id(&self) -> StrategyId {
        StrategyId::new(&self.config.strategy.id, &self.config.strategy.version)
    }

    fn name(&self) -> &'static str {
        "Pullback em Tendência de Alta v1"
    }

    fn source(&self) -> &'static str {
        "Al Brooks - Trading Price Action Trends"
    }

    fn version(&self) -> &'static str {
        "1.0.0"
    }

    fn analyze(
        &self,
        ctx: &MarketContext,
        _state: &StrategyState,
        candles: &[Candle],
    ) -> SignalResult {
        if candles.is_empty() {
            return SignalResult::Rejected {
                reason: RejectionReason::IncompleteSetup,
                details: Some(serde_json::json!({ "reason": "empty candle series" })),
            };
        }

        self.analyze_candles(&ctx.symbol, candles)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};
    use rust_decimal::Decimal;

    fn candle(
        timestamp: chrono::DateTime<Utc>,
        open: Decimal,
        high: Decimal,
        low: Decimal,
        close: Decimal,
    ) -> Candle {
        Candle::new(
            "SPY",
            TimeFrame::M15,
            timestamp,
            open,
            high,
            low,
            close,
            Decimal::from(1000),
        )
        .expect("candle válido")
    }

    fn make_uptrend_candles(signal_close: Decimal) -> Vec<Candle> {
        // Começa às 14:30 UTC (abertura do pregão) para caber dentro do horário regular.
        let base = Utc.with_ymd_and_hms(2026, 7, 2, 14, 30, 0).unwrap();
        let mut candles = Vec::new();

        // Série de alta forte com 60 candles para formar contexto de uptrend robusto.
        // Usamos intervalos de 5 minutos para caber dentro do horário regular.
        for i in 0..60 {
            let close = Decimal::from(400 + i);
            candles.push(candle(
                base + chrono::Duration::minutes(i as i64 * 5),
                close - Decimal::ONE,
                close + Decimal::ONE,
                close - Decimal::ONE,
                close,
            ));
        }

        // Nova máxima em 481.
        let last_ts = candles.last().unwrap().timestamp;
        candles.push(candle(
            last_ts + chrono::Duration::minutes(5),
            Decimal::from(480),
            Decimal::from(482),
            Decimal::from(479),
            Decimal::from(481),
        ));

        // Pullback raso: dois candles de baixa leve.
        let last_ts = candles.last().unwrap().timestamp;
        candles.push(candle(
            last_ts + chrono::Duration::minutes(5),
            Decimal::from(481),
            Decimal::from(481),
            Decimal::from(479),
            Decimal::from(479),
        ));

        // Segunda perna do pullback com mínima menor que a da barra de sinal.
        let last_ts = candles.last().unwrap().timestamp;
        candles.push(candle(
            last_ts + chrono::Duration::minutes(5),
            Decimal::from(479),
            Decimal::from(480),
            Decimal::from(476),
            Decimal::from(477),
        ));

        // Barra de sinal bullish com sombra inferior grande e fechamento forte.
        // low=477, open=478, close=signal_close, high=481 -> fechamento no terço superior.
        let last_ts = candles.last().unwrap().timestamp;
        candles.push(candle(
            last_ts + chrono::Duration::minutes(5),
            Decimal::from(478),
            Decimal::from(481),
            Decimal::from(477),
            signal_close,
        ));

        candles
    }

    #[test]
    fn perfect_setup_generates_buy_signal() {
        let candles = make_uptrend_candles(Decimal::from(480));
        let strategy = PullbackTrendV1::new(PullbackTrendV1Config::default());

        match strategy.analyze_candles("SPY", &candles) {
            SignalResult::Signal(signal) => {
                assert_eq!(signal.direction, trader_domain::Direction::Long);
                assert!(signal.entry_price.is_some());
                assert!(signal.stop_price.is_some());
                assert!(signal.target_price.is_some());

                let snapshot = &signal.market_snapshot;
                assert!(
                    snapshot.get("ema_20").is_some(),
                    "market_snapshot deve conter ema_20"
                );
                assert!(
                    snapshot.get("trend_state").is_some(),
                    "market_snapshot deve conter trend_state"
                );
                assert!(
                    snapshot.get("signal_bar_index").is_some(),
                    "market_snapshot deve conter signal_bar_index"
                );
            }
            SignalResult::Rejected { reason, details } => {
                panic!("esperado sinal, rejeitado por {:?}: {:?}", reason, details)
            }
            _ => panic!("esperado sinal"),
        }
    }

    #[test]
    fn no_uptrend_rejects_signal() {
        let base = Utc.with_ymd_and_hms(2026, 7, 2, 15, 0, 0).unwrap();
        let mut candles = Vec::new();

        // Série lateral/baixista.
        for i in 0..10 {
            let close = Decimal::from(400);
            candles.push(candle(
                base + chrono::Duration::minutes(i as i64 * 15),
                close - Decimal::ONE,
                close + Decimal::ONE,
                close - Decimal::ONE,
                close,
            ));
        }

        let strategy = PullbackTrendV1::new(PullbackTrendV1Config::default());
        match strategy.analyze_candles("SPY", &candles) {
            SignalResult::Rejected { reason, .. } => {
                assert!(
                    matches!(
                        reason,
                        RejectionReason::NoContext | RejectionReason::IncompleteSetup
                    ),
                    "esperado rejeição por contexto, obtido {:?}",
                    reason
                );
            }
            _ => panic!("esperado rejeição"),
        }
    }

    #[test]
    fn outside_trading_hours_rejects() {
        let mut candles = make_uptrend_candles(Decimal::from(480));
        // Força timestamp fora do horário de pregão.
        for c in &mut candles {
            c.timestamp = Utc.with_ymd_and_hms(2026, 7, 2, 3, 0, 0).unwrap();
        }

        let strategy = PullbackTrendV1::new(PullbackTrendV1Config::default());
        match strategy.analyze_candles("SPY", &candles) {
            SignalResult::Rejected { reason, .. } => {
                assert_eq!(reason, RejectionReason::OutsideTradingHours);
            }
            _ => panic!("esperado rejeição por horário"),
        }
    }
}
