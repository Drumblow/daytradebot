//! Estratégia: Range Extreme Fade v1.
//!
//! Baseada em *Reading Price Charts Bar by Bar*, de Al Brooks (Cap. 9:
//! "Failed Higher High and Lower Low Breakouts"; Cap. 5: "Barb Wire" e os
//! vetos de contexto). Especificação:
//! `docs/strategies/range-extreme-fade-v1.md`.
//!
//! Em dias de trading range (~80% dos dias, Cap. 10), o mercado rompe um
//! extremo do dia sem momentum e falha; entramos contra o rompimento com
//! barra de sinal forte, mirando o retorno ao interior do range. É a
//! estratégia do contexto que o portfólio rejeita (as irmãs exigem tendência,
//! breakout real ou a primeira hora).

pub mod config;
pub mod context;
pub mod entry;
pub mod setup;

#[cfg(test)]
mod tests;

use tracing::{debug, info};

use crate::context::MarketContextAnalyzer;
use crate::strategies::range_extreme_fade_v1::config::RangeExtremeFadeV1Config;
use trader_domain::{
    Candle, MarketContext, RejectionReason, SignalResult, Strategy as StrategyTrait, StrategyId,
    StrategyState, TimeFrame,
};

/// Estratégia range extreme fade (fade de rompimentos falhos em dias de range).
#[derive(Debug, Clone)]
pub struct RangeExtremeFadeV1 {
    config: RangeExtremeFadeV1Config,
    analyzer: MarketContextAnalyzer,
}

impl RangeExtremeFadeV1 {
    pub fn new(config: RangeExtremeFadeV1Config) -> Self {
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
        let config: RangeExtremeFadeV1Config = toml::from_str(toml_str)?;
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

        if candles.len() < 40 {
            return reject(
                RejectionReason::IncompleteSetup,
                serde_json::json!({ "reason": "série de candles insuficiente (mín. 40: EMA/janela/ATR)" }),
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
            debug!(?reason, "fora da janela operacional");
            return reject(reason, details);
        }

        let Some(atr_value) = context::atr(candles, params.atr_period) else {
            return reject(
                RejectionReason::IncompleteSetup,
                serde_json::json!({ "reason": "histórico insuficiente para ATR" }),
            );
        };

        let Some(daily_atr_value) = context::daily_atr(candles, params.daily_atr_period) else {
            return reject(
                RejectionReason::IncompleteSetup,
                serde_json::json!({ "reason": "série sem dias anteriores suficientes para o ATR diário" }),
            );
        };

        // Seção 4.1: o dia é de trading range?
        if let Err((reason, details)) = context::check_range_day(candles, daily_atr_value, params) {
            debug!(?reason, "dia não é de trading range");
            return reject(reason, details);
        }

        // Seção 4.4: veto meio do dia + meio do range (literal, Cap. 5).
        if context::is_midday_midrange(candles, params) {
            debug!("veto meio do dia + meio do range");
            return reject(
                RejectionReason::MiddayMidrange,
                serde_json::json!({ "reason": "meio do dia e preço no terço central do range (Cap. 5)" }),
            );
        }

        // Seção 4.5: veto Barb Wire (literal, Cap. 5).
        if context::is_barb_wire(candles, params) {
            debug!("veto Barb Wire");
            return reject(
                RejectionReason::BarbWire,
                serde_json::json!({ "reason": "Barb Wire detectado — equilíbrio total (Cap. 5)" }),
            );
        }

        let setup = match setup::detect_setup(candles, atr_value, params) {
            Ok(setup) => setup,
            Err((reason, details)) => {
                debug!(?reason, "setup de range extreme fade rejeitado");
                return reject(reason, details);
            }
        };

        let prices = match entry::evaluate_prices(candles, &setup, params) {
            Ok(prices) => prices,
            Err((reason, details)) => {
                debug!(?reason, "preços rejeitados");
                return reject(reason, details);
            }
        };

        let ema_slope = context::ema_slope_per_bar(candles, params.structure_lookback)
            .unwrap_or(rust_decimal::Decimal::ZERO);
        let day_range = context::day_range(candles);

        info!(
            direction = ?setup.direction,
            broken_extreme = %setup.broken_extreme,
            extension_atr = %setup.extension_atr,
            entry = %prices.entry_price,
            stop = %prices.stop_price,
            target = %prices.target_price,
            "setup de range extreme fade detectado"
        );

        let signal = entry::build_signal(
            symbol,
            timeframe,
            &setup,
            &prices,
            atr_value,
            daily_atr_value,
            ema_slope,
            day_range,
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

impl StrategyTrait for RangeExtremeFadeV1 {
    fn id(&self) -> StrategyId {
        StrategyId::new(&self.config.strategy.id, &self.config.strategy.version)
    }

    fn name(&self) -> &'static str {
        "Range Extreme Fade v1"
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
