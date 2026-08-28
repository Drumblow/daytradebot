//! Testes unitários com candles sintéticos — os 12 casos da seção 12 do doc
//! (`docs/strategies/trendline-break-test-v1.md`).
//!
//! Série canônica: 210 barras de enchimento planas em 110 (para o
//! `MarketContextAnalyzer`, que precisa de 200 para a SMA200) seguidas de uma
//! estrutura de 21 barras montada à mão:
//!
//! - barras 0–11: tendência de BAIXA com dois lower highs (112 e 110,5) e dois
//!   lower lows (107 e 104). O extremo é 104 na barra 11.
//! - barras 12–15: perna contrária que rompe a trendline 112→110,5 no
//!   fechamento da barra 14 e supera o último lower high na barra 15.
//! - barras 16–20: volta para testar o extremo; a barra 20 é a barra de sinal,
//!   com mínima 104,2 (undershoot de 0,20 sobre o extremo 104).
//!
//! O timestamp da última barra é fixado em 15:00 UTC, dentro da janela
//! operacional 14:00–19:15.

use chrono::{Duration, TimeZone, Utc};
use rust_decimal::Decimal;

use super::*;
use crate::strategies::trendline_break_test_v1::context::{trendline_value_at, TrendKind};
use trader_domain::{Direction, RejectionReason, SignalResult, TimeFrame};

/// (open, high, low, close) em centavos-de-unidade para evitar float.
type Bar = (i64, i64, i64, i64);

fn d(v: i64) -> Decimal {
    Decimal::new(v, 2)
}

fn candle(ts: chrono::DateTime<Utc>, b: Bar) -> Candle {
    Candle::new(
        "IWM",
        TimeFrame::M15,
        ts,
        d(b.0),
        d(b.1),
        d(b.2),
        d(b.3),
        Decimal::from(1000),
    )
    .expect("candle válido")
}

/// Estrutura base: baixa → rompimento → teste do extremo (gera sinal long).
fn base_structure() -> Vec<Bar> {
    vec![
        (11000, 11050, 10950, 11000), // 0
        (11000, 11100, 11000, 11080), // 1
        (11080, 11200, 11080, 11150), // 2  PEAK A = 112,00
        (11150, 11150, 11050, 11080), // 3
        (11080, 11080, 10950, 10980), // 4
        (10980, 10980, 10700, 10750), // 5  TROUGH A = 107,00
        (10750, 10900, 10750, 10880), // 6
        (10880, 11000, 10850, 10980), // 7
        (10980, 11050, 10950, 11000), // 8  PEAK B = 110,50 (lower high)
        (11000, 11000, 10850, 10880), // 9
        (10880, 10880, 10650, 10700), // 10
        (10700, 10700, 10400, 10450), // 11 TROUGH B = 104,00 (extremo)
        (10450, 10700, 10450, 10680), // 12
        (10680, 10920, 10650, 10900), // 13
        (10900, 11100, 10880, 11060), // 14 ROMPIMENTO (linha = 109,00)
        (11060, 11250, 11040, 11200), // 15 supera o último lower high
        (11200, 11200, 11000, 11020), // 16
        (11020, 11050, 10800, 10830), // 17
        (10830, 10850, 10600, 10630), // 18
        (10630, 10650, 10460, 10490), // 19
        (10470, 10550, 10420, 10535), // 20 BARRA DE SINAL (teste de 104,00)
    ]
}

/// Espelha a estrutura em torno de 220 para virar alta + reversão short.
fn mirrored(bars: &[Bar]) -> Vec<Bar> {
    const M: i64 = 22000;
    bars.iter()
        .map(|(o, h, l, c)| (M - o, M - l, M - h, M - c))
        .collect()
}

/// Monta a série completa: enchimento plano + estrutura, terminando às 15:00 UTC.
fn series(structure: &[Bar]) -> Vec<Candle> {
    series_ending_at(
        structure,
        Utc.with_ymd_and_hms(2026, 7, 22, 15, 0, 0).unwrap(),
    )
}

