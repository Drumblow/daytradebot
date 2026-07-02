use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::TimeFrame;

/// Estado da tendência de mercado.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrendState {
    Uptrend,
    Downtrend,
    Neutral,
    Unknown,
}

/// Regime de volatilidade.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VolatilityRegime {
    High,
    Normal,
    Low,
    Unknown,
}

/// Fase do mercado (pré-market, regular, after-hours).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MarketPhase {
    PreMarket,
    Regular,
    AfterHours,
    Unknown,
}

/// Contexto de mercado classificado para um candle.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarketContext {
    pub symbol: String,
    pub timeframe: TimeFrame,
    pub timestamp: DateTime<Utc>,
    pub candle_timestamp: Option<DateTime<Utc>>,

    pub trend_state: TrendState,
    pub volatility_regime: VolatilityRegime,
    pub market_phase: MarketPhase,

    pub ema_20: Option<Decimal>,
    pub ema_50: Option<Decimal>,
    pub sma_200: Option<Decimal>,
    pub atr_14: Option<Decimal>,
    pub atr_percent_14: Option<Decimal>,
    pub volume_relative: Option<Decimal>,
    pub hh_hl_count: Option<i32>,
    pub lh_ll_count: Option<i32>,
    pub range_percent: Option<Decimal>,

    pub is_tradeable: bool,
    pub raw_values: serde_json::Value,
}

impl MarketContext {
    pub fn new(symbol: impl Into<String>, timeframe: TimeFrame, timestamp: DateTime<Utc>) -> Self {
        Self {
            symbol: symbol.into(),
            timeframe,
            timestamp,
            candle_timestamp: None,
            trend_state: TrendState::Unknown,
            volatility_regime: VolatilityRegime::Unknown,
            market_phase: MarketPhase::Unknown,
            ema_20: None,
            ema_50: None,
            sma_200: None,
            atr_14: None,
            atr_percent_14: None,
            volume_relative: None,
            hh_hl_count: None,
            lh_ll_count: None,
            range_percent: None,
            is_tradeable: false,
            raw_values: serde_json::Value::Object(Default::default()),
        }
    }
}
