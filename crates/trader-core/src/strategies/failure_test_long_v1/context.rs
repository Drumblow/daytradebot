//! Regras de contexto de mercado para a estratégia Failure Test Long.
//!
//! O setup só é válido com o mercado "primed for reversal" (seção 4 do doc):
//! pelo menos uma condição de sobreextensão (toque no Keltner inferior, queda
//! ≥ N×ATR do último swing high ou divergência no MACD modificado), sem
//! clímax de venda em andamento e sem volatilidade anormal.

use chrono::Timelike;
use rust_decimal::Decimal;
use serde_json::json;

use crate::indicators::{atr, ema};
use crate::strategies::failure_test_long_v1::config::StrategyParameters;
use trader_domain::{Candle, MarketContext, MarketPhase, RejectionReason};

/// Resultado da avaliação de contexto.
#[derive(Debug, Clone, PartialEq)]
pub enum ContextCheck {
    Approved(Box<ContextData>),
    Rejected(RejectionReason, serde_json::Value),
}

/// Valores calculados na avaliação de contexto, reutilizados no snapshot
/// auditável do sinal.
#[derive(Debug, Clone, PartialEq)]
pub struct ContextData {
    pub ema: Option<Decimal>,
    pub atr: Decimal,
    pub atr_pct: Option<Decimal>,
    pub keltner_lower: Option<Decimal>,
    pub macd_fast: Option<Decimal>,
    /// Qual condição de sobreextensão disparou: "keltner", "drop" ou "divergence".
    pub overextension: &'static str,
}

/// Avalia se o contexto permite buscar um failure test de compra.
pub fn check_context(
    candles: &[Candle],
    ctx: &MarketContext,
    params: &StrategyParameters,
) -> ContextCheck {
    let Some(last) = candles.last() else {
        return ContextCheck::Rejected(
            RejectionReason::IncompleteSetup,
            json!({ "reason": "empty candle series" }),
        );
    };

    // Janela operacional: fase regular + horário configurado da estratégia
    // (09:45–15:30 ET, mais estreito que a fase "regular" ampla do analyzer).
    if !matches!(ctx.market_phase, MarketPhase::Regular) || !within_trading_window(last, params) {
        return ContextCheck::Rejected(
            RejectionReason::OutsideTradingHours,
            json!({
                "reason": "candle fora da janela operacional",
                "phase": format!("{:?}", ctx.market_phase),
                "timestamp": last.timestamp.to_rfc3339(),
                "trading_start_time": params.trading_start_time,
                "trading_end_time": params.trading_end_time,
            }),
        );
    }

    let Some(atr_value) = atr(candles, params.atr_period) else {
        return ContextCheck::Rejected(
            RejectionReason::IncompleteSetup,
            json!({ "reason": "unable to compute ATR", "candles": candles.len() }),
        );
    };
    if atr_value.is_zero() {
        return ContextCheck::Rejected(
            RejectionReason::IncompleteSetup,
            json!({ "reason": "ATR is zero" }),
        );
    }

    // Volatilidade anormal (padrão do projeto).
    let atr_pct = crate::indicators::atr_percent(candles, params.atr_period);
    if let Some(pct) = atr_pct {
        if pct > params.max_atr_pct {
            return ContextCheck::Rejected(
                RejectionReason::HighVolatility,
                json!({ "reason": "ATR% acima do limite", "atr_pct": pct, "max_atr_pct": params.max_atr_pct }),
            );
        }
    }

    let ema_value = ema(candles, params.keltner_ema_period);
    let keltner = ema_value.map(|e| e - params.keltner_atr_mult * atr_value);
    let macd_fast = macd_fast_value(&closes(candles), params.macd_fast_sma, params.macd_slow_sma);

    // Clímax de venda em andamento: impulso de baixa fresco e extremo — o
    // indicador "should be disregarded" nessas situações (Cap. 7).
    if climax_in_progress(candles, params, atr_value) {
        return ContextCheck::Rejected(
            RejectionReason::ClimaxInProgress,
            json!({
                "reason": "clímax de venda em andamento",
                "last_range": last.range(),
                "atr": atr_value,
                "climax_bar_atr_mult": params.climax_bar_atr_mult,
                "macd_fast": macd_fast,
            }),
        );
    }

    // Sobreextensão: pelo menos UMA das condições da seção 4[1] do doc.
    let overextension = if touched_lower_keltner(candles, params) {
        Some("keltner")
    } else if drop_from_swing_high(candles, params, atr_value) {
        Some("drop")
    } else if macd_divergence(candles, params) {
        Some("divergence")
    } else {
        None
    };

    let Some(overextension) = overextension else {
        return ContextCheck::Rejected(
            RejectionReason::NotOverextended,
            json!({
                "reason": "nenhuma condição de sobreextensão presente",
                "keltner_lower": keltner,
                "atr": atr_value,
                "macd_fast": macd_fast,
            }),
        );
    };

    ContextCheck::Approved(Box::new(ContextData {
        ema: ema_value,
        atr: atr_value,
        atr_pct,
        keltner_lower: keltner,
        macd_fast,
        overextension,
    }))
}

