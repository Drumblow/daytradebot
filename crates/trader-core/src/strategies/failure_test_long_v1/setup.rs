//! Detecção do nível de suporte e da sonda (failure test) — seções 5.1–5.3
//! do documento de especificação.
//!
//! Nível S: pivô de mínima com pelo menos `level_min_touches` toques na
//! janela de lookback, tolerância de `level_touch_tolerance_pct`, nenhum
//! fechamento prévio abaixo e idade mínima de `level_min_age_candles`.
//!
//! Sonda: penetração abaixo de S em no máximo `probe_max_bars` barras e
//! profundidade de até `probe_max_atr_mult` × ATR, seguida de fechamento de
//! volta acima de S na mesma barra ou na seguinte.

use rust_decimal::Decimal;
use serde_json::json;
use tracing::debug;

use crate::strategies::failure_test_long_v1::config::StrategyParameters;
use crate::strategies::failure_test_long_v1::context::ContextData;
use crate::strategies::failure_test_long_v1::entry;
use trader_domain::{Candle, RejectionReason};

/// Nível de suporte validado.
#[derive(Debug, Clone, PartialEq)]
pub struct SupportLevel {
    /// Preço do nível (mínima do pivô de referência).
    pub price: Decimal,
    /// Quantidade de toques dentro da tolerância na janela de lookback.
    pub touches: usize,
    /// Índice do primeiro toque (para a regra de idade do nível).
    pub first_touch_index: usize,
}

/// A sonda abaixo do suporte e a recuperação.
#[derive(Debug, Clone, PartialEq)]
pub struct Probe {
    /// Índice da primeira barra da sonda.
    pub start_index: usize,
    /// Índice da barra de recuperação (sempre o último candle da série).
    pub recovery_index: usize,
    /// Extremo da excursão E = mínima das barras da sonda (inclui a barra
    /// anterior à entrada quando ela marcou o extremo — exemplo EURUSD do
    /// Cap. 6).
    pub extreme: Decimal,
    /// Profundidade da sonda (S − E).
    pub depth: Decimal,
    /// Barras da sonda.
    pub bars: usize,
}

