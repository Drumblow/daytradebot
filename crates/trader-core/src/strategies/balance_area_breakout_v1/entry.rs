//! Regras de entrada, stop e alvo para Balance-Area Breakout v1 (seções 5–6
//! do doc).
//!
//! - Entrada: stop 1 tick além do extremo do candle de rompimento (ADR-009).
//! - Stop: de volta dentro da área (`stop_buffer_atr_mult` × ATR) — retorno
//!   à área indica rejeição (literal, Cap. 4).
//! - Alvo: 2R (adaptação do "much bigger move" ao bracket de TP único).

use rust_decimal::Decimal;
use serde_json::json;

use crate::strategies::balance_area_breakout_v1::config::StrategyParameters;
use crate::strategies::balance_area_breakout_v1::context::BalanceArea;
use crate::strategies::balance_area_breakout_v1::setup::Setup;
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

/// Calcula e valida os preços do trade (seções 5–6 do doc).
pub fn evaluate_prices(
    candles: &[Candle],
    setup: &Setup,
    area: &BalanceArea,
    atr_value: Decimal,
    params: &StrategyParameters,
) -> Result<EntryPrices, (RejectionReason, serde_json::Value)> {
    let bar = &candles[setup.breakout_index];
    let tick = params.entry_offset_ticks * params.tick_size;
    let buffer = params.stop_buffer_atr_mult * atr_value;

    let (entry_price, stop_price) = match setup.direction {
        Direction::Long => (bar.high + tick, area.high - buffer),
        Direction::Short => (bar.low - tick, area.low + buffer),
    };

    let risk = (entry_price - stop_price).abs();
    if risk.is_zero() {
        return Err((
            RejectionReason::StopWithinNoise,
            json!({ "reason": "risco zero (stop coincide com a entrada)" }),
        ));
    }
    if risk > params.max_stop_atr_mult * atr_value {
        return Err((
            RejectionReason::StopTooWide,
            json!({
                "reason": "stop largo demais (retorno à área distante)",
                "risk": risk,
                "atr": atr_value,
                "max_stop_atr_mult": params.max_stop_atr_mult,
            }),
        ));
    }

    let target_price = match setup.direction {
        Direction::Long => entry_price + params.target_r_multiple * risk,
        Direction::Short => entry_price - params.target_r_multiple * risk,
    };

    let risk_reward = (target_price - entry_price).abs() / risk;
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

/// Constrói um sinal aceito a partir de um setup válido, com metadados
/// auditáveis (seção 9 do doc).
#[allow(clippy::too_many_arguments)]
pub fn build_signal(
    symbol: impl Into<String>,
    timeframe: TimeFrame,
    setup: &Setup,
    prices: &EntryPrices,
    area: &BalanceArea,
    atr_value: Decimal,
    ctx: &MarketContext,
    strategy_id: impl Into<String>,
    strategy_version: impl Into<String>,
    config_hash: impl Into<String>,
    entry_order_type: trader_domain::EntryOrderType,
    params: &StrategyParameters,
) -> Signal {
    let risk = (prices.entry_price - prices.stop_price).abs();
    let rr = ((prices.target_price - prices.entry_price) / risk).abs();

    let market_snapshot = json!({
        "trend_state": format!("{:?}", ctx.trend_state),
        "volatility_regime": format!("{:?}", ctx.volatility_regime),
        "market_phase": format!("{:?}", ctx.market_phase),
        "is_tradeable": ctx.is_tradeable,
        "breakout_index": setup.breakout_index,
        // Metadados da área de balanceamento (seção 9 do doc).
        "area_high": area.high,
        "area_low": area.low,
        "area_width_pct": area.width_pct,
        "area_width_atr": area.width_atr,
        "atr": atr_value,
        "risk_reward": rr,
        "stop_distance_atr": if atr_value.is_zero() { Decimal::ZERO } else { risk / atr_value },
    });

    Signal {
        symbol: symbol.into(),
        strategy_id: strategy_id.into(),
        strategy_version: strategy_version.into(),
        config_hash: config_hash.into(),
        timeframe,
        timestamp: chrono::Utc::now(),
        direction: setup.direction,
        status: SignalStatus::Accepted,
        entry_order_type,
        entry_price: Some(prices.entry_price),
        stop_price: Some(prices.stop_price),
        target_price: Some(prices.target_price),
        risk_reward_ratio: Some(rr),
        risk_amount: None,
        risk_percent: params
            .risk_per_trade_pct
            .map(|pct| pct / Decimal::from(100)),
        position_size: None,
        entry_reason: Some(format!(
            "balance_area_breakout: rompimento {:?} da área [{}, {}] (largura {}×ATR)",
            setup.direction, area.low, area.high, area.width_atr,
        )),
        rejection_reason: None,
        rejection_details: None,
        market_snapshot,
        correlation_id: uuid::Uuid::new_v4().to_string(),
    }
}
