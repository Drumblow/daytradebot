//! Regras de entrada, stop e alvo para Breakout — Primeiro Pullback v1
//! (seção 6 do doc).
//!
//! - Entrada: buy stop 1 tick acima da máxima da barra de gatilho (ADR-009).
//! - Stop: pivô pré-breakout − 1 tick (literal: o nível rompido NÃO é bom stop).
//! - Alvo: min(MMO = H + impulso, `target_r_multiple` × risco) — adaptação do
//!   "deixar espaço para o movimento maior" ao bracket de TP único.

use rust_decimal::Decimal;
use serde_json::json;

use crate::strategies::breakout_first_pullback_v1::config::StrategyParameters;
use crate::strategies::breakout_first_pullback_v1::context::avg_range;
use crate::strategies::breakout_first_pullback_v1::setup::Setup;
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

/// Calcula e valida os preços do trade (seção 6 do doc).
pub fn evaluate_prices(
    candles: &[Candle],
    setup: &Setup,
    atr_value: Decimal,
    params: &StrategyParameters,
) -> Result<EntryPrices, (RejectionReason, serde_json::Value)> {
    let signal_bar = &candles[setup.signal_index];

    let entry_price = signal_bar.high + params.entry_offset_ticks * params.tick_size;
    let stop_price = setup.pivot - params.tick_size;

    let risk = entry_price - stop_price;
    if risk <= Decimal::ZERO {
        return Err((
            RejectionReason::StopTooWide,
            json!({ "reason": "risco não positivo (pivô acima do gatilho)", "entry": entry_price, "stop": stop_price }),
        ));
    }

    // Sanidade do stop (Cap. 8): nunca dentro do range médio de 1 barra.
    if let Some(avg_r) = avg_range(candles, candles.len(), params.avg_period) {
        if risk < params.min_stop_bar_ranges * avg_r {
            return Err((
                RejectionReason::StopWithinNoise,
                json!({
                    "reason": "stop dentro do ruído (< 1x range médio de barra)",
                    "risk": risk,
                    "avg_bar_range": avg_r,
                }),
            ));
        }
    }

    if risk > params.max_stop_atr_mult * atr_value {
        return Err((
            RejectionReason::StopTooWide,
            json!({
                "reason": "stop largo demais para alvo intraday",
                "risk": risk,
                "atr": atr_value,
                "max_stop_atr_mult": params.max_stop_atr_mult,
            }),
        ));
    }

    // Alvo: o menor entre o MMO (projeção do impulso) e o teto em R.
    let mmo = setup.impulse_high + setup.impulse;
    let target_price = mmo.min(entry_price + params.target_r_multiple * risk);
    if target_price <= entry_price {
        return Err((
            RejectionReason::PoorRiskReward,
            json!({ "reason": "MMO abaixo da entrada", "mmo": mmo, "entry": entry_price }),
        ));
    }

    let risk_reward = (target_price - entry_price) / risk;
    if risk_reward < params.min_risk_reward {
        return Err((
            RejectionReason::PoorRiskReward,
            json!({
                "reason": "risco/retorno abaixo do mínimo",
                "risk_reward": risk_reward,
                "min_risk_reward": params.min_risk_reward,
                "mmo": mmo,
            }),
        ));
    }

    Ok(EntryPrices {
        entry_price,
        stop_price,
        target_price,
    })
}

/// Constrói um sinal aceito a partir de um setup válido, com metadados
/// auditáveis (seção 9 do doc).
#[allow(clippy::too_many_arguments)]
pub fn build_signal(
    symbol: impl Into<String>,
    timeframe: TimeFrame,
    setup: &Setup,
    prices: &EntryPrices,
    atr_value: Decimal,
    ctx: &MarketContext,
    strategy_id: impl Into<String>,
    strategy_version: impl Into<String>,
    config_hash: impl Into<String>,
    entry_order_type: trader_domain::EntryOrderType,
    params: &StrategyParameters,
) -> Signal {
    let risk = prices.entry_price - prices.stop_price;
    let rr = ((prices.target_price - prices.entry_price) / risk).abs();
    let mmo = setup.impulse_high + setup.impulse;

    let market_snapshot = json!({
        "trend_state": format!("{:?}", ctx.trend_state),
        "volatility_regime": format!("{:?}", ctx.volatility_regime),
        "market_phase": format!("{:?}", ctx.market_phase),
        "is_tradeable": ctx.is_tradeable,
        "signal_bar_index": setup.signal_index,
        // Metadados do breakout-pullback (seção 9 do doc).
        "resistance_level": setup.breakout.level.price,
        "level_touches": setup.breakout.level.touches,
        "breakout_index": setup.breakout.index,
        "breakout_range_ratio": setup.breakout.range_ratio,
        "breakout_volume_ratio": setup.breakout.volume_ratio,
        "impulse_high": setup.impulse_high,
        "impulse": setup.impulse,
        "retrace": setup.retrace,
        "retrace_pct_of_impulse": setup.retrace / setup.impulse,
        "pivot": setup.pivot,
        "mmo": mmo,
        "atr": atr_value,
        "risk_reward": rr,
        "stop_distance_atr": risk / atr_value,
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
        entry_price: Some(prices.entry_price),
        stop_price: Some(prices.stop_price),
        target_price: Some(prices.target_price),
        risk_reward_ratio: Some(rr),
        risk_amount: None,
        risk_percent: params.risk_per_trade_pct.map(|pct| pct / Decimal::from(100)),
        position_size: None,
        entry_reason: Some(format!(
            "breakout_first_pullback: breakout de R={} ({} toques), pullback {}/{}, stop no pivô {}",
            setup.breakout.level.price,
            setup.breakout.level.touches,
            setup.retrace,
            setup.impulse,
            setup.pivot,
        )),
        rejection_reason: None,
        rejection_details: None,
        market_snapshot,
        correlation_id: uuid::Uuid::new_v4().to_string(),
    }
}
