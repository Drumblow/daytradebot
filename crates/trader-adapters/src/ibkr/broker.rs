//! Broker adapter para Interactive Brokers.

use async_trait::async_trait;
use chrono::Utc;
use futures::StreamExt;
use ibapi::prelude::*;
use rust_decimal::Decimal;
use tokio::sync::mpsc::Sender;
use tracing::{debug, info, warn};

use trader_domain::market::{OrderEvent, SubscriptionHandle};
use trader_domain::{
    AccountSummary, BrokerError, Order, OrderId, OrderSide, OrderStatus, OrderType, Position,
};
use trader_infra::ports::Broker;

use super::config::IbkrConfig;

/// Adapter concreto de broker para Interactive Brokers.
#[derive(Debug, Clone)]
pub struct IbkrBrokerAdapter {
    config: IbkrConfig,
}

impl IbkrBrokerAdapter {
    pub fn new(config: IbkrConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Broker for IbkrBrokerAdapter {
    async fn place_order(&self, order: Order) -> Result<OrderId, BrokerError> {
        info!(symbol = %order.symbol, side = ?order.side, qty = %order.quantity, "enviando ordem para IBKR");

        let connection_string = self.config.connection_string();
        let client = Client::connect(&connection_string, self.config.client_id)
            .await
            .map_err(|e| BrokerError::ConnectionFailed(e.to_string()))?;

        let contract = Contract::stock(&order.symbol).build();
        let quantity = f64_from_decimal(order.quantity)?;

        let result = match order.order_type {
            OrderType::Market => match order.side {
                OrderSide::Buy => {
                    client
                        .order(&contract)
                        .buy(quantity)
                        .market()
                        .submit()
                        .await
                }
                OrderSide::Sell => {
                    client
                        .order(&contract)
                        .sell(quantity)
                        .market()
                        .submit()
                        .await
                }
            },
            OrderType::Limit => {
                let price: f64 = order
                    .price
                    .and_then(|p| p.try_into().ok())
                    .ok_or_else(|| BrokerError::OrderRejected("preço limit ausente".to_string()))?;
                match order.side {
                    OrderSide::Buy => {
                        client
                            .order(&contract)
                            .buy(quantity)
                            .limit(price)
                            .submit()
                            .await
                    }
                    OrderSide::Sell => {
                        client
                            .order(&contract)
                            .sell(quantity)
                            .limit(price)
                            .submit()
                            .await
                    }
                }
            }
            OrderType::Stop => {
                let stop: f64 = order
                    .stop_price
                    .and_then(|p| p.try_into().ok())
                    .ok_or_else(|| BrokerError::OrderRejected("stop ausente".to_string()))?;
                match order.side {
                    OrderSide::Buy => {
                        client
                            .order(&contract)
                            .buy(quantity)
                            .stop(stop)
                            .submit()
                            .await
                    }
                    OrderSide::Sell => {
                        client
                            .order(&contract)
                            .sell(quantity)
                            .stop(stop)
                            .submit()
                            .await
                    }
                }
            }
            OrderType::Bracket => {
                let entry_price: f64 =
                    order.price.and_then(|p| p.try_into().ok()).ok_or_else(|| {
                        BrokerError::OrderRejected("preço de entrada ausente".to_string())
                    })?;
                let stop_price: f64 = order
                    .stop_price
                    .and_then(|p| p.try_into().ok())
                    .ok_or_else(|| BrokerError::OrderRejected("stop ausente".to_string()))?;
                let take_profit: f64 = order
                    .target_price
                    .and_then(|p| p.try_into().ok())
                    .ok_or_else(|| BrokerError::OrderRejected("alvo ausente".to_string()))?;

                match order.side {
                    OrderSide::Buy => client
                        .order(&contract)
                        .buy(quantity)
                        .bracket()
                        .entry_limit(entry_price)
                        .stop_loss(stop_price)
                        .take_profit(take_profit)
                        .submit_all()
                        .await
                        .map(|ids| ids.parent),
                    OrderSide::Sell => client
                        .order(&contract)
                        .sell(quantity)
                        .bracket()
                        .entry_limit(entry_price)
                        .stop_loss(stop_price)
                        .take_profit(take_profit)
                        .submit_all()
                        .await
                        .map(|ids| ids.parent),
                }
            }
            OrderType::StopLimit => {
                return Err(BrokerError::OrderRejected(
                    "stop-limit não suportado ainda".to_string(),
                ))
            }
        };

        result
            .map(|broker_id| OrderId::from(broker_id.to_string()))
            .map_err(|e| BrokerError::OrderRejected(e.to_string()))
    }

    async fn cancel_order(&self, id: &OrderId) -> Result<(), BrokerError> {
        info!(%id, "cancelando ordem na IBKR");

        let connection_string = self.config.connection_string();
        let client = Client::connect(&connection_string, self.config.client_id)
            .await
            .map_err(|e| BrokerError::ConnectionFailed(e.to_string()))?;

        let numeric_id: i32 =
            id.0.parse()
                .map_err(|e| BrokerError::Internal(format!("ID inválido: {e}")))?;

        let mut subscription = client
            .cancel_order(numeric_id, "")
            .await
            .map_err(|e| BrokerError::Internal(e.to_string()))?;

        // Consome a subscrição para confirmar cancelamento.
        while let Some(item) = subscription.next().await {
            match item {
                Ok(SubscriptionItem::Data(_)) => {}
                Ok(SubscriptionItem::Notice(n)) => {
                    warn!(notice = %n, "aviso no cancelamento");
                }
                Err(e) => {
                    return Err(BrokerError::Internal(e.to_string()));
                }
            }
        }

        Ok(())
    }

    async fn get_order_status(&self, id: &OrderId) -> Result<OrderStatus, BrokerError> {
        debug!(%id, "consultando status da ordem na IBKR");

        let open_orders = self.get_open_orders().await?;
        let order = open_orders
            .iter()
            .find(|o| o.broker_order_id.as_deref() == Some(&id.0))
            .cloned();

        match order {
            Some(o) => Ok(o.status),
            None => Ok(OrderStatus::Filled),
        }
    }

    async fn get_open_orders(&self) -> Result<Vec<Order>, BrokerError> {
        warn!("get_open_orders: requer validação manual com conta IBKR liberada");
        Ok(Vec::new())
    }

    async fn get_position(&self, symbol: &str) -> Result<Option<Position>, BrokerError> {
        let positions = self.get_positions().await?;
        Ok(positions.into_iter().find(|p| p.symbol == symbol))
    }

    async fn get_positions(&self) -> Result<Vec<Position>, BrokerError> {
        warn!("get_positions: requer validação manual com conta IBKR liberada");
        Ok(Vec::new())
    }

    async fn get_account_summary(&self) -> Result<AccountSummary, BrokerError> {
        warn!("get_account_summary: requer validação manual com conta IBKR liberada");
        Ok(AccountSummary {
            broker: "ibkr".to_string(),
            account_id: self.config.account_id.clone(),
            cash: Decimal::ZERO,
            equity: Decimal::ZERO,
            buying_power: Decimal::ZERO,
            daily_pnl: Decimal::ZERO,
            timestamp: Utc::now(),
        })
    }

    async fn subscribe_order_events(
        &self,
        _tx: Sender<OrderEvent>,
    ) -> Result<SubscriptionHandle, BrokerError> {
        warn!("subscribe_order_events: requer validação manual com conta IBKR liberada");
        Ok(SubscriptionHandle {
            id: "ibkr-orders-stub".to_string(),
        })
    }
}

fn f64_from_decimal(value: Decimal) -> Result<f64, BrokerError> {
    value
        .try_into()
        .map_err(|e| BrokerError::Internal(format!("falha ao converter Decimal: {e}")))
}
