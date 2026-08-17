//! Setup da Range Extreme Fade v1: rompimento falho do extremo do dia com
//! extensão limitada + barra de sinal forte contra o rompimento. Seções 4.2,
//! 4.3 e 5 do doc (`docs/strategies/range-extreme-fade-v1.md`).

use rust_decimal::Decimal;
use serde_json::json;

use crate::strategies::range_extreme_fade_v1::config::StrategyParameters;
use crate::strategies::range_extreme_fade_v1::context;
use trader_domain::{Candle, Direction, RejectionReason};

/// Setup completo, pronto para cálculo de preços.
#[derive(Debug, Clone, PartialEq)]
pub struct Setup {
    pub direction: Direction,
    /// Extremo do dia que foi rompido (máxima para short, mínima para long).
    pub broken_extreme: Decimal,
    /// Extensão do rompimento além do extremo, em múltiplos de ATR14(15min).
    pub extension_atr: Decimal,
    /// Índice da barra de sinal (última da série).
    pub signal_index: usize,
    pub body_pct: Decimal,
    pub wick_pct: Decimal,
}

/// Detecta o setup na última barra da série (seções 4.2–5 do doc).
pub fn detect_setup(
    candles: &[Candle],
    atr_value: Decimal,
    params: &StrategyParameters,
) -> Result<Setup, (RejectionReason, serde_json::Value)> {
    let signal_index = candles.len() - 1;
    let bar = &candles[signal_index];

    // Extremo do dia ANTES da barra de sinal (o nível que ela rompe).
    let Some((prev_high, prev_low)) = context::day_extremes_before_signal(candles) else {
        return Err((
            RejectionReason::IncompleteSetup,
            json!({ "reason": "barra de sinal é a primeira do dia (sem extremo prévio)" }),
        ));
    };

    // Rompimento por pouco (seção 4.2): supera o extremo do dia, mas por no
    // máximo `max_extension_atr_mult` × ATR — rompimento "sem energia".
    let ext_up = bar.high - prev_high;
    let ext_dn = prev_low - bar.low;
    let max_ext = params.max_extension_atr_mult * atr_value;
    let (direction, broken_extreme, extension) = if ext_up > Decimal::ZERO && ext_up <= max_ext {
        (Direction::Short, prev_high, ext_up)
    } else if ext_dn > Decimal::ZERO && ext_dn <= max_ext {
        (Direction::Long, prev_low, ext_dn)
    } else if ext_up > max_ext || ext_dn > max_ext {
        return Err((
            RejectionReason::BreakoutTooStrong,
            json!({
                "reason": "rompimento com extensão além do máximo (momentum real)",
                "extension_up": ext_up,
                "extension_down": ext_dn,
                "max_extension": max_ext,
            }),
        ));
    } else {
        return Err((
            RejectionReason::NoProbe,
            json!({
                "reason": "barra não rompeu o extremo do dia",
                "prev_day_high": prev_high,
                "prev_day_low": prev_low,
                "bar_high": bar.high,
                "bar_low": bar.low,
            }),
        ));
    };
    let extension_atr = if atr_value.is_zero() {
        Decimal::ZERO
    } else {
        extension / atr_value
    };

    // Regra da EMA (seção 4.3; opcional por configuração — ver §12.1 do doc).
    if params.use_ema_side_rule && context::ema_side_veto(candles, direction) {
        return Err((
            RejectionReason::WrongSideOfEma,
            json!({ "reason": "regra da EMA violada (Cap. 5)", "direction": format!("{direction:?}") }),
        ));
    }

    // Barra de sinal forte contra o rompimento (seção 5.1; mesma família da
    // opening-reversal-v1): corpo na direção do fade, fechamento no terço
    // favorável, sombra do lado do teste ≥ 1/3 do range, corpo ≥ mínimo.
    if bar.range().is_zero() {
        return Err((
            RejectionReason::WeakConfirmation,
            json!({ "reason": "barra de sinal sem range" }),
        ));
    }
    let body = (bar.close - bar.open).abs();
    let body_pct = body / bar.range();
    let body_ok = body_pct >= params.signal_body_min_pct;
    let (direction_ok, close_ok, wick) = match direction {
        Direction::Long => {
            let lower_wick = bar.open.min(bar.close) - bar.low;
            (
                bar.close > bar.open,
                bar.close >= bar.low + bar.range() * Decimal::from(2) / Decimal::from(3),
                lower_wick,
            )
        }
        Direction::Short => {
            let upper_wick = bar.high - bar.open.max(bar.close);
            (
                bar.close < bar.open,
                bar.close <= bar.high - bar.range() * Decimal::from(2) / Decimal::from(3),
                upper_wick,
            )
        }
    };
    let wick_pct = wick / bar.range();
    let wick_ok = wick_pct >= params.signal_wick_min_pct;
    if !(body_ok && direction_ok && close_ok && wick_ok) {
        return Err((
            RejectionReason::WeakConfirmation,
            json!({
                "reason": "barra de sinal fraca (corpo/direção/terço/sombra)",
                "body_pct": body_pct,
                "wick_pct": wick_pct,
                "direction_ok": direction_ok,
                "close_ok": close_ok,
            }),
        ));
    }

    Ok(Setup {
        direction,
        broken_extreme,
        extension_atr,
        signal_index,
        body_pct,
        wick_pct,
    })
}
