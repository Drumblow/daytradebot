//! Testes unitários com candles sintéticos — os 12 casos da seção 13 do doc
//! (`docs/strategies/range-extreme-fade-v1.md`).
//!
//! Série canônica:
//! - 15 dias anteriores (2026-07-01 a 2026-07-21, dias úteis): 26 candles de
//!   15min por dia oscilando flat em 99,50–100,50 (range diário ~1,0 → ATR
//!   diário = 1,0). Alta/baixa exatas repetidas → pivôs iguais consecutivos
//!   não formam estrutura de tendência.
//! - "Hoje" (2026-07-22 ET): 3 candles flat 100,20–100,80 (13:30–14:00 UTC) +
//!   barra de sinal às 14:15 UTC (10:15 ET — dentro da janela 13:45–19:15,
//!   fora do veto do meio do dia 15:30–18:00 UTC).
//! - ATR14(15min) dos candles flat = 0,60 → extensão máxima default = 0,30.

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
    Candle::new("IWM", TimeFrame::M15, ts, open, high, low, close, dec(1000))
        .expect("candle válido")
}

/// Um dia útil flat: 26 candles alternando entre dois ranges fixos com
/// corpos reais (barras ímpares bear, pares bull) — dojis caracterizariam
/// Barb Wire e disparariam o veto do Cap. 5. Extremos iguais repetidos →
/// pivôs iguais consecutivos → sem estrutura de tendência.
fn flat_day(year: i32, month: u32, day: u32) -> Vec<Candle> {
    let base = Utc.with_ymd_and_hms(year, month, day, 13, 30, 0).unwrap();
    let mut candles = Vec::new();
    for i in 0..26 {
        let ts = base + Duration::minutes(i * 15);
        // Ranges 0,60 fixos; corpo 0,40 (67% — longe de doji).
        let (open, high, low, close) = if i % 2 == 0 {
            (
                dec_centi(9960),
                dec_centi(10020),
                dec_centi(9960),
                dec_centi(10000),
            )
        } else {
            (
                dec_centi(10010),
                dec_centi(10010),
                dec_centi(9950),
                dec_centi(9970),
            )
        };
        candles.push(candle(ts, open, high, low, close));
    }
    candles
}

/// 15 dias úteis flat anteriores a 2026-07-22.
fn prior_days() -> Vec<Candle> {
    let dates = [
        (2026, 7, 1),
        (2026, 7, 2),
        (2026, 7, 3),
        (2026, 7, 6),
        (2026, 7, 7),
        (2026, 7, 8),
        (2026, 7, 9),
        (2026, 7, 10),
        (2026, 7, 13),
        (2026, 7, 14),
        (2026, 7, 15),
        (2026, 7, 16),
        (2026, 7, 17),
        (2026, 7, 20),
        (2026, 7, 21),
    ];
    let mut candles = Vec::new();
    for (y, m, d) in dates {
        candles.extend(flat_day(y, m, d));
    }
    candles
}

/// Abertura de hoje: 3 candles flat 100,20–100,80 com corpo (13:30, 13:45,
/// 14:00 UTC). Extremos do dia antes do sinal: máx 100,80 / mín 100,20.
fn today_open() -> Vec<Candle> {
    let base = Utc.with_ymd_and_hms(2026, 7, 22, 13, 30, 0).unwrap();
    (0..3)
        .map(|i| {
            candle(
                base + Duration::minutes(i * 15),
                dec_centi(10040),
                dec_centi(10080),
                dec_centi(10020),
                dec_centi(10060),
            )
        })
        .collect()
}

/// Abertura estendida com N barras (para o teste da regra da EMA, que olha
/// os últimos 8 closes).
fn today_open_n(n: i64) -> Vec<Candle> {
    let base = Utc.with_ymd_and_hms(2026, 7, 22, 13, 30, 0).unwrap();
    (0..n)
        .map(|i| {
            candle(
                base + Duration::minutes(i * 15),
                dec_centi(10040),
                dec_centi(10080),
                dec_centi(10020),
                dec_centi(10060),
            )
        })
        .collect()
}

