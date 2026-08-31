//! Contexto da Range Extreme Fade v1: detector de dia de trading range e os
//! vetos literais do livro (meio do dia/meio do range, Barb Wire, regra da
//! EMA). Seção 4 do doc (`docs/strategies/range-extreme-fade-v1.md`).

use chrono::{DateTime, Utc};
use chrono_tz::America::New_York;
use rust_decimal::Decimal;
use serde_json::json;

use crate::indicators::ema;
use crate::strategies::range_extreme_fade_v1::config::StrategyParameters;
use trader_domain::{Candle, RejectionReason};

/// Data do candle em America/New_York (o pregão americano é definido em ET).
fn et_date(ts: DateTime<Utc>) -> chrono::NaiveDate {
    ts.with_timezone(&New_York).date_naive()
}

/// Índice da primeira barra do dia corrente (em ET) na série.
pub fn current_day_start(candles: &[Candle]) -> usize {
    let Some(last) = candles.last() else { return 0 };
    let today = et_date(last.timestamp);
    let mut start = candles.len() - 1;
    while start > 0 && et_date(candles[start - 1].timestamp) == today {
        start -= 1;
    }
    start
}

/// Máxima e mínima do dia corrente ATÉ a penúltima barra (o extremo que a
/// barra de sinal pode estar rompendo). `None` se a barra de sinal é a
/// primeira do dia.
pub fn day_extremes_before_signal(candles: &[Candle]) -> Option<(Decimal, Decimal)> {
    let start = current_day_start(candles);
    if candles.len() - 1 <= start {
        return None;
    }
    let mut high = candles[start].high;
    let mut low = candles[start].low;
    for c in &candles[start..candles.len() - 1] {
        high = high.max(c.high);
        low = low.min(c.low);
    }
    Some((high, low))
}

/// Range do dia corrente incluindo a barra de sinal.
pub fn day_range(candles: &[Candle]) -> Decimal {
    let start = current_day_start(candles);
    let mut high = candles[start].high;
    let mut low = candles[start].low;
    for c in &candles[start..] {
        high = high.max(c.high);
        low = low.min(c.low);
    }
    high - low
}

