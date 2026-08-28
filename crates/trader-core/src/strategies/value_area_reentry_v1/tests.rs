//! Testes unitários com candles sintéticos — os 11 casos da seção 12 do doc
//! (`docs/strategies/value-area-reentry-v1.md`).
//!
//! Série canônica:
//! - 16 dias úteis anteriores (2026-07-01 a 2026-07-22), 26 candles de 15min
//!   cada, todos cobrindo 99,50–100,50 → range diário 1,00 → ATR diário 1,00.
//! - O ÚLTIMO desses dias é o "dia anterior" cuja área de valor a estratégia
//!   calcula. Os testes não fixam a VA na mão: chamam `compute_value_area` e
//!   constroem "hoje" relativo às bordas retornadas — assim o teste valida o
//!   comportamento sem depender da aritmética de faixas.
//! - "Hoje" (2026-07-23 ET): 4 candles às 13:30, 13:45, 14:00 e 14:15 UTC. A
//!   barra de sinal (14:15 UTC = 10:15 ET) cai dentro da janela 14:00–19:00.

use chrono::{Datelike, Duration, TimeZone, Utc};
use rust_decimal::Decimal;

use super::*;
use crate::strategies::value_area_reentry_v1::context::{compute_value_area, ValueArea};
use trader_domain::{Direction, RejectionReason, SignalResult, TimeFrame};

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
    Candle::new(
        "IWM",
        TimeFrame::M15,
        ts,
        open,
        high,
        low,
        close,
        Decimal::from(1000),
    )
    .expect("candle válido")
}

/// Um dia útil de 26 candles cobrindo `low`–`high` (perfil uniforme).
fn uniform_day(year: i32, month: u32, day: u32, low: Decimal, high: Decimal) -> Vec<Candle> {
    let base = Utc.with_ymd_and_hms(year, month, day, 13, 30, 0).unwrap();
    let mid = (low + high) / Decimal::from(2);
    (0..26)
        .map(|i| candle(base + Duration::minutes(i * 15), mid, high, low, mid))
        .collect()
}

/// 16 dias úteis anteriores a 2026-07-23. O último (`prev_*`) é o dia cuja
/// área de valor a estratégia usa.
fn prior_days(prev_low: Decimal, prev_high: Decimal) -> Vec<Candle> {
    let mut candles = Vec::new();
    let mut date = chrono::NaiveDate::from_ymd_opt(2026, 7, 1).unwrap();
    let mut days = Vec::new();
    while days.len() < 16 {
        if date.weekday().num_days_from_monday() < 5 {
            days.push(date);
        }
        date = date.succ_opt().unwrap();
    }
    let last = *days.last().unwrap();
    for d in &days {
        let (lo, hi) = if *d == last {
            (prev_low, prev_high)
        } else {
            (dec_centi(9950), dec_centi(10050))
        };
        candles.extend(uniform_day(d.year(), d.month(), d.day(), lo, hi));
    }
    candles
}

/// Data de "hoje" nos testes: o primeiro dia útil após os 16 anteriores.
fn today_base() -> chrono::DateTime<Utc> {
    let mut date = chrono::NaiveDate::from_ymd_opt(2026, 7, 1).unwrap();
    let mut days = Vec::new();
    while days.len() < 17 {
        if date.weekday().num_days_from_monday() < 5 {
            days.push(date);
        }
        date = date.succ_opt().unwrap();
    }
    let today = *days.last().unwrap();
    Utc.with_ymd_and_hms(today.year(), today.month(), today.day(), 13, 30, 0)
        .unwrap()
}

/// Área de valor do dia anterior, calculada pelo mesmo código de produção.
fn value_area_of(candles: &[Candle], strategy: &ValueAreaReentryV1) -> ValueArea {
    let params = strategy.parameters();
    let prev = context::previous_day_slice(candles).expect("dia anterior presente");
    compute_value_area(prev, params.va_buckets, params.va_percent, params.tick_size)
        .expect("VA calculável")
}

fn strategy() -> ValueAreaReentryV1 {
    ValueAreaReentryV1::new(config::ValueAreaReentryV1Config::default())
}

