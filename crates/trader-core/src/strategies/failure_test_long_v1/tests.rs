//! Testes unitários com candles sintéticos — plano da seção 15 do documento
//! de especificação (`docs/strategies/failure-test-long-v1.md`).
//!
//! O fixture base ("setup perfeito") constrói: downtrend estendido, suporte
//! S=100,02 com 2 toques (pivôs em 100,02 e 99,98) e barra de sonda com
//! recuperação (low 99,45, close 100,30). ATR ≈ 0,64 ponto.

use super::*;
use crate::risk::{RiskConfig, RiskManager, RiskState};
use crate::strategies::failure_test_long_v1::config::FailureTestLongV1Config;
use chrono::{DateTime, TimeZone, Utc};
use rust_decimal::Decimal;
use trader_domain::{
    Direction, MarketPhase, RejectionReason, SignalResult, TimeFrame, TrendState, VolatilityRegime,
};

/// Base de timestamps dentro da janela operacional (13:45–19:30 UTC).
fn base_ts() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 8, 3, 14, 0, 0).unwrap()
}

fn dec(s: &str) -> Decimal {
    s.parse().unwrap()
}

/// Preço a partir de centésimos inteiros (evita f64 em dinheiro).
fn cents(v: i64) -> Decimal {
    Decimal::new(v, 2)
}

fn candle(
    base: DateTime<Utc>,
    idx: usize,
    o: Decimal,
    h: Decimal,
    l: Decimal,
    c: Decimal,
) -> Candle {
    Candle::new(
        "SPY",
        TimeFrame::M15,
        base + chrono::Duration::minutes(idx as i64 * 5),
        o,
        h,
        l,
        c,
        Decimal::from(1000),
    )
    .expect("candle válido")
}

fn push(
    bars: &mut Vec<(Decimal, Decimal, Decimal, Decimal)>,
    o: Decimal,
    h: Decimal,
    l: Decimal,
    c: Decimal,
) {
    bars.push((o, h, l, c));
}

/// Barras (OHLC) do setup perfeito — CASO 1 do plano de testes.
fn perfect_bars() -> Vec<(Decimal, Decimal, Decimal, Decimal)> {
    let mut bars = Vec::new();

    // Segmento A (0..8): primeira perna de baixa, 108,00 → 100,65.
    for i in 0..8i64 {
        let c = cents(10800 - 105 * i);
        push(&mut bars, c + cents(50), c + cents(60), c - cents(50), c);
    }

    // Toque 1 (8): pivô em 100,02.
    push(
        &mut bars,
        cents(10060),
        cents(10070),
        cents(10002),
        cents(10050),
    );

    // Segmento C (9..15): repique até 103,30.
    for c in [10120, 10190, 10250, 10300, 10330, 10320] {
        let c = cents(c);
        push(&mut bars, c - cents(40), c + cents(20), c - cents(60), c);
    }

    // Segmento D (15..22): segunda perna de baixa até 100,60.
    for c in [10240, 10180, 10130, 10100, 10080, 10070, 10060] {
        let c = cents(c);
        push(&mut bars, c + cents(25), c + cents(30), c - cents(50), c);
    }

    // Toque 2 (22): pivô em 99,98.
    push(
        &mut bars,
        cents(10060),
        cents(10075),
        cents(9998),
        cents(10055),
    );

    // Segmento F (23..30): repique até 102,50.
    for c in [10110, 10160, 10200, 10230, 10245, 10250, 10240] {
        let c = cents(c);
        push(&mut bars, c - cents(40), c + cents(20), c - cents(60), c);
    }

    // Segmento G (30..45): perna final de baixa, 102,18 → 100,43
    // (mínimas sempre acima de S — a sonda é só da barra de sinal).
    for (k, drop) in [
        12, 25, 37, 50, 62, 75, 87, 100, 112, 125, 137, 150, 162, 175, 187,
    ]
    .iter()
    .enumerate()
    {
        let _ = k;
        let c = cents(10230 - drop);
        push(&mut bars, c + cents(15), c + cents(30), c - cents(30), c);
    }

    // Sonda + recuperação na mesma barra (45): low 99,45 (0,57 abaixo de S),
    // close 100,30 (> S, terço superior do range).
    push(
        &mut bars,
        cents(10050),
        cents(10050),
        cents(9945),
        cents(10030),
    );

    bars
}

