//! Testes unitários com candles sintéticos — os 12 casos da seção 13 do doc
//! (`docs/strategies/breakout-first-pullback-v1.md`).
//!
//! Série canônica ("setup perfeito"):
//! - 81 candles de base lateral (~99,5–100) com 2 toques em 100,00
//!   (candles 10 e 60), máxima dos fundos (pivô) = 99,80;
//! - candle 81: breakout — máxima 103,50, mínima 99,20, fechamento 103,20,
//!   volume 3x a média, range ~6x a média;
//! - candles 82–83: pullback controlado (retração 2,00 ≤ 61,8% do impulso 4,30);
//! - candle 84 (sinal): fecha 102,60 > 102,20 (anterior); entrada 102,91,
//!   stop 99,79, alvo min(MMO 107,80, 2R) → RR ≈ 1,57.

use chrono::{Duration, TimeZone, Utc};
use rust_decimal::Decimal;

use super::*;
use trader_domain::{RejectionReason, SignalResult, TimeFrame};

fn dec(v: i64) -> Decimal {
    Decimal::from(v)
}

fn dec_centi(v: i64) -> Decimal {
    Decimal::new(v, 2)
}

fn candle(
    ts: chrono::DateTime<Utc>,
    open: Decimal,
    high: Decimal,
    low: Decimal,
    close: Decimal,
    volume: Decimal,
) -> Candle {
    Candle::new("SPY", TimeFrame::M15, ts, open, high, low, close, volume).expect("candle válido")
}

/// Constrói a base lateral de 81 candles: máximas 99,85 (toques em 100,00
/// nos candles 10 e 60), mínimas 99,05, pivô 99,80 (candle 75), range ~0,80.
fn base_candles(start: chrono::DateTime<Utc>) -> Vec<Candle> {
    let mut candles = Vec::new();
    for i in 0..81i64 {
        let ts = start + Duration::minutes(i * 15);
        // Toques no nível 100,00 nos candles 10 e 60.
        let high = if i == 10 || i == 60 {
            dec(100)
        } else {
            dec_centi(9985)
        };
        // Pivô = 99,80 (maior mínima da base, candle 75).
        let (low, close) = if i == 75 {
            (dec_centi(9980), dec_centi(9982))
        } else {
            (dec_centi(9905), dec_centi(9950))
        };
        let open = close;
        candles.push(candle(ts, open, high, low, close, dec(1000)));
    }
    candles
}

/// Série do setup perfeito (base + breakout + pullback 3 + sinal).
fn perfect_series() -> Vec<Candle> {
    let start = Utc.with_ymd_and_hms(2026, 7, 1, 13, 45, 0).unwrap();
    let mut candles = base_candles(start);
    let mut ts = candles.last().unwrap().timestamp + Duration::minutes(15);

    // Breakout (índice 81): range 4,30 (média da base ~0,7), volume 3x.
    candles.push(candle(
        ts,
        dec_centi(9990),
        dec_centi(10350),
        dec_centi(9920),
        dec_centi(10320),
        dec(3000),
    ));
    ts += Duration::minutes(15);
    // Pullback candle 1 (índice 82).
    candles.push(candle(
        ts,
        dec_centi(10270),
        dec_centi(10280),
        dec_centi(10150),
        dec(102),
        dec(1200),
    ));
    ts += Duration::minutes(15);
    // Pullback candle 2 (índice 83).
    candles.push(candle(
        ts,
        dec(102),
        dec_centi(10240),
        dec_centi(10160),
        dec_centi(10220),
        dec(1100),
    ));
    ts += Duration::minutes(15);
    // Barra de sinal (índice 84): fecha acima da anterior. Timestamp em
    // horário operacional (16:00 UTC de 2026-07-02).
    let signal_ts = Utc.with_ymd_and_hms(2026, 7, 2, 16, 0, 0).unwrap();
    debug_assert!(signal_ts > ts);
    candles.push(candle(
        signal_ts,
        dec_centi(10220),
        dec_centi(10290),
        dec_centi(10190),
        dec_centi(10260),
        dec(1300),
    ));
    candles
}

