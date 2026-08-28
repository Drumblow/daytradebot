//! Contexto da Trendline Break Test v1: estrutura de tendência, cálculo e
//! extrapolação de trendline por pivôs, e detecção do rompimento com momentum.
//! Seções 4 e 5 do doc (`docs/strategies/trendline-break-test-v1.md`).

use rust_decimal::Decimal;
use serde_json::json;

use crate::indicators::ema;
use crate::strategies::trendline_break_test_v1::config::StrategyParameters;
use trader_domain::{Candle, RejectionReason};

/// Sentido da tendência ANTIGA (a que a reversão pretende encerrar).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrendKind {
    /// Tendência de baixa — candidata a reversão LONG (fundo).
    Bear,
    /// Tendência de alta — candidata a reversão SHORT (topo).
    Bull,
}

/// Tendência estabelecida e seus pontos de referência.
#[derive(Debug, Clone, PartialEq)]
pub struct TrendContext {
    pub kind: TrendKind,
    /// Extremo da tendência: mínima (bear) ou máxima (bull).
    pub extreme_price: Decimal,
    pub extreme_index: usize,
    /// Último swing contrário: o último lower high (bear) ou higher low (bull)
    /// — o nível que a perna contrária precisa superar (Cap. 8).
    pub last_counter_swing: Decimal,
    /// Os dois pivôs que definem a trendline.
    pub line_p1: usize,
    pub line_p2: usize,
}

/// Rompimento da trendline localizado na série.
#[derive(Debug, Clone, PartialEq)]
pub struct TrendlineBreak {
    /// Índice da barra cujo fechamento rompeu a linha.
    pub index: usize,
    /// Valor da linha extrapolada nessa barra.
    pub line_value: Decimal,
    /// Barras da perna contrária no momento do rompimento.
    pub leg_bars: usize,
    /// Fechamentos além da EMA20 na perna contrária.
    pub closes_beyond_ema: usize,
}

type Rejection = (RejectionReason, serde_json::Value);

/// Pivôs de swing com `pivot_bars` barras de cada lado, dentro da janela final.
/// Retorna (índices de swing highs, índices de swing lows), absolutos.
pub fn pivots(candles: &[Candle], lookback: usize, pivot_bars: usize) -> (Vec<usize>, Vec<usize>) {
    let n = candles.len();
    let p = pivot_bars.max(1);
    let mut highs = Vec::new();
    let mut lows = Vec::new();
    if n < 2 * p + 1 {
        return (highs, lows);
    }
    let from = n.saturating_sub(lookback).max(p);
    for i in from..n - p {
        let h = candles[i].high;
        let l = candles[i].low;
        let is_high =
            (1..=p).all(|k| h > candles[i - k].high) && (1..=p).all(|k| h >= candles[i + k].high);
        let is_low =
            (1..=p).all(|k| l < candles[i - k].low) && (1..=p).all(|k| l <= candles[i + k].low);
        if is_high {
            highs.push(i);
        }
        if is_low {
            lows.push(i);
        }
    }
    (highs, lows)
}