fn candles_from(base: DateTime<Utc>, bars: &[(Decimal, Decimal, Decimal, Decimal)]) -> Vec<Candle> {
    bars.iter()
        .enumerate()
        .map(|(i, &(o, h, l, c))| candle(base, i, o, h, l, c))
        .collect()
}

/// CASO 1 — fixture do setup perfeito.
fn perfect_setup() -> Vec<Candle> {
    candles_from(base_ts(), &perfect_bars())
}

fn strategy() -> FailureTestLongV1 {
    FailureTestLongV1::new(FailureTestLongV1Config::default())
}

fn expect_rejection(candles: &[Candle], expected: RejectionReason) {
    match strategy().analyze_candles("SPY", candles) {
        SignalResult::Rejected { reason, details } => assert_eq!(
            reason, expected,
            "esperado {:?}, obtido {:?} ({:?})",
            expected, reason, details
        ),
        other => panic!("esperado rejeição {:?}, obtido {:?}", expected, other),
    }
}

// --- CASO 1: setup perfeito gera sinal ---
#[test]
fn case1_perfect_setup_generates_signal() {
    let candles = perfect_setup();
    let atr = crate::indicators::atr(&candles, 14).unwrap();

    match strategy().analyze_candles("SPY", &candles) {
        SignalResult::Signal(signal) => {
            assert_eq!(signal.direction, Direction::Long);
            assert_eq!(signal.entry_order_type, trader_domain::EntryOrderType::Stop);

            // Entrada: máxima da barra de recuperação + 1 tick.
            let expected_entry = dec("100.51");
            assert_eq!(signal.entry_price, Some(expected_entry));

            // Stop: extremo da sonda (99,45) − 1 tick − jitter (ponto médio
            // determinístico de [0, 0.10 × ATR]).
            let jitter = dec("0.10") * atr / Decimal::from(2);
            let expected_stop = dec("99.45") - dec("0.01") - jitter;
            assert_eq!(signal.stop_price, Some(expected_stop));

            // Alvo: 1,5R.
            let risk = expected_entry - expected_stop;
            assert_eq!(
                signal.target_price,
                Some(expected_entry + dec("1.5") * risk)
            );
            assert_eq!(signal.risk_reward_ratio, Some(dec("1.5")));

            // Metadados auditáveis (seção 10 do doc).
            let snapshot = &signal.market_snapshot;
            assert!(snapshot.get("support_level").is_some());
            assert!(snapshot.get("level_touches").is_some());
            assert!(snapshot.get("probe_low").is_some());
            assert!(snapshot.get("probe_depth_atr").is_some());
            assert!(snapshot.get("keltner_lower").is_some());
            assert!(snapshot.get("macd_fast").is_some());
            assert!(snapshot.get("overextension").is_some());
            assert!(signal.entry_reason.is_some());
        }
        SignalResult::Rejected { reason, details } => {
            panic!("esperado sinal, rejeitado por {:?}: {:?}", reason, details)
        }
        _ => panic!("esperado sinal"),
    }
}

