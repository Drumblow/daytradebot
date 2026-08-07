//! Estratégia: Opening Reversal v1.
//!
//! Baseada em *Reading Price Charts Bar by Bar*, de Al Brooks (Cap. 11:
//! "Opening Patterns and Reversals"). Especificação:
//! `docs/strategies/opening-reversal-v1.md`.
//!
//! Na primeira hora (09:30–10:30 ET), fade do teste da máxima/mínima de
//! ontem quando há barra de reversão forte — os extremos do dia costumam se
//! formar na primeira hora (Cap. 11). Primeira estratégia do bot com short.

pub mod config;
pub mod context;
pub mod entry;
pub mod setup;

#[cfg(test)]
mod tests;

use tracing::{debug, info};

use crate::context::MarketContextAnalyzer;
use crate::strategies::opening_reversal_v1::config::OpeningReversalV1Config;
use trader_domain::{
    Candle, MarketContext, RejectionReason, SignalResult, Strategy as StrategyTrait, StrategyId,
    StrategyState, TimeFrame,
};

/// Estratégia opening reversal (fade do teste dos níveis de ontem).
#[derive(Debug, Clone)]
pub struct OpeningReversalV1 {
    config: OpeningReversalV1Config,
    analyzer: MarketContextAnalyzer,
}

impl OpeningReversalV1 {
    pub fn new(config: OpeningReversalV1Config) -> Self {
        let params = &config.strategy.parameters;
        let analyzer_config = crate::context::ContextAnalyzerConfig {
            ema_period: 20,
            sma_period: 200,
            atr_period: params.atr_period,
            volume_period: 20,
            high_volatility_threshold_pct: params.max_atr_pct,
            low_volatility_threshold_pct: rust_decimal::Decimal::from(3)
                / rust_decimal::Decimal::from(10),
        };
        let analyzer = MarketContextAnalyzer::new(analyzer_config);
        Self { config, analyzer }
    }

    /// Carrega a configuração a partir de uma string TOML.
    pub fn from_toml(toml_str: &str) -> Result<Self, toml::de::Error> {
        let config: OpeningReversalV1Config = toml::from_str(toml_str)?;
        Ok(Self::new(config))
    }

    /// Retorna referência para os parâmetros da estratégia.
    pub fn parameters(&self) -> &config::StrategyParameters {
        &self.config.strategy.parameters
    }

    /// Hash de auditoria da configuração carregada.
    pub fn config_hash(&self) -> String {
        self.config.config_hash()
    }

    /// Analisa uma série de candles e retorna sinal ou rejeição.
    pub fn analyze_candles(&self, symbol: &str, candles: &[Candle]) -> SignalResult {
        let params = &self.config.strategy.parameters;
        let reject = |reason: RejectionReason, details: serde_json::Value| SignalResult::Rejected {
            reason,
            details: Some(details),
        };

        if candles.len() < 2 {
            return reject(
                RejectionReason::IncompleteSetup,
                serde_json::json!({ "reason": "série de candles insuficiente" }),
            );
        }
        let timeframe = match params.operational_timeframe.parse::<TimeFrame>() {
            Ok(tf) => tf,
            Err(_) => {
                return reject(
                    RejectionReason::IncompleteSetup,
                    serde_json::json!({ "reason": "timeframe operacional inválido" }),
                )
            }
        };

        let ctx = match self.analyzer.analyze(symbol, timeframe, candles) {
            Some(ctx) => ctx,
            None => {
                return reject(
                    RejectionReason::IncompleteSetup,
                    serde_json::json!({ "reason": "não foi possível computar o contexto" }),
                )
            }
        };

        if let Err((reason, details)) = context::check_trading_hours(candles, params) {
            debug!(?reason, "fora da janela da primeira hora");
            return reject(reason, details);
        }

        let Some(levels) = context::yesterday_levels(candles) else {
            return reject(
                RejectionReason::IncompleteSetup,
                serde_json::json!({ "reason": "série sem candles do dia anterior" }),
            );
        };

        let Some(atr_value) = context::atr(candles, params.atr_period) else {
            return reject(
                RejectionReason::IncompleteSetup,
                serde_json::json!({ "reason": "histórico insuficiente para ATR" }),
            );
        };

        let setup = match setup::detect_setup(candles, &levels, params) {
            Ok(setup) => setup,
            Err((reason, details)) => {
                debug!(?reason, "setup de opening reversal rejeitado");
                return reject(reason, details);
            }
        };

        let prices = match entry::evaluate_prices(candles, &setup, atr_value, params) {
            Ok(prices) => prices,
            Err((reason, details)) => {
                debug!(?reason, "preços rejeitados");
                return reject(reason, details);
            }
        };

        info!(
            direction = ?setup.direction,
            level = %setup.level,
            entry = %prices.entry_price,
            stop = %prices.stop_price,
            target = %prices.target_price,
            "setup de opening reversal detectado"
        );

        let signal = entry::build_signal(
            symbol,
            timeframe,
            &setup,
            &prices,
            &levels,
            atr_value,
            &ctx,
            &self.config.strategy.id,
            &self.config.strategy.version,
            self.config.config_hash(),
            parse_entry_order_type(&params.entry_order_type),
            params,
        );
        SignalResult::Signal(signal)
    }
}

impl StrategyTrait for OpeningReversalV1 {
    fn id(&self) -> StrategyId {
        StrategyId::new(&self.config.strategy.id, &self.config.strategy.version)
    }

    fn name(&self) -> &'static str {
        "Opening Reversal v1"
    }

    fn source(&self) -> &'static str {
        "Al Brooks - Reading Price Charts Bar by Bar"
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
                details: Some(serde_json::json!({ "reason": "série de candles vazia" })),
            };
        }
        self.analyze_candles(&ctx.symbol, candles)
    }
}

/// Faz parse do parâmetro `entry_order_type` ("stop" | "limit").
fn parse_entry_order_type(raw: &str) -> trader_domain::EntryOrderType {
    match raw.trim().to_lowercase().as_str() {
        "limit" => trader_domain::EntryOrderType::Limit,
        _ => trader_domain::EntryOrderType::Stop,
    }
}
