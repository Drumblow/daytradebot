//! Ports de infraestrutura (`async`).
//!
//! Essas traits vivem em `trader-infra` porque dependem de operações async
//! e, eventualmente, de SQL, HTTP ou conectividade com corretoras.
//! O domínio (`trader-domain`) permanece puro e sem dependência de runtime async.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use tokio::sync::mpsc::Sender;

use trader_domain::{
    AccountSummary, Candle, CandleRequest, DataError, Order, OrderEvent, OrderId, OrderStatus,
    Position, ProviderHealth, Quote, RepositoryError, SubscriptionHandle, TimeFrame,
};

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
    async fn place_order(&self, order: Order) -> Result<OrderId, trader_domain::BrokerError>;
    async fn cancel_order(&self, id: &OrderId) -> Result<(), trader_domain::BrokerError>;
    async fn get_order_status(
        &self,
        id: &OrderId,
    ) -> Result<OrderStatus, trader_domain::BrokerError>;
    async fn get_open_orders(&self) -> Result<Vec<Order>, trader_domain::BrokerError>;
    async fn get_position(
        &self,
        symbol: &str,
    ) -> Result<Option<Position>, trader_domain::BrokerError>;
    async fn get_positions(&self) -> Result<Vec<Position>, trader_domain::BrokerError>;
    async fn get_account_summary(&self) -> Result<AccountSummary, trader_domain::BrokerError>;
    async fn subscribe_order_events(
        &self,
        tx: Sender<OrderEvent>,
    ) -> Result<SubscriptionHandle, trader_domain::BrokerError>;
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