/// Seção 4: existe tendência estabelecida na janela?
///
/// A estrutura é avaliada **até o extremo da tendência**, nunca incluindo a
/// perna contrária: é ela que vai reverter, e o pivô que ela cria desfaria a
/// própria estrutura que estamos tentando reconhecer.
pub fn detect_trend(
    candles: &[Candle],
    params: &StrategyParameters,
) -> Result<TrendContext, Rejection> {
    let n = candles.len();
    let window = params.trend_lookback + params.break_max_age;
    let from = n.saturating_sub(window);
    if n < from + 2 * params.pivot_bars + 3 {
        return Err((
            RejectionReason::NoTrendToReverse,
            json!({ "reason": "janela curta demais para reconhecer estrutura" }),
        ));
    }

    // Extremos candidatos dentro da janela.
    //
    // O extremo precisa deixar espaço para a perna contrária E para o teste que
    // vem depois dela. Sem esse recuo, um teste em OVERSHOOT (Lower Low, que o
    // Cap. 8 trata como metade dos casos válidos) seria eleito como o próprio
    // extremo da tendência e o setup nunca dispararia.
    let min_gap = params.break_min_bars + 2;
    let last_candidate = n.saturating_sub(min_gap + 1);
    if last_candidate <= from {
        return Err((
            RejectionReason::NoTrendToReverse,
            json!({ "reason": "janela sem espaço para tendência + perna contrária + teste" }),
        ));
    }
    let mut bear_idx = from;
    let mut bull_idx = from;
    for i in from..=last_candidate {
        if candles[i].low < candles[bear_idx].low {
            bear_idx = i;
        }
        if candles[i].high > candles[bull_idx].high {
            bull_idx = i;
        }
    }

    let bear = qualify_trend(candles, params, bear_idx, TrendKind::Bear);
    let bull = qualify_trend(candles, params, bull_idx, TrendKind::Bull);

    match (bear, bull) {
        // Se os dois qualificam, vale o extremo mais recente.
        (Some(a), Some(b)) => Ok(if a.extreme_index >= b.extreme_index {
            a
        } else {
            b
        }),
        (Some(a), None) => Ok(a),
        (None, Some(b)) => Ok(b),
        (None, None) => Err((
            RejectionReason::NoTrendToReverse,
            json!({
                "reason": "sem estrutura LH/LL nem HH/HL até o extremo da janela",
                "bear_extreme_index": bear_idx,
                "bull_extreme_index": bull_idx,
            }),
        )),
    }
}

/// Verifica se os pivôs ATÉ `extreme_idx` formam a tendência de `kind`.
fn qualify_trend(
    candles: &[Candle],
    params: &StrategyParameters,
    extreme_idx: usize,
    kind: TrendKind,
) -> Option<TrendContext> {
    // Precisa existir perna contrária depois do extremo.
    if extreme_idx + 1 >= candles.len() {
        return None;
    }
    let upto = &candles[..=extreme_idx];
    let window = params.trend_lookback + params.break_max_age;
    let (highs, lows) = pivots(upto, window, params.pivot_bars);

    match kind {
        TrendKind::Bear => {
            if highs.len() < 2 {
                return None;
            }
            let h1 = highs[highs.len() - 2];
            let h2 = highs[highs.len() - 1];
            // Lower highs na tendência...
            if candles[h2].high >= candles[h1].high {
                return None;
            }
            // ...e o extremo é um lower low em relação ao último swing low.
            let prev_low = *lows.last()?;
            if candles[extreme_idx].low >= candles[prev_low].low {
                return None;
            }
            Some(TrendContext {
                kind,
                extreme_price: candles[extreme_idx].low,
                extreme_index: extreme_idx,
                last_counter_swing: candles[h2].high,
                line_p1: h1,
                line_p2: h2,
            })
        }
        TrendKind::Bull => {
            if lows.len() < 2 {
                return None;
            }
            let l1 = lows[lows.len() - 2];
            let l2 = lows[lows.len() - 1];
            if candles[l2].low <= candles[l1].low {
                return None;
            }
            let prev_high = *highs.last()?;
            if candles[extreme_idx].high <= candles[prev_high].high {
                return None;
            }
            Some(TrendContext {
                kind,
                extreme_price: candles[extreme_idx].high,
                extreme_index: extreme_idx,
                last_counter_swing: candles[l2].low,
                line_p1: l1,
                line_p2: l2,
            })
        }
    }
}

/// Valor da trendline (reta pelos dois pivôs) extrapolada até `at_index`.
///
/// Componente novo no projeto: nenhuma estratégia anterior calcula trendline.
pub fn trendline_value_at(
    candles: &[Candle],
    trend: &TrendContext,
    at_index: usize,
) -> Option<Decimal> {
    if trend.line_p2 <= trend.line_p1 {
        return None;
    }
    let (y1, y2) = match trend.kind {
        TrendKind::Bear => (candles[trend.line_p1].high, candles[trend.line_p2].high),
        TrendKind::Bull => (candles[trend.line_p1].low, candles[trend.line_p2].low),
    };
    let span = Decimal::from((trend.line_p2 - trend.line_p1) as i64);
    let slope = (y2 - y1) / span;
    let delta = Decimal::from(at_index as i64) - Decimal::from(trend.line_p2 as i64);
    Some(y2 + slope * delta)
}

