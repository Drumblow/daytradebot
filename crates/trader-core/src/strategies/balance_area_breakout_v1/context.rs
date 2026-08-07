//! Contexto da estratégia Balance-Area Breakout v1: detecção da área de
//! balanceamento (largura e cobertura multi-dia), janela de horário e ATR.
//! Regras da seção 4 do doc (`docs/strategies/balance-area-breakout-v1.md`).

use chrono::{DateTime, Utc};
use chrono_tz::America::New_York;
use rust_decimal::Decimal;
use serde_json::json;

use crate::strategies::balance_area_breakout_v1::config::StrategyParameters;
use trader_domain::{Candle, RejectionReason};

/// Área de balanceamento detectada.
#[derive(Debug, Clone, PartialEq)]
pub struct BalanceArea {
    pub high: Decimal,
    pub low: Decimal,
    pub width_pct: Decimal,
    pub width_atr: Decimal,
}

/// ATR simples (média dos TRs) das últimas `period` barras, em preço.
pub fn atr(candles: &[Candle], period: usize) -> Option<Decimal> {
    if candles.len() < period + 1 || period == 0 {
        return None;
    }
    let start = candles.len() - period;
    let mut sum = Decimal::ZERO;
    for i in start..candles.len() {
        let c = &candles[i];
        let prev_close = candles[i - 1].close;
        let tr = (c.high - c.low)
            .max((c.high - prev_close).abs())
            .max((c.low - prev_close).abs());
        sum += tr;
    }
    Some(sum / Decimal::from(period))
}

/// Janela de horário (UTC) da última barra da série.
pub fn check_trading_hours(
    candles: &[Candle],
    params: &StrategyParameters,
) -> Result<(), (RejectionReason, serde_json::Value)> {
    let Some(last) = candles.last() else {
        return Err((
            RejectionReason::IncompleteSetup,
            json!({ "reason": "série vazia" }),
        ));
    };
    let time = last.timestamp.time();
    let parse = |s: &str| {
        let p: Vec<&str> = s.split(':').collect();
        chrono::NaiveTime::from_hms_opt(
            p.first().and_then(|v| v.parse().ok()).unwrap_or(0),
            p.get(1).and_then(|v| v.parse().ok()).unwrap_or(0),
            p.get(2).and_then(|v| v.parse().ok()).unwrap_or(0),
        )
        .unwrap_or_default()
    };
    let start = parse(&params.trading_start_time);
    let end = parse(&params.trading_end_time);
    if time < start || time > end {
        return Err((
            RejectionReason::OutsideTradingHours,
            json!({ "time": time.to_string(), "start": start.to_string(), "end": end.to_string() }),
        ));
    }
    Ok(())
}

/// Data do candle em America/New_York.
fn et_date(ts: DateTime<Utc>) -> chrono::NaiveDate {
    ts.with_timezone(&New_York).date_naive()
}

/// Detecta a área de balanceamento nos `balance_lookback_candles` anteriores
/// à última barra (seção 4 do doc): largura dentro dos tetos E cobertura de
/// pelo menos 2 dias de pregão (ET).
pub fn detect_balance_area(
    candles: &[Candle],
    atr_value: Decimal,
    params: &StrategyParameters,
) -> Result<BalanceArea, (RejectionReason, serde_json::Value)> {
    let n = params.balance_lookback_candles;
    if candles.len() < n + 1 {
        return Err((
            RejectionReason::IncompleteSetup,
            json!({ "reason": "histórico insuficiente para a área de balanceamento" }),
        ));
    }
    // A área EXCLUI a última barra (a candidata a rompimento).
    let area = &candles[candles.len() - 1 - n..candles.len() - 1];

    let high = area.iter().map(|c| c.high).max().unwrap_or_default();
    let low = area.iter().map(|c| c.low).min().unwrap_or_default();
    let mid = (high + low) / Decimal::from(2);
    if mid.is_zero() {
        return Err((
            RejectionReason::IncompleteSetup,
            json!({ "reason": "área com preço médio zero" }),
        ));
    }
    let width = high - low;
    let width_pct = width / mid;
    let width_atr = if atr_value.is_zero() {
        Decimal::ZERO
    } else {
        width / atr_value
    };

    // Cobertura multi-dia (ET): balanceamento é fenômeno de vários dias.
    let mut days: Vec<chrono::NaiveDate> = area.iter().map(|c| et_date(c.timestamp)).collect();
    days.sort();
    days.dedup();
    if days.len() < 2 {
        return Err((
            RejectionReason::NoBalanceArea,
            json!({ "reason": "área cobre menos de 2 dias de pregão", "days": days.len() }),
        ));
    }

    if width_pct > params.balance_max_width_pct || width_atr > params.balance_max_width_atr_mult {
        return Err((
            RejectionReason::NoBalanceArea,
            json!({
                "reason": "largura acima dos tetos de balanceamento",
                "width_pct": width_pct,
                "width_atr": width_atr,
                "max_width_pct": params.balance_max_width_pct,
                "max_width_atr_mult": params.balance_max_width_atr_mult,
            }),
        ));
    }

    Ok(BalanceArea {
        high,
        low,
        width_pct,
        width_atr,
    })
}
