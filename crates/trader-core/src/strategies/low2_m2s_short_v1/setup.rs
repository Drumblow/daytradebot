//! Detecção do setup Low 2 Pullback (espelho do High 2 da `pullback-trend-v1`).
//!
//! O pullback em bear trend é uma correção para CIMA em duas pernas até a
//! EMA; a barra de sinal é bear e o gatilho é a venda 1 tick abaixo dela.

use rust_decimal::Decimal;
use serde_json::json;
use tracing::debug;

use crate::strategies::low2_m2s_short_v1::config::StrategyParameters;
use trader_domain::{Candle, RejectionReason};

/// Descrição de um setup válido encontrado.
#[derive(Debug, Clone)]
pub struct Setup {
    /// Índice da barra de sinal no vetor de candles.
    pub signal_index: usize,
    /// Índice do início do pullback (correção para cima).
    pub pullback_start_index: usize,
    /// Preço de entrada (sell stop abaixo da mínima da barra de sinal).
    pub entry_price: Decimal,
    /// Stop inicial (acima da máxima da barra de sinal).
    pub stop_price: Decimal,
    /// Alvo (múltiplo do risco, para baixo).
    pub target_price: Decimal,
}

/// Resultado da busca por setup.
#[derive(Debug, Clone)]
pub enum SetupResult {
    Found(Setup),
    NotFound(RejectionReason, serde_json::Value),
}

/// Tenta detectar um setup de pullback em tendência de baixa.
///
/// Espelho da irmã long: olha os últimos `max_pullback_candles + 2` candles
/// e procura uma barra de sinal bear que:
/// - tenha corpo negativo;
/// - tenha sombra superior >= `min_signal_body_ratio` * corpo;
/// - feche no terço inferior da barra;
/// - não seja a máxima do pullback (o topo da correção).
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

        if !is_bearish_signal_bar(signal, params) {
            continue;
        }

        let pullback_start_index = find_pullback_start(candles, signal_index);
        if pullback_start_index == signal_index {
            continue;
        }

        if is_highest_high_of_pullback(signal, candles, pullback_start_index, signal_index) {
            debug!("barra de sinal é a máxima do pullback; rejeitada");
            continue;
        }

        let entry_price = signal.low - params.entry_offset_ticks * params.tick_size;
        let stop_price = signal.high + params.stop_offset_ticks * params.tick_size;
        let risk = stop_price - entry_price;

        if risk <= Decimal::ZERO {
            continue;
        }

        let target_price = entry_price - params.reward_multiple * risk;

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
        json!({ "reason": "no valid bearish signal bar found in pullback" }),
    )
}

fn is_bearish_signal_bar(candle: &Candle, params: &StrategyParameters) -> bool {
    if !candle.is_bearish() {
        return false;
    }

    let body = candle.body();
    let upper_shadow = candle.high - candle.close;

    if body.is_zero() {
        return false;
    }

    let ratio = upper_shadow / body;
    if ratio < params.min_signal_body_ratio {
        return false;
    }

    // Fechamento no terço inferior.
    let range = candle.range();
    if range.is_zero() {
        return false;
    }

    let close_position = (candle.close - candle.low) / range;
    match params.signal_close_position.as_str() {
        "lower_third" => close_position <= Decimal::ONE / Decimal::from(3),
        "lower_half" => close_position <= Decimal::ONE / Decimal::from(2),
        _ => close_position <= Decimal::ONE / Decimal::from(3),
    }
}

fn find_pullback_start(candles: &[Candle], signal_index: usize) -> usize {
    // Espelho da irmã: o pullback (correção para cima) começa após a última
    // mínima antes da barra de sinal.
    if signal_index == 0 {
        return 0;
    }

    let mut lowest_idx = signal_index - 1;
    for i in (0..signal_index).rev() {
        if candles[i].low <= candles[lowest_idx].low {
            lowest_idx = i;
        } else {
            break;
        }
    }

    lowest_idx
}

