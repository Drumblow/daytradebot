//! Estratégia: Low 2 / M2S Short v1.
//!
//! Baseada em *Reading Price Charts Bar by Bar*, de Al Brooks (Cap. 4:
//! "High/Low 1, 2, 3, and 4"; Cap. 15: "Pullbacks in a Strong Trend").
//! Especificação: `docs/strategies/low2-m2s-short-v1.md`.
//!
//! Espelho da `pullback-trend-v1` para tendências de baixa: pullback de duas
//! pernas até a EMA 20 (correção para cima) + barra de sinal bear → venda na
//! continuação da queda. Módulo espelhado (decisão §14 do doc): a irmã long
//! está em produção e não é tocada.

pub mod config;
pub mod context;
pub mod entry;
pub mod setup;

use rust_decimal::Decimal;
use tracing::{debug, info};

use crate::context::MarketContextAnalyzer;
use crate::strategies::low2_m2s_short_v1::config::Low2M2sShortV1Config;
use crate::strategies::low2_m2s_short_v1::context::check_context;
use crate::strategies::low2_m2s_short_v1::entry::build_signal;
use crate::strategies::low2_m2s_short_v1::setup::SetupResult;
use trader_domain::{
    Candle, MarketContext, RejectionReason, SignalResult, Strategy as StrategyTrait, StrategyId,
    StrategyState, TimeFrame,
};

/// Estratégia Low 2 / M2S short (pullback em tendência de baixa).
#[derive(Debug, Clone)]
pub struct Low2M2sShortV1 {
    config: Low2M2sShortV1Config,
    analyzer: MarketContextAnalyzer,
}