fn series_ending_at(structure: &[Bar], last_ts: chrono::DateTime<Utc>) -> Vec<Candle> {
    const FILLER: usize = 210;
    let total = FILLER + structure.len();
    let mut out = Vec::with_capacity(total);
    for i in 0..FILLER {
        let ts = last_ts - Duration::minutes(15 * (total - 1 - i) as i64);
        out.push(candle(ts, (11000, 11020, 10980, 11000)));
    }
    for (k, b) in structure.iter().enumerate() {
        let idx = FILLER + k;
        let ts = last_ts - Duration::minutes(15 * (total - 1 - idx) as i64);
        out.push(candle(ts, *b));
    }
    out
}

fn strategy() -> TrendlineBreakTestV1 {
    TrendlineBreakTestV1::new(config::TrendlineBreakTestV1Config::default())
}

fn reason_of(r: &SignalResult) -> RejectionReason {
    match r {
        SignalResult::Rejected { reason, .. } => *reason,
        other => panic!("esperava rejeição, veio {other:?}"),
    }
}

// --- 1 --------------------------------------------------------------------

#[test]
fn baixa_rompida_e_testada_gera_long() {
    let s = strategy();
    let candles = series(&base_structure());
    match s.analyze_candles("IWM", &candles) {
        SignalResult::Signal(sig) => {
            assert_eq!(sig.direction, Direction::Long);
            let entry = sig.entry_price.unwrap();
            let stop = sig.stop_price.unwrap();
            let target = sig.target_price.unwrap();
            assert!(
                entry > d(10550),
                "entrada acima da máxima da barra de sinal"
            );
            assert!(stop < d(10420), "stop abaixo do extremo do teste");
            // alvo = 2R
            assert_eq!(target, entry + (entry - stop) * Decimal::from(2));
        }
        other => panic!("esperava sinal long, veio {other:?}"),
    }
}

// --- 2 --------------------------------------------------------------------

#[test]
fn alta_rompida_e_testada_gera_short() {
    let s = strategy();
    let candles = series(&mirrored(&base_structure()));
    match s.analyze_candles("IWM", &candles) {
        SignalResult::Signal(sig) => {
            assert_eq!(sig.direction, Direction::Short);
            let entry = sig.entry_price.unwrap();
            let stop = sig.stop_price.unwrap();
            assert!(stop > entry, "stop acima da entrada em short");
            assert_eq!(
                sig.target_price.unwrap(),
                entry - (stop - entry) * Decimal::from(2)
            );
        }
        other => panic!("esperava sinal short, veio {other:?}"),
    }
}

// --- 3 --------------------------------------------------------------------

#[test]
fn sem_estrutura_de_tendencia_rejeita() {
    let s = strategy();
    // Só enchimento plano: nenhum pivô, nenhuma estrutura.
    let flat: Vec<Bar> = (0..21).map(|_| (11000, 11020, 10980, 11000)).collect();
    let candles = series(&flat);
    assert_eq!(
        reason_of(&s.analyze_candles("IWM", &candles)),
        RejectionReason::NoTrendToReverse
    );
}

// --- 4 --------------------------------------------------------------------

#[test]
fn tendencia_sem_rompimento_de_trendline_rejeita() {
    let s = strategy();
    let mut st = base_structure();
    // A perna contrária nunca fecha acima da linha (fica abaixo de 106).
    for bar in st.iter_mut().skip(12) {
        bar.1 = bar.1.min(10600);
        bar.3 = bar.3.min(10550);
        bar.0 = bar.0.min(10550);
        bar.2 = bar.2.min(10500);
    }
    let candles = series(&st);
    assert_eq!(
        reason_of(&s.analyze_candles("IWM", &candles)),
        RejectionReason::NoTrendlineBreak
    );
}

// --- 5 --------------------------------------------------------------------

#[test]
fn rompimento_sem_momentum_rejeita() {
    let s = strategy();
    let mut st = base_structure();
    // Perna contrária fraca: rompe a linha na barra 14 (linha = 109,00), mas o
    // ápice fica em 110,00 — abaixo do último lower high (110,50). É o sinal de
    // força que falta, não o tamanho da perna.
    st[12] = (10450, 10700, 10450, 10680);
    st[13] = (10680, 10850, 10650, 10830);
    st[14] = (10830, 10950, 10800, 10930);
    st[15] = (10930, 11000, 10900, 10980);
    st[16] = (10980, 10990, 10850, 10870);
    st[17] = (10870, 10880, 10800, 10830);
    let candles = series(&st);
    let reason = reason_of(&s.analyze_candles("IWM", &candles));
    assert_eq!(reason, RejectionReason::BreakWithoutMomentum);
}

