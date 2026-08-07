//! Estratégia: Breakout — Primeiro Pullback v1.
//!
//! Baseada em *The Art and Science of Technical Analysis*, de Adam Grimes
//! (Cap. 6: "Breakouts, Entering on First Pullback Following").
//! Especificação: `docs/strategies/breakout-first-pullback-v1.md`.
//!
//! Opera o nascimento de tendências — contexto que a `pullback-trend-v1`
//! (que exige tendência estabelecida) não cobre: rompimento válido de
//! resistência testada + primeiro pullback controlado, stop no pivô
//! pré-breakout.

pub mod config;
pub mod context;
pub mod entry;
pub mod setup;

#[cfg(test)]
mod tests;

use tracing::{debug, info};

use crate::context::MarketContextAnalyzer;
use crate::strategies::breakout_first_pullback_v1::config::BreakoutFirstPullbackV1Config;
use trader_domain::{
    Candle, MarketContext, RejectionReason, SignalResult, Strategy as StrategyTrait, StrategyId,
    StrategyState, TimeFrame,
};

/// Estratégia breakout + primeiro pullback.
#[derive(Debug, Clone)]
pub struct BreakoutFirstPullbackV1 {
    config: BreakoutFirstPullbackV1Config,
    analyzer: MarketContextAnalyzer,
}

impl BreakoutFirstPullbackV1 {
    pub fn new(config: BreakoutFirstPullbackV1Config) -> Self {
        let params = &config.strategy.parameters;
        let analyzer_config = crate::context::ContextAnalyzerConfig {
            ema_period: 20,
            sma_period: 200,
            atr_period: params.atr_period,
            volume_period: params.avg_period,
            high_volatility_threshold_pct: params.max_atr_pct,
            low_volatility_threshold_pct: rust_decimal::Decimal::from(3)
                / rust_decimal::Decimal::from(10),
        };
        let analyzer = MarketContextAnalyzer::new(analyzer_config);
        Self { config, analyzer }
    }

    /// Carrega a configuração a partir de uma string TOML.
    pub fn from_toml(toml_str: &str) -> Result<Self, toml::de::Error> {
        let config: BreakoutFirstPullbackV1Config = toml::from_str(toml_str)?;
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

        if candles.is_empty() {
            return reject(
                RejectionReason::IncompleteSetup,
                serde_json::json!({ "reason": "série de candles vazia" }),
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
            debug!(?reason, "fora da janela de horário");
            return reject(reason, details);
        }

        let Some(atr_value) = context::atr(candles, params.atr_period) else {
            return reject(
                RejectionReason::IncompleteSetup,
                serde_json::json!({ "reason": "histórico insuficiente para ATR" }),
            );
        };

        // Procura a barra de breakout numa janela que permita pullback de
        // `min` a `max` candles até a barra atual (seção 5 do doc).
        let mut first_structural_error: Option<(RejectionReason, serde_json::Value)> = None;
        for pullback_len in params.pullback_min_candles..=params.pullback_max_candles {
            if candles.len() <= pullback_len {
                continue;
            }
            let breakout_index = candles.len() - 1 - pullback_len;
            match context::detect_breakout(candles, breakout_index, atr_value, params) {
                Ok(breakout) => {
                    return self.finish(symbol, timeframe, candles, breakout, atr_value, &ctx);
                }
                Err((reason, details)) => {
                    if reason != RejectionReason::IncompleteSetup
                        && first_structural_error.is_none()
                    {
                        first_structural_error = Some((reason, details));
                    }
                }
            }
        }

        // Breakout mais antigo que a janela de pullback: rejeição explícita
        // de "pullback longo demais" (seção 5.3 do doc).
        let old_index = candles
            .len()
            .saturating_sub(1 + params.pullback_max_candles + 1);
        if old_index > 0 {
            if let Ok(_breakout) = context::detect_breakout(candles, old_index, atr_value, params) {
                return reject(
                    RejectionReason::PullbackTooLong,
                    serde_json::json!({
                        "reason": "breakout antigo demais; pullback passou do máximo sem gatilho",
                        "breakout_index": old_index,
                        "pullback_max_candles": params.pullback_max_candles,
                    }),
                );
            }
        }

        let (reason, details) = first_structural_error.unwrap_or((
            RejectionReason::IncompleteSetup,
            serde_json::json!({ "reason": "nenhum breakout válido na janela de pullback" }),
        ));
        reject(reason, details)
    }

    /// Continua a pipeline com um breakout validado: setup → preços → sinal.
    fn finish(
        &self,
        symbol: &str,
        timeframe: TimeFrame,
        candles: &[Candle],
        breakout: context::Breakout,
        atr_value: rust_decimal::Decimal,
        ctx: &MarketContext,
    ) -> SignalResult {
        let params = &self.config.strategy.parameters;
        let reject = |reason: RejectionReason, details: serde_json::Value| SignalResult::Rejected {
            reason,
            details: Some(details),
        };

        let setup = match setup::detect_setup(candles, breakout, params) {
            Ok(setup) => setup,
            Err((reason, details)) => {
                debug!(?reason, "setup de pullback rejeitado");
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

        // Guard anti-latência estrutural: como a barra de gatilho é sempre a
        // última da série, a entrada (máxima dela + 1 tick) está por
        // construção acima do último fechamento — o caso "preço já além do
        // gatilho" da pullback-trend-v1 não ocorre aqui. Ordens stop que não
        // rompem expiram por `entry_validity_candles` (ADR-009).

        info!(
            level = %setup.breakout.level.price,
            entry = %prices.entry_price,
            stop = %prices.stop_price,
            target = %prices.target_price,
            "setup de breakout + primeiro pullback detectado"
        );

        let signal = entry::build_signal(
            symbol,
            timeframe,
            &setup,
            &prices,
            atr_value,
            ctx,
            &self.config.strategy.id,
            &self.config.strategy.version,
            self.config.config_hash(),
            parse_entry_order_type(&params.entry_order_type),
            params,
        );
        SignalResult::Signal(signal)
    }
}

impl StrategyTrait for BreakoutFirstPullbackV1 {
    fn id(&self) -> StrategyId {
        StrategyId::new(&self.config.strategy.id, &self.config.strategy.version)
    }

    fn name(&self) -> &'static str {
        "Breakout — Primeiro Pullback v1"
    }

    fn source(&self) -> &'static str {
        "Adam Grimes - The Art and Science of Technical Analysis"
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