/// Horário do candle dentro da janela configurada ("HH:MM:SS", horário de
/// NOVA YORK — a janela em UTC fixo deslizava na virada do DST, A2 da
/// auditoria de 30/08/2026).
fn within_trading_window(candle: &Candle, params: &StrategyParameters) -> bool {
    let Some(start) = parse_hhmmss(&params.trading_start_time) else {
        return false;
    };
    let Some(end) = parse_hhmmss(&params.trading_end_time) else {
        return false;
    };
    let time = crate::session::et_time(candle.timestamp);
    let current = time.hour() * 3600 + time.minute() * 60 + time.second();
    current >= start && current <= end
}

fn parse_hhmmss(raw: &str) -> Option<u32> {
    let parts: Vec<&str> = raw.split(':').collect();
    if parts.len() != 3 {
        return None;
    }
    let h: u32 = parts[0].parse().ok()?;
    let m: u32 = parts[1].parse().ok()?;
    let s: u32 = parts[2].parse().ok()?;
    Some(h * 3600 + m * 60 + s)
}

fn closes(candles: &[Candle]) -> Vec<Decimal> {
    candles.iter().map(|c| c.close).collect()
}

/// SMA dos últimos `period` valores de uma série de preços.
fn sma_values(values: &[Decimal], period: usize) -> Option<Decimal> {
    if values.len() < period || period == 0 {
        return None;
    }
    let sum: Decimal = values.iter().rev().take(period).sum();
    Some(sum / Decimal::from(period))
}

/// Linha rápida do MACD modificado de Grimes (Apêndice B): SMA(3) − SMA(10)
/// dos fechamentos.
pub fn macd_fast_value(
    closes: &[Decimal],
    fast_period: usize,
    slow_period: usize,
) -> Option<Decimal> {
    let fast = sma_values(closes, fast_period)?;
    let slow = sma_values(closes, slow_period)?;
    Some(fast - slow)
}

/// Série da linha rápida do MACD, alinhada aos índices dos candles
/// (`None` enquanto não há dados para a SMA lenta).
fn macd_fast_series(
    closes: &[Decimal],
    fast_period: usize,
    slow_period: usize,
) -> Vec<Option<Decimal>> {
    (0..closes.len())
        .map(|i| macd_fast_value(&closes[..=i], fast_period, slow_period))
        .collect()
}

/// Condição [1a]: o preço tocou/ficou abaixo do canal de Keltner inferior
/// (EMA20 − 2,25×ATR14) em algum dos últimos `climax_lookback_candles`.
fn touched_lower_keltner(candles: &[Candle], params: &StrategyParameters) -> bool {
    let start = candles.len().saturating_sub(params.climax_lookback_candles);
    for i in start..candles.len() {
        let prefix = &candles[..=i];
        let (Some(ema_value), Some(atr_value)) = (
            ema(prefix, params.keltner_ema_period),
            atr(prefix, params.atr_period),
        ) else {
            continue;
        };
        let lower = ema_value - params.keltner_atr_mult * atr_value;
        if candles[i].low <= lower {
            return true;
        }
    }
    false
}

/// Condição [1b]: queda acumulada ≥ `overextension_atr_mult` × ATR14 desde a
/// máxima de swing da janela de lookback do nível.
fn drop_from_swing_high(
    candles: &[Candle],
    params: &StrategyParameters,
    atr_value: Decimal,
) -> bool {
    let start = candles.len().saturating_sub(params.level_lookback_candles);
    let Some(swing_high) = candles[start..].iter().map(|c| c.high).max() else {
        return false;
    };
    let last_close = candles.last().map(|c| c.close).unwrap_or_default();
    swing_high - last_close >= params.overextension_atr_mult * atr_value
}

/// Condição [1c]: divergência de momentum — nova mínima de preço (janela
/// recente) sem nova mínima na linha rápida do MACD, contra a janela de
/// `macd_lookback_candles` barras anterior (Cap. 7).
fn macd_divergence(candles: &[Candle], params: &StrategyParameters) -> bool {
    const RECENT_BARS: usize = 3;
    let window = params.macd_lookback_candles;
    if candles.len() < window + RECENT_BARS {
        return false;
    }

    let split = candles.len() - RECENT_BARS;
    let (prior, recent) = candles.split_at(split);

    let Some(prior_low) = prior.iter().map(|c| c.low).min() else {
        return false;
    };
    let Some(recent_low) = recent.iter().map(|c| c.low).min() else {
        return false;
    };
    if recent_low >= prior_low {
        return false; // sem nova mínima de preço, não há divergência possível
    }

    let prior_closes = closes(&candles[..split]);
    let prior_macd_min =
        macd_fast_series(&prior_closes, params.macd_fast_sma, params.macd_slow_sma)
            .into_iter()
            .flatten()
            .min();
    let all_closes = closes(candles);
    let recent_macd_min = macd_fast_series(&all_closes, params.macd_fast_sma, params.macd_slow_sma)
        .into_iter()
        .skip(split)
        .flatten()
        .min();

    match (prior_macd_min, recent_macd_min) {
        (Some(prior_min), Some(recent_min)) => recent_min > prior_min,
        _ => false,
    }
}