/// Monta "hoje": abertura em `open`, duas barras fora do valor e depois
/// `inside` barras fechando dentro, sendo a última a barra de sinal com
/// extremos `signal_low`/`signal_high`.
#[allow(clippy::too_many_arguments)]
fn today_series(
    open: Decimal,
    outside_close: Decimal,
    inside_close: Decimal,
    inside_bars: usize,
    signal_low: Decimal,
    signal_high: Decimal,
) -> Vec<Candle> {
    let base = today_base();
    let mut out = Vec::new();
    // Duas barras fora do valor (a segunda é a barra "antes" da aceitação).
    for i in 0..2 {
        let hi = open.max(outside_close);
        let lo = open.min(outside_close);
        out.push(candle(
            base + Duration::minutes(i * 15),
            open,
            hi,
            lo,
            outside_close,
        ));
    }
    // Barras de aceitação.
    for i in 0..inside_bars {
        let ts = base + Duration::minutes((2 + i as i64) * 15);
        let is_last = i + 1 == inside_bars;
        let (lo, hi) = if is_last {
            (signal_low, signal_high)
        } else {
            (inside_close - dec_centi(5), inside_close + dec_centi(5))
        };
        out.push(candle(ts, inside_close, hi, lo, inside_close));
    }
    out
}

fn series(prev_low: Decimal, prev_high: Decimal, today: Vec<Candle>) -> Vec<Candle> {
    let mut c = prior_days(prev_low, prev_high);
    c.extend(today);
    c
}

fn reason_of(result: &SignalResult) -> RejectionReason {
    match result {
        SignalResult::Rejected { reason, .. } => *reason,
        other => panic!("esperava rejeição, veio {other:?}"),
    }
}

// --- 1 --------------------------------------------------------------------

#[test]
fn abertura_abaixo_do_valor_com_aceitacao_gera_long() {
    let s = strategy();
    let base = series(dec_centi(9950), dec_centi(10050), Vec::new());
    let va = value_area_of(&base, &s);

    let today = today_series(
        va.low - dec_centi(30), // abre 0,30 abaixo do valor
        va.low - dec_centi(20), // fecha fora
        va.low + dec_centi(6),  // fecha dentro
        2,
        va.low + dec_centi(2),
        va.low + dec_centi(10),
    );
    let candles = series(dec_centi(9950), dec_centi(10050), today);

    match s.analyze_candles("IWM", &candles) {
        SignalResult::Signal(sig) => {
            assert_eq!(sig.direction, Direction::Long);
            assert_eq!(
                sig.target_price,
                Some(va.high),
                "alvo é a borda oposta da VA"
            );
            assert!(sig.stop_price.unwrap() < va.low, "stop fica fora da VA");
            assert!(sig.entry_price.unwrap() > va.low + dec_centi(2));
        }
        other => panic!("esperava sinal long, veio {other:?}"),
    }
}

// --- 2 --------------------------------------------------------------------

#[test]
fn abertura_acima_do_valor_com_aceitacao_gera_short() {
    let s = strategy();
    let base = series(dec_centi(9950), dec_centi(10050), Vec::new());
    let va = value_area_of(&base, &s);

    let today = today_series(
        va.high + dec_centi(30),
        va.high + dec_centi(20),
        va.high - dec_centi(6),
        2,
        va.high - dec_centi(10),
        va.high - dec_centi(2),
    );
    let candles = series(dec_centi(9950), dec_centi(10050), today);

    match s.analyze_candles("IWM", &candles) {
        SignalResult::Signal(sig) => {
            assert_eq!(sig.direction, Direction::Short);
            assert_eq!(sig.target_price, Some(va.low));
            assert!(sig.stop_price.unwrap() > va.high);
        }
        other => panic!("esperava sinal short, veio {other:?}"),
    }
}

// --- 3 --------------------------------------------------------------------