/// ATR diário: média dos ranges dos últimos `period` dias COMPLETOS
/// (anteriores ao dia corrente). `None` se não há dias suficientes na série.
pub fn daily_atr(candles: &[Candle], period: usize) -> Option<Decimal> {
    let today = et_date(candles.last()?.timestamp);
    let mut ranges: Vec<Decimal> = Vec::new();
    let mut current: Option<(chrono::NaiveDate, Decimal, Decimal)> = None;
    for c in candles {
        let d = et_date(c.timestamp);
        if d >= today {
            break;
        }
        match current {
            Some((cd, hi, lo)) if cd == d => {
                current = Some((cd, hi.max(c.high), lo.min(c.low)));
            }
            Some((_, hi, lo)) => {
                ranges.push(hi - lo);
                current = Some((d, c.high, c.low));
            }
            None => current = Some((d, c.high, c.low)),
        }
    }
    if let Some((_, hi, lo)) = current {
        ranges.push(hi - lo);
    }
    if ranges.len() < period {
        return None;
    }
    let sum: Decimal = ranges.iter().rev().take(period).sum();
    Some(sum / Decimal::from(period))
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

/// Pivôs de 2 barras de cada lado dentro da janela final da série.
/// Retorna (índices de swing highs, índices de swing lows), absolutos.
fn pivots(candles: &[Candle], lookback: usize) -> (Vec<usize>, Vec<usize>) {
    let n = candles.len();
    let from = n.saturating_sub(lookback);
    let mut highs = Vec::new();
    let mut lows = Vec::new();
    // Pivô confirmado precisa de 2 barras de cada lado: último avaliável é n-3.
    if n < 5 {
        return (highs, lows);
    }
    for i in from.max(2)..n - 2 {
        let h = candles[i].high;
        let l = candles[i].low;
        if h > candles[i - 1].high
            && h > candles[i - 2].high
            && h >= candles[i + 1].high
            && h >= candles[i + 2].high
        {
            highs.push(i);
        }
        if l < candles[i - 1].low
            && l < candles[i - 2].low
            && l <= candles[i + 1].low
            && l <= candles[i + 2].low
        {
            lows.push(i);
        }
    }
    (highs, lows)
}

/// Estrutura de tendência nas últimas `lookback` barras: 2+ pivôs formando
/// HH/HL (alta) ou LH/LL (baixa).
fn has_trend_structure(candles: &[Candle], lookback: usize) -> bool {
    let (highs, lows) = pivots(candles, lookback);
    if highs.len() < 2 || lows.len() < 2 {
        return false;
    }
    let hh = candles[highs[highs.len() - 1]].high > candles[highs[highs.len() - 2]].high;
    let hl = candles[lows[lows.len() - 1]].low > candles[lows[lows.len() - 2]].low;
    let lh = candles[highs[highs.len() - 1]].high < candles[highs[highs.len() - 2]].high;
    let ll = candles[lows[lows.len() - 1]].low < candles[lows[lows.len() - 2]].low;
    (hh && hl) || (lh && ll)
}

/// Inclinação média da EMA20 por barra na janela de estrutura, em fração do
/// preço (|ema_agora − ema_(lookback-1 barras atrás)| / lookback / ema_agora).
/// `None` se não há histórico suficiente.
pub fn ema_slope_per_bar(candles: &[Candle], lookback: usize) -> Option<Decimal> {
    let n = candles.len();
    if n <= lookback {
        return None;
    }
    let ema_now = ema(candles, 20)?;
    let ema_then = ema(&candles[..n - lookback + 1], 20)?;
    if ema_now.is_zero() {
        return None;
    }
    Some((ema_now - ema_then).abs() / Decimal::from(lookback) / ema_now)
}

/// Seção 4.1 do doc: o dia é de trading range? (EMA flat + sem estrutura de
/// tendência + range do dia contido no ATR diário).
pub fn check_range_day(
    candles: &[Candle],
    daily_atr_value: Decimal,
    params: &StrategyParameters,
) -> Result<(), (RejectionReason, serde_json::Value)> {
    let lookback = params.structure_lookback;

    // EMA20 flat: inclinação média por barra na janela abaixo do limiar.
    match ema_slope_per_bar(candles, lookback) {
        Some(slope) => {
            if slope >= params.max_ema_slope_pct_per_bar {
                return Err((
                    RejectionReason::NotARangeDay,
                    json!({ "reason": "EMA20 inclinada (dia com viés direcional)", "slope_per_bar": slope }),
                ));
            }
        }
        None => {
            return Err((
                RejectionReason::IncompleteSetup,
                json!({ "reason": "histórico insuficiente para EMA20 na janela" }),
            ));
        }
    }

    // Sem estrutura de tendência por pivôs.
    if has_trend_structure(candles, lookback) {
        return Err((
            RejectionReason::NotARangeDay,
            json!({ "reason": "estrutura de tendência (HH/HL ou LH/LL) na janela" }),
        ));
    }

    // Range do dia contido.
    let range = day_range(candles);
    if range >= params.max_day_range_atr_mult * daily_atr_value {
        return Err((
            RejectionReason::NotARangeDay,
            json!({ "reason": "range do dia grande demais (possível trend day)", "day_range": range, "daily_atr": daily_atr_value }),
        ));
    }

    Ok(())
}

/// Seção 4.4 do doc (literal, Cap. 5): meio do dia (11:30–14:00 ET) E preço
/// no terço central do range do dia → não operar.
pub fn is_midday_midrange(candles: &[Candle], params: &StrategyParameters) -> bool {
    let Some(last) = candles.last() else {
        return false;
    };
    let parse = |s: &str| {
        let p: Vec<&str> = s.split(':').collect();
        chrono::NaiveTime::from_hms_opt(
            p.first().and_then(|v| v.parse().ok()).unwrap_or(0),
            p.get(1).and_then(|v| v.parse().ok()).unwrap_or(0),
            p.get(2).and_then(|v| v.parse().ok()).unwrap_or(0),
        )
        .unwrap_or_default()
    };
    // A janela de veto é definida em UTC no TOML (como as demais); comparar
    // em UTC para consistência com check_trading_hours.
    let time_utc = last.timestamp.time();
    let in_midday =
        time_utc >= parse(&params.midday_start_time) && time_utc <= parse(&params.midday_end_time);
    if !in_midday {
        return false;
    }
    let start = current_day_start(candles);
    let mut high = candles[start].high;
    let mut low = candles[start].low;
    for c in &candles[start..] {
        high = high.max(c.high);
        low = low.min(c.low);
    }
    let range = high - low;
    if range.is_zero() {
        return true; // dia totalmente flat no meio do dia = equilíbrio puro
    }
    let third = range / Decimal::from(3);
    let price = last.close;
    price >= low + third && price <= high - third
}

/// Seção 4.5 do doc (literal, Cap. 5): Barb Wire — 3+ barras com sobreposição
/// ≥ 50% do range médio e ao menos 1 doji (corpo < 30% do range).
pub fn is_barb_wire(candles: &[Candle], params: &StrategyParameters) -> bool {
    let n = candles.len();
    let count = params.barb_wire_bars;
    if n < count + 1 {
        return false;
    }
    let Some(avg_range) = atr(candles, params.atr_period) else {
        return false;
    };
    let tail = &candles[n - count..];
    let mut doji = false;
    for (j, c) in tail.iter().enumerate() {
        let range = c.range();
        if !range.is_zero() && (c.close - c.open).abs() < params.barb_wire_doji_body_pct * range {
            doji = true;
        }
        if j == 0 {
            continue;
        }
        let prev = &tail[j - 1];
        let overlap = prev.high.min(c.high) - prev.low.max(c.low);
        if overlap < params.barb_wire_overlap_pct * avg_range {
            return false; // uma barra fora do padrão desfaz o arame farpado
        }
    }
    doji
}

/// Seção 4.3 do doc (Cap. 5): regra da EMA — long só se a maioria dos últimos
/// `ema_side_lookback` closes está ABAIXO da EMA20; short só se ACIMA.
/// Retorna true quando a regra é violada (veto).
pub fn ema_side_veto(candles: &[Candle], direction: trader_domain::Direction) -> bool {
    let Some(ema_value) = ema(candles, 20) else {
        return false; // sem EMA não há veto (a checagem de contexto já falha antes)
    };
    let lookback = 8usize;
    if candles.len() < lookback {
        return false;
    }
    let tail = &candles[candles.len() - lookback..];
    let above = tail.iter().filter(|c| c.close > ema_value).count();
    match direction {
        trader_domain::Direction::Long => above * 2 >= lookback, // maioria acima → veta long
        trader_domain::Direction::Short => above * 2 < lookback, // maioria abaixo → veta short
    }
}

/// Janela operacional (mesmo padrão das irmãs): último candle dentro de
/// `trading_start_time`–`trading_end_time` (horário de NY, ver TOML).
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
    // Horário de NOVA YORK, não UTC: a janela em UTC fixo deslizava uma hora
    // na virada do DST (A2 da auditoria de 30/08/2026).
    let time = crate::session::et_time(last.timestamp);
    let start = crate::session::parse_et_time(&params.trading_start_time);
    let end = crate::session::parse_et_time(&params.trading_end_time);
    if time < start || time > end {
        return Err((
            RejectionReason::OutsideTradingHours,
            json!({ "time": time.to_string(), "start": start.to_string(), "end": end.to_string() }),
        ));
    }
    Ok(())
}

/// Fração do corpo da barra em relação ao range (0 em barra sem range).
pub fn body_fraction(candle: &Candle) -> Decimal {
    if candle.range().is_zero() {
        return Decimal::ZERO;
    }
    (candle.close - candle.open).abs() / candle.range()
}
