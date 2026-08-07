//! Setup da estratégia Breakout — Primeiro Pullback v1: o pullback após o
//! breakout (duração, retração máxima, pivô pré-breakout) e a barra de
//! gatilho. Regras da seção 5 do doc.

use rust_decimal::Decimal;
use serde_json::json;

use crate::strategies::breakout_first_pullback_v1::config::StrategyParameters;
use crate::strategies::breakout_first_pullback_v1::context::Breakout;
use trader_domain::{Candle, RejectionReason};

/// Setup completo, pronto para cálculo de preços.
#[derive(Debug, Clone, PartialEq)]
pub struct Setup {
    pub breakout: Breakout,
    /// Índice da barra de sinal (última da série).
    pub signal_index: usize,
    /// Máxima pós-breakout (H).
    pub impulse_high: Decimal,
    /// Impulso pós-breakout: H − mínima da barra de breakout.
    pub impulse: Decimal,
    /// Retração do pullback em relação a H.
    pub retrace: Decimal,
    /// Pivô pré-breakout (maior mínima da base) — referência de stop.
    pub pivot: Decimal,
}

/// Valida o pullback entre a barra de breakout e a última barra (gatilho).
pub fn detect_setup(
    candles: &[Candle],
    breakout: Breakout,
    params: &StrategyParameters,
) -> Result<Setup, (RejectionReason, serde_json::Value)> {
    let signal_index = candles.len() - 1;
    let pullback_len = signal_index - breakout.index;
    if pullback_len < params.pullback_min_candles {
        return Err((
            RejectionReason::IncompleteSetup,
            json!({ "reason": "pullback ainda não formou 2 candles", "pullback_len": pullback_len }),
        ));
    }
    if pullback_len > params.pullback_max_candles {
        return Err((
            RejectionReason::PullbackTooLong,
            json!({
                "reason": "pullback passou do máximo de candles sem gatilho",
                "pullback_len": pullback_len,
                "max": params.pullback_max_candles,
            }),
        ));
    }

    let bar_b = &candles[breakout.index];
    let impulse_high = candles[breakout.index..=signal_index]
        .iter()
        .map(|c| c.high)
        .max()
        .unwrap_or_default();
    let impulse = impulse_high - bar_b.low;
    if impulse <= Decimal::ZERO {
        return Err((
            RejectionReason::IncompleteSetup,
            json!({ "reason": "impulso pós-breakout não positivo" }),
        ));
    }

    let pullback_low = candles[breakout.index + 1..=signal_index]
        .iter()
        .map(|c| c.low)
        .min()
        .unwrap_or_default();
    if pullback_low >= impulse_high {
        return Err((
            RejectionReason::IncompleteSetup,
            json!({ "reason": "sem retração após o breakout (preço só subiu)" }),
        ));
    }
    let retrace = impulse_high - pullback_low;
    if retrace > params.max_retrace_pct * impulse {
        return Err((
            RejectionReason::PullbackTooDeep,
            json!({
                "reason": "retração além do máximo do impulso",
                "retrace": retrace,
                "impulse": impulse,
                "max_retrace_pct": params.max_retrace_pct,
            }),
        ));
    }

    // Pivô pré-breakout: maior mínima da base (literal: o nível do breakout
    // NÃO é bom stop; o pivô é a referência de última instância).
    let pivot_start = breakout.index.saturating_sub(params.pivot_lookback_candles);
    let pivot = candles[pivot_start..breakout.index]
        .iter()
        .map(|c| c.low)
        .max()
        .unwrap_or(bar_b.low);

    // Breakout falho: qualquer fechamento pós-breakout abaixo do pivô.
    let closed_below_pivot = candles[breakout.index + 1..=signal_index]
        .iter()
        .any(|c| c.close < pivot);
    if closed_below_pivot {
        return Err((
            RejectionReason::BreakoutFailed,
            json!({
                "reason": "fechamento abaixo do pivô pré-breakout",
                "pivot": pivot,
            }),
        ));
    }

    // Gatilho: última barra fecha acima do fechamento da anterior.
    let signal_bar = &candles[signal_index];
    let prev_bar = &candles[signal_index - 1];
    if signal_bar.close <= prev_bar.close {
        return Err((
            RejectionReason::IncompleteSetup,
            json!({
                "reason": "barra de gatilho ainda não fechou acima da anterior",
                "signal_close": signal_bar.close,
                "prev_close": prev_bar.close,
            }),
        ));
    }

    Ok(Setup {
        breakout,
        signal_index,
        impulse_high,
        impulse,
        retrace,
        pivot,
    })
}
