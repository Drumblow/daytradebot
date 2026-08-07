//! Regras de entrada, stop e alvo para a estratégia Pullback em Tendência de Alta.

use rust_decimal::Decimal;
use serde_json::json;

use crate::strategies::pullback_trend_v1::setup::Setup;
use trader_domain::{Direction, MarketContext, Signal, SignalStatus, TimeFrame};

/// Constrói um sinal aceito a partir de um setup válido.
#[allow(clippy::too_many_arguments)]
pub fn build_signal(
    symbol: impl Into<String>,
    timeframe: TimeFrame,
    setup: &Setup,
    ctx: &MarketContext,
    strategy_id: impl Into<String>,
    strategy_version: impl Into<String>,
    config_hash: impl Into<String>,
    entry_order_type: trader_domain::EntryOrderType,
) -> Signal {
    let rr = ((setup.target_price - setup.entry_price)
        / (setup.entry_price - setup.stop_price).abs())
    .abs();

    let market_snapshot = json!({
        "trend_state": format!("{:?}", ctx.trend_state),
        "volatility_regime": format!("{:?}", ctx.volatility_regime),
        "market_phase": format!("{:?}", ctx.market_phase),
        "ema_20": ctx.ema_20,
        "ema_50": ctx.ema_50,
        "sma_200": ctx.sma_200,
        "atr_14": ctx.atr_14,
        "atr_percent_14": ctx.atr_percent_14,
        "volume_relative": ctx.volume_relative,
        "range_percent": ctx.range_percent,
        "is_tradeable": ctx.is_tradeable,
        "raw_values": ctx.raw_values,
        "signal_bar_index": setup.signal_index,
        "pullback_start_index": setup.pullback_start_index,
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
        risk_percent: None,
        position_size: None,
        entry_reason: Some("high 2 pullback em tendência de alta".to_string()),
        rejection_reason: None,
        rejection_details: None,
        market_snapshot,
        correlation_id: uuid::Uuid::new_v4().to_string(),
    }
}

/// Calcula o tamanho da posição com base no capital, risco percentual e stop.
pub fn position_size(
    capital: Decimal,
    risk_per_trade_pct: Decimal,
    entry_price: Decimal,
    stop_price: Decimal,
) -> Option<Decimal> {
    let risk_amount = capital * risk_per_trade_pct / Decimal::from(100);
    let risk_distance = (entry_price - stop_price).abs();

    if risk_distance.is_zero() {
        return None;
    }

    let size = risk_amount / risk_distance;
    // Arredonda para quantidade inteira de ações.
    Some(size.trunc())
}

#[cfg(test)]
mod tests {
    use super::*;
    use trader_domain::{MarketPhase, TrendState, VolatilityRegime};

    fn make_setup() -> Setup {
        Setup {
            signal_index: 3,
            pullback_start_index: 2,
            entry_price: Decimal::from(101),
            stop_price: Decimal::from(100),
            target_price: Decimal::from(103),
        }
    }

    fn make_context() -> MarketContext {
        MarketContext {
            symbol: "SPY".to_string(),
            timeframe: TimeFrame::M15,
            timestamp: chrono::Utc::now(),
            candle_timestamp: None,
            trend_state: TrendState::Uptrend,
            volatility_regime: VolatilityRegime::Normal,
            market_phase: MarketPhase::Regular,
            ema_20: Some(Decimal::from(100)),
            ema_50: None,
            sma_200: None,
            atr_14: None,
            atr_percent_14: None,
            volume_relative: None,
            hh_hl_count: None,
            lh_ll_count: None,
            range_percent: None,
            is_tradeable: true,
            raw_values: serde_json::Value::Object(Default::default()),
        }
    }

    #[test]
    fn signal_carries_risk_reward_and_snapshot() {
        let signal = build_signal(
            "SPY",
            TimeFrame::M15,
            &make_setup(),
            &make_context(),
            "pullback-trend-v1",
            "1.0.0",
            "hash123",
            trader_domain::EntryOrderType::Stop,
        );

        assert_eq!(signal.direction, Direction::Long);
        assert_eq!(signal.status, SignalStatus::Accepted);
        assert_eq!(signal.risk_reward_ratio, Some(Decimal::from(2)));
        assert_eq!(signal.config_hash, "hash123");
        assert!(signal.market_snapshot.get("trend_state").is_some());
        assert!(signal.market_snapshot.get("signal_bar_index").is_some());
    }

    #[test]
    fn position_size_risks_one_percent_of_capital() {
        // capital 100k, risco 1% = 1000; distância 1.02 → 980 ações (truncado).
        let size = position_size(
            Decimal::from(100_000),
            Decimal::ONE,
            "101.01".parse().unwrap(),
            "99.99".parse().unwrap(),
        );
        assert_eq!(size, Some(Decimal::from(980)));
    }

    #[test]
    fn position_size_zero_distance_is_none() {
        let size = position_size(
            Decimal::from(100_000),
            Decimal::ONE,
            Decimal::from(100),
            Decimal::from(100),
        );
        assert_eq!(size, None);
    }
}
