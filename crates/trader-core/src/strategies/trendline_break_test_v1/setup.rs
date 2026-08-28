//! Detecção do setup da Trendline Break Test v1 (seções 6 e 7 do doc):
//! o teste do extremo antigo depois do rompimento, com barra de reversão.

use rust_decimal::Decimal;
use serde_json::json;

use crate::strategies::trendline_break_test_v1::config::StrategyParameters;
use crate::strategies::trendline_break_test_v1::context::{
    TrendContext, TrendKind, TrendlineBreak,
};
use trader_domain::{Candle, Direction, RejectionReason};

/// Setup detectado: a reversão que vamos operar.
#[derive(Debug, Clone, PartialEq)]
pub struct Setup {
    pub direction: Direction,
    /// Índice da barra de sinal (a barra do teste).
    pub signal_index: usize,
    /// Extremo do teste — referência do stop (Cap. 8).
    pub test_extreme: Decimal,
    /// Distância assinada do teste ao extremo antigo: positiva = undershoot
    /// (não alcançou), negativa = overshoot (furou).
    pub test_offset: Decimal,
    /// `true` quando o teste furou o extremo antigo (Lower Low / Higher High).
    pub is_overshoot: bool,
    pub body_pct: Decimal,
    pub wick_pct: Decimal,
}

type Rejection = (RejectionReason, serde_json::Value);

/// Detecta o setup na última barra da série.
pub fn detect_setup(
    candles: &[Candle],
    trend: &TrendContext,
    brk: &TrendlineBreak,
    atr_value: Decimal,
    params: &StrategyParameters,
) -> Result<Setup, Rejection> {
    let n = candles.len();
    let signal_index = n - 1;

    // O teste tem que vir DEPOIS do rompimento.
    if signal_index <= brk.index {
        return Err((
            RejectionReason::NoExtremeTest,
            json!({ "reason": "barra de sinal não é posterior ao rompimento" }),
        ));
    }

    let bar = &candles[signal_index];
    let tolerance = trend.extreme_price.abs() * params.test_tolerance_pct / Decimal::from(100);
    let max_overshoot = params.max_overshoot_atr * atr_value;

    // Seção 6: o teste do extremo, com undershoot ou overshoot.
    let (direction, test_extreme, offset, is_overshoot) = match trend.kind {
        TrendKind::Bear => {
            let offset = bar.low - trend.extreme_price; // >0 undershoot, <0 overshoot
            (Direction::Long, bar.low, offset, offset < Decimal::ZERO)
        }
        TrendKind::Bull => {
            let offset = trend.extreme_price - bar.high; // >0 undershoot, <0 overshoot
            (Direction::Short, bar.high, offset, offset < Decimal::ZERO)
        }
    };

    if is_overshoot {
        // "the reversal is nullified, and the old trend has resumed" (Cap. 8)
        if offset.abs() > max_overshoot {
            return Err((
                RejectionReason::ReversalNullified,
                json!({
                    "reason": "overshoot além do limite — tendência antiga retomada (Cap. 8)",
                    "overshoot": offset.abs(),
                    "max_overshoot": max_overshoot,
                }),
            ));
        }
    } else if offset > tolerance {
        return Err((
            RejectionReason::NoExtremeTest,
            json!({
                "reason": "preço não voltou perto o bastante do extremo antigo",
                "distancia": offset,
                "tolerancia": tolerance,
            }),
        ));
    }

    // Seção 7: barra de reversão a favor da direção.
    let range = bar.range();
    if range.is_zero() {
        return Err((
            RejectionReason::WeakConfirmation,
            json!({ "reason": "barra de sinal sem range" }),
        ));
    }
    let body_pct = (bar.close - bar.open).abs() / range;
    let (wick, favorable_close, right_side) = match direction {
        Direction::Long => (
            bar.close.min(bar.open) - bar.low,
            bar.close >= bar.low + range * Decimal::from(2) / Decimal::from(3),
            bar.close > bar.open,
        ),
        Direction::Short => (
            bar.high - bar.close.max(bar.open),
            bar.close <= bar.low + range / Decimal::from(3),
            bar.close < bar.open,
        ),
    };
    let wick_pct = wick / range;

    if !right_side
        || body_pct < params.signal_body_min_pct
        || wick_pct < params.signal_wick_min_pct
        || !favorable_close
    {
        return Err((
            RejectionReason::WeakConfirmation,
            json!({
                "reason": "barra de sinal não atende aos critérios de reversão (Cap. 1)",
                "direcao_correta": right_side,
                "body_pct": body_pct,
                "wick_pct": wick_pct,
                "fechamento_favoravel": favorable_close,
            }),
        ));
    }

    Ok(Setup {
        direction,
        signal_index,
        test_extreme,
        test_offset: offset,
        is_overshoot,
        body_pct,
        wick_pct,
    })
}
