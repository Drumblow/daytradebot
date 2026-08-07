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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::strategies::pullback_trend_v1::config::PullbackTrendV1Config;
    use chrono::{TimeZone, Utc};
    use trader_domain::TimeFrame;

    fn candle(idx: u32, open: &str, high: &str, low: &str, close: &str) -> Candle {
        let ts = Utc.with_ymd_and_hms(2026, 8, 3, 14, 30, 0).unwrap()
            + chrono::Duration::minutes(i64::from(idx) * 15);
        Candle::new(
            "SPY",
            TimeFrame::M15,
            ts,
            open.parse().unwrap(),
            high.parse().unwrap(),
            low.parse().unwrap(),
            close.parse().unwrap(),
            Decimal::from(1000),
        )
        .expect("candle válido")
    }

    fn default_params() -> StrategyParameters {
        PullbackTrendV1Config::default().strategy.parameters
    }

    /// Série com pullback de duas pernas e barra de sinal válida no final:
    /// alta, alta, perna de baixa (mínima do pullback), barra de sinal
    /// bullish com sombra inferior grande e fechamento no terço superior.
    fn valid_setup_series() -> Vec<Candle> {
        vec![
            candle(0, "99.00", "100.50", "98.50", "100.00"),
            candle(1, "100.00", "101.00", "99.90", "100.80"),
            candle(2, "100.80", "101.20", "99.80", "100.30"),
            candle(3, "100.30", "101.00", "100.00", "100.90"),
        ]
    }

    #[test]
    fn detects_valid_setup_with_prices() {
        let params = default_params();
        match detect_setup(&valid_setup_series(), &params) {
            SetupResult::Found(setup) => {
                // entrada = máxima da barra de sinal + 1 tick
                assert_eq!(setup.entry_price, "101.01".parse().unwrap());
                // stop = mínima da barra de sinal - 1 tick
                assert_eq!(setup.stop_price, "99.99".parse().unwrap());
                // alvo = entrada + 2x risco (risco = 1.02)
                assert_eq!(setup.target_price, "103.05".parse().unwrap());
            }
            SetupResult::NotFound(reason, details) => {
                panic!("esperado setup, rejeitado por {:?}: {:?}", reason, details)
            }
        }
    }

    #[test]
    fn rejects_signal_bar_that_is_pullback_low() {
        let mut candles = valid_setup_series();
        // Barra de sinal com a mínima mais baixa do pullback: deve ser ignorada.
        candles[3] = candle(3, "100.30", "101.00", "99.70", "100.90");

        match detect_setup(&candles, &default_params()) {
            SetupResult::NotFound(..) => {}
            SetupResult::Found(setup) => panic!(
                "barra de sinal na mínima do pullback deveria ser rejeitada (setup em {:?})",
                setup.signal_index
            ),
        }
    }

    #[test]
    fn rejects_when_no_bullish_signal_bar() {
        let candles = vec![
            candle(0, "100.00", "100.50", "99.50", "99.60"),
            candle(1, "99.60", "99.70", "99.00", "99.10"),
            candle(2, "99.10", "99.20", "98.50", "98.60"),
        ];

        match detect_setup(&candles, &default_params()) {
            SetupResult::NotFound(reason, _) => {
                assert_eq!(reason, RejectionReason::IncompleteSetup)
            }
            SetupResult::Found(_) => panic!("série só com candles de baixa não pode gerar setup"),
        }
    }

    #[test]
    fn rejects_short_shadow_ratio() {
        let mut candles = valid_setup_series();
        // Sombra inferior curta: ratio (close-low)/(close-open) = 0.8/0.6 = 1.33 < 1.5.
        // Fechamento no terço superior passa, isolando a regra da sombra.
        candles[3] = candle(3, "100.30", "101.00", "100.10", "100.90");

        match detect_setup(&candles, &default_params()) {
            SetupResult::NotFound(..) => {}
            SetupResult::Found(_) => panic!("sombra inferior curta deveria ser rejeitada"),
        }
    }

    #[test]
    fn rejects_close_outside_upper_third() {
        let mut candles = valid_setup_series();
        // Fechamento fraco (meio da barra): (100.6-100.0)/(101.0-100.0) = 0.6 < 2/3.
        // Sombra 0.6/corpo 0.3 = 2.0 passa no ratio, então isola a regra do terço.
        candles[3] = candle(3, "100.30", "101.00", "100.00", "100.60");

        match detect_setup(&candles, &default_params()) {
            SetupResult::NotFound(..) => {}
            SetupResult::Found(_) => {
                panic!("fechamento fora do terço superior deveria ser rejeitado")
            }
        }
    }

    #[test]
    fn rejects_series_with_too_few_candles() {
        let candles = vec![candle(0, "99.00", "100.50", "98.50", "100.00")];
        match detect_setup(&candles, &default_params()) {
            SetupResult::NotFound(reason, _) => {
                assert_eq!(reason, RejectionReason::IncompleteSetup)
            }
            SetupResult::Found(_) => panic!("menos de 3 candles não pode gerar setup"),
        }
    }
}