fn strategy() -> BreakoutFirstPullbackV1 {
    BreakoutFirstPullbackV1::new(config::BreakoutFirstPullbackV1Config::default())
}

fn analyze(candles: &[Candle]) -> SignalResult {
    strategy().analyze_candles("SPY", candles)
}

// --- CASO 1: setup perfeito gera sinal ---
#[test]
fn perfect_setup_generates_signal() {
    let candles = perfect_series();
    match analyze(&candles) {
        SignalResult::Signal(signal) => {
            assert_eq!(signal.direction, trader_domain::Direction::Long);
            assert_eq!(signal.entry_price, Some(dec_centi(10291)));
            assert_eq!(signal.stop_price, Some(dec_centi(9979)));
            // Alvo = min(MMO 107,80, 2R) = 107,80.
            assert_eq!(signal.target_price, Some(dec_centi(10780)));
            let snap = &signal.market_snapshot;
            for key in [
                "resistance_level",
                "level_touches",
                "impulse",
                "retrace",
                "pivot",
                "mmo",
                "risk_reward",
            ] {
                assert!(snap.get(key).is_some(), "snapshot sem {key}");
            }
            assert_eq!(snap["level_touches"], serde_json::json!(2));
        }
        SignalResult::Rejected { reason, details } => {
            panic!("esperado sinal, rejeitado por {reason:?}: {details:?}")
        }
        _ => panic!("esperado sinal"),
    }
}

// --- CASO 2: nível com 1 toque só ---
#[test]
fn level_with_single_touch_rejects() {
    let mut candles = perfect_series();
    // Apaga o segundo toque (candle 60 volta à máxima comum da base).
    candles[60].high = dec_centi(9985);
    match analyze(&candles) {
        SignalResult::Rejected { reason, .. } => {
            assert_eq!(reason, RejectionReason::ResistanceLevelNotFound)
        }
        other => panic!("esperado ResistanceLevelNotFound, obtido {other:?}"),
    }
}

// --- CASO 3: breakout sem expansão de volume ---
#[test]
fn breakout_without_volume_rejects() {
    let mut candles = perfect_series();
    candles[81].volume = dec(1000); // igual à média
    match analyze(&candles) {
        SignalResult::Rejected { reason, .. } => {
            assert_eq!(reason, RejectionReason::WeakBreakout)
        }
        other => panic!("esperado WeakBreakout, obtido {other:?}"),
    }
}

// --- CASO 4: breakout sem expansão de range ---
#[test]
fn breakout_without_range_rejects() {
    let mut candles = perfect_series();
    // Breakout miúdo: fecha acima do nível, mas range < 1,5x a média (~0,80).
    let b = &mut candles[81];
    b.open = dec_centi(9990);
    b.high = dec_centi(10050);
    b.low = dec_centi(9960);
    b.close = dec_centi(10040);
    // O restante da série acompanha o cenário fraco (nenhuma barra posterior
    // fecha acima da máxima do breakout miúdo — senão elas virariam
    // candidatas com "breakout prévio" no lookback).
    let c82 = &mut candles[82];
    c82.open = dec_centi(10030);
    c82.high = dec_centi(10040);
    c82.low = dec_centi(10010);
    c82.close = dec_centi(10030);
    let c83 = &mut candles[83];
    c83.open = dec_centi(10030);
    c83.high = dec_centi(10045);
    c83.low = dec_centi(10015);
    c83.close = dec_centi(10035);
    let c84 = &mut candles[84];
    c84.open = dec_centi(10035);
    c84.high = dec_centi(10060);
    c84.low = dec_centi(10025);
    c84.close = dec_centi(10050);
    match analyze(&candles) {
        SignalResult::Rejected { reason, .. } => {
            assert_eq!(reason, RejectionReason::WeakBreakout)
        }
        other => panic!("esperado WeakBreakout, obtido {other:?}"),
    }
}