/// Seção 5: a trendline foi rompida em FECHAMENTO, com momentum, e o
/// rompimento é recente o bastante?
///
/// O momentum é medido sobre a PRIMEIRA PERNA da reversão inteira — do extremo
/// da tendência até o ápice dessa perna — e não sobre o trecho anterior ao
/// cruzamento da linha. Medir até o cruzamento penalizava exatamente o que o
/// Cap. 8 chama de força: um rompimento rápido ficava com "perna curta demais".
pub fn find_trendline_break(
    candles: &[Candle],
    trend: &TrendContext,
    params: &StrategyParameters,
) -> Result<TrendlineBreak, Rejection> {
    let n = candles.len();
    let leg_start = trend.extreme_index;
    if n <= leg_start + 1 {
        return Err((
            RejectionReason::NoTrendlineBreak,
            json!({ "reason": "sem barras após o extremo da tendência" }),
        ));
    }

    // Ápice da perna contrária: o ponto mais distante do extremo alcançado
    // depois dele. É a "primeira perna da reversão" do Cap. 8.
    let mut leg_peak = leg_start + 1;
    for i in (leg_start + 1)..n {
        let better = match trend.kind {
            TrendKind::Bear => candles[i].high > candles[leg_peak].high,
            TrendKind::Bull => candles[i].low < candles[leg_peak].low,
        };
        if better {
            leg_peak = i;
        }
    }

    // (d) Rompimento: primeiro FECHAMENTO além da linha dentro da perna.
    let mut found: Option<(usize, Decimal)> = None;
    for i in (leg_start + 1)..=leg_peak {
        let Some(line) = trendline_value_at(candles, trend, i) else {
            continue;
        };
        let broke = match trend.kind {
            TrendKind::Bear => candles[i].close > line,
            TrendKind::Bull => candles[i].close < line,
        };
        if broke {
            found = Some((i, line));
            break;
        }
    }

    let Some((index, line_value)) = found else {
        return Err((
            RejectionReason::NoTrendlineBreak,
            json!({ "reason": "nenhum fechamento além da trendline na perna contrária" }),
        ));
    };

    if n - 1 - index > params.break_max_age {
        return Err((
            RejectionReason::BreakTooOld,
            json!({
                "reason": "rompimento antigo demais para o teste ser desta reversão",
                "idade_barras": n - 1 - index,
                "break_max_age": params.break_max_age,
            }),
        ));
    }

    // (a) tamanho da perna, (b) fechamentos além da EMA20, (c) superação do
    // último swing contrário — todos medidos sobre a perna inteira.
    let leg_bars = leg_peak - leg_start;

    let mut closes_beyond_ema = 0usize;
    for i in (leg_start + 1)..=leg_peak {
        if let Some(ema20) = ema(&candles[..=i], 20) {
            let beyond = match trend.kind {
                TrendKind::Bear => candles[i].close > ema20,
                TrendKind::Bull => candles[i].close < ema20,
            };
            if beyond {
                closes_beyond_ema += 1;
            }
        }
    }

    let exceeded_swing = match trend.kind {
        TrendKind::Bear => candles[leg_peak].high > trend.last_counter_swing,
        TrendKind::Bull => candles[leg_peak].low < trend.last_counter_swing,
    };

    if leg_bars < params.break_min_bars
        || closes_beyond_ema < params.break_min_closes_beyond_ema
        || !exceeded_swing
    {
        return Err((
            RejectionReason::BreakWithoutMomentum,
            json!({
                "reason": "primeira perna sem os sinais de força do Cap. 8",
                "leg_bars": leg_bars,
                "break_min_bars": params.break_min_bars,
                "closes_beyond_ema": closes_beyond_ema,
                "break_min_closes_beyond_ema": params.break_min_closes_beyond_ema,
                "exceeded_last_counter_swing": exceeded_swing,
            }),
        ));
    }

    Ok(TrendlineBreak {
        index,
        line_value,
        leg_bars,
        closes_beyond_ema,
    })
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

/// Janela operacional (mesmo padrão das irmãs), em UTC.
pub fn check_trading_hours(
    candles: &[Candle],
    params: &StrategyParameters,
) -> Result<(), Rejection> {
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