fn is_highest_high_of_pullback(
    signal: &Candle,
    candles: &[Candle],
    pullback_start: usize,
    signal_index: usize,
) -> bool {
    candles[pullback_start..=signal_index]
        .iter()
        .all(|c| c.high <= signal.high)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::strategies::low2_m2s_short_v1::config::Low2M2sShortV1Config;
    use chrono::{TimeZone, Utc};
    use trader_domain::TimeFrame;

    fn candle(idx: u32, open: &str, high: &str, low: &str, close: &str) -> Candle {
        let ts = Utc.with_ymd_and_hms(2026, 8, 3, 14, 30, 0).unwrap()
            + chrono::Duration::minutes(i64::from(idx) * 15);
        Candle::new(
            "IWM",
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
        Low2M2sShortV1Config::default().strategy.parameters
    }

    /// Série com correção de duas pernas e barra de sinal bear válida:
    /// baixa, baixa, perna de alta (máxima da correção), barra de sinal bear
    /// com sombra superior grande e fechamento no terço inferior.
    fn valid_setup_series() -> Vec<Candle> {
        vec![
            candle(0, "101.00", "101.50", "99.50", "100.00"),
            candle(1, "100.00", "100.10", "99.00", "99.20"),
            candle(2, "99.20", "100.20", "98.80", "99.70"),
            candle(3, "99.70", "100.00", "99.00", "99.10"),
        ]
    }

    #[test]
    fn detects_valid_setup_with_prices() {
        let params = default_params();
        match detect_setup(&valid_setup_series(), &params) {
            SetupResult::Found(setup) => {
                // entrada = mínima da barra de sinal - 1 tick
                assert_eq!(setup.entry_price, "98.99".parse().unwrap());
                // stop = máxima da barra de sinal + 1 tick
                assert_eq!(setup.stop_price, "100.01".parse().unwrap());
                // alvo = entrada - 2x risco (risco = 1.02)
                assert_eq!(setup.target_price, "96.95".parse().unwrap());
            }
            SetupResult::NotFound(reason, details) => {
                panic!("esperado setup, rejeitado por {:?}: {:?}", reason, details)
            }
        }
    }

    #[test]
    fn rejects_signal_bar_that_is_pullback_high() {
        let mut candles = valid_setup_series();
        // Barra de sinal com a máxima mais alta da correção: deve ser ignorada.
        candles[3] = candle(3, "99.70", "100.30", "99.00", "99.10");

        match detect_setup(&candles, &default_params()) {
            SetupResult::NotFound(..) => {}
            SetupResult::Found(setup) => panic!(
                "barra de sinal na máxima da correção deveria ser rejeitada (setup em {:?})",
                setup.signal_index
            ),
        }
    }

    #[test]
    fn rejects_when_no_bearish_signal_bar() {
        let candles = vec![
            candle(0, "100.00", "100.50", "99.50", "100.40"),
            candle(1, "100.40", "101.00", "100.30", "100.90"),
            candle(2, "100.90", "101.50", "100.80", "101.40"),
        ];

        match detect_setup(&candles, &default_params()) {
            SetupResult::NotFound(reason, _) => {
                assert_eq!(reason, RejectionReason::IncompleteSetup)
            }
            SetupResult::Found(_) => panic!("série só com candles de alta não pode gerar setup"),
        }
    }

    #[test]
    fn rejects_short_upper_shadow_ratio() {
        let mut candles = valid_setup_series();
        // Sombra superior curta: (high-close)/corpo = 0.3/0.6 = 0.5 < 1.5.
        // Fechamento no terço inferior passa, isolando a regra da sombra.
        candles[3] = candle(3, "99.70", "99.80", "99.00", "99.10");

        match detect_setup(&candles, &default_params()) {
            SetupResult::NotFound(..) => {}
            SetupResult::Found(_) => panic!("sombra superior curta deveria ser rejeitada"),
        }
    }

    #[test]
    fn rejects_close_outside_lower_third() {
        let mut candles = valid_setup_series();
        // Fechamento no meio da barra: (99.4-99.0)/(100.0-99.0) = 0.4 > 1/3.
        // Sombra superior 0.6/corpo 0.3 = 2.0 passa, isolando a regra do terço.
        candles[3] = candle(3, "99.70", "100.00", "99.00", "99.40");

        match detect_setup(&candles, &default_params()) {
            SetupResult::NotFound(..) => {}
            SetupResult::Found(_) => {
                panic!("fechamento fora do terço inferior deveria ser rejeitado")
            }
        }
    }

    #[test]
    fn rejects_series_with_too_few_candles() {
        let candles = vec![candle(0, "101.00", "101.50", "99.50", "100.00")];
        match detect_setup(&candles, &default_params()) {
            SetupResult::NotFound(reason, _) => {
                assert_eq!(reason, RejectionReason::IncompleteSetup)
            }
            SetupResult::Found(_) => panic!("menos de 3 candles não pode gerar setup"),
        }
    }
}
