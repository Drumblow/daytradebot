//! Detecção do setup High 2 Pullback.

use rust_decimal::Decimal;
use serde_json::json;
use tracing::debug;

use crate::strategies::pullback_trend_v1::config::StrategyParameters;
use trader_domain::{Candle, RejectionReason};

/// Descrição de um setup válido encontrado.
#[derive(Debug, Clone)]
pub struct Setup {
    /// Índice da barra de sinal no vetor de candles.
    pub signal_index: usize,
    /// Índice do início do pullback.
    pub pullback_start_index: usize,
    /// Preço de entrada (buy stop acima da máxima da barra de sinal).
    pub entry_price: Decimal,
    /// Stop inicial (abaixo da mínima da barra de sinal).
    pub stop_price: Decimal,
    /// Alvo (múltiplo do risco).
    pub target_price: Decimal,
}

/// Resultado da busca por setup.
#[derive(Debug, Clone)]
pub enum SetupResult {
    Found(Setup),
    NotFound(RejectionReason, serde_json::Value),
}

/// Tenta detectar um setup de pullback em tendência de alta.
///
/// A estratégia olha para os últimos `max_pullback_candles` candles e procura
/// uma barra de sinal bullish que:
/// - tenha corpo positivo;
/// - tenha sombra inferior >= `min_signal_body_ratio` * corpo;
/// - feche no terço superior da barra;
/// - não seja a mínima do pullback.
pub fn detect_setup(candles: &[Candle], params: &StrategyParameters) -> SetupResult {
    if candles.len() < 3 {
        return SetupResult::NotFound(
            RejectionReason::IncompleteSetup,
            json!({ "reason": "not enough candles" }),
        );
    }

    let max_lookback = std::cmp::min(params.max_pullback_candles + 2, candles.len() - 1);

    for signal_offset in 1..=max_lookback {
        let signal_index = candles.len() - signal_offset;
        let signal = &candles[signal_index];

        if !is_bullish_signal_bar(signal, params) {
            continue;
        }

        let pullback_start_index = find_pullback_start(candles, signal_index);
        if pullback_start_index == signal_index {
            continue;
        }

        if is_lowest_low_of_pullback(signal, candles, pullback_start_index, signal_index) {
            debug!("barra de sinal é a mínima do pullback; rejeitada");
            continue;
        }

        let entry_price = signal.high + params.entry_offset_ticks * params.tick_size;
        let stop_price = signal.low - params.stop_offset_ticks * params.tick_size;
        let risk = entry_price - stop_price;

        if risk <= Decimal::ZERO {
            continue;
        }

        let target_price = entry_price + params.reward_multiple * risk;

        return SetupResult::Found(Setup {
            signal_index,
            pullback_start_index,
            entry_price,
            stop_price,
            target_price,
        });
    }

    SetupResult::NotFound(
        RejectionReason::IncompleteSetup,
        json!({ "reason": "no valid bullish signal bar found in pullback" }),
    )
}

fn is_bullish_signal_bar(candle: &Candle, params: &StrategyParameters) -> bool {
    if !candle.is_bullish() {
        return false;
    }

    let body = candle.body();
    let lower_shadow = candle.close - candle.low;

    if body.is_zero() {
        return false;
    }

    let ratio = lower_shadow / body;
    if ratio < params.min_signal_body_ratio {
        return false;
    }

    // Fechamento no terço superior.
    let range = candle.range();
    if range.is_zero() {
        return false;
    }

    let close_position = (candle.close - candle.low) / range;
    match params.signal_close_position.as_str() {
        "upper_third" => close_position >= Decimal::from(2) / Decimal::from(3),
        "upper_half" => close_position >= Decimal::ONE / Decimal::from(2),
        _ => close_position >= Decimal::from(2) / Decimal::from(3),
    }
}

fn find_pullback_start(candles: &[Candle], signal_index: usize) -> usize {
    // O pullback começa após a última máxima antes da barra de sinal.
    // Simplificação: procuramos o primeiro candle antes do sinal que fez
    // máxima maior que as barras seguintes.
    if signal_index == 0 {
        return 0;
    }

    let mut highest_idx = signal_index - 1;
    for i in (0..signal_index).rev() {
        if candles[i].high >= candles[highest_idx].high {
            highest_idx = i;
        } else {
            break;
        }
    }

    highest_idx
}

fn is_lowest_low_of_pullback(
    signal: &Candle,
    candles: &[Candle],
    pullback_start: usize,
    signal_index: usize,
) -> bool {
    candles[pullback_start..=signal_index]
        .iter()
        .all(|c| c.low >= signal.low)
}