/// Barra de sinal bear perfeita: rompe a máxima do dia (100,80) por 0,20
/// (≤ 0,30 de extensão máxima) e fecha forte para baixo.
/// open 100,75 / high 101,00 / low 100,30 / close 100,40:
/// range 0,70; corpo 0,35 = 50% ✓; close no terço inferior (≤ 100,5333) ✓;
/// sombra superior 0,25 ≥ 0,2333 ✓.
fn bear_signal_bar() -> Candle {
    candle(
        Utc.with_ymd_and_hms(2026, 7, 22, 14, 15, 0).unwrap(),
        dec_centi(10075),
        dec_centi(10100),
        dec_centi(10030),
        dec_centi(10040),
    )
}

/// Barra de sinal bull perfeita: rompe a mínima do dia (100,20) por 0,20 e
/// fecha forte para cima. Espelho da bear.
fn bull_signal_bar() -> Candle {
    candle(
        Utc.with_ymd_and_hms(2026, 7, 22, 14, 15, 0).unwrap(),
        dec_centi(10025),
        dec_centi(10070),
        dec_centi(10000),
        dec_centi(10060),
    )
}

fn short_series() -> Vec<Candle> {
    let mut candles = prior_days();
    candles.extend(today_open());
    candles.push(bear_signal_bar());
    candles
}

fn long_series() -> Vec<Candle> {
    let mut candles = prior_days();
    candles.extend(today_open());
    candles.push(bull_signal_bar());
    candles
}

fn strategy() -> RangeExtremeFadeV1 {
    RangeExtremeFadeV1::new(config::RangeExtremeFadeV1Config::default())
}

fn assert_signal(result: &SignalResult, direction: Direction) -> trader_domain::Signal {
    match result {
        SignalResult::Signal(signal) => {
            assert_eq!(signal.direction, direction);
            signal.clone()
        }
        other => panic!("esperava Signal, veio {other:?}"),
    }
}

fn assert_rejection(result: &SignalResult, reason: RejectionReason) {
    match result {
        SignalResult::Rejected { reason: r, .. } => assert_eq!(*r, reason),
        other => panic!("esperava Rejected({reason:?}), veio {other:?}"),
    }
}

// 1. Setup perfeito short (rompimento falho da máxima do dia) → sinal short.
#[test]
fn failed_day_high_breakout_gera_sinal_short() {
    let result = strategy().analyze_candles("IWM", &short_series());
    let signal = assert_signal(&result, Direction::Short);
    // Entrada = low da sinal − 1 tick = 100,29; stop = high + 1 tick = 101,01.
    assert_eq!(signal.entry_price, Some(dec_centi(10029)));
    assert_eq!(signal.stop_price, Some(dec_centi(10101)));
    // Risco 0,72 → alvo 1,5R = 100,29 − 1,08 = 99,21.
    assert_eq!(signal.target_price, Some(dec_centi(9921)));
}

// 2. Setup perfeito long (rompimento falho da mínima do dia) → sinal long.
#[test]
fn failed_day_low_breakout_gera_sinal_long() {
    let result = strategy().analyze_candles("IWM", &long_series());
    let signal = assert_signal(&result, Direction::Long);
    // Entrada = high da sinal + 1 tick = 100,71; stop = low − 1 tick = 99,99.
    assert_eq!(signal.entry_price, Some(dec_centi(10071)));
    assert_eq!(signal.stop_price, Some(dec_centi(9999)));
    // Risco 0,72 → alvo = 100,71 + 1,08 = 101,79.
    assert_eq!(signal.target_price, Some(dec_centi(10179)));
}

