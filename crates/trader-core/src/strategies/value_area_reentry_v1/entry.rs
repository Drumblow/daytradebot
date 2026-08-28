//! Regras de entrada, stop e alvo da Value Area Reentry v1 (seção 6 do doc).
//!
//! - Entrada: stop 1 tick além do extremo da barra de aceitação, na direção da
//!   travessia (ADR-009).
//! - Stop: 1 tick FORA da borda da VA por onde entramos — a invalidação
//!   literal da premissa ("rejeitado de volta para fora do valor").
//! - Alvo: borda OPOSTA da VA — alvo estrutural do livro ("auction completely
//!   through that value area"), não múltiplo de R.

use rust_decimal::Decimal;
use serde_json::json;

use crate::strategies::value_area_reentry_v1::config::StrategyParameters;
use crate::strategies::value_area_reentry_v1::context::ValueArea;
use crate::strategies::value_area_reentry_v1::setup::Setup;
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

/// Calcula e valida os preços do trade (seção 6 do doc).
pub fn evaluate_prices(
    candles: &[Candle],
    setup: &Setup,
    va: &ValueArea,
    atr_value: Decimal,
    params: &StrategyParameters,
) -> Result<EntryPrices, Rejection> {
    let bar = &candles[setup.signal_index];
    let tick = params.entry_offset_ticks * params.tick_size;

    // O stop fica FORA da borda da VA com uma folga em ATR: a borda exata é
    // varrida por ruído (calibração de 2026-08-28, §16 do doc).
    let buffer = params.tick_size + params.stop_buffer_atr * atr_value;

    // Entrada: com `entry_order_type = "stop"` esperamos o rompimento do
    // extremo da barra de aceitação (convenção da ADR-009). Com `"limit"` a
    // entrada é a PRÓPRIA aceitação, no fechamento da barra — leitura literal
    // do Cap. 4 ("entrada = aceitação dentro da VA").
    let is_limit_entry = params.entry_order_type.trim().eq_ignore_ascii_case("limit");
    let entry_price = if is_limit_entry {
        bar.close
    } else {
        match setup.direction {
            Direction::Long => bar.high + tick,
            Direction::Short => bar.low - tick,
        }
    };
    let (stop_price, target_price) = match setup.direction {
        Direction::Long => (va.low - buffer, va.high),
        Direction::Short => (va.high + buffer, va.low),
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

    // A travessia restante precisa existir: se a barra de aceitação já levou o
    // preço até (ou além) da borda oposta, não há alvo estrutural.
    let reward = match setup.direction {
        Direction::Long => target_price - entry_price,
        Direction::Short => entry_price - target_price,
    };
    if reward <= Decimal::ZERO {
        return Err((
            RejectionReason::PoorRiskReward,
            json!({
                "reason": "travessia já consumida — entrada além da borda oposta da VA",
                "entry": entry_price,
                "target": target_price,
            }),
        ));
    }

    let risk_reward = reward / risk;
    if risk_reward < params.min_risk_reward {
        return Err((
            RejectionReason::PoorRiskReward,
            json!({
                "reason": "risco/retorno abaixo do mínimo (alvo estrutural curto para o stop)",
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
    va: &ValueArea,
    atr_value: Decimal,
    daily_atr_value: Decimal,
    ema_slope: Decimal,
    ctx: &MarketContext,
    strategy_id: impl Into<String>,
    strategy_version: impl Into<String>,
    config_hash: impl Into<String>,
    entry_order_type: trader_domain::EntryOrderType,
    params: &StrategyParameters,
) -> Signal {
    let risk = (prices.entry_price - prices.stop_price).abs();
    let rr = ((prices.target_price - prices.entry_price) / risk).abs();
    let va_width = va.width();

    let ratio = |value: Decimal| {
        if daily_atr_value.is_zero() {
            Decimal::ZERO
        } else {
            value / daily_atr_value
        }
    };

    let market_snapshot = json!({
        "trend_state": format!("{:?}", ctx.trend_state),
        "volatility_regime": format!("{:?}", ctx.volatility_regime),
        "market_phase": format!("{:?}", ctx.market_phase),
        "is_tradeable": ctx.is_tradeable,
        "signal_bar_index": setup.signal_index,
        // Metadados da value area reentry (seção 9 do doc).
        "va_high": va.high,
        "va_low": va.low,
        "poc": va.poc,
        "va_width": va_width,
        "va_width_atr_ratio": ratio(va_width),
        "open_today": setup.open_today,
        "open_distance": setup.open_distance,
        "open_distance_atr_ratio": ratio(setup.open_distance),
        "ema_slope_per_bar": ema_slope,
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
            "value_area_reentry: abertura {} do valor de ontem (dist={}), aceita de volta em [{}, {}], travessia {:?} até {}",
            if setup.direction == Direction::Long {
                "abaixo"
            } else {
                "acima"
            },
            setup.open_distance.round_dp(2),
            va.low.round_dp(2),
            va.high.round_dp(2),
            setup.direction,
            prices.target_price.round_dp(2),
        )),
        rejection_reason: None,
        rejection_details: None,
        market_snapshot,
        correlation_id: uuid::Uuid::new_v4().to_string(),
    }
}