/// Descrição de um setup válido encontrado.
#[derive(Debug, Clone)]
pub struct Setup {
    /// Índice da barra de sinal (recuperação) no vetor de candles.
    pub signal_index: usize,
    pub level: SupportLevel,
    pub probe: Probe,
    /// Preço de entrada (buy stop acima da máxima da barra de recuperação).
    pub entry_price: Decimal,
    /// Stop inicial (abaixo do extremo da excursão, com jitter).
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

/// Tenta detectar um failure test de compra no último candle da série.
pub fn detect_setup(
    candles: &[Candle],
    context: &ContextData,
    params: &StrategyParameters,
) -> SetupResult {
    let min_candles = params.level_min_age_candles + params.probe_max_bars + 3;
    if candles.len() < min_candles {
        return SetupResult::NotFound(
            RejectionReason::IncompleteSetup,
            json!({ "reason": "not enough candles", "candles": candles.len(), "min": min_candles }),
        );
    }

    let level = match find_support_level(candles, params) {
        Ok(level) => level,
        Err((reason, details)) => return SetupResult::NotFound(reason, details),
    };

    let probe = match detect_probe(candles, &level, context.atr, params) {
        Ok(probe) => probe,
        Err((reason, details)) => return SetupResult::NotFound(reason, details),
    };

    match entry::evaluate_prices(candles, &level, &probe, context.atr, params) {
        Ok(prices) => SetupResult::Found(Setup {
            signal_index: probe.recovery_index,
            level,
            probe,
            entry_price: prices.entry_price,
            stop_price: prices.stop_price,
            target_price: prices.target_price,
        }),
        Err((reason, details)) => SetupResult::NotFound(reason, details),
    }
}

/// Localiza o nível de suporte S na janela de lookback (seção 5.1 do doc).
///
/// Os últimos `probe_max_bars + 1` candles ficam fora da janela: são a zona
/// da sonda atual, que não pode votar no próprio nível.
fn find_support_level(
    candles: &[Candle],
    params: &StrategyParameters,
) -> Result<SupportLevel, (RejectionReason, serde_json::Value)> {
    let len = candles.len();
    let zone_end = len - (params.probe_max_bars + 1);
    let start = zone_end.saturating_sub(params.level_lookback_candles);

    // Pivôs de mínima com 2 barras de cada lado (a confirmação pode entrar
    // na zona da sonda — barras abaixo do nível nunca confirmam pivô nela).
    let mut pivots: Vec<(usize, Decimal)> = Vec::new();
    for i in (start + 2)..zone_end {
        if is_swing_low(candles, i) {
            pivots.push((i, candles[i].low));
        }
    }

    if pivots.is_empty() {
        return Err((
            RejectionReason::SupportLevelNotFound,
            json!({ "reason": "nenhum pivô de mínima na janela de lookback" }),
        ));
    }

    let candidates: Vec<SupportLevel> = pivots
        .iter()
        .map(|(_, price)| build_level(candles, start, zone_end, *price, params))
        .collect();

    // Filtros em cascata: o primeiro que elimina todos os candidatos define
    // o motivo da rejeição.
    let tested: Vec<_> = candidates
        .iter()
        .filter(|l| l.touches >= params.level_min_touches)
        .cloned()
        .collect();
    if tested.is_empty() {
        return Err((
            RejectionReason::SupportNotTestedEnough,
            json!({
                "reason": "nível com toques insuficientes",
                "min_touches": params.level_min_touches,
                "candidates": candidates.iter().map(|l| json!({ "price": l.price, "touches": l.touches })).collect::<Vec<_>>(),
            }),
        ));
    }

    let intact: Vec<_> = tested
        .into_iter()
        .filter(|l| !level_broken(candles, start, zone_end, l.price, params))
        .collect();
    if intact.is_empty() {
        return Err((
            RejectionReason::SupportAlreadyBroken,
            json!({ "reason": "fechamento prévio abaixo do nível (suporte já rompido)" }),
        ));
    }

    let aged: Vec<_> = intact
        .into_iter()
        .filter(|l| (len - 1) - l.first_touch_index >= params.level_min_age_candles)
        .collect();
    if aged.is_empty() {
        return Err((
            RejectionReason::LevelTooRecent,
            json!({ "reason": "nível formado há menos candles que a idade mínima", "min_age": params.level_min_age_candles }),
        ));
    }

    // Mais testado vence; em empate, o mais alto (mais próximo do preço).
    let best = aged
        .into_iter()
        .max_by(|a, b| {
            a.touches
                .cmp(&b.touches)
                .then_with(|| a.price.cmp(&b.price))
        })
        .expect("lista de níveis não vazia");

    debug!(level = %best.price, touches = best.touches, "nível de suporte validado");
    Ok(best)
}

/// Pivô de mínima: mínima não maior que as 2 vizinhas de cada lado e
/// estritamente menor que pelo menos uma de cada lado.
fn is_swing_low(candles: &[Candle], i: usize) -> bool {
    if i < 2 || i + 2 >= candles.len() {
        return false;
    }
    let low = candles[i].low;
    let left = [candles[i - 1].low, candles[i - 2].low];
    let right = [candles[i + 1].low, candles[i + 2].low];

    low <= left[0]
        && low <= left[1]
        && (low < left[0] || low < left[1])
        && low <= right[0]
        && low <= right[1]
        && (low < right[0] || low < right[1])
}

/// Conta os toques do nível na janela: barras cuja mínima está dentro da
/// tolerância de S.
fn build_level(
    candles: &[Candle],
    start: usize,
    end: usize,
    price: Decimal,
    params: &StrategyParameters,
) -> SupportLevel {
    let tolerance = price * params.level_touch_tolerance_pct;
    let mut touches = 0usize;
    let mut first_touch_index = end;
    for (i, candle) in candles.iter().enumerate().take(end).skip(start) {
        if (candle.low - price).abs() <= tolerance {
            touches += 1;
            first_touch_index = first_touch_index.min(i);
        }
    }
    SupportLevel {
        price,
        touches,
        first_touch_index,
    }
}

/// `true` se algum fechamento da janela ficou decisivamente abaixo do nível
/// (o suporte já falhou — não é mais "support holding").
fn level_broken(
    candles: &[Candle],
    start: usize,
    end: usize,
    price: Decimal,
    params: &StrategyParameters,
) -> bool {
    let tolerance = price * params.level_touch_tolerance_pct;
    candles[start..end]
        .iter()
        .any(|c| c.close < price - tolerance)
}

/// Detecta a sonda e a recuperação no fim da série (seções 5.2–5.3 do doc).
fn detect_probe(
    candles: &[Candle],
    level: &SupportLevel,
    atr: Decimal,
    params: &StrategyParameters,
) -> Result<Probe, (RejectionReason, serde_json::Value)> {
    let j = candles.len() - 1;
    let last = &candles[j];
    let s = level.price;

    if last.close > s {
        // Recuperação presente: na mesma barra (spring, low[j] < S) ou na
        // barra seguinte à sonda (sonda terminou em j-1).
        let run_end = if last.low < s { j } else { j - 1 };
        let run_start = probe_run_start(candles, run_end, s);

        let Some(start) = run_start else {
            return Err((
                RejectionReason::NoProbe,
                json!({ "reason": "sem penetração do suporte", "level": s }),
            ));
        };

        // Recuperação na barra seguinte exige que a barra da sonda não tenha
        // se recuperado sozinha (senão o sinal teria saído no candle passado).
        if run_end == j - 1 && candles[j - 1].close > s {
            return Err((
                RejectionReason::NoProbe,
                json!({ "reason": "recuperação já aconteceu antes do candle atual (setup velho)", "level": s }),
            ));
        }

        let bars = run_end - start + 1;
        if bars > params.probe_max_bars {
            return Err((
                RejectionReason::ProbeTooLong,
                json!({ "reason": "sonda excedeu o máximo de barras", "bars": bars, "max": params.probe_max_bars }),
            ));
        }

        return validate_probe(candles, level, atr, params, start, j);
    }

    // Sem recuperação no último candle.
    let Some(start) = probe_run_start(candles, j, s) else {
        return Err((
            RejectionReason::NoProbe,
            json!({ "reason": "sem penetração do suporte", "level": s }),
        ));
    };

    let bars = j - start + 1;
    if bars > params.probe_max_bars {
        return Err((
            RejectionReason::ProbeTooLong,
            json!({ "reason": "sonda sem recuperação excedeu o máximo de barras", "bars": bars, "max": params.probe_max_bars }),
        ));
    }

    Err((
        RejectionReason::NoRecoveryClose,
        json!({ "reason": "sonda sem fechamento de volta acima do suporte", "level": s, "bars": bars }),
    ))
}

/// Início da sequência de barras com mínima abaixo de S terminando em `end`.
fn probe_run_start(candles: &[Candle], end: usize, s: Decimal) -> Option<usize> {
    if candles[end].low >= s {
        return None;
    }
    let mut start = end;
    while start > 0 && candles[start - 1].low < s {
        start -= 1;
    }
    Some(start)
}

/// Valida profundidade da sonda e força da barra de recuperação.
fn validate_probe(
    candles: &[Candle],
    level: &SupportLevel,
    atr: Decimal,
    params: &StrategyParameters,
    start: usize,
    recovery_index: usize,
) -> Result<Probe, (RejectionReason, serde_json::Value)> {
    let s = level.price;
    let extreme = candles[start..=recovery_index]
        .iter()
        .map(|c| c.low)
        .min()
        .expect("sonda não vazia");
    let depth = s - extreme;

    if depth > params.probe_max_atr_mult * atr {
        return Err((
            RejectionReason::ProbeTooDeep,
            json!({
                "reason": "sonda profunda demais (sugere rompimento real)",
                "depth": depth,
                "atr": atr,
                "max_atr_mult": params.probe_max_atr_mult,
            }),
        ));
    }

    let recovery = &candles[recovery_index];
    let range = recovery.range();
    if range.is_zero() {
        return Err((
            RejectionReason::WeakRecoveryBar,
            json!({ "reason": "barra de recuperação sem range" }),
        ));
    }
    let close_position = (recovery.close - recovery.low) / range;
    if close_position < params.signal_close_min_position {
        return Err((
            RejectionReason::WeakRecoveryBar,
            json!({
                "reason": "recuperação fechou abaixo da posição mínima do range",
                "close_position": close_position,
                "min_position": params.signal_close_min_position,
            }),
        ));
    }

    Ok(Probe {
        start_index: start,
        recovery_index,
        extreme,
        depth,
        bars: recovery_index - start + 1,
    })
}
