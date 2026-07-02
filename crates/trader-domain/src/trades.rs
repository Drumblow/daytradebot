use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::Direction;

/// Posição aberta.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Position {
    pub id: Option<i64>,
    pub symbol: String,
    pub signal_id: i64,
    pub direction: Direction,
    pub quantity: Decimal,
    pub avg_entry_price: Decimal,
    pub entry_time: DateTime<Utc>,
    pub stop_price: Decimal,
    pub target_price: Option<Decimal>,
    pub unrealized_pnl: Decimal,
    pub realized_pnl: Decimal,
    pub exit_price: Option<Decimal>,
    pub exit_reason: Option<ExitReason>,
    pub status: PositionStatus,
    pub closed_at: Option<DateTime<Utc>>,
    pub broker: String,
    pub metadata: serde_json::Value,
    pub correlation_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PositionStatus {
    Open,
    Closed,
}

impl Position {
    pub fn new(
        symbol: impl Into<String>,
        signal_id: i64,
        direction: Direction,
        quantity: Decimal,
        avg_entry_price: Decimal,
        stop_price: Decimal,
        broker: impl Into<String>,
    ) -> Result<Self, crate::ValidationError> {
        if quantity <= Decimal::ZERO {
            return Err(crate::ValidationError::InvalidQuantity(
                "quantidade da posição deve ser positiva".to_string(),
            ));
        }
        Ok(Self {
            id: None,
            symbol: symbol.into(),
            signal_id,
            direction,
            quantity,
            avg_entry_price,
            entry_time: Utc::now(),
            stop_price,
            target_price: None,
            unrealized_pnl: Decimal::ZERO,
            realized_pnl: Decimal::ZERO,
            exit_price: None,
            exit_reason: None,
            status: PositionStatus::Open,
            closed_at: None,
            broker: broker.into(),
            metadata: serde_json::Value::Object(Default::default()),
            correlation_id: uuid::Uuid::new_v4().to_string(),
        })
    }
}

/// Trade fechado.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Trade {
    pub id: Option<i64>,
    pub symbol: String,
    pub signal_id: i64,
    pub position_id: Option<i64>,
    pub direction: Direction,
    pub entry_price: Decimal,
    pub exit_price: Decimal,
    pub quantity: Decimal,
    pub entry_time: DateTime<Utc>,
    pub exit_time: DateTime<Utc>,
    pub stop_price: Decimal,
    pub target_price: Option<Decimal>,
    pub gross_pnl: Decimal,
    pub commissions: Decimal,
    pub fees: Decimal,
    pub net_pnl: Decimal,
    pub risk_amount: Decimal,
    pub result_in_r: Decimal,
    pub exit_reason: ExitReason,
    pub strategy_id: String,
    pub strategy_version: String,
    pub config_hash: String,
    pub journal: serde_json::Value,
    pub correlation_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExitReason {
    Target,
    Stop,
    Time,
    Manual,
    RiskManager,
}

/// Resumo da conta no broker.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AccountSummary {
    pub broker: String,
    pub account_id: Option<String>,
    pub cash: Decimal,
    pub equity: Decimal,
    pub buying_power: Decimal,
    pub daily_pnl: Decimal,
    pub timestamp: DateTime<Utc>,
}
