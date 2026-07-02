//! Análise de contexto de mercado.
//!
//! O `MarketContextAnalyzer` classifica o estado do mercado (tendência,
//! volatilidade, fase) a partir de candles e indicadores calculados.

use chrono::{DateTime, Timelike, Utc};
use rust_decimal::Decimal;
use serde_json::json;
use tracing::debug;

use crate::indicators::{atr_percent, ema, range_percent, relative_volume, sma};
use trader_domain::{Candle, MarketContext, MarketPhase, TimeFrame, TrendState, VolatilityRegime};

/// Configuração do analisador de contexto.
#[derive(Debug, Clone, Copy)]
pub struct ContextAnalyzerConfig {
    pub ema_period: usize,
    pub sma_period: usize,
    pub atr_period: usize,
    pub volume_period: usize,
    pub high_volatility_threshold_pct: Decimal,
    pub low_volatility_threshold_pct: Decimal,
}

impl Default for ContextAnalyzerConfig {
    fn default() -> Self {
        Self {
            ema_period: 20,
            sma_period: 200,
            atr_period: 14,
            volume_period: 20,
            high_volatility_threshold_pct: Decimal::from(15) / Decimal::from(10),
            low_volatility_threshold_pct: Decimal::from(3) / Decimal::from(10),
        }
    }
}

/// Analisador de contexto de mercado.
#[derive(Debug, Clone)]
pub struct MarketContextAnalyzer {
    config: ContextAnalyzerConfig,
}

impl MarketContextAnalyzer {
    pub fn new(config: ContextAnalyzerConfig) -> Self {
        Self { config }
    }

    /// Analisa o contexto do mercado para o último candle da série.
    pub fn analyze(
        &self,
        symbol: impl Into<String>,
        timeframe: TimeFrame,
        candles: &[Candle],
    ) -> Option<MarketContext> {
        if candles.is_empty() {
            return None;
        }

        let symbol = symbol.into();
        let last = candles.last()?;
        let timestamp = last.timestamp;

        let ema_20 = ema(candles, self.config.ema_period);
        let ema_50 = ema(candles, 50);
        let sma_200 = sma(candles, self.config.sma_period);
        let atr_14 = atr_percent(candles, self.config.atr_period);
        let volume_relative = relative_volume(candles, self.config.volume_period);
        let range_pct = range_percent(candles);

        let trend_state = classify_trend(last.close, ema_20, sma_200);
        let volatility_regime = classify_volatility(
            atr_14,
            self.config.high_volatility_threshold_pct,
            self.config.low_volatility_threshold_pct,
        );
        let market_phase = classify_market_phase(timestamp);

        let is_tradeable = matches!(trend_state, TrendState::Uptrend | TrendState::Downtrend)
            && !matches!(volatility_regime, VolatilityRegime::High)
            && matches!(market_phase, MarketPhase::Regular);

        debug!(
            symbol = %symbol,
            timeframe = %timeframe,
            trend = ?trend_state,
            volatility = ?volatility_regime,
            phase = ?market_phase,
            "contexto de mercado classificado"
        );

        Some(MarketContext {
            symbol,
            timeframe,
            timestamp,
            candle_timestamp: Some(timestamp),
            trend_state,
            volatility_regime,
            market_phase,
            ema_20,
            ema_50,
            sma_200,
            atr_14,
            atr_percent_14: atr_14,
            volume_relative,
            hh_hl_count: None,
            lh_ll_count: None,
            range_percent: range_pct,
            is_tradeable,
            raw_values: json!({
                "ema_period": self.config.ema_period,
                "sma_period": self.config.sma_period,
                "atr_period": self.config.atr_period,
                "volume_period": self.config.volume_period,
            }),
        })
    }
}

fn classify_trend(close: Decimal, ema_20: Option<Decimal>, sma_200: Option<Decimal>) -> TrendState {
    match (ema_20, sma_200) {
        (Some(ema), Some(sma)) if close > ema && ema > sma => TrendState::Uptrend,
        (Some(ema), Some(sma)) if close < ema && ema < sma => TrendState::Downtrend,
        (Some(ema), None) if close > ema => TrendState::Uptrend,
        (Some(ema), None) if close < ema => TrendState::Downtrend,
        _ => TrendState::Neutral,
    }
}

fn classify_volatility(
    atr_percent: Option<Decimal>,
    high_threshold: Decimal,
    low_threshold: Decimal,
) -> VolatilityRegime {
    match atr_percent {
        Some(atr) if atr >= high_threshold => VolatilityRegime::High,
        Some(atr) if atr <= low_threshold => VolatilityRegime::Low,
        Some(_) => VolatilityRegime::Normal,
        None => VolatilityRegime::Unknown,
    }
}

/// Classifica a fase do mercado com base no timestamp UTC.
///
/// Para o MVP, considera-se "regular" o horário de pregão dos EUA em UTC
/// (aproximadamente 14:30–21:00 UTC). O risk manager deve aplicar filtros mais
/// específicos de timezone e horário de estratégia.
fn classify_market_phase(timestamp: DateTime<Utc>) -> MarketPhase {
    let time = timestamp.time();
    let hour = time.hour();

    if (14..=20).contains(&hour) {
        MarketPhase::Regular
    } else if hour < 14 {
        MarketPhase::PreMarket
    } else {
        MarketPhase::AfterHours
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};
    use rust_decimal::Decimal;
    use trader_domain::Candle;

    fn candle_at(timestamp: DateTime<Utc>, close: Decimal) -> Candle {
        Candle::new(
            "SPY",
            TimeFrame::M15,
            timestamp,
            close - Decimal::ONE,
            close + Decimal::ONE,
            close - Decimal::ONE,
            close,
            Decimal::from(1000),
        )
        .expect("candle válido")
    }

    #[test]
    fn uptrend_when_price_above_ema_and_sma() {
        // 15:00 UTC está dentro do horário regular configurado (14:30–21:00).
        let base = Utc.with_ymd_and_hms(2026, 7, 2, 15, 0, 0).unwrap();
        let mut candles = Vec::new();
        for i in 0..50 {
            let close = Decimal::from(100 + i);
            candles.push(candle_at(
                base + chrono::Duration::minutes(i as i64 * 5),
                close,
            ));
        }

        let analyzer = MarketContextAnalyzer::new(ContextAnalyzerConfig::default());
        let ctx = analyzer.analyze("SPY", TimeFrame::M5, &candles);

        assert!(ctx.is_some());
        let ctx = ctx.unwrap();
        assert_eq!(ctx.trend_state, TrendState::Uptrend);
        assert!(ctx.is_tradeable);
    }

    #[test]
    fn returns_none_for_empty_series() {
        let analyzer = MarketContextAnalyzer::new(ContextAnalyzerConfig::default());
        assert!(analyzer.analyze("SPY", TimeFrame::M15, &[]).is_none());
    }
}