// 3. Dia de tendência (sequência crescente nas últimas barras) → NotARangeDay.
#[test]
fn trend_day_rejeita_como_nao_range() {
    let mut candles = prior_days();
    candles.extend(today_open());
    // Tendência de alta nas últimas 12+ barras: cada candle mais alto.
    let base = Utc.with_ymd_and_hms(2026, 7, 22, 14, 15, 0).unwrap();
    for i in 0..12 {
        let low = dec_centi(10100 + i * 20);
        let high = low + dec_centi(30);
        candles.push(candle(
            base + Duration::minutes(i * 15),
            low + dec_centi(5),
            high,
            low,
            high - dec_centi(5),
        ));
    }
    let result = strategy().analyze_candles("IWM", &candles);
    assert_rejection(&result, RejectionReason::NotARangeDay);
}

// 4. Rompimento com extensão além do máximo (0,60 > 0,30) → BreakoutTooStrong.
#[test]
fn rompimento_forte_rejeita() {
    let mut candles = prior_days();
    candles.extend(today_open());
    candles.push(candle(
        Utc.with_ymd_and_hms(2026, 7, 22, 14, 15, 0).unwrap(),
        dec_centi(10090),
        dec_centi(10120), // máxima do dia era 100,80 → extensão 0,40 > 0,30 (e range do dia 1,00 < 1,05)
        dec_centi(10070),
        dec_centi(10110),
    ));
    let result = strategy().analyze_candles("IWM", &candles);
    assert_rejection(&result, RejectionReason::BreakoutTooStrong);
}

// 5. Regra da EMA habilitada: maioria dos closes acima da EMA → long vetado.
#[test]
fn regra_da_ema_veta_long_com_closes_acima() {
    let mut config = config::RangeExtremeFadeV1Config::default();
    config.strategy.parameters.use_ema_side_rule = true;
    let strategy = RangeExtremeFadeV1::new(config);
    // 5 barras de abertura fechando em 100,60 → maioria dos últimos 8 closes
    // acima da EMA20 (que ainda puxa dos dias em ~99,85–100,00).
    let mut candles = prior_days();
    candles.extend(today_open_n(5));
    candles.push(bull_signal_bar_at(
        Utc.with_ymd_and_hms(2026, 7, 22, 14, 45, 0).unwrap(),
    ));
    let result = strategy.analyze_candles("IWM", &candles);
    assert_rejection(&result, RejectionReason::WrongSideOfEma);
}

fn bull_signal_bar_at(ts: chrono::DateTime<Utc>) -> Candle {
    candle(
        ts,
        dec_centi(10025),
        dec_centi(10070),
        dec_centi(10000),
        dec_centi(10060),
    )
}

// 6a. Meio do dia + meio do range → MiddayMidrange.
#[test]
fn meio_do_dia_meio_do_range_vetado() {
    let mut candles = prior_days();
    candles.extend(today_open());
    // 12:00 ET (16:00 UTC — dentro do veto), fechamento no terço central.
    candles.push(candle(
        Utc.with_ymd_and_hms(2026, 7, 22, 16, 0, 0).unwrap(),
        dec_centi(10050),
        dec_centi(10060),
        dec_centi(10040),
        dec_centi(10050),
    ));
    let result = strategy().analyze_candles("IWM", &candles);
    assert_rejection(&result, RejectionReason::MiddayMidrange);
}

// 6b. Mesmo preço fora do horário do veto → não é MiddayMidrange.
#[test]
fn meio_do_range_fora_do_horario_nao_veta() {
    let mut candles = prior_days();
    candles.extend(today_open());
    candles.push(candle(
        Utc.with_ymd_and_hms(2026, 7, 22, 14, 15, 0).unwrap(),
        dec_centi(10050),
        dec_centi(10060),
        dec_centi(10040),
        dec_centi(10050),
    ));
    let result = strategy().analyze_candles("IWM", &candles);
    match result {
        SignalResult::Rejected { reason, .. } => {
            assert_ne!(reason, RejectionReason::MiddayMidrange)
        }
        other => panic!("esperava alguma rejeição (sem rompimento), veio {other:?}"),
    }
}