#[test]
fn abertura_dentro_do_valor_rejeita() {
    let s = strategy();
    let base = series(dec_centi(9950), dec_centi(10050), Vec::new());
    let va = value_area_of(&base, &s);
    let mid = (va.low + va.high) / Decimal::from(2);

    let today = today_series(mid, mid, mid, 2, mid - dec_centi(5), mid + dec_centi(5));
    let candles = series(dec_centi(9950), dec_centi(10050), today);

    assert_eq!(
        reason_of(&s.analyze_candles("IWM", &candles)),
        RejectionReason::OpenInsideValueArea
    );
}

// --- 4 --------------------------------------------------------------------

#[test]
fn apenas_um_fechamento_dentro_nao_e_aceitacao() {
    let s = strategy();
    let base = series(dec_centi(9950), dec_centi(10050), Vec::new());
    let va = value_area_of(&base, &s);

    let today = today_series(
        va.low - dec_centi(30),
        va.low - dec_centi(20),
        va.low + dec_centi(6),
        1, // só uma barra dentro
        va.low + dec_centi(2),
        va.low + dec_centi(10),
    );
    let candles = series(dec_centi(9950), dec_centi(10050), today);

    assert_eq!(
        reason_of(&s.analyze_candles("IWM", &candles)),
        RejectionReason::NoValueAreaReentry
    );
}

// --- 5 --------------------------------------------------------------------

#[test]
fn area_de_valor_larga_demais_rejeita() {
    let s = strategy();
    // Dia anterior com range 3,00 → VA muito maior que 0,8 x ATR diário (~1,1).
    let prev_low = dec_centi(9850);
    let prev_high = dec_centi(10150);
    let base = series(prev_low, prev_high, Vec::new());
    let va = value_area_of(&base, &s);

    let today = today_series(
        va.low - dec_centi(30),
        va.low - dec_centi(20),
        va.low + dec_centi(6),
        2,
        va.low + dec_centi(2),
        va.low + dec_centi(10),
    );
    let candles = series(prev_low, prev_high, today);

    assert_eq!(
        reason_of(&s.analyze_candles("IWM", &candles)),
        RejectionReason::ValueAreaTooWide
    );
}

// --- 6 --------------------------------------------------------------------

#[test]
fn abertura_longe_demais_do_valor_rejeita() {
    let s = strategy();
    let base = series(dec_centi(9950), dec_centi(10050), Vec::new());
    let va = value_area_of(&base, &s);

    // ATR diário ~1,00 → limite 1,00. Abre 1,50 abaixo da borda.
    let today = today_series(
        va.low - dec_centi(150),
        va.low - dec_centi(140),
        va.low + dec_centi(6),
        2,
        va.low + dec_centi(2),
        va.low + dec_centi(10),
    );
    let candles = series(dec_centi(9950), dec_centi(10050), today);

    assert_eq!(
        reason_of(&s.analyze_candles("IWM", &candles)),
        RejectionReason::OpenTooFarFromValue
    );
}

// --- 7 --------------------------------------------------------------------

#[test]
fn travessia_contra_a_tendencia_rejeita() {
    let s = strategy();
    let base = series(dec_centi(9950), dec_centi(10050), Vec::new());
    let va = value_area_of(&base, &s);

    // Hoje cai forte antes da reentrada → EMA20 inclinada para baixo, o que
    // veta a travessia ascendente (filtro de direção do Cap. 4).
    let base_ts = today_base();
    let mut today = Vec::new();
    for i in 0..10 {
        let px = va.low - dec_centi(20) - Decimal::from(i) * dec_centi(40);
        today.push(candle(
            base_ts + Duration::minutes(i * 15),
            px + dec_centi(10),
            px + dec_centi(15),
            px - dec_centi(5),
            px,
        ));
    }
    // Reentrada no valor (2 fechamentos dentro).
    for i in 0..2 {
        let ts = base_ts + Duration::minutes((10 + i) * 15);
        today.push(candle(
            ts,
            va.low + dec_centi(6),
            va.low + dec_centi(10),
            va.low + dec_centi(2),
            va.low + dec_centi(6),
        ));
    }
    let candles = series(dec_centi(9950), dec_centi(10050), today);

    let result = s.analyze_candles("IWM", &candles);
    let reason = reason_of(&result);
    assert!(
        reason == RejectionReason::TrendAgainstTraversal
            || reason == RejectionReason::OpenTooFarFromValue,
        "esperava veto de tendência (ou de distância, se a queda afastou a abertura), veio {reason:?}"
    );
}