// --- CASO 5: retração além de 61,8% do impulso ---
#[test]
fn pullback_too_deep_rejects() {
    let mut candles = perfect_series();
    // Mínima do pullback em 100,50: retração 3,00 > 0,618 × 4,30 = 2,657.
    candles[82].low = dec_centi(10050);
    candles[83].low = dec_centi(10080);
    candles[84].low = dec_centi(10100);
    match analyze(&candles) {
        SignalResult::Rejected { reason, .. } => {
            assert_eq!(reason, RejectionReason::PullbackTooDeep)
        }
        other => panic!("esperado PullbackTooDeep, obtido {other:?}"),
    }
}

// --- CASO 6: pullback longo demais (breakout 7+ candles atrás) ---
#[test]
fn pullback_too_long_rejects() {
    let start = Utc.with_ymd_and_hms(2026, 7, 1, 13, 45, 0).unwrap();
    let mut candles = base_candles(start);
    let mut ts = candles.last().unwrap().timestamp + Duration::minutes(15);
    // Breakout no índice 81.
    candles.push(candle(
        ts,
        dec_centi(9990),
        dec_centi(10350),
        dec_centi(9920),
        dec_centi(10320),
        dec(3000),
    ));
    ts += Duration::minutes(15);
    // 6 candles de pullback (índices 82..87) + sinal no 88 → pullback_len = 7.
    let mut prev_close = dec_centi(10300);
    for _ in 0..6 {
        let close = prev_close - dec_centi(20);
        candles.push(candle(
            ts,
            prev_close,
            prev_close + dec_centi(30),
            close - dec_centi(10),
            close,
            dec(1000),
        ));
        prev_close = close;
        ts += Duration::minutes(15);
    }
    let signal_ts = Utc.with_ymd_and_hms(2026, 7, 2, 16, 0, 0).unwrap();
    candles.push(candle(
        signal_ts,
        prev_close,
        prev_close + dec_centi(40),
        prev_close - dec_centi(10),
        prev_close + dec_centi(30),
        dec(1200),
    ));
    match analyze(&candles) {
        SignalResult::Rejected { reason, .. } => {
            assert_eq!(reason, RejectionReason::PullbackTooLong)
        }
        other => panic!("esperado PullbackTooLong, obtido {other:?}"),
    }
}

// --- CASO 7: pullback fecha abaixo do pivô (breakout falho) ---
#[test]
fn breakout_failed_rejects() {
    let start = Utc.with_ymd_and_hms(2026, 7, 1, 13, 45, 0).unwrap();
    let mut candles = base_candles(start);
    let mut ts = candles.last().unwrap().timestamp + Duration::minutes(15);
    // Breakout menor (impulso 1,50) para a retração caber no limite mesmo
    // com o fechamento abaixo do pivô (99,80).
    candles.push(candle(
        ts,
        dec_centi(9990),
        dec_centi(10070),
        dec_centi(9920),
        dec_centi(10055),
        dec(3000),
    ));
    ts += Duration::minutes(15);
    // Pullback candle 1.
    candles.push(candle(
        ts,
        dec_centi(10050),
        dec_centi(10050),
        dec_centi(10010),
        dec_centi(10030),
        dec(1100),
    ));
    ts += Duration::minutes(15);
    // Pullback candle 2: fecha 99,79 < pivô 99,80, mas mínima 99,78 mantém a
    // retração (1,02) dentro do limite (0,618 × 1,70 = 1,05).
    candles.push(candle(
        ts,
        dec_centi(10020),
        dec_centi(10020),
        dec_centi(9978),
        dec_centi(9979),
        dec(1100),
    ));
    ts += Duration::minutes(15);
    let signal_ts = Utc.with_ymd_and_hms(2026, 7, 2, 16, 0, 0).unwrap();
    candles.push(candle(
        signal_ts,
        dec_centi(9990),
        dec_centi(10020),
        dec_centi(9985),
        dec(100),
        dec(1000),
    ));
    match analyze(&candles) {
        SignalResult::Rejected { reason, .. } => {
            assert_eq!(reason, RejectionReason::BreakoutFailed)
        }
        other => panic!("esperado BreakoutFailed, obtido {other:?}"),
    }
}