/// Condição [3]: clímax de venda — MACD rápido em nova mínima extrema da
/// janela E última barra com range > `climax_bar_atr_mult` × ATR14.
fn climax_in_progress(candles: &[Candle], params: &StrategyParameters, atr_value: Decimal) -> bool {
    let Some(last) = candles.last() else {
        return false;
    };
    if last.range() <= params.climax_bar_atr_mult * atr_value {
        return false;
    }

    let closes = closes(candles);
    let series = macd_fast_series(&closes, params.macd_fast_sma, params.macd_slow_sma);
    let start = series.len().saturating_sub(params.macd_lookback_candles);
    let window: Vec<Decimal> = series[start..].iter().flatten().copied().collect();
    let Some(current) = series.last().copied().flatten() else {
        return false;
    };
    match window.iter().min() {
        Some(min) => current <= *min,
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::strategies::failure_test_long_v1::config::FailureTestLongV1Config;
    use chrono::{TimeZone, Utc};
    use trader_domain::TimeFrame;

    fn candle_at(idx: u32, open: &str, high: &str, low: &str, close: &str) -> Candle {
        let ts = Utc.with_ymd_and_hms(2026, 8, 3, 14, 0, 0).unwrap()
            + chrono::Duration::minutes(i64::from(idx) * 5);
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

    #[test]
    fn macd_fast_is_sma3_minus_sma10() {
        // 10 fechamentos crescentes: SMA3 = 9, SMA10 = 5.5 → fast = 3.5.
        let closes: Vec<Decimal> = (1..=10).map(Decimal::from).collect();
        let fast = macd_fast_value(&closes, 3, 10).unwrap();
        assert_eq!(fast, Decimal::from(35) / Decimal::from(10));
    }

    #[test]
    fn macd_fast_none_without_enough_data() {
        let closes: Vec<Decimal> = (1..=5).map(Decimal::from).collect();
        assert_eq!(macd_fast_value(&closes, 3, 10), None);
    }

    #[test]
    fn divergence_detects_price_low_without_macd_low() {
        let params = StrategyParameters {
            macd_lookback_candles: 20,
            ..FailureTestLongV1Config::default().strategy.parameters
        };

        // Primeira perna de baixa rápida (MACD faz mínima profunda), repique,
        // segunda perna mais lenta que furta a mínima de preço SEM nova
        // mínima no MACD rápido → divergência de momentum.
        let mut candles = Vec::new();
        let mut price: i64 = 120;
        // Perna 1: 8 barras de −2 (120 → 104).
        for i in 0..8u32 {
            candles.push(candle_at(
                i,
                &price.to_string(),
                &(price + 1).to_string(),
                &(price - 2).to_string(),
                &(price - 2).to_string(),
            ));
            price -= 2;
        }
        // Repique: 6 barras de +1 (104 → 110).
        for i in 8..14u32 {
            candles.push(candle_at(
                i,
                &price.to_string(),
                &(price + 1).to_string(),
                &price.to_string(),
                &(price + 1).to_string(),
            ));
            price += 1;
        }
        // Perna 2: 11 barras de −1 (110 → 99) — nova mínima de preço (99 <
        // 104), mas descida mais lenta → MACD rápido não confirma a mínima.
        for i in 14..25u32 {
            candles.push(candle_at(
                i,
                &price.to_string(),
                &(price + 1).to_string(),
                &(price - 1).to_string(),
                &(price - 1).to_string(),
            ));
            price -= 1;
        }

        assert!(
            macd_divergence(&candles, &params),
            "nova mínima de preço sem nova mínima do MACD deveria ser divergência"
        );
    }

    #[test]
    fn no_divergence_when_price_low_is_not_new() {
        let params = StrategyParameters {
            macd_lookback_candles: 20,
            ..FailureTestLongV1Config::default().strategy.parameters
        };

        // Série lateral: nenhuma nova mínima de preço na janela recente.
        let candles: Vec<Candle> = (0..25u32)
            .map(|i| candle_at(i, "100", "101", "99.5", "100"))
            .collect();

        assert!(!macd_divergence(&candles, &params));
    }
}
