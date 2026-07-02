use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::{MarketContext, SignalResult};

/// Identificador versionado de uma estratégia.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StrategyId {
    pub id: String,
    pub version: String,
}

impl StrategyId {
    pub fn new(id: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            version: version.into(),
        }
    }

    pub fn full(&self) -> String {
        format!("{}@{}", self.id, self.version)
    }
}

/// Configuração genérica de uma estratégia (carregada de TOML/JSON).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StrategyConfig {
    pub strategy_id: String,
    pub version: String,
    pub source: String,
    pub parameters: serde_json::Value,
}

/// Estado interno de uma estratégia entre candles.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct StrategyState {
    pub last_signal_id: Option<i64>,
    pub consecutive_losses: i32,
    pub daily_trades: i32,
    pub daily_pnl: Decimal,
}

/// Contrato mínimo de uma estratégia.
pub trait Strategy: Send + Sync {
    fn id(&self) -> StrategyId;
    fn name(&self) -> &'static str;
    fn source(&self) -> &'static str;
    fn version(&self) -> &'static str;
    fn analyze(&self, ctx: &MarketContext, state: &StrategyState) -> SignalResult;
}
