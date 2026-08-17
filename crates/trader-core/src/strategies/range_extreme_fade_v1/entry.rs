//! Regras de entrada, stop e alvo para Range Extreme Fade v1 (seção 6 do doc).
//!
//! - Entrada: stop 1 tick além da barra de sinal, na direção do fade
//!   (literal, Cap. 10).
//! - Stop: 1 tick além do extremo oposto da barra de sinal (o extremo do
//!   rompimento falho).
//! - Alvo: 1,5R fixo (adaptação do "scalp de volta ao interior do range" ao
//!   bracket de TP único; alvo estrutural é candidato de v2).

use rust_decimal::Decimal;
use serde_json::json;

use crate::strategies::range_extreme_fade_v1::config::StrategyParameters;
use crate::strategies::range_extreme_fade_v1::setup::Setup;
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
    params: &StrategyParameters,
) -> Result<EntryPrices, (RejectionReason, serde_json::Value)> {
    let bar = &candles[setup.signal_index];
    let tick = params.entry_offset_ticks * params.tick_size;

    let (entry_price, stop_price) = match setup.direction {
        Direction::Long => (bar.high + tick, bar.low - params.tick_size),
        Direction::Short => (bar.low - tick, bar.high + params.tick_size),
    };

    let risk = (entry_price - stop_price).abs();
    if risk.is_zero() {
        return Err((
            RejectionReason::StopWithinNoise,
            json!({ "reason": "risco zero na barra de sinal" }),
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
    atr_value: Decimal,
    daily_atr_value: Decimal,
    ema_slope_per_bar: Decimal,
    day_range_value: Decimal,
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
        "signal_bar_index": setup.signal_index,
        // Metadados do range extreme fade (seção 9 do doc).
        "broken_extreme": setup.broken_extreme,
        "extension_atr": setup.extension_atr,
        "signal_body_pct": setup.body_pct,
        "signal_wick_pct": setup.wick_pct,
        "ema_slope_per_bar": ema_slope_per_bar,
        "day_range": day_range_value,
        "daily_atr": daily_atr_value,
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
            "range_extreme_fade: rompimento falho de {} do dia ({}), fade {:?}, extensão={} ATR",
            if setup.direction == Direction::Long {
                "mínima"
            } else {
                "máxima"
            },
            setup.broken_extreme,
            setup.direction,
            setup.extension_atr.round_dp(2),
        )),
        rejection_reason: None,
        rejection_details: None,
        market_snapshot,
        correlation_id: uuid::Uuid::new_v4().to_string(),
    }
}