// --- CASO 2: sonda sem recuperação ---
#[test]
fn case2_probe_without_recovery_rejects() {
    let mut bars = perfect_bars();
    // Sonda fecha ABAIXO de S (99,95 < 100,02, mas ainda acima de S−tol).
    bars[45] = (cents(10040), cents(10045), cents(9940), cents(9995));
    let candles = candles_from(base_ts(), &bars);
    expect_rejection(&candles, RejectionReason::NoRecoveryClose);

    // Segunda barra também fecha abaixo: ainda dentro do limite de barras.
    let mut bars2 = bars.clone();
    bars2.push((cents(9995), cents(10000), cents(9950), cents(9970)));
    let candles2 = candles_from(base_ts(), &bars2);
    expect_rejection(&candles2, RejectionReason::NoRecoveryClose);

    // Terceira barra abaixo: sonda excedeu probe_max_bars (2).
    let mut bars3 = bars2;
    bars3.push((cents(9970), cents(9975), cents(9930), cents(9950)));
    let candles3 = candles_from(base_ts(), &bars3);
    expect_rejection(&candles3, RejectionReason::ProbeTooLong);
}

// --- CASO 3: suporte fraco / já rompido ---
#[test]
fn case3_weak_support_rejects() {
    let mut bars = perfect_bars();
    // Remove o primeiro toque: barra 8 vira um flush profundo isolado
    // (pivô em 99,50 com 1 toque só, fora da zona de toque do pivô em 99,98).
    bars[8] = (cents(10060), cents(10070), cents(9950), cents(10050));
    let candles = candles_from(base_ts(), &bars);
    expect_rejection(&candles, RejectionReason::SupportNotTestedEnough);
}

#[test]
fn case3_variant_broken_support_rejects() {
    let mut bars = perfect_bars();
    // Fechamento prévio decisivo abaixo do nível (99,85 < 100,02 − 0,10).
    bars[30] = (cents(10040), cents(10045), cents(9980), cents(9985));
    let candles = candles_from(base_ts(), &bars);
    expect_rejection(&candles, RejectionReason::SupportAlreadyBroken);
}

// --- CASO 4: sem sobreextensão ---
#[test]
fn case4_not_overextended_rejects() {
    let mut bars = Vec::new();
    // Mercado lateral dentro do canal de Keltner: 45 barras em torno de 100,35.
    for _ in 0..45 {
        push(
            &mut bars,
            cents(10030),
            cents(10060),
            cents(10005),
            cents(10035),
        );
    }
    // Dois pivôs formando o nível (toques válidos).
    bars[8] = (cents(10030), cents(10050), cents(9998), cents(10030));
    bars[22] = (cents(10030), cents(10050), cents(9999), cents(10030));
    // Flush antigo: garante que a sonda NÃO é nova mínima da janela de 40
    // barras (sem divergência de MACD possível).
    bars[26] = (cents(10030), cents(10060), cents(9950), cents(10040));
    // Sonda rasa com recuperação perfeita.
    bars[45 - 1] = (cents(10030), cents(10060), cents(10005), cents(10035));
    bars.push((cents(10030), cents(10045), cents(9960), cents(10035)));

    let candles = candles_from(base_ts(), &bars);
    expect_rejection(&candles, RejectionReason::NotOverextended);
}

// --- CASO 5: sonda profunda demais ---
#[test]
fn case5_probe_too_deep_rejects() {
    let mut bars = perfect_bars();
    // Low 99,00: profundidade 1,02 > 1,0 × ATR (~0,67), mas range da barra
    // (1,50) abaixo do limiar de clímax (2,5 × ATR) — isola a regra de
    // profundidade.
    bars[45] = (cents(10045), cents(10050), cents(9900), cents(10030));
    let candles = candles_from(base_ts(), &bars);
    expect_rejection(&candles, RejectionReason::ProbeTooDeep);
}

// --- CASO 6: stop no ruído / RR ruim ---
#[test]
fn case6_stop_within_noise_rejects() {
    let mut bars = perfect_bars();
    // Sonda rasa demais (low 99,95): stop ficaria a ~0,50 da entrada, menos
    // que 1× o range médio de barra (~0,66).
    bars[45] = (cents(10030), cents(10040), cents(9995), cents(10032));
    let candles = candles_from(base_ts(), &bars);
    expect_rejection(&candles, RejectionReason::StopWithinNoise);
}