// 7. Barb Wire (3 barras sobrepostas com doji) → BarbWire.
#[test]
fn barb_wire_vetado() {
    let mut candles = prior_days();
    candles.extend(today_open());
    let base = Utc.with_ymd_and_hms(2026, 7, 22, 14, 15, 0).unwrap();
    // 3 barras quase idênticas 100,30–100,70; a última é doji (open≈close).
    let shapes = [
        (10045i64, 10055i64),
        (10050, 10040),
        (10050, 10050), // doji
    ];
    for (i, (open, close)) in shapes.iter().enumerate() {
        candles.push(candle(
            base + Duration::minutes((i as i64) * 15),
            dec_centi(*open),
            dec_centi(10070),
            dec_centi(10030),
            dec_centi(*close),
        ));
    }
    let result = strategy().analyze_candles("IWM", &candles);
    assert_rejection(&result, RejectionReason::BarbWire);
}

// 8. Rompimento com barra de sinal fraca (doji) → WeakConfirmation.
#[test]
fn barra_de_sinal_fraca_rejeita() {
    let mut candles = prior_days();
    candles.extend(today_open());
    candles.push(candle(
        Utc.with_ymd_and_hms(2026, 7, 22, 14, 15, 0).unwrap(),
        dec_centi(10080),
        dec_centi(10100), // rompe por 0,20
        dec_centi(10060),
        dec_centi(10082), // doji: corpo 0,02 de range 0,40 = 5%
    ));
    let result = strategy().analyze_candles("IWM", &candles);
    assert_rejection(&result, RejectionReason::WeakConfirmation);
}

// 9. Sinal fora da janela (13:30 UTC = 09:30 ET, antes das 09:45) → OutsideTradingHours.
#[test]
fn fora_da_janela_rejeita() {
    let mut candles = prior_days();
    // Sinal logo na primeira barra do dia (sem extremo prévio, mas a janela
    // é checada antes): fora do horário operacional.
    candles.push(bear_signal_bar_at(
        Utc.with_ymd_and_hms(2026, 7, 22, 13, 30, 0).unwrap(),
    ));
    let result = strategy().analyze_candles("IWM", &candles);
    assert_rejection(&result, RejectionReason::OutsideTradingHours);
}

fn bear_signal_bar_at(ts: chrono::DateTime<Utc>) -> Candle {
    candle(
        ts,
        dec_centi(10075),
        dec_centi(10100),
        dec_centi(10030),
        dec_centi(10040),
    )
}

// 10. RR abaixo do mínimo configurado → PoorRiskReward.
#[test]
fn rr_abaixo_do_minimo_rejeita() {
    let mut config = config::RangeExtremeFadeV1Config::default();
    config.strategy.parameters.min_risk_reward = dec(2); // alvo 1,5R < 2
    let strategy = RangeExtremeFadeV1::new(config);
    let result = strategy.analyze_candles("IWM", &short_series());
    assert_rejection(&result, RejectionReason::PoorRiskReward);
}

// 11. Extensão exatamente no limite (0,30 = 0,5 × ATR 0,60) → sinal válido.
#[test]
fn extensao_na_borda_do_limite_passa() {
    let mut candles = prior_days();
    candles.extend(today_open());
    candles.push(candle(
        Utc.with_ymd_and_hms(2026, 7, 22, 14, 15, 0).unwrap(),
        dec_centi(10080),
        dec_centi(10110), // máxima do dia 100,80 + 0,30 exatos
        dec_centi(10025),
        dec_centi(10035), // corpo 0,45 (53%); sombra sup. 0,30 ≥ 0,2833 ✓; close no terço inf.
    ));
    let result = strategy().analyze_candles("IWM", &candles);
    assert_signal(&result, Direction::Short);
}

// 12. Barra de sinal como primeira do dia (sem extremo prévio) → IncompleteSetup.
#[test]
fn primeira_barra_do_dia_sem_extremo_previo() {
    let mut candles = prior_days();
    candles.push(bear_signal_bar_at(
        Utc.with_ymd_and_hms(2026, 7, 22, 13, 45, 0).unwrap(),
    ));
    let result = strategy().analyze_candles("IWM", &candles);
    assert_rejection(&result, RejectionReason::IncompleteSetup);
}
