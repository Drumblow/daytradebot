//! Regras de contexto de mercado para a estratégia Low 2 / M2S Short v1 —
//! espelho da `pullback-trend-v1` para tendência de baixa (seção 4 do doc).

use rust_decimal::Decimal;
use serde_json::json;

use crate::strategies::low2_m2s_short_v1::config::StrategyParameters;
use trader_domain::{
    Candle, MarketContext, MarketPhase, RejectionReason, TrendState, VolatilityRegime,
};

/// Resultado da avaliação de contexto.
#[derive(Debug, Clone, PartialEq)]
pub enum ContextCheck {
    Approved,
    Rejected(RejectionReason, serde_json::Value),
}

/// Avalia se o contexto de mercado permite buscar setups de venda (short).
pub fn check_context(ctx: &MarketContext, params: &StrategyParameters) -> ContextCheck {
    if !matches!(ctx.trend_state, TrendState::Downtrend) {
        return ContextCheck::Rejected(
            RejectionReason::NoContext,
            json!({ "reason": "trend_state is not downtrend", "value": format!("{:?}", ctx.trend_state) }),
        );
    }

    if matches!(ctx.volatility_regime, VolatilityRegime::High) {
        return ContextCheck::Rejected(
            RejectionReason::HighVolatility,
            json!({ "reason": "volatility regime is high", "atr_14": ctx.atr_14 }),
        );
    }

    if !matches!(ctx.market_phase, MarketPhase::Regular) {
        return ContextCheck::Rejected(
            RejectionReason::OutsideTradingHours,
            json!({ "reason": "market is not in regular hours", "phase": format!("{:?}", ctx.market_phase) }),
        );
    }

    let ema_period = params.ema_context_period;

    if let Some(ema) = ctx.ema_20 {
        if let Some(last_close) = ctx
            .raw_values
            .get("last_close")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<Decimal>().ok())
        {
            if last_close > ema {
                return ContextCheck::Rejected(
                    RejectionReason::NoContext,
                    json!({ "reason": "close above ema", "close": last_close, "ema_period": ema_period, "ema": ema }),
                );
            }
        }
    }

    ContextCheck::Approved
}

/// Conta quantos candles consecutivos (do mais recente para trás) fecharam
/// abaixo da EMA do período de contexto, calculada candle a candle sobre o
/// prefixo da série (sem lookahead). Espelho de
/// `consecutive_closes_above_ema` da irmã long.
pub fn consecutive_closes_below_ema(candles: &[Candle], ema_period: usize) -> usize {
    let mut streak = 0;
    for i in (0..candles.len()).rev() {
        let Some(ema_value) = crate::indicators::ema(&candles[..=i], ema_period) else {
            break;
        };
        if candles[i].close < ema_value {
            streak += 1;
        } else {
            break;
        }
    }
    streak
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};
    use trader_domain::TimeFrame;

    fn candle_at(idx: u32, close: i64) -> Candle {
        let ts = Utc.with_ymd_and_hms(2026, 8, 3, 14, 30, 0).unwrap()
            + chrono::Duration::minutes(i64::from(idx) * 15);
        Candle::new(
            "SPY",
            TimeFrame::M15,
            ts,
            Decimal::from(close) + Decimal::ONE,
            Decimal::from(close) + Decimal::ONE,
            Decimal::from(close) - Decimal::ONE,
            Decimal::from(close),
            Decimal::from(1000),
        )
        .expect("candle válido")
    }

    #[test]
    fn falling_series_counts_full_streak() {
        let candles: Vec<Candle> = (0..30).map(|i| candle_at(i, 130 - i64::from(i))).collect();
        // Série caindo 1 ponto por candle: todo fechamento fica abaixo da EMA.
        let streak = consecutive_closes_below_ema(&candles, 20);
        // Os primeiros `period - 1` candles não têm EMA; o streak máximo é 30 - 19.
        assert_eq!(streak, 11);
    }

    #[test]
    fn rally_breaks_streak() {
        let mut candles: Vec<Candle> = (0..30).map(|i| candle_at(i, 130 - i64::from(i))).collect();
        // Último candle dispara para cima da EMA.
        let last = candles.len() - 1;
        candles[last] = candle_at(last as u32, 150);

        assert_eq!(consecutive_closes_below_ema(&candles, 20), 0);
    }

    #[test]
    fn insufficient_data_returns_zero() {
        let candles: Vec<Candle> = (0..5).map(|i| candle_at(i, 130 - i64::from(i))).collect();
        assert_eq!(consecutive_closes_below_ema(&candles, 20), 0);
    }
}
