//! Testes unitários com candles sintéticos — os 10 casos da seção 12 do doc
//! (`docs/strategies/balance-area-breakout-v1.md`).
//!
//! Série canônica: 3 dias de área de balanceamento 100,00–100,80 (78 candles,
//! largura 0,8% e 1,0×ATR) + candle de rompimento no 4º dia.

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

/// 3 dias de área 100,00–100,80 (26 candles/dia, 13:30–19:45 UTC).
fn balance_candles() -> Vec<Candle> {
    let mut candles = Vec::new();
    for day in 0..3i64 {
        let base = Utc
            .with_ymd_and_hms(2026, 6, 29, 13, 30, 0)
            .unwrap()
            .checked_add_days(chrono::Days::new(day as u64))
            .unwrap();
        for i in 0..26 {
            let ts = base + Duration::minutes(i * 15);
            let close = dec_centi(10040);
            candles.push(candle(ts, close, dec_centi(10080), dec(100), close));
        }
    }
    candles
}

/// Candle de rompimento long: fecha 101,00 (acima da área), TR = 0,80 (igual
/// ao da área, mantendo ATR = 0,80 exato), às 13:45 UTC do 4º dia.
fn long_breakout_candle() -> Candle {
    candle(
        Utc.with_ymd_and_hms(2026, 7, 2, 13, 45, 0).unwrap(),
        dec_centi(10060),
        dec_centi(10120),
        dec_centi(10040),
        dec(101),
    )
}

fn long_series() -> Vec<Candle> {
    let mut candles = balance_candles();
    candles.push(long_breakout_candle());
    candles
}

fn short_series() -> Vec<Candle> {
    let mut candles = balance_candles();
    // Rompimento short: fecha 99,80 (abaixo da área), TR = 0,80.
    candles.push(candle(
        Utc.with_ymd_and_hms(2026, 7, 2, 13, 45, 0).unwrap(),
        dec_centi(10020),
        dec_centi(10030),
        dec_centi(9960),
        dec_centi(9980),
    ));
    candles
}

fn strategy() -> BalanceAreaBreakoutV1 {
    BalanceAreaBreakoutV1::new(config::BalanceAreaBreakoutV1Config::default())
}

// --- CASO 1: setup perfeito long ---
#[test]
fn perfect_long_setup_generates_signal() {
    let candles = long_series();
    match strategy().analyze_candles("SPY", &candles) {
        SignalResult::Signal(signal) => {
            assert_eq!(signal.direction, Direction::Long);
            assert_eq!(signal.entry_price, Some(dec_centi(10121)));
            // Stop: 100,80 − 0,3×0,80 = 100,56.
            assert_eq!(signal.stop_price, Some(dec_centi(10056)));
            // 2R: 101,21 + 2×0,65 = 102,51.
            assert_eq!(signal.target_price, Some(dec_centi(10251)));
            let snap = &signal.market_snapshot;
            for key in ["area_high", "area_low", "area_width_pct", "area_width_atr"] {
                assert!(snap.get(key).is_some(), "snapshot sem {key}");
            }
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
            assert_eq!(signal.entry_price, Some(dec_centi(9959)));
            // Stop: 100,00 + 0,3×0,80 = 100,24.
            assert_eq!(signal.stop_price, Some(dec_centi(10024)));
            // 2R short: 99,59 − 2×0,65 = 98,29.
            assert_eq!(signal.target_price, Some(dec_centi(9829)));
        }
        SignalResult::Rejected { reason, details } => {
            panic!("esperado sinal, rejeitado por {reason:?}: {details:?}")
        }
        _ => panic!("esperado sinal"),
    }
}

// --- CASO 3: sem balanceamento (tendência) ---
#[test]
fn trending_series_rejects() {
    let mut candles = Vec::new();
    let base = Utc.with_ymd_and_hms(2026, 6, 29, 13, 30, 0).unwrap();
    // Série crescente: largura muito acima dos tetos.
    for i in 0..78i64 {
        let ts = base + Duration::minutes(i * 15);
        let close = dec(100) + Decimal::new(i * 30, 2);
        candles.push(candle(
            ts,
            close,
            close + dec_centi(50),
            close - dec_centi(50),
            close,
        ));
    }
    candles.push(long_breakout_candle());
    match strategy().analyze_candles("SPY", &candles) {
        SignalResult::Rejected { reason, .. } => {
            assert_eq!(reason, RejectionReason::NoBalanceArea)
        }
        other => panic!("esperado NoBalanceArea, obtido {other:?}"),
    }
}