// --- 6 --------------------------------------------------------------------

#[test]
fn rompimento_antigo_demais_rejeita() {
    // Config com janela de idade curta: o rompimento da barra 14 fica a 6
    // barras do sinal, acima do limite de 3.
    let mut cfg = config::TrendlineBreakTestV1Config::default();
    cfg.strategy.parameters.break_max_age = 3;
    let s = TrendlineBreakTestV1::new(cfg);
    let candles = series(&base_structure());
    assert_eq!(
        reason_of(&s.analyze_candles("IWM", &candles)),
        RejectionReason::BreakTooOld
    );
}

// --- 7 --------------------------------------------------------------------

#[test]
fn sem_teste_do_extremo_rejeita() {
    let s = strategy();
    let mut st = base_structure();
    // A barra de sinal fica muito acima do extremo (108,00 contra 104,00).
    st[20] = (10810, 10870, 10800, 10855);
    let candles = series(&st);
    assert_eq!(
        reason_of(&s.analyze_candles("IWM", &candles)),
        RejectionReason::NoExtremeTest
    );
}

// --- 8 --------------------------------------------------------------------

#[test]
fn overshoot_excessivo_anula_a_reversao() {
    let s = strategy();
    let mut st = base_structure();
    // Fura o extremo em 3,00 — muito além de 0,5 x ATR.
    st[19] = (10630, 10650, 10150, 10200);
    st[20] = (10170, 10280, 10100, 10265);
    let candles = series(&st);
    assert_eq!(
        reason_of(&s.analyze_candles("IWM", &candles)),
        RejectionReason::ReversalNullified
    );
}

// --- 9 --------------------------------------------------------------------

#[test]
fn barra_de_sinal_fraca_rejeita() {
    let s = strategy();
    let mut st = base_structure();
    // Doji no nível do teste: corpo minúsculo, sem sombra inferior relevante.
    st[20] = (10500, 10550, 10420, 10505);
    let candles = series(&st);
    assert_eq!(
        reason_of(&s.analyze_candles("IWM", &candles)),
        RejectionReason::WeakConfirmation
    );
}

// --- 10 -------------------------------------------------------------------

#[test]
fn risco_retorno_insuficiente_rejeita() {
    // min_risk_reward acima do alvo fixo de 2R torna o setup irrecebível.
    let mut cfg = config::TrendlineBreakTestV1Config::default();
    cfg.strategy.parameters.min_risk_reward = Decimal::from(3);
    let s = TrendlineBreakTestV1::new(cfg);
    let candles = series(&base_structure());
    assert_eq!(
        reason_of(&s.analyze_candles("IWM", &candles)),
        RejectionReason::PoorRiskReward
    );
}

// --- 11 -------------------------------------------------------------------

#[test]
fn fora_da_janela_de_horario_rejeita() {
    let s = strategy();
    let candles = series_ending_at(
        &base_structure(),
        Utc.with_ymd_and_hms(2026, 7, 22, 13, 0, 0).unwrap(),
    );
    assert_eq!(
        reason_of(&s.analyze_candles("IWM", &candles)),
        RejectionReason::OutsideTradingHours
    );
}

// --- 12 -------------------------------------------------------------------

#[test]
fn trendline_extrapola_pelos_dois_pivos() {
    let candles = series(&base_structure());
    let params = config::TrendlineBreakTestV1Config::default();
    let trend = context::detect_trend(&candles, &params.strategy.parameters)
        .expect("tendência de baixa reconhecida");

    assert_eq!(trend.kind, TrendKind::Bear);
    assert_eq!(trend.extreme_price, d(10400));
    assert_eq!(trend.last_counter_swing, d(11050));

    // Linha por PEAK A (112,00) e PEAK B (110,50), 6 barras de distância:
    // inclinação −0,25 por barra. Seis barras depois de B: 110,50 − 1,50.
    let at_b = trendline_value_at(&candles, &trend, trend.line_p2).unwrap();
    assert_eq!(at_b, d(11050));
    let six_after = trendline_value_at(&candles, &trend, trend.line_p2 + 6).unwrap();
    assert_eq!(six_after, d(10900));
}
