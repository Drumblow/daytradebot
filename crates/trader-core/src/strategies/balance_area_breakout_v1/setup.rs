//! Setup da estratégia Balance-Area Breakout v1: rompimento com aceitação
//! (fechamento fora da área) e direção. Seção 5 do doc.

use serde_json::json;

use crate::strategies::balance_area_breakout_v1::context::BalanceArea;
use trader_domain::{Candle, Direction, RejectionReason};

/// Setup completo, pronto para cálculo de preços.
#[derive(Debug, Clone, PartialEq)]
pub struct Setup {
    pub direction: Direction,
    /// Índice do candle de rompimento (último da série).
    pub breakout_index: usize,
}

/// Detecta o rompimento aceito na última barra (seção 5 do doc).
pub fn detect_setup(
    candles: &[Candle],
    area: &BalanceArea,
) -> Result<Setup, (RejectionReason, serde_json::Value)> {
    let breakout_index = candles.len() - 1;
    let bar = &candles[breakout_index];

    if bar.close > area.high {
        return Ok(Setup {
            direction: Direction::Long,
            breakout_index,
        });
    }
    if bar.close < area.low {
        return Ok(Setup {
            direction: Direction::Short,
            breakout_index,
        });
    }

    Err((
        RejectionReason::IncompleteSetup,
        json!({
            "reason": "último candle fechou dentro da área (sem rompimento)",
            "close": bar.close,
            "area_high": area.high,
            "area_low": area.low,
        }),
    ))
}
