//! Contexto da Value Area Reentry v1: cálculo da área de valor do dia anterior
//! por proxy TPO (Apêndice 1 do livro) e os três filtros obrigatórios do autor.
//! Seções 4 e 5.3 do doc (`docs/strategies/value-area-reentry-v1.md`).

use chrono::{DateTime, Utc};
use chrono_tz::America::New_York;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use serde_json::json;

use crate::indicators::ema;
use crate::strategies::value_area_reentry_v1::config::StrategyParameters;
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

/// Fatia de candles do último dia COMPLETO anterior ao dia corrente (em ET).
/// `None` se a série não contém um dia anterior inteiro.
pub fn previous_day_slice(candles: &[Candle]) -> Option<&[Candle]> {
    let today_start = current_day_start(candles);
    if today_start == 0 {
        return None;
    }
    let prev_end = today_start; // exclusivo
    let prev_date = et_date(candles[prev_end - 1].timestamp);
    let mut prev_start = prev_end - 1;
    while prev_start > 0 && et_date(candles[prev_start - 1].timestamp) == prev_date {
        prev_start -= 1;
    }
    // Se o dia anterior começa no índice 0 da série, ele pode estar truncado
    // (a série pode ter começado no meio do pregão). Nesse caso não confiamos
    // no perfil: uma VA calculada sobre meio dia não é a VA do livro.
    if prev_start == 0 {
        return None;
    }
    Some(&candles[prev_start..prev_end])
}

/// Área de valor do dia anterior (proxy TPO, seção 4 do doc).
#[derive(Debug, Clone, PartialEq)]
pub struct ValueArea {
    pub high: Decimal,
    pub low: Decimal,
    pub poc: Decimal,
}

impl ValueArea {
    pub fn width(&self) -> Decimal {
        self.high - self.low
    }

    pub fn contains(&self, price: Decimal) -> bool {
        price >= self.low && price <= self.high
    }
}

/// Calcula a área de valor de um dia pelo algoritmo do Apêndice 1:
/// TPOs por faixa de preço, expansão a partir do POC em pares até cobrir
/// `va_percent` dos TPOs.
pub fn compute_value_area(
    day: &[Candle],
    buckets: usize,
    va_percent: Decimal,
    tick_size: Decimal,
) -> Option<ValueArea> {
    if day.is_empty() || buckets == 0 {
        return None;
    }

    let day_high = day.iter().map(|c| c.high).fold(day[0].high, Decimal::max);
    let day_low = day.iter().map(|c| c.low).fold(day[0].low, Decimal::min);
    let range = day_high - day_low;
    if range <= Decimal::ZERO {
        return None;
    }

    // Largura da faixa: o range dividido em `buckets`, nunca menor que 1 tick.
    let mut width = range / Decimal::from(buckets);
    if width < tick_size {
        width = tick_size;
    }
    let n = ((range / width).floor().to_usize().unwrap_or(1)).max(1) + 1;

    let bucket_of = |price: Decimal| -> usize {
        let raw = ((price - day_low) / width).floor().to_usize().unwrap_or(0);
        raw.min(n - 1)
    };

    // Cada candle imprime 1 TPO em toda faixa que seu [low, high] toca.
    let mut counts = vec![0usize; n];
    for c in day {
        let lo = bucket_of(c.low);
        let hi = bucket_of(c.high);
        for count in counts.iter_mut().take(hi + 1).skip(lo) {
            *count += 1;
        }
    }

    let total: usize = counts.iter().sum();
    if total == 0 {
        return None;
    }

    // POC = faixa mais densa; empate resolvido pela mais próxima do meio.
    let middle = (n - 1) as i64 / 2;
    let mut poc = 0usize;
    for (i, &c) in counts.iter().enumerate() {
        let best = counts[poc];
        if c > best || (c == best && ((i as i64) - middle).abs() < ((poc as i64) - middle).abs()) {
            poc = i;
        }
    }

    let needed = (Decimal::from(total) * va_percent / Decimal::from(100))
        .ceil()
        .to_usize()
        .unwrap_or(total);

    // Expansão em pares (Apêndice 1): compara as 2 faixas acima com as 2
    // abaixo e incorpora o lado mais denso, até cobrir `needed` TPOs.
    let mut lo = poc;
    let mut hi = poc;
    let mut included = counts[poc];
    while included < needed && (lo > 0 || hi < n - 1) {
        let above: usize = ((hi + 1)..=(hi + 2).min(n - 1))
            .map(|i| counts.get(i).copied().unwrap_or(0))
            .sum();
        let below: usize = (lo.saturating_sub(2)..lo)
            .rev()
            .take(2)
            .map(|i| counts[i])
            .sum();

        let can_go_up = hi < n - 1;
        let can_go_down = lo > 0;

        if can_go_up && (!can_go_down || above >= below) {
            let step = (hi + 2).min(n - 1);
            for c in counts.iter().take(step + 1).skip(hi + 1) {
                included += c;
            }
            hi = step;
        } else if can_go_down {
            let step = lo.saturating_sub(2);
            for c in counts.iter().take(lo).skip(step) {
                included += c;
            }
            lo = step;
        } else {
            break;
        }
    }

    let va_low = day_low + Decimal::from(lo as i64) * width;
    let va_high = day_low + Decimal::from((hi + 1) as i64) * width;
    let poc_price =
        day_low + (Decimal::from(poc as i64) + Decimal::from(5) / Decimal::from(10)) * width;

    Some(ValueArea {
        high: va_high.min(day_high),
        low: va_low.max(day_low),
        poc: poc_price,
    })
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

/// Inclinação da EMA20 por barra, COM SINAL, em fração do preço.
/// Positiva = tendência de alta. `None` sem histórico suficiente.
pub fn ema_slope_signed(candles: &[Candle], lookback: usize) -> Option<Decimal> {
    let n = candles.len();
    if n <= lookback || lookback == 0 {
        return None;
    }
    let ema_now = ema(candles, 20)?;
    let ema_then = ema(&candles[..n - lookback + 1], 20)?;
    if ema_now.is_zero() {
        return None;
    }
    Some((ema_now - ema_then) / Decimal::from(lookback) / ema_now)
}

/// Filtro de direção do autor (seção 5.3): a travessia não pode ser contra uma
/// tendência estabelecida. Retorna `true` quando a travessia é vetada.
pub fn trend_against_traversal(
    slope: Decimal,
    direction: trader_domain::Direction,
    threshold: Decimal,
) -> bool {
    match direction {
        // Travessia ascendente vetada se a EMA cai com inclinação relevante.
        trader_domain::Direction::Long => slope < -threshold,
        // Travessia descendente vetada se a EMA sobe com inclinação relevante.
        trader_domain::Direction::Short => slope > threshold,
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
