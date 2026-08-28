//! Regras de entrada, stop e alvo da Trendline Break Test v1 (seção 8 do doc).
//!
//! - Entrada: stop 1 tick além do extremo da barra de sinal (ADR-009).
//! - Stop: 1 tick além do extremo do TESTE — literal do Cap. 8.
//! - Alvo: múltiplo de R (default 2,0). O livro pede duas pernas com saída
//!   parcial ("swing part of your position", Cap. 15), o que nosso bracket de
//!   TP único não representa — ver §9 do doc.

use rust_decimal::Decimal;
use serde_json::json;

use crate::strategies::trendline_break_test_v1::config::StrategyParameters;
use crate::strategies::trendline_break_test_v1::context::{TrendContext, TrendlineBreak};
use crate::strategies::trendline_break_test_v1::setup::Setup;
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

type Rejection = (RejectionReason, serde_json::Value);

/// Calcula e valida os preços do trade (seção 8 do doc).
pub fn evaluate_prices(
    candles: &[Candle],
    setup: &Setup,
    atr_value: Decimal,
    params: &StrategyParameters,
) -> Result<EntryPrices, Rejection> {
    let bar = &candles[setup.signal_index];
    let tick = params.entry_offset_ticks * params.tick_size;

    let (entry_price, stop_price) = match setup.direction {
        Direction::Long => (bar.high + tick, setup.test_extreme - params.tick_size),
        Direction::Short => (bar.low - tick, setup.test_extreme + params.tick_size),
    };

    let risk = (entry_price - stop_price).abs();
    if risk <= params.tick_size || (!atr_value.is_zero() && risk < params.min_stop_atr * atr_value)
    {
        return Err((
            RejectionReason::StopWithinNoise,
            json!({
                "reason": "stop dentro do ruído (risco abaixo do mínimo em ATR)",
                "risk": risk,
                "atr": atr_value,
                "min_stop_atr": params.min_stop_atr,
            }),
        ));
    }

    if !atr_value.is_zero() && risk > params.max_stop_atr * atr_value {
        return Err((
            RejectionReason::StopTooWide,
            json!({
                "reason": "stop mais largo que o máximo em ATR",
                "risk": risk,
                "atr": atr_value,
                "max_stop_atr": params.max_stop_atr,
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

/// Constrói um sinal aceito, com metadados auditáveis.
#[allow(clippy::too_many_arguments)]
pub fn build_signal(
    symbol: impl Into<String>,
    timeframe: TimeFrame,
    setup: &Setup,
    prices: &EntryPrices,
    trend: &TrendContext,
    brk: &TrendlineBreak,
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
        "signal_bar_index": setup.signal_index,
        // Metadados da trendline break test (seção 9 do doc).
        "trend_kind": format!("{:?}", trend.kind),
        "trend_extreme": trend.extreme_price,
        "trend_extreme_index": trend.extreme_index,
        "last_counter_swing": trend.last_counter_swing,
        "trendline_p1": trend.line_p1,
        "trendline_p2": trend.line_p2,
        "break_index": brk.index,
        "break_line_value": brk.line_value,
        "break_age_bars": setup.signal_index.saturating_sub(brk.index),
        "break_leg_bars": brk.leg_bars,
        "break_closes_beyond_ema": brk.closes_beyond_ema,
        "test_offset": setup.test_offset,
        "test_is_overshoot": setup.is_overshoot,
        "signal_body_pct": setup.body_pct,
        "signal_wick_pct": setup.wick_pct,
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
            "trendline_break_test: tendência {:?} rompida na barra {}, teste do extremo {} ({}), reversão {:?}",
            trend.kind,
            brk.index,
            trend.extreme_price.round_dp(2),
            if setup.is_overshoot {
                "overshoot"
            } else {
                "undershoot"
            },
            setup.direction,
        )),
        rejection_reason: None,
        rejection_details: None,
        market_snapshot,
        correlation_id: uuid::Uuid::new_v4().to_string(),
    }
}
