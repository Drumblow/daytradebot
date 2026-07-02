use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// Tipo de ordem suportado.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderType {
    Market,
    Limit,
    Stop,
    StopLimit,
    Bracket,
}

/// Lado da ordem.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderSide {
    Buy,
    Sell,
}

impl OrderSide {
    pub fn opposite(&self) -> Self {
        match self {
            OrderSide::Buy => OrderSide::Sell,
            OrderSide::Sell => OrderSide::Buy,
        }
    }
}

/// Status de uma ordem.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderStatus {
    Pending,
    Submitted,
    Accepted,
    PartiallyFilled,
    Filled,
    Cancelled,
    Rejected,
    Expired,
}

/// Time-in-force de uma ordem.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimeInForce {
    Day,
    Gtc,
    Ioc,
    Fok,
}

/// Identificador de ordem.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct OrderId(pub String);

impl From<String> for OrderId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for OrderId {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

impl std::fmt::Display for OrderId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Ordem enviada ao broker.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Order {
    pub id: Option<i64>,
    pub signal_id: Option<i64>,
    pub broker_order_id: Option<String>,
    pub parent_order_id: Option<i64>,
    pub symbol: String,
    pub side: OrderSide,
    pub order_type: OrderType,
    pub status: OrderStatus,
    pub time_in_force: TimeInForce,
    pub quantity: Decimal,
    pub filled_quantity: Decimal,
    pub price: Option<Decimal>,
    pub stop_price: Option<Decimal>,
    pub target_price: Option<Decimal>,
    pub avg_fill_price: Option<Decimal>,
    pub broker: String,
    pub error_message: Option<String>,
    pub metadata: serde_json::Value,
    pub correlation_id: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub submitted_at: Option<DateTime<Utc>>,
    pub filled_at: Option<DateTime<Utc>>,
    pub cancelled_at: Option<DateTime<Utc>>,
}

impl Order {
    pub fn new(
        symbol: impl Into<String>,
        side: OrderSide,
        order_type: OrderType,
        quantity: Decimal,
        broker: impl Into<String>,
    ) -> Result<Self, crate::ValidationError> {
        if quantity <= Decimal::ZERO {
            return Err(crate::ValidationError::InvalidQuantity(
                "quantidade deve ser positiva".to_string(),
            ));
        }
        let now = Utc::now();
        Ok(Self {
            id: None,
            signal_id: None,
            broker_order_id: None,
            parent_order_id: None,
            symbol: symbol.into(),
            side,
            order_type,
            status: OrderStatus::Pending,
            time_in_force: TimeInForce::Day,
            quantity,
            filled_quantity: Decimal::ZERO,
            price: None,
            stop_price: None,
            target_price: None,
            avg_fill_price: None,
            broker: broker.into(),
            error_message: None,
            metadata: serde_json::Value::Object(Default::default()),
            correlation_id: uuid::Uuid::new_v4().to_string(),
            created_at: now,
            updated_at: now,
            submitted_at: None,
            filled_at: None,
            cancelled_at: None,
        })
    }

    pub fn remaining_quantity(&self) -> Decimal {
        self.quantity - self.filled_quantity
    }

    pub fn is_filled(&self) -> bool {
        self.status == OrderStatus::Filled
    }
}

/// Execução parcial ou total de uma ordem.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Fill {
    pub id: Option<i64>,
    pub order_id: i64,
    pub symbol: String,
    pub fill_price: Decimal,
    pub quantity: Decimal,
    pub commission: Decimal,
    pub fees: Decimal,
    pub broker_fill_id: Option<String>,
    pub timestamp: DateTime<Utc>,
}

impl Fill {
    pub fn new(
        order_id: i64,
        symbol: impl Into<String>,
        fill_price: Decimal,
        quantity: Decimal,
        timestamp: DateTime<Utc>,
    ) -> Result<Self, crate::ValidationError> {
        if quantity <= Decimal::ZERO {
            return Err(crate::ValidationError::InvalidQuantity(
                "quantidade do fill deve ser positiva".to_string(),
            ));
        }
        Ok(Self {
            id: None,
            order_id,
            symbol: symbol.into(),
            fill_price,
            quantity,
            commission: Decimal::ZERO,
            fees: Decimal::ZERO,
            broker_fill_id: None,
            timestamp,
        })
    }
}
