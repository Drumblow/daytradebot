//! Contexto da estratégia Opening Reversal v1: níveis do dia anterior
//! (máxima/mínima, por data em America/New_York), janela da primeira hora e
//! vetos de momentum. Regras da seção 4 do doc
//! (`docs/strategies/opening-reversal-v1.md`).

use chrono::{DateTime, Utc};
use chrono_tz::America::New_York;
use rust_decimal::Decimal;
use serde_json::json;

use crate::strategies::opening_reversal_v1::config::StrategyParameters;
use trader_domain::{Candle, RejectionReason};

/// Níveis do dia anterior (pregão completo em ET).
#[derive(Debug, Clone, PartialEq)]
pub struct YesterdayLevels {
    pub high: Decimal,
    pub low: Decimal,
}

/// Data do candle em America/New_York (o pregão americano é definido em ET).
fn et_date(ts: DateTime<Utc>) -> chrono::NaiveDate {
    ts.with_timezone(&New_York).date_naive()
}

/// Extrai a máxima e a mínima do dia anterior ao candle mais recente.
///
/// Retorna `None` se a série não contém candles de um dia anterior.
pub fn yesterday_levels(candles: &[Candle]) -> Option<YesterdayLevels> {
    let today = et_date(candles.last()?.timestamp);
    let mut yesterday: Option<chrono::NaiveDate> = None;
    let mut high: Option<Decimal> = None;
    let mut low: Option<Decimal> = None;
    // Varre de trás para frente: o primeiro dia diferente de "hoje" é ontem;
    // acumula os extremos dele e para ao retroceder mais um dia.
    for candle in candles.iter().rev() {
        let date = et_date(candle.timestamp);
        if date >= today {
            continue;
        }
        match yesterday {
            None => {
                yesterday = Some(date);
                high = Some(candle.high);
                low = Some(candle.low);
            }
            Some(y) if y == date => {
                high = high.max(Some(candle.high));
                low = low.min(Some(candle.low));
            }
            Some(_) => break,
        }
    }
    match (high, low) {
        (Some(high), Some(low)) => Some(YesterdayLevels { high, low }),
        _ => None,
    }
}

/// Janela operacional (primeira hora): o último candle deve estar dentro de
/// `trading_start_time`–`trading_end_time` (UTC, ver TOML).
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

/// Fração do corpo da barra em relação ao range (0 em barra sem range).
pub fn body_fraction(candle: &Candle) -> Decimal {
    if candle.range().is_zero() {
        return Decimal::ZERO;
    }
    (candle.close - candle.open).abs() / candle.range()
}

/// Veto de momentum (seção 4 do doc): as `momentum_bars` barras anteriores à
/// de sinal fecharam ALÉM do nível com corpos fortes — o teste já virou
/// rompimento de verdade, não falha.
///
/// `against_up = true` quando o fade é para baixo (momentum de alta contra).
pub fn momentum_veto(
    candles: &[Candle],
    level: Decimal,
    against_up: bool,
    params: &StrategyParameters,
) -> bool {
    if candles.len() < params.momentum_bars + 1 {
        return false;
    }
    let end = candles.len() - 1; // exclui a barra de sinal
    let start = end.saturating_sub(params.momentum_bars);
    let mut strong_beyond = 0usize;
    for candle in &candles[start..end] {
        let beyond = if against_up {
            candle.close > level
        } else {
            candle.close < level
        };
        if beyond && body_fraction(candle) >= params.momentum_body_pct {
            strong_beyond += 1;
        }
    }
    strong_beyond >= params.momentum_bars
}

/// Veto de trend bars contra (Cap. 1): `counter_trend_bars` ou mais barras
/// fortes contra a direção do fade na janela recente.
pub fn counter_trend_veto(
    candles: &[Candle],
    against_up: bool,
    params: &StrategyParameters,
) -> bool {
    if candles.len() < params.counter_window + 1 {
        return false;
    }
    let end = candles.len() - 1;
    let start = end.saturating_sub(params.counter_window);
    let mut count = 0usize;
    for candle in &candles[start..end] {
        let is_trend_bar = body_fraction(candle) >= params.momentum_body_pct;
        let direction_up = candle.close > candle.open;
        if is_trend_bar && direction_up == against_up {
            count += 1;
        }
    }
    count >= params.counter_trend_bars
}
