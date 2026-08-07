//! Setup da estratégia Opening Reversal v1: zona de teste do nível de ontem,
//! barra de sinal (reversal bar forte, Cap. 1) e vetos. Seção 5 do doc.

use rust_decimal::Decimal;
use serde_json::json;

use crate::strategies::opening_reversal_v1::config::StrategyParameters;
use crate::strategies::opening_reversal_v1::context::{
    counter_trend_veto, momentum_veto, YesterdayLevels,
};
use trader_domain::{Candle, Direction, RejectionReason};

/// Setup completo, pronto para cálculo de preços.
#[derive(Debug, Clone, PartialEq)]
pub struct Setup {
    pub direction: Direction,
    /// Nível de ontem testado (máxima para short, mínima para long).
    pub level: Decimal,
    /// Índice da barra de sinal (última da série).
    pub signal_index: usize,
    pub body_pct: Decimal,
    pub wick_pct: Decimal,
}

/// Detecta o setup na última barra da série (seções 4–5 do doc).
pub fn detect_setup(
    candles: &[Candle],
    levels: &YesterdayLevels,
    params: &StrategyParameters,
) -> Result<Setup, (RejectionReason, serde_json::Value)> {
    let signal_index = candles.len() - 1;
    let bar = &candles[signal_index];

    // Zona de teste: a barra toca (ou quase toca, dentro da banda) o nível.
    let zone_high = levels.high * (Decimal::ONE - params.level_zone_pct);
    let zone_low = levels.low * (Decimal::ONE + params.level_zone_pct);
    let touches_high = bar.high >= zone_high;
    let touches_low = bar.low <= zone_low;

    let (direction, level) = match (touches_high, touches_low) {
        (true, false) => (Direction::Short, levels.high),
        (false, true) => (Direction::Long, levels.low),
        (true, true) => {
            return Err((
                RejectionReason::IncompleteSetup,
                json!({ "reason": "barra tocou os dois níveis de ontem (ambiguo)" }),
            ))
        }
        (false, false) => {
            return Err((
                RejectionReason::YesterdayLevelNotTested,
                json!({
                    "reason": "barra fora da zona de teste dos níveis de ontem",
                    "yesterday_high": levels.high,
                    "yesterday_low": levels.low,
                    "bar_high": bar.high,
                    "bar_low": bar.low,
                }),
            ))
        }
    };

    // Barra de sinal forte (Cap. 1): corpo na direção do fade, fechamento no
    // terço favorável, sombra do lado do teste ≥ 1/3 do range, corpo ≥ 30%.
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

    // Vetos de momentum (seção 4 do doc).
    let against_up = direction == Direction::Short;
    if momentum_veto(candles, level, against_up, params)
        || counter_trend_veto(candles, against_up, params)
    {
        return Err((
            RejectionReason::MomentumAgainst,
            json!({ "reason": "momentum contra o fade (vetos do doc §4)" }),
        ));
    }

    Ok(Setup {
        direction,
        level,
        signal_index,
        body_pct,
        wick_pct,
    })
}
