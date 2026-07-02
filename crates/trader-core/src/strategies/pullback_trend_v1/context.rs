//! Regras de contexto de mercado para a estratégia Pullback em Tendência de Alta.

use rust_decimal::Decimal;
use serde_json::json;

use crate::strategies::pullback_trend_v1::config::StrategyParameters;
use trader_domain::{MarketContext, MarketPhase, RejectionReason, TrendState, VolatilityRegime};

/// Resultado da avaliação de contexto.
#[derive(Debug, Clone, PartialEq)]
pub enum ContextCheck {
    Approved,
    Rejected(RejectionReason, serde_json::Value),
}

/// Avalia se o contexto de mercado permite buscar setups de compra.
pub fn check_context(ctx: &MarketContext, params: &StrategyParameters) -> ContextCheck {
    if !matches!(ctx.trend_state, TrendState::Uptrend) {
        return ContextCheck::Rejected(
            RejectionReason::NoContext,
            json!({ "reason": "trend_state is not uptrend", "value": format!("{:?}", ctx.trend_state) }),
        );
    }

    if matches!(ctx.volatility_regime, VolatilityRegime::High) {
        return ContextCheck::Rejected(
            RejectionReason::HighVolatility,
            json!({ "reason": "volatility regime is high", "atr_14": ctx.atr_14 }),
        );
    }

    if !matches!(ctx.market_phase, MarketPhase::Regular) {
        return ContextCheck::Rejected(
            RejectionReason::OutsideTradingHours,
            json!({ "reason": "market is not in regular hours", "phase": format!("{:?}", ctx.market_phase) }),
        );
    }

    let ema_period = params.ema_context_period;

    if let Some(ema) = ctx.ema_20 {
        if let Some(last_close) = ctx
            .raw_values
            .get("last_close")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<Decimal>().ok())
        {
            if last_close < ema {
                return ContextCheck::Rejected(
                    RejectionReason::NoContext,
                    json!({ "reason": "close below ema", "close": last_close, "ema_period": ema_period, "ema": ema }),
                );
            }
        }
    }

    ContextCheck::Approved
}
