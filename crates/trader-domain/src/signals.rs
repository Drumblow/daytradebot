use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// Direção de uma operação ou posição.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    Long,
    Short,
}

impl Direction {
    pub fn opposite(&self) -> Self {
        match self {
            Direction::Long => Direction::Short,
            Direction::Short => Direction::Long,
        }
    }
}

/// Status de um sinal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignalStatus {
    /// Sinal aceito e pronto para execução.
    Accepted,
    /// Sinal rejeitado por alguma regra.
    Rejected,
    /// Sinal pendente de confirmação (ex: próximo candle).
    Pending,
    /// Sinal expirado antes da execução.
    Expired,
}

/// Motivo de rejeição de um sinal ou ordem.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RejectionReason {
    NoContext,
    MarketLateral,
    HighVolatility,
    LowVolatility,
    PoorRiskReward,
    HighSpread,
    OutsideTradingHours,
    DailyLossLimitReached,
    MaxTradesReached,
    ConsecutiveLosses,
    IncompleteSetup,
    WeakConfirmation,
    PositionAlreadyOpen,
    StopMissing,
    InvalidQuantity,
    BrokerError,
    Unknown,
}

/// Resultado da análise de uma estratégia.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[allow(clippy::large_enum_variant)]
pub enum SignalResult {
    None,
    Signal(Signal),
    Rejected {
        reason: RejectionReason,
        details: Option<serde_json::Value>,
    },
}

/// Sinal de entrada gerado por uma estratégia.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Signal {
    pub symbol: String,
    pub strategy_id: String,
    pub strategy_version: String,
    pub config_hash: String,
    pub timeframe: crate::TimeFrame,
    pub timestamp: DateTime<Utc>,
    pub direction: Direction,
    pub status: SignalStatus,

    pub entry_price: Option<Decimal>,
    pub stop_price: Option<Decimal>,
    pub target_price: Option<Decimal>,
    pub risk_reward_ratio: Option<Decimal>,

    pub risk_amount: Option<Decimal>,
    pub risk_percent: Option<Decimal>,
    pub position_size: Option<Decimal>,

    pub entry_reason: Option<String>,
    pub rejection_reason: Option<RejectionReason>,
    pub rejection_details: Option<serde_json::Value>,

    pub market_snapshot: serde_json::Value,
    pub correlation_id: String,
}

impl Signal {
    /// Cria um sinal aceito.
    pub fn accepted(
        symbol: impl Into<String>,
        strategy_id: impl Into<String>,
        strategy_version: impl Into<String>,
        config_hash: impl Into<String>,
        timeframe: crate::TimeFrame,
        timestamp: DateTime<Utc>,
        direction: Direction,
    ) -> Self {
        Self {
            symbol: symbol.into(),
            strategy_id: strategy_id.into(),
            strategy_version: strategy_version.into(),
            config_hash: config_hash.into(),
            timeframe,
            timestamp,
            direction,
            status: SignalStatus::Accepted,
            entry_price: None,
            stop_price: None,
            target_price: None,
            risk_reward_ratio: None,
            risk_amount: None,
            risk_percent: None,
            position_size: None,
            entry_reason: None,
            rejection_reason: None,
            rejection_details: None,
            market_snapshot: serde_json::Value::Object(Default::default()),
            correlation_id: uuid::Uuid::new_v4().to_string(),
        }
    }

    /// Cria um sinal rejeitado.
    #[allow(clippy::too_many_arguments)]
    pub fn rejected(
        symbol: impl Into<String>,
        strategy_id: impl Into<String>,
        strategy_version: impl Into<String>,
        config_hash: impl Into<String>,
        timeframe: crate::TimeFrame,
        timestamp: DateTime<Utc>,
        direction: Option<Direction>,
        reason: RejectionReason,
        details: Option<serde_json::Value>,
    ) -> Self {
        Self {
            symbol: symbol.into(),
            strategy_id: strategy_id.into(),
            strategy_version: strategy_version.into(),
            config_hash: config_hash.into(),
            timeframe,
            timestamp,
            direction: direction.unwrap_or(Direction::Long),
            status: SignalStatus::Rejected,
            entry_price: None,
            stop_price: None,
            target_price: None,
            risk_reward_ratio: None,
            risk_amount: None,
            risk_percent: None,
            position_size: None,
            entry_reason: None,
            rejection_reason: Some(reason),
            rejection_details: details,
            market_snapshot: serde_json::Value::Object(Default::default()),
            correlation_id: uuid::Uuid::new_v4().to_string(),
        }
    }
}
