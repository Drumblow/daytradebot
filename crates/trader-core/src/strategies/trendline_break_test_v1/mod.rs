//! Estratégia: Trendline Break Test v1.
//!
//! Baseada em *Reading Price Charts Bar by Bar*, de Al Brooks (Cap. 15,
//! "Major Reversals"; Cap. 8, "Trendline Break"). Especificação:
//! `docs/strategies/trendline-break-test-v1.md`.
//!
//! Depois que uma tendência tem a linha de tendência rompida COM MOMENTUM, o
//! mercado volta para testar o extremo antigo — e é nesse teste que se compra o
//! fundo (ou se vende o topo). A pré-condição do rompimento é inegociável no
//! livro: *"don't trade Countertrend until after there has been a trendline
//! break"* (Fig. 8.3).
//!
//! É a primeira estratégia do portfólio que opera SHORT por desenho, e não por
//! simetria oportunista.

pub mod config;
pub mod context;
pub mod entry;
pub mod setup;

#[cfg(test)]
mod tests;

use tracing::{debug, info};

use crate::context::MarketContextAnalyzer;
use crate::execution::time_exit::TimeExitConfig;
use crate::strategies::trendline_break_test_v1::config::TrendlineBreakTestV1Config;
use trader_domain::{
    Candle, MarketContext, RejectionReason, SignalResult, Strategy as StrategyTrait, StrategyId,
    StrategyState, TimeFrame,
};

/// Estratégia trendline break test (reversão maior após quebra de tendência).
#[derive(Debug, Clone)]
pub struct TrendlineBreakTestV1 {
    config: TrendlineBreakTestV1Config,
    analyzer: MarketContextAnalyzer,
}

impl TrendlineBreakTestV1 {
    pub fn new(config: TrendlineBreakTestV1Config) -> Self {
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
        let config: TrendlineBreakTestV1Config = toml::from_str(toml_str)?;
        Ok(Self::new(config))
    }

    /// Retorna referência para os parâmetros da estratégia.
    pub fn parameters(&self) -> &config::StrategyParameters {
        &self.config.strategy.parameters
    }

    /// Saída por tempo (seção 8 do doc): o trade precisa se validar dentro da
    /// janela, senão sai. Compensa parcialmente a ausência de saída parcial.
    pub fn time_exit(&self) -> Option<TimeExitConfig> {
        let te = &self.config.strategy.time_exit;
        te.enabled.then_some(TimeExitConfig {
            enabled: te.enabled,
            min_r: te.min_r,
            candles: te.candles,
        })
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

        let min_bars = params.trend_lookback + params.break_max_age + params.atr_period + 5;
        if candles.len() < min_bars {
            return reject(
                RejectionReason::IncompleteSetup,
                serde_json::json!({
                    "reason": "série de candles insuficiente para tendência + rompimento + ATR",
                    "necessario": min_bars,
                    "disponivel": candles.len(),
                }),
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

        // Seção 4: existe tendência estabelecida para reverter?
        let trend = match context::detect_trend(candles, params) {
            Ok(t) => t,
            Err((reason, details)) => {
                debug!(?reason, "sem tendência para reverter");
                return reject(reason, details);
            }
        };

        // Seção 5: a trendline foi rompida com momentum? (pré-condição do livro)
        let brk = match context::find_trendline_break(candles, &trend, params) {
            Ok(b) => b,
            Err((reason, details)) => {
                debug!(?reason, "rompimento de trendline ausente ou fraco");
                return reject(reason, details);
            }
        };

        // Seções 6 e 7: o teste do extremo antigo, com barra de reversão.
        let setup = match setup::detect_setup(candles, &trend, &brk, atr_value, params) {
            Ok(s) => s,
            Err((reason, details)) => {
                debug!(?reason, "teste do extremo rejeitado");
                return reject(reason, details);
            }
        };

        let prices = match entry::evaluate_prices(candles, &setup, atr_value, params) {
            Ok(p) => p,
            Err((reason, details)) => {
                debug!(?reason, "preços rejeitados");
                return reject(reason, details);
            }
        };

        info!(
            direction = ?setup.direction,
            trend = ?trend.kind,
            trend_extreme = %trend.extreme_price,
            break_index = brk.index,
            overshoot = setup.is_overshoot,
            entry = %prices.entry_price,
            stop = %prices.stop_price,
            target = %prices.target_price,
            "setup de trendline break test detectado"
        );

        let signal = entry::build_signal(
            symbol,
            timeframe,
            &setup,
            &prices,
            &trend,
            &brk,
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

impl StrategyTrait for TrendlineBreakTestV1 {
    fn id(&self) -> StrategyId {
        StrategyId::new(&self.config.strategy.id, &self.config.strategy.version)
    }

    fn name(&self) -> &'static str {
        "Trendline Break Test v1"
    }

    fn source(&self) -> &'static str {
        "Al Brooks - Reading Price Charts Bar by Bar, Cap. 15 (Major Reversals)"
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
