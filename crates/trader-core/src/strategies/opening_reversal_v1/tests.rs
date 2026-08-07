//! Testes unitários com candles sintéticos — os 12 casos da seção 13 do doc
//! (`docs/strategies/opening-reversal-v1.md`).
//!
//! Série canônica:
//! - "Ontem" (2026-07-01 ET): 15 candles 13:30–17:00 UTC, máxima 505 / mínima 500;
//! - "Hoje" (2026-07-02 ET): 2 candles de abertura neutros + barra de sinal
//!   às 13:45 UTC (9:45 ET — dentro da janela 13:30–14:30 UTC).

use chrono::{Duration, TimeZone, Utc};
use rust_decimal::Decimal;

use super::*;
use trader_domain::{Direction, RejectionReason, SignalResult, TimeFrame};

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
) -> Candle {
    Candle::new("SPY", TimeFrame::M15, ts, open, high, low, close, dec(1000))
        .expect("candle válido")
}

/// Dia anterior: máxima 505, mínima 500 (13:30–17:00 UTC de 2026-07-01).
fn yesterday_candles() -> Vec<Candle> {
    let base = Utc.with_ymd_and_hms(2026, 7, 1, 13, 30, 0).unwrap();
    let mut candles = Vec::new();
    for i in 0..15 {
        let ts = base + Duration::minutes(i * 15);
        let (open, high, low, close) = match i {
            // máxima do dia
            3 => (dec(504), dec(505), dec_centi(50350), dec(504)),
            // mínima do dia
            9 => (
                dec_centi(50040),
                dec_centi(50080),
                dec(500),
                dec_centi(50040),
            ),
            _ => (
                dec_centi(50200),
                dec_centi(50280),
                dec_centi(50120),
                dec_centi(50200),
            ),
        };
        candles.push(candle(ts, open, high, low, close));
    }
    candles
}

/// Abertura de hoje: 2 candles neutros acima da mínima de ontem (sem veto).
fn today_open_candles() -> Vec<Candle> {
    let base = Utc.with_ymd_and_hms(2026, 7, 2, 13, 30, 0).unwrap();
    vec![
        candle(
            base,
            dec_centi(50150),
            dec_centi(50190),
            dec_centi(50110),
            dec_centi(50160),
        ),
        candle(
            base + Duration::minutes(15),
            dec_centi(50160),
            dec_centi(50180),
            dec_centi(50100),
            dec_centi(50140),
        ),
    ]
}

/// Barra de sinal bull perfeita testando a mínima de ontem (500).
/// open 500.20, high 500.60, low 499.60, close 500.50 às 13:45 UTC.
fn bull_signal_bar() -> Candle {
    candle(
        Utc.with_ymd_and_hms(2026, 7, 2, 13, 45, 0).unwrap(),
        dec_centi(50020),
        dec_centi(50060),
        dec_centi(49960),
        dec_centi(50050),
    )
}

/// Série do setup perfeito long.
fn long_series() -> Vec<Candle> {
    let mut candles = yesterday_candles();
    candles.extend(today_open_candles());
    candles.push(bull_signal_bar());
    candles
}

/// Série do setup perfeito short: sinal bear testando a máxima de ontem.
fn short_series() -> Vec<Candle> {
    let mut candles = yesterday_candles();
    // Abertura neutra abaixo da máxima.
    let base = Utc.with_ymd_and_hms(2026, 7, 2, 13, 30, 0).unwrap();
    candles.push(candle(
        base,
        dec_centi(50350),
        dec_centi(50390),
        dec_centi(50310),
        dec_centi(50360),
    ));
    candles.push(candle(
        base + Duration::minutes(15),
        dec_centi(50360),
        dec_centi(50380),
        dec_centi(50300),
        dec_centi(50340),
    ));
    // Barra de sinal bear testando a MÁXIMA de ontem (505): open 504.85,
    // high 505.40, low 504.60, close 504.60 — fecha no terço inferior,
    // sombra superior 0,55 ≥ 1/3 do range, corpo 0,25/0,80 = 31%.
    candles.push(candle(
        Utc.with_ymd_and_hms(2026, 7, 2, 13, 45, 0).unwrap(),
        dec_centi(50485),
        dec_centi(50540),
        dec_centi(50460),
        dec_centi(50460),
    ));
    candles
}

fn strategy() -> OpeningReversalV1 {
    OpeningReversalV1::new(config::OpeningReversalV1Config::default())
}

// --- CASO 1: setup perfeito long ---
#[test]
fn perfect_long_setup_generates_signal() {
    let candles = long_series();
    match strategy().analyze_candles("SPY", &candles) {
        SignalResult::Signal(signal) => {
            assert_eq!(signal.direction, Direction::Long);
            assert_eq!(signal.entry_price, Some(dec_centi(50061)));
            assert_eq!(signal.stop_price, Some(dec_centi(49959)));
            // 2R: 500,61 + 2×1,02 = 502,65.
            assert_eq!(signal.target_price, Some(dec_centi(50265)));
            let snap = &signal.market_snapshot;
            for key in [
                "yesterday_high",
                "yesterday_low",
                "tested_level",
                "risk_reward",
            ] {
                assert!(snap.get(key).is_some(), "snapshot sem {key}");
            }
            assert_eq!(snap["tested_level"], serde_json::json!("500"));
        }
        SignalResult::Rejected { reason, details } => {
            panic!("esperado sinal, rejeitado por {reason:?}: {details:?}")
        }
        _ => panic!("esperado sinal"),
    }
}

