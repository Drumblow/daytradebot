//! Ports (contratos) do domínio.
//!
//! Este módulo contém as traits que isolam o domínio de provedores externos.
//! Elas dependem de `async` porque envolvem I/O (broker, market data, repositórios),
//! mas vivem no crate `trader-domain` para garantir que adapters e infraestrutura
//! dependam do domínio, e não o contrário (Ports & Adapters).

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use tokio::sync::mpsc::Sender;

use crate::{
    AccountSummary, Candle, CandleRequest, DataError, Order, OrderEvent, OrderId, OrderStatus,
    Position, ProviderHealth, Quote, RepositoryError, SubscriptionHandle, TimeFrame,
};

/// Abstração de relógio para testes determinísticos.
pub trait Clock: Send + Sync {
    fn now(&self) -> DateTime<Utc>;
}

/// Porta para provedores de dados de mercado.
#[async_trait]
pub trait MarketDataProvider: Send + Sync {
    async fn get_historical_candles(
        &self,
        request: CandleRequest,
    ) -> Result<Vec<Candle>, DataError>;

    async fn subscribe_realtime_bars(
        &self,
        symbol: &str,
        timeframe: TimeFrame,
        tx: Sender<Candle>,
    ) -> Result<SubscriptionHandle, DataError>;

    async fn get_quote(&self, symbol: &str) -> Result<Quote, DataError>;

    async fn health_check(&self) -> Result<ProviderHealth, DataError>;
}

/// Porta para brokers (execução de ordens).
#[async_trait]
pub trait Broker: Send + Sync {
    async fn place_order(&self, order: Order) -> Result<OrderId, crate::BrokerError>;
    async fn cancel_order(&self, id: &OrderId) -> Result<(), crate::BrokerError>;
    async fn get_order_status(&self, id: &OrderId) -> Result<OrderStatus, crate::BrokerError>;
    async fn get_open_orders(&self) -> Result<Vec<Order>, crate::BrokerError>;
    async fn get_position(&self, symbol: &str) -> Result<Option<Position>, crate::BrokerError>;
    async fn get_positions(&self) -> Result<Vec<Position>, crate::BrokerError>;
    async fn get_account_summary(&self) -> Result<AccountSummary, crate::BrokerError>;
    async fn subscribe_order_events(
        &self,
        tx: Sender<OrderEvent>,
    ) -> Result<SubscriptionHandle, crate::BrokerError>;
}

/// Porta para repositório de candles.
#[async_trait]
pub trait CandleRepository: Send + Sync {
    async fn save(&self, candles: &[Candle]) -> Result<usize, RepositoryError>;

    async fn get_range(
        &self,
        symbol: &str,
        timeframe: TimeFrame,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<Vec<Candle>, RepositoryError>;

    async fn exists(
        &self,
        symbol: &str,
        timeframe: TimeFrame,
        timestamp: DateTime<Utc>,
    ) -> Result<bool, RepositoryError>;
}
