//! Estratégia: Value Area Reentry v1.
//!
//! Baseada em *Mind over Markets*, de James Dalton (Cap. 4, "Special
//! Situations — The Value-Area Rule"; Apêndice 1 para o cálculo da área de
//! valor por TPO). Especificação:
//! `docs/strategies/value-area-reentry-v1.md`.
//!
//! O mercado abre FORA da área de valor de ontem, é rejeitado, volta para
//! dentro dela e é aceito ("double TPO prints") — e o leilão tende a
//! atravessar a área inteira até o lado oposto. É o único setup do portfólio
//! com alvo estrutural (a borda oposta da VA), não múltiplo de R.

pub mod config;
pub mod context;
pub mod entry;
pub mod setup;

#[cfg(test)]
mod tests;

use tracing::{debug, info};

use crate::context::MarketContextAnalyzer;
use crate::strategies::value_area_reentry_v1::config::ValueAreaReentryV1Config;
use trader_domain::{
    Candle, MarketContext, RejectionReason, SignalResult, Strategy as StrategyTrait, StrategyId,
    StrategyState, TimeFrame,
};

/// Estratégia value area reentry (travessia da área de valor de ontem).
#[derive(Debug, Clone)]
pub struct ValueAreaReentryV1 {
    config: ValueAreaReentryV1Config,
    analyzer: MarketContextAnalyzer,
}

impl ValueAreaReentryV1 {
    pub fn new(config: ValueAreaReentryV1Config) -> Self {
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
        let config: ValueAreaReentryV1Config = toml::from_str(toml_str)?;
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
                serde_json::json!({ "reason": "série de candles insuficiente (mín. 40: EMA/ATR/dia anterior)" }),
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

        // Seção 4: área de valor do dia anterior (proxy TPO, Apêndice 1).
        let Some(prev_day) = context::previous_day_slice(candles) else {
            return reject(
                RejectionReason::IncompleteSetup,
                serde_json::json!({ "reason": "série sem um dia anterior completo para calcular a área de valor" }),
            );
        };
        let Some(va) = context::compute_value_area(
            prev_day,
            params.va_buckets,
            params.va_percent,
            params.tick_size,
        ) else {
            return reject(
                RejectionReason::IncompleteSetup,
                serde_json::json!({ "reason": "não foi possível calcular a área de valor (dia anterior sem range)" }),
            );
        };

        // Seção 5.3 — filtro do autor: VA estreita atravessa mais fácil.
        let va_width = va.width();
        if va_width > params.max_va_width_atr * daily_atr_value {
            debug!(%va_width, "área de valor larga demais");
            return reject(
                RejectionReason::ValueAreaTooWide,
                serde_json::json!({
                    "reason": "área de valor larga demais para travessia (Cap. 4)",
                    "va_width": va_width,
                    "daily_atr": daily_atr_value,
                    "max_va_width_atr": params.max_va_width_atr,
                }),
            );
        }

        let setup = match setup::detect_setup(candles, &va, params) {
            Ok(setup) => setup,
            Err((reason, details)) => {
                debug!(?reason, "setup de value area reentry rejeitado");
                return reject(reason, details);
            }
        };

        // Seção 5.3 — filtro do autor: quanto mais perto do valor a abertura,
        // maior a chance de atravessar.
        if setup.open_distance > params.max_open_distance_atr * daily_atr_value {
            debug!(distance = %setup.open_distance, "abertura longe demais do valor");
            return reject(
                RejectionReason::OpenTooFarFromValue,
                serde_json::json!({
                    "reason": "abertura distante demais da área de valor (Cap. 4)",
                    "open_distance": setup.open_distance,
                    "daily_atr": daily_atr_value,
                    "max_open_distance_atr": params.max_open_distance_atr,
                }),
            );
        }

        // Seção 5.3 — filtro do autor: travessia a favor da direção do mercado.
        let ema_slope = context::ema_slope_signed(candles, params.trend_lookback)
            .unwrap_or(rust_decimal::Decimal::ZERO);
        if context::trend_against_traversal(
            ema_slope,
            setup.direction,
            params.trend_slope_threshold,
        ) {
            debug!(%ema_slope, "travessia contra a direção do mercado");
            return reject(
                RejectionReason::TrendAgainstTraversal,
                serde_json::json!({
                    "reason": "travessia contra a tendência estabelecida (Cap. 4)",
                    "ema_slope_per_bar": ema_slope,
                    "direction": format!("{:?}", setup.direction),
                    "threshold": params.trend_slope_threshold,
                }),
            );
        }

        let prices = match entry::evaluate_prices(candles, &setup, &va, atr_value, params) {
            Ok(prices) => prices,
            Err((reason, details)) => {
                debug!(?reason, "preços rejeitados");
                return reject(reason, details);
            }
        };

        info!(
            direction = ?setup.direction,
            va_low = %va.low,
            va_high = %va.high,
            open_today = %setup.open_today,
            entry = %prices.entry_price,
            stop = %prices.stop_price,
            target = %prices.target_price,
            "setup de value area reentry detectado"
        );

        let signal = entry::build_signal(
            symbol,
            timeframe,
            &setup,
            &prices,
            &va,
            atr_value,
            daily_atr_value,
            ema_slope,
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

impl StrategyTrait for ValueAreaReentryV1 {
    fn id(&self) -> StrategyId {
        StrategyId::new(&self.config.strategy.id, &self.config.strategy.version)
    }

    fn name(&self) -> &'static str {
        "Value Area Reentry v1"
    }

    fn source(&self) -> &'static str {
        "James Dalton - Mind over Markets, Cap. 4 (The Value-Area Rule)"
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