// --- 8 --------------------------------------------------------------------

#[test]
fn risco_retorno_insuficiente_rejeita() {
    let s = strategy();
    let base = series(dec_centi(9950), dec_centi(10050), Vec::new());
    let va = value_area_of(&base, &s);

    // Barra de sinal quase na borda oposta: sobra travessia curta para um
    // stop que continua na borda de entrada → RR abaixo de 1,2.
    let today = today_series(
        va.low - dec_centi(30),
        va.low - dec_centi(20),
        va.high - dec_centi(4),
        2,
        va.high - dec_centi(8),
        va.high - dec_centi(2),
    );
    let candles = series(dec_centi(9950), dec_centi(10050), today);

    assert_eq!(
        reason_of(&s.analyze_candles("IWM", &candles)),
        RejectionReason::PoorRiskReward
    );
}

// --- 9 --------------------------------------------------------------------

#[test]
fn serie_sem_dia_anterior_completo_rejeita() {
    let s = strategy();
    // Só o dia de hoje, sem histórico: sem VA e sem ATR diário.
    let today = today_series(
        dec_centi(9900),
        dec_centi(9910),
        dec_centi(10000),
        2,
        dec_centi(9995),
        dec_centi(10005),
    );
    let mut candles = uniform_day(2026, 7, 22, dec_centi(9950), dec_centi(10050));
    candles.extend(today);

    assert_eq!(
        reason_of(&s.analyze_candles("IWM", &candles)),
        RejectionReason::IncompleteSetup
    );
}

// --- 10 -------------------------------------------------------------------

#[test]
fn fora_da_janela_de_horario_rejeita() {
    let s = strategy();
    let base = series(dec_centi(9950), dec_centi(10050), Vec::new());
    let va = value_area_of(&base, &s);

    // Mesma configuração do caso 1, mas a barra de sinal cai às 13:45 UTC
    // (09:45 ET), antes da janela 14:00–19:00.
    let base_ts = today_base();
    let today = vec![
        candle(
            base_ts,
            va.low - dec_centi(30),
            va.low - dec_centi(18),
            va.low - dec_centi(32),
            va.low - dec_centi(20),
        ),
        candle(
            base_ts + Duration::minutes(15),
            va.low + dec_centi(6),
            va.low + dec_centi(10),
            va.low + dec_centi(2),
            va.low + dec_centi(6),
        ),
    ];
    let candles = series(dec_centi(9950), dec_centi(10050), today);

    assert_eq!(
        reason_of(&s.analyze_candles("IWM", &candles)),
        RejectionReason::OutsideTradingHours
    );
}

// --- 11 -------------------------------------------------------------------

#[test]
fn value_area_segue_o_algoritmo_do_apendice_1() {
    // Distribuição conhecida: uma barra cobrindo 100,00–101,00 e duas
    // concentradas em 100,40–100,60. Faixas de 0,10 (range 1,00 / 10).
    let base = Utc.with_ymd_and_hms(2026, 7, 21, 13, 30, 0).unwrap();
    let day = vec![
        candle(
            base,
            dec_centi(10000),
            dec_centi(10100),
            dec_centi(10000),
            dec_centi(10050),
        ),
        candle(
            base + Duration::minutes(15),
            dec_centi(10040),
            dec_centi(10060),
            dec_centi(10040),
            dec_centi(10050),
        ),
        candle(
            base + Duration::minutes(30),
            dec_centi(10040),
            dec_centi(10060),
            dec_centi(10040),
            dec_centi(10050),
        ),
    ];

    let va = compute_value_area(&day, 10, Decimal::from(70), dec_centi(1)).expect("VA");

    // POC na faixa mais densa, no centro do perfil.
    assert_eq!(va.poc, dec_centi(10055));
    assert_eq!(va.low, dec_centi(10030));
    assert_eq!(va.high, dec_centi(10100));
    assert!(va.width() > Decimal::ZERO);
    assert!(va.contains(dec_centi(10050)));
    assert!(!va.contains(dec_centi(10010)));
}
