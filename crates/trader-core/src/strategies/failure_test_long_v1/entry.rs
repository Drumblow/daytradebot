//! Regras de entrada, stop e alvo para a estratégia Failure Test Long.
//!
//! - Entrada: buy stop 1 tick acima da máxima da barra de recuperação
//!   (adaptação de infra — o livro entra "no fechamento", mas nossos candles
//!   chegam ~30s atrasados; seção 6 do doc).
//! - Stop: extremo da excursão − 1 tick − jitter (Cap. 6), com sanidade de
//!   distância mínima (fora do ruído, Cap. 8) e máxima (RR intraday).
//! - Alvo: `target_r_multiple` × risco (TP único — adaptação do parcial em
//!   1R do livro ao bracket de TP único da infra).

use rust_decimal::Decimal;
use serde_json::json;

use crate::strategies::failure_test_long_v1::config::StrategyParameters;
use crate::strategies::failure_test_long_v1::context::ContextData;
use crate::strategies::failure_test_long_v1::setup::{Probe, Setup, SupportLevel};
use trader_domain::{
    Candle, Direction, MarketContext, RejectionReason, Signal, SignalStatus, TimeFrame,
};

/// Preços calculados de entrada, stop e alvo.
#[derive(Debug, Clone, PartialEq)]
pub struct EntryPrices {
    pub entry_price: Decimal,
    pub stop_price: Decimal,
    pub target_price: Decimal,
}

/// Calcula e valida os preços do trade (seções 6–7 do doc).
pub fn evaluate_prices(
    candles: &[Candle],
    _level: &SupportLevel,
    probe: &Probe,
    atr: Decimal,
    params: &StrategyParameters,
) -> Result<EntryPrices, (RejectionReason, serde_json::Value)> {
    let recovery = &candles[probe.recovery_index];

    let entry_price = if params.entry_order_type == "market_next_open" {
        // Aproximação da "entrada no fechamento" do livro: executa na
        // primeira oportunidade (fill imediato no simulador, market no live).
        recovery.close
    } else {
        recovery.high + params.entry_offset_ticks * params.tick_size
    };

    // O livro prescreve um jitter ALEATÓRIO em [0, mult × ATR] ("markets tend
    // to seek out those stop levels", Cap. 6). Aleatoriedade quebraria a
    // reprodutibilidade de backtest e testes, então usamos o ponto médio do
    // intervalo — desvio documentado, parametrizado por `stop_jitter_atr_mult`.
    let jitter = params.stop_jitter_atr_mult * atr / Decimal::from(2);
    let stop_price = probe.extreme - params.tick_size - jitter;

    let risk = entry_price - stop_price;
    if risk <= Decimal::ZERO {
        return Err((
            RejectionReason::StopWithinNoise,
            json!({ "reason": "distância de risco não positiva", "entry": entry_price, "stop": stop_price }),
        ));
    }

    // Sanidade do stop (literal, Cap. 8): nunca dentro do range médio de 1 barra.
    let avg_range = avg_bar_range(candles, 20);
    if let Some(avg_range) = avg_range {
        if risk < params.min_stop_bar_ranges * avg_range {
            return Err((
                RejectionReason::StopWithinNoise,
                json!({
                    "reason": "stop dentro do ruído (< 1x range médio de barra)",
                    "risk": risk,
                    "avg_bar_range": avg_range,
                }),
            ));
        }
    }

    if risk > params.max_stop_atr_mult * atr {
        return Err((
            RejectionReason::StopTooWide,
            json!({
                "reason": "stop largo demais (RR ruim para alvo intraday)",
                "risk": risk,
                "atr": atr,
                "max_stop_atr_mult": params.max_stop_atr_mult,
            }),
        ));
    }

    let target_price = entry_price + params.target_r_multiple * risk;
    let risk_reward = (target_price - entry_price) / risk;
    if risk_reward < params.min_risk_reward {
        return Err((
            RejectionReason::PoorRiskReward,
            json!({
                "reason": "risco/retorno abaixo do mínimo",
                "risk_reward": risk_reward,
                "min_risk_reward": params.min_risk_reward,
            }),
        ));
    }

    Ok(EntryPrices {
        entry_price,
        stop_price,
        target_price,
    })
}

/// Range médio das últimas `period` barras.
fn avg_bar_range(candles: &[Candle], period: usize) -> Option<Decimal> {
    if candles.len() < period || period == 0 {
        return None;
    }
    let sum: Decimal = candles.iter().rev().take(period).map(|c| c.range()).sum();
    Some(sum / Decimal::from(period))
}

/// Constrói um sinal aceito a partir de um setup válido, com metadados
/// auditáveis (seção 10 do doc).
#[allow(clippy::too_many_arguments)]
pub fn build_signal(
    symbol: impl Into<String>,
    timeframe: TimeFrame,
    setup: &Setup,
    context: &ContextData,
    ctx: &MarketContext,
    strategy_id: impl Into<String>,
    strategy_version: impl Into<String>,
    config_hash: impl Into<String>,
    entry_order_type: trader_domain::EntryOrderType,
    params: &StrategyParameters,
) -> Signal {
    let risk = setup.entry_price - setup.stop_price;
    let rr = ((setup.target_price - setup.entry_price) / risk).abs();

    let market_snapshot = json!({
        "trend_state": format!("{:?}", ctx.trend_state),
        "volatility_regime": format!("{:?}", ctx.volatility_regime),
        "market_phase": format!("{:?}", ctx.market_phase),
        "ema_20": ctx.ema_20,
        "atr_14": ctx.atr_14,
        "is_tradeable": ctx.is_tradeable,
        "signal_bar_index": setup.signal_index,
        // Metadados do failure test (seção 10 do doc).
        "support_level": setup.level.price,
        "level_touches": setup.level.touches,
        "probe_low": setup.probe.extreme,
        "probe_depth_atr": setup.probe.depth / context.atr,
        "probe_bars": setup.probe.bars,
        "ema_keltner": context.ema,
        "atr": context.atr,
        "keltner_lower": context.keltner_lower,
        "macd_fast": context.macd_fast,
        "overextension": context.overextension,
        "risk_reward": rr,
        "stop_distance_atr": risk / context.atr,
    });

    Signal {
        symbol: symbol.into(),
        strategy_id: strategy_id.into(),
        strategy_version: strategy_version.into(),
        config_hash: config_hash.into(),
        timeframe,
        timestamp: chrono::Utc::now(),
        direction: Direction::Long,
        status: SignalStatus::Accepted,
        entry_order_type,
        entry_price: Some(setup.entry_price),
        stop_price: Some(setup.stop_price),
        target_price: Some(setup.target_price),
        risk_reward_ratio: Some(rr),
        risk_amount: None,
        risk_percent: params.risk_per_trade_pct.map(|pct| pct / Decimal::from(100)),
        position_size: None,
        entry_reason: Some(format!(
            "failure_test_long: sonda abaixo de S={} falhou, recuperação em {} barra(s), contexto={}",
            setup.level.price, setup.probe.bars, context.overextension
        )),
        rejection_reason: None,
        rejection_details: None,
        market_snapshot,
        correlation_id: uuid::Uuid::new_v4().to_string(),
    }
}