impl Low2M2sShortV1 {
    pub fn new(config: Low2M2sShortV1Config) -> Self {
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
        let config: Low2M2sShortV1Config = toml::from_str(toml_str)?;
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

        // Regra de tendência (min_candles_below_ema20): preço abaixo da EMA de
        // contexto por N candles consecutivos (espelho da irmã long).
        let params = &self.config.strategy.parameters;
        let streak = context::consecutive_closes_below_ema(candles, params.ema_context_period);
        if streak < params.min_candles_below_ema20 {
            debug!(
                streak,
                min = params.min_candles_below_ema20,
                "sequência de candles abaixo da EMA insuficiente"
            );
            return SignalResult::Rejected {
                reason: RejectionReason::NoContext,
                details: Some(serde_json::json!({
                    "reason": "price not below context EMA for enough consecutive candles",
                    "streak": streak,
                    "min_required": params.min_candles_below_ema20,
                })),
            };
        }

        match setup::detect_setup(candles, &self.config.strategy.parameters) {
            SetupResult::Found(setup) => {
                // Guard anti-latência (espelho da irmã): se o candle mais
                // recente já fechou além do gatilho (abaixo da entrada), o
                // rompimento aconteceu antes da nossa ordem estar trabalhando.
                let last_close = candles.last().map(|c| c.close).unwrap_or_default();
                if last_close <= setup.entry_price {
                    debug!(
                        %last_close,
                        entry = %setup.entry_price,
                        "setup invalidado: preço já além do gatilho"
                    );
                    return SignalResult::Rejected {
                        reason: RejectionReason::SetupInvalidated,
                        details: Some(serde_json::json!({
                            "reason": "price already beyond entry trigger",
                            "last_close": last_close,
                            "entry_price": setup.entry_price,
                        })),
                    };
                }

                info!(
                    entry = %setup.entry_price,
                    stop = %setup.stop_price,
                    target = %setup.target_price,
                    "setup de low 2 / m2s short detectado"
                );

                let signal = build_signal(
                    symbol,
                    timeframe,
                    &setup,
                    &ctx,
                    &self.config.strategy.id,
                    &self.config.strategy.version,
                    self.config.config_hash(),
                    parse_entry_order_type(&self.config.strategy.parameters.entry_order_type),
                    params.risk_per_trade_pct,
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

impl StrategyTrait for Low2M2sShortV1 {
    fn id(&self) -> StrategyId {
        StrategyId::new(&self.config.strategy.id, &self.config.strategy.version)
    }

    fn name(&self) -> &'static str {
        "Low 2 / M2S Short v1"
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
                details: Some(serde_json::json!({ "reason": "empty candle series" })),
            };
        }

        self.analyze_candles(&ctx.symbol, candles)
    }
}

/// Faz parse do parâmetro `entry_order_type` da config ("stop" | "limit").
/// Qualquer valor desconhecido cai no default seguro do livro: stop.
fn parse_entry_order_type(raw: &str) -> trader_domain::EntryOrderType {
    match raw.trim().to_lowercase().as_str() {
        "limit" => trader_domain::EntryOrderType::Limit,
        _ => trader_domain::EntryOrderType::Stop,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};

    fn candle(
        timestamp: chrono::DateTime<Utc>,
        open: Decimal,
        high: Decimal,
        low: Decimal,
        close: Decimal,
    ) -> Candle {
        Candle::new(
            "IWM",
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

    fn make_downtrend_candles(signal_close: Decimal) -> Vec<Candle> {
        // Começa às 14:30 UTC (dentro do horário regular) e cai 1 ponto por candle.
        let base = Utc.with_ymd_and_hms(2026, 7, 2, 14, 30, 0).unwrap();
        let mut candles = Vec::new();

        // Série de baixa forte com 60 candles (intervalos de 5 min).
        for i in 0..60 {
            let close = Decimal::from(460 - i);
            candles.push(candle(
                base + chrono::Duration::minutes(i as i64 * 5),
                close + Decimal::ONE,
                close + Decimal::ONE,
                close - Decimal::ONE,
                close,
            ));
        }

        // Nova mínima em 399.
        let last_ts = candles.last().unwrap().timestamp;
        candles.push(candle(
            last_ts + chrono::Duration::minutes(5),
            Decimal::from(400),
            Decimal::from(401),
            Decimal::from(398),
            Decimal::from(399),
        ));

        // Correção rasa (pullback para cima): duas pernas.
        let last_ts = candles.last().unwrap().timestamp;
        candles.push(candle(
            last_ts + chrono::Duration::minutes(5),
            Decimal::from(399),
            Decimal::from(401),
            Decimal::from(399),
            Decimal::from(401),
        ));

        // Segunda perna da correção com máxima acima da barra de sinal.
        let last_ts = candles.last().unwrap().timestamp;
        candles.push(candle(
            last_ts + chrono::Duration::minutes(5),
            Decimal::from(401),
            Decimal::from(404),
            Decimal::from(400),
            Decimal::from(403),
        ));

        // Barra de sinal bear: sombra superior grande e fechamento no terço
        // inferior. open=403, high=404, low=399, close=signal_close.
        let last_ts = candles.last().unwrap().timestamp;
        candles.push(candle(
            last_ts + chrono::Duration::minutes(5),
            Decimal::from(403),
            Decimal::from(404),
            Decimal::from(399),
            signal_close,
        ));

        candles
    }

    #[test]
    fn perfect_setup_generates_sell_signal() {
        let candles = make_downtrend_candles(Decimal::from(400));
        let strategy = Low2M2sShortV1::new(Low2M2sShortV1Config::default());

        match strategy.analyze_candles("IWM", &candles) {
            SignalResult::Signal(signal) => {
                assert_eq!(signal.direction, trader_domain::Direction::Short);
                assert!(signal.entry_price.is_some());
                assert!(signal.stop_price.is_some());
                assert!(signal.target_price.is_some());
                // Entrada abaixo, stop acima, alvo mais abaixo.
                assert!(signal.entry_price.unwrap() < signal.stop_price.unwrap());
                assert!(signal.target_price.unwrap() < signal.entry_price.unwrap());

                let snapshot = &signal.market_snapshot;
                assert!(snapshot.get("ema_20").is_some());
                assert!(snapshot.get("trend_state").is_some());
                assert!(snapshot.get("signal_bar_index").is_some());
            }
            SignalResult::Rejected { reason, details } => {
                panic!("esperado sinal, rejeitado por {:?}: {:?}", reason, details)
            }
            _ => panic!("esperado sinal"),
        }
    }

    #[test]
    fn no_downtrend_rejects_signal() {
        let base = Utc.with_ymd_and_hms(2026, 7, 2, 15, 0, 0).unwrap();
        let mut candles = Vec::new();

        // Série lateral/altista.
        for i in 0..10 {
            let close = Decimal::from(400);
            candles.push(candle(
                base + chrono::Duration::minutes(i as i64 * 15),
                close + Decimal::ONE,
                close + Decimal::ONE,
                close - Decimal::ONE,
                close,
            ));
        }

        let strategy = Low2M2sShortV1::new(Low2M2sShortV1Config::default());
        match strategy.analyze_candles("IWM", &candles) {
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
    fn price_beyond_trigger_invalidates_setup() {
        let mut candles = make_downtrend_candles(Decimal::from(400));
        // O último candle fecha ABAIXO do gatilho do setup (entrada = mínima
        // da barra encontrada - tick = 397.99) ANTES da ordem ser enviada.
        // Não pode ser uma barra de sinal bear por conta própria: sombra
        // superior mínima (ratio 0.1 << 1.5).
        let last_ts = candles.last().unwrap().timestamp;
        candles.push(candle(
            last_ts + chrono::Duration::minutes(5),
            Decimal::new(3985, 1),
            Decimal::new(3986, 1),
            Decimal::new(3972, 1),
            Decimal::new(3975, 1),
        ));

        let strategy = Low2M2sShortV1::new(Low2M2sShortV1Config::default());
        match strategy.analyze_candles("IWM", &candles) {
            SignalResult::Rejected { reason, details } => {
                assert_eq!(reason, RejectionReason::SetupInvalidated);
                assert!(details.unwrap().get("entry_price").is_some());
            }
            other => panic!("esperado SetupInvalidated, obtido {:?}", other),
        }
    }
}