// --- CASO 2: setup perfeito short ---
#[test]
fn perfect_short_setup_generates_signal() {
    let candles = short_series();
    match strategy().analyze_candles("SPY", &candles) {
        SignalResult::Signal(signal) => {
            assert_eq!(signal.direction, Direction::Short);
            assert_eq!(signal.entry_price, Some(dec_centi(50459)));
            assert_eq!(signal.stop_price, Some(dec_centi(50541)));
            // 2R short: 504,59 − 2×0,82 = 502,95.
            assert_eq!(signal.target_price, Some(dec_centi(50295)));
        }
        SignalResult::Rejected { reason, details } => {
            panic!("esperado sinal, rejeitado por {reason:?}: {details:?}")
        }
        _ => panic!("esperado sinal"),
    }
}

// --- CASO 3: sem toque no nível ---
#[test]
fn no_touch_rejects() {
    let mut candles = yesterday_candles();
    candles.extend(today_open_candles());
    // Barra longe da zona (mínima 501,60 > zona 501,50).
    candles.push(candle(
        Utc.with_ymd_and_hms(2026, 7, 2, 13, 45, 0).unwrap(),
        dec_centi(50170),
        dec_centi(50200),
        dec_centi(50160),
        dec_centi(50190),
    ));
    match strategy().analyze_candles("SPY", &candles) {
        SignalResult::Rejected { reason, .. } => {
            assert_eq!(reason, RejectionReason::YesterdayLevelNotTested)
        }
        other => panic!("esperado YesterdayLevelNotTested, obtido {other:?}"),
    }
}

// --- CASO 4: barra de sinal fraca (doji) ---
#[test]
fn weak_signal_bar_rejects() {
    let mut candles = yesterday_candles();
    candles.extend(today_open_candles());
    // Doji na zona: corpo 0,05/0,90 < 30%.
    candles.push(candle(
        Utc.with_ymd_and_hms(2026, 7, 2, 13, 45, 0).unwrap(),
        dec_centi(50030),
        dec_centi(50060),
        dec_centi(49970),
        dec_centi(50035),
    ));
    match strategy().analyze_candles("SPY", &candles) {
        SignalResult::Rejected { reason, .. } => {
            assert_eq!(reason, RejectionReason::WeakConfirmation)
        }
        other => panic!("esperado WeakConfirmation, obtido {other:?}"),
    }
}

// --- CASO 5: momentum contra (2 barras fortes além da zona) ---
#[test]
fn momentum_beyond_level_rejects() {
    let mut candles = yesterday_candles();
    let base = Utc.with_ymd_and_hms(2026, 7, 2, 13, 30, 0).unwrap();
    // Duas trend bars bearish fechando ABAIXO da mínima de ontem (500).
    candles.push(candle(
        base,
        dec_centi(50050),
        dec_centi(50060),
        dec_centi(49990),
        dec_centi(49995),
    ));
    candles.push(candle(
        base + Duration::minutes(15),
        dec_centi(49990),
        dec_centi(49995),
        dec_centi(49965),
        dec_centi(49972),
    ));
    candles.push(bull_signal_bar());
    match strategy().analyze_candles("SPY", &candles) {
        SignalResult::Rejected { reason, .. } => {
            assert_eq!(reason, RejectionReason::MomentumAgainst)
        }
        other => panic!("esperado MomentumAgainst, obtido {other:?}"),
    }
}

// --- CASO 6: 4+ trend bars contra ---
#[test]
fn counter_trend_bars_rejects() {
    let mut candles = yesterday_candles();
    let base = Utc.with_ymd_and_hms(2026, 7, 2, 13, 30, 0).unwrap();
    // 4 barras bearish fortes (mas fechando acima da mínima, para isolar o
    // veto de trend bars do veto de momentum-além-do-nível).
    let mut open = dec_centi(50260);
    for i in 0..4 {
        let close = open - dec_centi(40);
        candles.push(candle(
            base + Duration::minutes(i * 5),
            open,
            open + dec_centi(10),
            close - dec_centi(5),
            close,
        ));
        open = close;
    }
    candles.push(bull_signal_bar());
    match strategy().analyze_candles("SPY", &candles) {
        SignalResult::Rejected { reason, .. } => {
            assert_eq!(reason, RejectionReason::MomentumAgainst)
        }
        other => panic!("esperado MomentumAgainst, obtido {other:?}"),
    }
}