// --- CASO 4: área larga demais vs preço (width_pct > 2%) ---
#[test]
fn wide_area_rejects() {
    let mut candles = Vec::new();
    for day in 0..3i64 {
        let base = Utc
            .with_ymd_and_hms(2026, 6, 29, 13, 30, 0)
            .unwrap()
            .checked_add_days(chrono::Days::new(day as u64))
            .unwrap();
        for i in 0..26 {
            let ts = base + Duration::minutes(i * 15);
            // Swings de 99–103: largura ~4% do preço médio.
            let close = dec_centi(10100);
            candles.push(candle(ts, close, dec(103), dec(99), close));
        }
    }
    candles.push(long_breakout_candle());
    match strategy().analyze_candles("SPY", &candles) {
        SignalResult::Rejected { reason, .. } => {
            assert_eq!(reason, RejectionReason::NoBalanceArea)
        }
        other => panic!("esperado NoBalanceArea, obtido {other:?}"),
    }
}

// --- CASO 5: sem rompimento (último candle dentro da área) ---
#[test]
fn no_breakout_rejects() {
    let mut candles = balance_candles();
    candles.push(candle(
        Utc.with_ymd_and_hms(2026, 7, 2, 13, 45, 0).unwrap(),
        dec_centi(10040),
        dec_centi(10070),
        dec_centi(10010),
        dec_centi(10040),
    ));
    match strategy().analyze_candles("SPY", &candles) {
        SignalResult::Rejected { reason, .. } => {
            assert_eq!(reason, RejectionReason::IncompleteSetup)
        }
        other => panic!("esperado IncompleteSetup, obtido {other:?}"),
    }
}

// --- CASO 6: stop largo demais (rompimento gigante) ---
#[test]
fn stop_too_wide_rejects() {
    let mut candles = balance_candles();
    // Candle de rompimento enorme: entrada muito distante do topo da área.
    candles.push(candle(
        Utc.with_ymd_and_hms(2026, 7, 2, 13, 45, 0).unwrap(),
        dec_centi(10060),
        dec(104),
        dec_centi(10050),
        dec_centi(10350),
    ));
    match strategy().analyze_candles("SPY", &candles) {
        SignalResult::Rejected { reason, .. } => {
            assert_eq!(reason, RejectionReason::StopTooWide)
        }
        other => panic!("esperado StopTooWide, obtido {other:?}"),
    }
}

// --- CASO 7: RR ruim (via config) ---
#[test]
fn poor_risk_reward_rejects() {
    let candles = long_series();
    let mut cfg = config::BalanceAreaBreakoutV1Config::default();
    cfg.strategy.parameters.min_risk_reward = dec(3);
    let s = BalanceAreaBreakoutV1::new(cfg);
    match s.analyze_candles("SPY", &candles) {
        SignalResult::Rejected { reason, .. } => {
            assert_eq!(reason, RejectionReason::PoorRiskReward)
        }
        other => panic!("esperado PoorRiskReward, obtido {other:?}"),
    }
}

// --- CASO 8: fora de horário ---
#[test]
fn outside_trading_hours_rejects() {
    let mut candles = long_series();
    let last = candles.len() - 1;
    candles[last].timestamp = Utc.with_ymd_and_hms(2026, 7, 2, 3, 0, 0).unwrap();
    match strategy().analyze_candles("SPY", &candles) {
        SignalResult::Rejected { reason, .. } => {
            assert_eq!(reason, RejectionReason::OutsideTradingHours)
        }
        other => panic!("esperado OutsideTradingHours, obtido {other:?}"),
    }
}

// --- CASO 9: área de 1 dia só ---
#[test]
fn single_day_area_rejects() {
    let mut candles = Vec::new();
    // 78 candles no mesmo dia ET (espaçamento de 5 minutos).
    let base = Utc.with_ymd_and_hms(2026, 7, 1, 13, 30, 0).unwrap();
    for i in 0..78i64 {
        let ts = base + Duration::minutes(i * 5);
        let close = dec_centi(10040);
        candles.push(candle(ts, close, dec_centi(10080), dec(100), close));
    }
    candles.push(long_breakout_candle());
    match strategy().analyze_candles("SPY", &candles) {
        SignalResult::Rejected { reason, .. } => {
            assert_eq!(reason, RejectionReason::NoBalanceArea)
        }
        other => panic!("esperado NoBalanceArea, obtido {other:?}"),
    }
}

// --- CASO 10: snapshot auditável ---
#[test]
fn signal_carries_auditable_snapshot() {
    let candles = long_series();
    match strategy().analyze_candles("SPY", &candles) {
        SignalResult::Signal(signal) => {
            let snap = &signal.market_snapshot;
            assert_eq!(snap["area_high"], serde_json::json!("100.80"));
            assert_eq!(snap["area_low"], serde_json::json!("100"));
            assert!(snap["risk_reward"].is_string() || snap["risk_reward"].is_number());
            assert!(!signal.config_hash.is_empty());
            assert_eq!(signal.strategy_id, "balance-area-breakout-v1");
        }
        other => panic!("esperado sinal, obtido {other:?}"),
    }
}
