//! Detecção do setup da Value Area Reentry v1 (seções 5.1 e 5.2 do doc):
//! abertura fora da área de valor de ontem + aceitação (2 fechamentos
//! consecutivos dentro dela).

use rust_decimal::Decimal;
use serde_json::json;

use crate::strategies::value_area_reentry_v1::config::StrategyParameters;
use crate::strategies::value_area_reentry_v1::context::{current_day_start, ValueArea};
use trader_domain::{Candle, Direction, RejectionReason};

/// Setup detectado: a travessia da área de valor que vamos operar.
#[derive(Debug, Clone, PartialEq)]
pub struct Setup {
    pub direction: Direction,
    /// Índice da barra de sinal (a última barra de aceitação).
    pub signal_index: usize,
    /// Abertura do dia corrente (primeiro candle RTH).
    pub open_today: Decimal,
    /// Distância da abertura à borda mais próxima da VA.
    pub open_distance: Decimal,
}

type Rejection = (RejectionReason, serde_json::Value);

/// Detecta o setup na última barra da série.
///
/// A aceitação é reconhecida **na transição**: as últimas `acceptance_bars`
/// barras fecham dentro da VA e a barra imediatamente anterior fecha fora.
/// Sem essa condição a estratégia re-emitiria sinal a cada barra enquanto o
/// preço permanecesse dentro do valor.
pub fn detect_setup(
    candles: &[Candle],
    va: &ValueArea,
    params: &StrategyParameters,
) -> Result<Setup, Rejection> {
    let n = candles.len();
    let today_start = current_day_start(candles);
    let open_today = candles[today_start].open;

    // Seção 5.1 — a abertura precisa estar FORA do valor de ontem.
    if va.contains(open_today) {
        return Err((
            RejectionReason::OpenInsideValueArea,
            json!({
                "reason": "abertura de hoje dentro da área de valor de ontem",
                "open": open_today,
                "va_low": va.low,
                "va_high": va.high,
            }),
        ));
    }

    let direction = if open_today < va.low {
        Direction::Long
    } else {
        Direction::Short
    };

    let bars = params.acceptance_bars.max(1);
    // Precisamos das `bars` barras de aceitação mais a barra anterior (fora).
    if n < bars + 1 || n - bars < today_start {
        return Err((
            RejectionReason::NoValueAreaReentry,
            json!({
                "reason": "barras insuficientes no dia para caracterizar aceitação",
                "acceptance_bars": bars,
            }),
        ));
    }

    // Últimas `bars` barras fecharam dentro da VA?
    let all_inside = candles[n - bars..].iter().all(|c| va.contains(c.close));
    if !all_inside {
        return Err((
            RejectionReason::NoValueAreaReentry,
            json!({
                "reason": "não há fechamentos consecutivos suficientes dentro da área de valor",
                "acceptance_bars": bars,
                "va_low": va.low,
                "va_high": va.high,
                "last_close": candles[n - 1].close,
            }),
        ));
    }

    // A barra anterior à janela precisa ter fechado FORA (transição).
    let before = &candles[n - bars - 1];
    if va.contains(before.close) {
        return Err((
            RejectionReason::NoValueAreaReentry,
            json!({
                "reason": "aceitação já havia ocorrido em barra anterior (sinal não é a transição)",
                "before_close": before.close,
            }),
        ));
    }

    let open_distance = if open_today < va.low {
        va.low - open_today
    } else {
        open_today - va.high
    };

    Ok(Setup {
        direction,
        signal_index: n - 1,
        open_today,
        open_distance,
    })
}