// --- CASO 7: fora da janela (11h ET) ---
#[test]
fn outside_window_rejects() {
    let mut candles = long_series();
    let last = candles.len() - 1;
    candles[last].timestamp = Utc.with_ymd_and_hms(2026, 7, 2, 15, 0, 0).unwrap();
    match strategy().analyze_candles("SPY", &candles) {
        SignalResult::Rejected { reason, .. } => {
            assert_eq!(reason, RejectionReason::OutsideTradingHours)
        }
        other => panic!("esperado OutsideTradingHours, obtido {other:?}"),
    }
}

// --- CASO 8: sem dia anterior ---
#[test]
fn no_yesterday_rejects() {
    let candles = today_open_candles();
    match strategy().analyze_candles("SPY", &candles) {
        SignalResult::Rejected { reason, .. } => {
            assert_eq!(reason, RejectionReason::IncompleteSetup)
        }
        other => panic!("esperado IncompleteSetup, obtido {other:?}"),
    }
}

// --- CASO 9: stop monetário de 60% (barra gigante) ---
#[test]
fn giant_signal_bar_uses_monetary_stop() {
    let mut candles = yesterday_candles();
    candles.extend(today_open_candles());
    // Barra de sinal com range 4,00: risco normal (3,92) > 1,5×ATR (~1,8).
    candles.push(candle(
        Utc.with_ymd_and_hms(2026, 7, 2, 13, 45, 0).unwrap(),
        dec_centi(50020),
        dec_centi(50150),
        dec_centi(49760),
        dec_centi(50140),
    ));
    match strategy().analyze_candles("SPY", &candles) {
        SignalResult::Signal(signal) => {
            assert_eq!(signal.entry_price, Some(dec_centi(50151)));
            // Stop monetário: 501,51 − 0,60×3,90 = 499,17.
            assert_eq!(signal.stop_price, Some(dec_centi(49917)));
            assert_eq!(
                signal.market_snapshot["monetary_stop"],
                serde_json::json!(true)
            );
        }
        SignalResult::Rejected { reason, details } => {
            panic!("esperado sinal, rejeitado por {reason:?}: {details:?}")
        }
        _ => panic!("esperado sinal"),
    }
}

// --- CASO 10: RR mínimo via config ---
#[test]
fn poor_risk_reward_rejects() {
    let candles = long_series();
    let mut cfg = config::OpeningReversalV1Config::default();
    cfg.strategy.parameters.min_risk_reward = dec(3);
    let s = OpeningReversalV1::new(cfg);
    match s.analyze_candles("SPY", &candles) {
        SignalResult::Rejected { reason, .. } => {
            assert_eq!(reason, RejectionReason::PoorRiskReward)
        }
        other => panic!("esperado PoorRiskReward, obtido {other:?}"),
    }
}

// --- CASO 11: toque na borda da zona (sem cruzar o nível) ---
#[test]
fn zone_edge_touch_is_valid() {
    let mut candles = yesterday_candles();
    candles.extend(today_open_candles());
    // Mínima 501,40: dentro da zona (≤ 501,50) mas sem cruzar o nível 500.
    candles.push(candle(
        Utc.with_ymd_and_hms(2026, 7, 2, 13, 45, 0).unwrap(),
        dec_centi(50190),
        dec_centi(50230),
        dec_centi(50140),
        dec_centi(50220),
    ));
    match strategy().analyze_candles("SPY", &candles) {
        SignalResult::Signal(signal) => {
            assert_eq!(signal.direction, Direction::Long);
        }
        SignalResult::Rejected { reason, details } => {
            panic!("esperado sinal, rejeitado por {reason:?}: {details:?}")
        }
        _ => panic!("esperado sinal"),
    }
}

// --- CASO 12: gap acima da máxima sem falha (sem barra de reversão) ---
#[test]
fn gap_without_failure_rejects() {
    let mut candles = yesterday_candles();
    let base = Utc.with_ymd_and_hms(2026, 7, 2, 13, 30, 0).unwrap();
    // Abertura em gap acima da máxima de ontem (505) com barras fortes de alta.
    candles.push(candle(
        base,
        dec_centi(50550),
        dec_centi(50600),
        dec_centi(50530),
        dec_centi(50590),
    ));
    candles.push(candle(
        base + Duration::minutes(15),
        dec_centi(50590),
        dec_centi(50640),
        dec_centi(50570),
        dec_centi(50630),
    ));
    // Barra bull (sem reversão) tocando a zona da máxima.
    candles.push(candle(
        Utc.with_ymd_and_hms(2026, 7, 2, 13, 45, 0).unwrap(),
        dec_centi(50630),
        dec_centi(50680),
        dec_centi(50610),
        dec_centi(50670),
    ));
    match strategy().analyze_candles("SPY", &candles) {
        SignalResult::Rejected { reason, .. } => {
            assert!(
                matches!(
                    reason,
                    RejectionReason::WeakConfirmation
                        | RejectionReason::MomentumAgainst
                        | RejectionReason::YesterdayLevelNotTested
                ),
                "esperado rejeição de confirmação/momentum, obtido {reason:?}"
            )
        }
        other => panic!("esperado rejeição, obtido {other:?}"),
    }
}
