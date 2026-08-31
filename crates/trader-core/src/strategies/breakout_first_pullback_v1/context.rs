//! Contexto da estratégia Breakout — Primeiro Pullback v1: nível de
//! resistência, validação do breakout (expansão de range/volume) e horário.
//!
//! Regras da seção 4 do doc (`docs/strategies/breakout-first-pullback-v1.md`):
//! nível testado ≥ 2× em 80 candles; breakout fecha além de R + 0,25×ATR com
//! range e volume ≥ 1,5× a média de 20; só a primeira tentativa de rompimento.

use rust_decimal::Decimal;
use serde_json::json;
use tracing::debug;

use crate::strategies::breakout_first_pullback_v1::config::StrategyParameters;
use trader_domain::{Candle, RejectionReason};

/// Nível de resistência detectado.
#[derive(Debug, Clone, PartialEq)]
pub struct ResistanceLevel {
    pub price: Decimal,
    pub touches: usize,
}

/// Barra de breakout validada.
#[derive(Debug, Clone, PartialEq)]
pub struct Breakout {
    /// Índice da barra de breakout na série.
    pub index: usize,
    pub level: ResistanceLevel,
    /// Range da barra de breakout ÷ range médio(20).
    pub range_ratio: Decimal,
    /// Volume da barra de breakout ÷ volume médio(20).
    pub volume_ratio: Decimal,
}

/// Janela de horário (ET) da última barra da série.
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

/// Média de range das `period` barras que terminam em `end_exclusive`.
pub fn avg_range(candles: &[Candle], end_exclusive: usize, period: usize) -> Option<Decimal> {
    if end_exclusive < period || end_exclusive > candles.len() || period == 0 {
        return None;
    }
    let sum: Decimal = candles[end_exclusive - period..end_exclusive]
        .iter()
        .map(|c| c.range())
        .sum();
    Some(sum / Decimal::from(period))
}

/// Média de volume das `period` barras que terminam em `end_exclusive`.
pub fn avg_volume(candles: &[Candle], end_exclusive: usize, period: usize) -> Option<Decimal> {
    if end_exclusive < period || end_exclusive > candles.len() || period == 0 {
        return None;
    }
    let sum: Decimal = candles[end_exclusive - period..end_exclusive]
        .iter()
        .map(|c| c.volume)
        .sum();
    Some(sum / Decimal::from(period))
}

/// Valida se `index` é uma barra de breakout do nível R (seção 4 do doc).
///
/// - Nível R = maior máxima do lookback, com ≥ `level_min_touches` toques e
///   nenhum fechamento acima dele antes (primeira tentativa).
/// - Breakout: close > R + `breakout_close_atr_mult`×ATR, range ≥ mult ×
///   média, volume ≥ mult × média.
pub fn detect_breakout(
    candles: &[Candle],
    index: usize,
    atr_value: Decimal,
    params: &StrategyParameters,
) -> Result<Breakout, (RejectionReason, serde_json::Value)> {
    if index < params.level_lookback_candles || index >= candles.len() {
        return Err((
            RejectionReason::IncompleteSetup,
            json!({ "reason": "histórico insuficiente para nível/breakout", "index": index }),
        ));
    }

    let lookback = &candles[index - params.level_lookback_candles..index];
    let level_price = lookback.iter().map(|c| c.high).max().unwrap_or_default();
    let margin = params.breakout_close_atr_mult * atr_value;
    let bar = &candles[index];

    // A barra precisa romper o nível com folga mínima — sem isso, ela não é
    // candidata a breakout (rejeição não-estrutural, para não mascarar o
    // motivo real de candidatas melhores na janela).
    if bar.close <= level_price + margin {
        return Err((
            RejectionReason::IncompleteSetup,
            json!({
                "reason": "barra não rompe o nível com folga mínima",
                "level": level_price,
                "close": bar.close,
                "atr": atr_value,
            }),
        ));
    }

    // Só a primeira tentativa do nível (literal, Cap. 6): se o nível já foi
    // rompido ANTES do lookback (algum fechamento acima de R na janela
    // anterior, de até 2× o lookback), a barra atual é uma segunda tentativa.
    // Como R é a máxima do lookback, nenhum fechamento dentro dele pode
    // superá-lo — por isso a checagem usa a janela anterior. Sem histórico
    // suficiente, não há como acusar segunda tentativa: segue o jogo.
    let lookback_start = index - params.level_lookback_candles;
    let prior_start = index.saturating_sub(2 * params.level_lookback_candles);
    if lookback_start > 0 {
        let prior_breakout = candles[prior_start..lookback_start]
            .iter()
            .any(|c| c.close > level_price);
        if prior_breakout {
            return Err((
                RejectionReason::BreakoutAlreadyTaken,
                json!({
                    "reason": "nível já foi rompido antes do lookback (segunda tentativa)",
                    "level": level_price,
                }),
            ));
        }
    }

    let tolerance = level_price * params.level_touch_tolerance_pct;
    let touches = lookback
        .iter()
        .filter(|c| c.high >= level_price - tolerance)
        .count();
    if touches < params.level_min_touches {
        return Err((
            RejectionReason::ResistanceLevelNotFound,
            json!({
                "reason": "nível sem toques suficientes no lookback",
                "level": level_price,
                "touches": touches,
                "min_touches": params.level_min_touches,
            }),
        ));
    }

    let avg_r = avg_range(candles, index, params.avg_period).unwrap_or(Decimal::ONE);
    let avg_v = avg_volume(candles, index, params.avg_period).unwrap_or(Decimal::ONE);
    let range_ratio = if avg_r.is_zero() {
        Decimal::ZERO
    } else {
        bar.range() / avg_r
    };
    let volume_ratio = if avg_v.is_zero() {
        Decimal::ZERO
    } else {
        bar.volume / avg_v
    };

    if range_ratio < params.breakout_range_mult || volume_ratio < params.breakout_volume_mult {
        debug!(
            %range_ratio,
            %volume_ratio,
            "breakout sem expansão suficiente"
        );
        return Err((
            RejectionReason::WeakBreakout,
            json!({
                "reason": "breakout sem expansão de range/volume",
                "range_ratio": range_ratio,
                "volume_ratio": volume_ratio,
                "range_mult": params.breakout_range_mult,
                "volume_mult": params.breakout_volume_mult,
            }),
        ));
    }

    Ok(Breakout {
        index,
        level: ResistanceLevel {
            price: level_price,
            touches,
        },
        range_ratio,
        volume_ratio,
    })
}