// --- CASO 8: stop dentro do ruído / largo demais (via config) ---
#[test]
fn stop_sanity_rejects() {
    let candles = perfect_series();

    let mut cfg_noise = config::BreakoutFirstPullbackV1Config::default();
    cfg_noise.strategy.parameters.min_stop_bar_ranges = dec(100);
    let s_noise = BreakoutFirstPullbackV1::new(cfg_noise);
    match s_noise.analyze_candles("SPY", &candles) {
        SignalResult::Rejected { reason, .. } => {
            assert_eq!(reason, RejectionReason::StopWithinNoise)
        }
        other => panic!("esperado StopWithinNoise, obtido {other:?}"),
    }

    let mut cfg_wide = config::BreakoutFirstPullbackV1Config::default();
    cfg_wide.strategy.parameters.max_stop_atr_mult = dec_centi(50); // 0,5×ATR
    let s_wide = BreakoutFirstPullbackV1::new(cfg_wide);
    match s_wide.analyze_candles("SPY", &candles) {
        SignalResult::Rejected { reason, .. } => {
            assert_eq!(reason, RejectionReason::StopTooWide)
        }
        other => panic!("esperado StopTooWide, obtido {other:?}"),
    }
}

// --- CASO 9: risco-retorno ruim (via config) ---
#[test]
fn poor_risk_reward_rejects() {
    let candles = perfect_series();
    let mut cfg = config::BreakoutFirstPullbackV1Config::default();
    cfg.strategy.parameters.min_risk_reward = dec(3);
    let s = BreakoutFirstPullbackV1::new(cfg);
    match s.analyze_candles("SPY", &candles) {
        SignalResult::Rejected { reason, .. } => {
            assert_eq!(reason, RejectionReason::PoorRiskReward)
        }
        other => panic!("esperado PoorRiskReward, obtido {other:?}"),
    }
}

// --- CASO 10: segundo breakout do nível ---
#[test]
fn second_breakout_attempt_rejects() {
    let mut candles = perfect_series();
    // Rompimento prévio ANTES do lookback (candle 0): fechou acima do nível.
    candles[0].open = dec_centi(9990);
    candles[0].high = dec_centi(10030);
    candles[0].low = dec_centi(9980);
    candles[0].close = dec_centi(10020);
    match analyze(&candles) {
        SignalResult::Rejected { reason, .. } => {
            assert_eq!(reason, RejectionReason::BreakoutAlreadyTaken)
        }
        other => panic!("esperado BreakoutAlreadyTaken, obtido {other:?}"),
    }
}

// --- CASO 11: gatilho logo após o breakout (pullback de 1 candle) ---
#[test]
fn one_candle_pullback_is_incomplete() {
    let start = Utc.with_ymd_and_hms(2026, 7, 1, 13, 45, 0).unwrap();
    let mut candles = base_candles(start);
    let ts = candles.last().unwrap().timestamp + Duration::minutes(15);
    candles.push(candle(
        ts,
        dec_centi(9990),
        dec_centi(10350),
        dec_centi(9920),
        dec_centi(10320),
        dec(3000),
    ));
    // Última barra já é a de gatilho (pullback_len = 1 < 2).
    let signal_ts = Utc.with_ymd_and_hms(2026, 7, 2, 16, 0, 0).unwrap();
    candles.push(candle(
        signal_ts,
        dec_centi(10300),
        dec_centi(10320),
        dec_centi(10250),
        dec_centi(10310),
        dec(1200),
    ));
    match analyze(&candles) {
        SignalResult::Rejected { reason, .. } => {
            assert_eq!(reason, RejectionReason::IncompleteSetup)
        }
        other => panic!("esperado IncompleteSetup, obtido {other:?}"),
    }
}

// --- CASO 12: fora de horário ---
#[test]
fn outside_trading_hours_rejects() {
    let mut candles = perfect_series();
    let after = Utc.with_ymd_and_hms(2026, 7, 2, 3, 0, 0).unwrap();
    let last = candles.len() - 1;
    candles[last].timestamp = after;
    match analyze(&candles) {
        SignalResult::Rejected { reason, .. } => {
            assert_eq!(reason, RejectionReason::OutsideTradingHours)
        }
        other => panic!("esperado OutsideTradingHours, obtido {other:?}"),
    }
}