#[test]
fn case6_variant_poor_risk_reward_rejects() {
    let candles = perfect_setup();
    let mut config = FailureTestLongV1Config::default();
    // RR efetivo é target_r_multiple (1,5): exigir 2,0 força a rejeição.
    config.strategy.parameters.min_risk_reward = Decimal::from(2);
    let strategy = FailureTestLongV1::new(config);
    match strategy.analyze_candles("SPY", &candles) {
        SignalResult::Rejected { reason, .. } => {
            assert_eq!(reason, RejectionReason::PoorRiskReward)
        }
        other => panic!("esperado PoorRiskReward, obtido {:?}", other),
    }
}

// --- CASO 7: fora de horário ---
#[test]
fn case7_outside_trading_hours_rejects() {
    // Série terminando 13:30 UTC (antes da janela 13:45–19:30 UTC, embora
    // dentro da fase "regular" ampla do analyzer).
    let early = Utc.with_ymd_and_hms(2026, 8, 3, 9, 45, 0).unwrap();
    let candles = candles_from(early, &perfect_bars());
    let last_ts = candles.last().unwrap().timestamp;
    assert_eq!(last_ts.time().to_string(), "13:30:00");
    expect_rejection(&candles, RejectionReason::OutsideTradingHours);

    // Série terminando 20:15 UTC (depois do fim da janela).
    let late = Utc.with_ymd_and_hms(2026, 8, 3, 16, 30, 0).unwrap();
    let candles = candles_from(late, &perfect_bars());
    let last_ts = candles.last().unwrap().timestamp;
    assert_eq!(last_ts.time().to_string(), "20:15:00");
    expect_rejection(&candles, RejectionReason::OutsideTradingHours);
}

// --- CASO 8: clímax de venda em andamento ---
#[test]
fn case8_climax_in_progress_rejects() {
    let mut bars = Vec::new();

    // Deriva lenta de baixa (MACD rápido raso).
    for i in 0..8i64 {
        let c = cents(10150 - 8 * i);
        push(&mut bars, c + cents(10), c + cents(15), c - cents(15), c);
    }
    // Mergulho 1 (8): pivô em 100,02.
    push(
        &mut bars,
        cents(10090),
        cents(10095),
        cents(10002),
        cents(10060),
    );
    // Deriva lateral (9..22).
    for c in [
        10100, 10105, 10110, 10112, 10110, 10105, 10100, 10095, 10090, 10085, 10080, 10075, 10070,
    ] {
        let c = cents(c);
        push(&mut bars, c + cents(10), c + cents(15), c - cents(15), c);
    }
    // Mergulho 2 (22): pivô em 99,98.
    push(
        &mut bars,
        cents(10070),
        cents(10075),
        cents(9998),
        cents(10045),
    );
    // Lateral (23..42).
    for c in [
        10090, 10095, 10100, 10100, 10095, 10090, 10085, 10080, 10075, 10070, 10065, 10070, 10072,
        10070, 10068, 10065, 10062, 10060, 10058,
    ] {
        let c = cents(c);
        push(&mut bars, c + cents(10), c + cents(15), c - cents(15), c);
    }
    // Aceleração de baixa (42..45): MACD rápido faz nova mínima extrema.
    push(
        &mut bars,
        cents(10055),
        cents(10060),
        cents(10030),
        cents(10040),
    );
    push(
        &mut bars,
        cents(10040),
        cents(10045),
        cents(10000),
        cents(10010),
    );
    push(
        &mut bars,
        cents(10010),
        cents(10015),
        cents(9985),
        cents(9990),
    );
    // Barra de clímax (45): range 1,65 > 2,5 × ATR (~0,4) e MACD em mínima.
    push(
        &mut bars,
        cents(9990),
        cents(9995),
        cents(9830),
        cents(9950),
    );

    let candles = candles_from(base_ts(), &bars);
    expect_rejection(&candles, RejectionReason::ClimaxInProgress);
}

// --- CASO 9: extremo na barra anterior ---
#[test]
fn case9_stop_below_previous_bar_extreme() {
    let mut bars = perfect_bars();
    // Sonda na barra 45 SEM recuperação (extremo 99,40 aqui)...
    bars[45] = (cents(10040), cents(10045), cents(9940), cents(9995));
    // ...recuperação só na barra 46, com mínima mais alta (99,70).
    bars.push((cents(9995), cents(10055), cents(9970), cents(10040)));

    let candles = candles_from(base_ts(), &bars);
    let atr = crate::indicators::atr(&candles, 14).unwrap();

    match strategy().analyze_candles("SPY", &candles) {
        SignalResult::Signal(signal) => {
            // Entrada: máxima da barra de recuperação (100,55) + 1 tick.
            assert_eq!(signal.entry_price, Some(dec("100.56")));
            // Stop abaixo da mínima da barra 45 (extremo da excursão), NÃO da
            // barra de recuperação — literal do livro (exemplo EURUSD, Cap. 6).
            let jitter = dec("0.10") * atr / Decimal::from(2);
            let expected_stop = dec("99.40") - dec("0.01") - jitter;
            assert_eq!(signal.stop_price, Some(expected_stop));
            assert!(signal.stop_price.unwrap() < dec("99.70") - dec("0.01") - jitter);
        }
        SignalResult::Rejected { reason, details } => {
            panic!("esperado sinal, rejeitado por {:?}: {:?}", reason, details)
        }
        _ => panic!("esperado sinal"),
    }
}

// --- CASO 10: validação em 3 barras (saída por tempo) ---
// A lógica vive em `execution::time_exit` (testes lá); aqui garantimos que a
// estratégia expõe a configuração habilitada com os defaults do doc.
#[test]
fn case10_time_exit_config_enabled_with_doc_defaults() {
    let strategy = strategy();
    let config = strategy.time_exit().expect("saída por tempo habilitada");
    assert!(config.enabled);
    assert_eq!(config.candles, 3);
    assert_eq!(config.min_r, dec("0.5"));
}

// --- CASO 11: limites de risco do processo ---
fn risk_ctx(timestamp: DateTime<Utc>) -> MarketContext {
    MarketContext {
        symbol: "SPY".to_string(),
        timeframe: TimeFrame::M15,
        timestamp,
        candle_timestamp: Some(timestamp),
        trend_state: TrendState::Downtrend,
        volatility_regime: VolatilityRegime::Normal,
        market_phase: MarketPhase::Regular,
        ema_20: Some(dec("101")),
        ema_50: None,
        sma_200: None,
        atr_14: Some(dec("0.6")),
        atr_percent_14: Some(dec("0.6")),
        volume_relative: None,
        hh_hl_count: None,
        lh_ll_count: None,
        range_percent: None,
        is_tradeable: true,
        raw_values: serde_json::Value::Object(Default::default()),
    }
}

fn risk_signal() -> trader_domain::Signal {
    trader_domain::Signal {
        symbol: "SPY".to_string(),
        strategy_id: "failure-test-long-v1".to_string(),
        strategy_version: "1.0.0".to_string(),
        config_hash: "abc".to_string(),
        timeframe: TimeFrame::M15,
        timestamp: Utc::now(),
        direction: Direction::Long,
        status: trader_domain::SignalStatus::Accepted,
        entry_order_type: trader_domain::EntryOrderType::Stop,
        entry_price: Some(dec("100.51")),
        stop_price: Some(dec("99.41")),
        target_price: Some(dec("102.16")),
        risk_reward_ratio: Some(dec("1.5")),
        risk_amount: None,
        risk_percent: None,
        position_size: None,
        entry_reason: None,
        rejection_reason: None,
        rejection_details: None,
        market_snapshot: serde_json::Value::Object(Default::default()),
        correlation_id: "corr".to_string(),
    }
}

/// RiskConfig como o CLI monta para esta estratégia: 0,5% por trade
/// (override da estratégia), demais limites do `[risk]` global.
fn risk_config() -> RiskConfig {
    RiskConfig {
        trading_mode: trader_domain::TradingMode::Paper,
        risk_per_trade_pct: dec("0.5"),
        max_daily_loss_pct: Decimal::from(2),
        max_trades_per_day: 3,
        max_consecutive_losses: 3,
        min_risk_reward: dec("1.2"),
        max_spread_pct: dec("0.05"),
        max_atr_pct: dec("1.5"),
        trading_start_time_utc: (13, 45, 0),
        trading_end_time_utc: (19, 30, 0),
        entry_overshoot_tolerance: dec("0.25"),
    }
}

fn within_hours() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 8, 3, 15, 0, 0).unwrap()
}

#[test]
fn case11_position_size_uses_half_percent_risk() {
    let manager = RiskManager::new(risk_config());
    let ctx = risk_ctx(within_hours());
    let signal = risk_signal();

    match manager.validate(
        &signal,
        &ctx,
        None,
        &RiskState::default(),
        Decimal::from(100_000),
    ) {
        crate::risk::RiskCheck::Approved {
            position_size,
            risk_amount,
        } => {
            // Orçamento: 0,5% de 100k = $500; stop a 1,10 → 454 ações
            // (truncado). O risk_amount gravado é o risco REAL da posição
            // (1,10 × 454 = $499,40), não o orçamento — ver fix de 2026-08-29
            // (o orçamento como denominador comprimia o R do gate ADR-010).
            assert_eq!(position_size, Decimal::from(454));
            assert_eq!(risk_amount, Decimal::new(49940, 2)); // 499.40
        }
        other => panic!("esperado aprovado, obtido {:?}", other),
    }
}

#[test]
fn case11_daily_loss_limit_rejects() {
    let manager = RiskManager::new(risk_config());
    let ctx = risk_ctx(within_hours());
    let signal = risk_signal();
    let state = RiskState {
        daily_pnl: Decimal::from(-2100),
        ..RiskState::default()
    };

    match manager.validate(&signal, &ctx, None, &state, Decimal::from(100_000)) {
        crate::risk::RiskCheck::Rejected(RejectionReason::DailyLossLimitReached, _) => {}
        other => panic!("esperado DailyLossLimitReached, obtido {:?}", other),
    }
}

#[test]
fn case11_max_trades_rejects() {
    let manager = RiskManager::new(risk_config());
    let ctx = risk_ctx(within_hours());
    let signal = risk_signal();
    let state = RiskState {
        daily_trades: 3,
        ..RiskState::default()
    };

    match manager.validate(&signal, &ctx, None, &state, Decimal::from(100_000)) {
        crate::risk::RiskCheck::Rejected(RejectionReason::MaxTradesReached, _) => {}
        other => panic!("esperado MaxTradesReached, obtido {:?}", other),
    }
}

#[test]
fn case11_consecutive_losses_rejects() {
    let manager = RiskManager::new(risk_config());
    let ctx = risk_ctx(within_hours());
    let signal = risk_signal();
    let state = RiskState {
        consecutive_losses: 3,
        ..RiskState::default()
    };

    match manager.validate(&signal, &ctx, None, &state, Decimal::from(100_000)) {
        crate::risk::RiskCheck::Rejected(RejectionReason::ConsecutiveLosses, _) => {}
        other => panic!("esperado ConsecutiveLosses, obtido {:?}", other),
    }
}

#[test]
fn empty_series_rejects() {
    match strategy().analyze_candles("SPY", &[]) {
        SignalResult::Rejected { reason, .. } => {
            assert_eq!(reason, RejectionReason::IncompleteSetup)
        }
        other => panic!("esperado IncompleteSetup, obtido {:?}", other),
    }
}
