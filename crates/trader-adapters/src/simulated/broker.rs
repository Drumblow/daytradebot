//! Broker simulado para testes e desenvolvimento.

use async_trait::async_trait;
use chrono::Utc;
use rust_decimal::Decimal;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc::Sender;
use tracing::{info, warn};

use trader_domain::market::{OrderEvent, SubscriptionHandle};
use trader_domain::{
    AccountSummary, BrokerError, Direction, Fill, Order, OrderId, OrderSide, OrderStatus,
    OrderType, Position, PositionStatus,
};
use trader_infra::ports::Broker;

/// Estado interno do broker simulado.
#[derive(Debug, Default, Clone)]
struct SimulatedState {
    next_order_id: i64,
    orders: HashMap<OrderId, Order>,
    positions: HashMap<String, Position>,
    cash: Decimal,
    equity: Decimal,
    buying_power: Decimal,
    daily_pnl: Decimal,
}

/// Broker em memória que imita respostas da Interactive Brokers.
///
/// - Ordens são aceitas imediatamente.
/// - Market orders geram fill total no último preço conhecido.
/// - Limit orders só preenchem se o preço for alcançável (simplificado).
/// - Mantém saldo, posições e equity em memória.
#[derive(Debug, Clone)]
pub struct SimulatedBroker {
    state: Arc<Mutex<SimulatedState>>,
    account_id: Option<String>,
}

impl SimulatedBroker {
    pub fn new(account_id: Option<String>, initial_cash: Decimal) -> Self {
        Self {
            state: Arc::new(Mutex::new(SimulatedState {
                cash: initial_cash,
                equity: initial_cash,
                buying_power: initial_cash,
                ..Default::default()
            })),
            account_id,
        }
    }

    /// Define o preço de mercado atual para um ativo (usado em fills futuros).
    pub fn set_market_price(&self, symbol: &str, price: Decimal) {
        let mut state = self.state.lock().unwrap();
        if let Some(position) = state.positions.get_mut(symbol) {
            let direction_multiplier = match position.direction {
                Direction::Long => Decimal::ONE,
                Direction::Short => -Decimal::ONE,
            };
            position.unrealized_pnl =
                (price - position.avg_entry_price) * position.quantity * direction_multiplier;
        }
    }
}

#[async_trait]
impl Broker for SimulatedBroker {
    async fn place_order(&self, mut order: Order) -> Result<OrderId, BrokerError> {
        let mut state = self.state.lock().unwrap();
        let id = OrderId::from(format!(
            "sim-{}-{}",
            Utc::now().timestamp_nanos_opt().unwrap_or(0),
            state.next_order_id
        ));
        state.next_order_id += 1;

        order.broker_order_id = Some(id.to_string());
        order.status = OrderStatus::Submitted;
        order.submitted_at = Some(Utc::now());

        // Fill imediato simplificado para market orders.
        if order.order_type == OrderType::Market {
            order.status = OrderStatus::Filled;
            order.filled_quantity = order.quantity;
            order.avg_fill_price = order.price;
            order.filled_at = Some(Utc::now());

            let fill_price = order.price.unwrap_or(Decimal::from(100));
            let commission = Decimal::from_f64_retain(0.35).unwrap_or_default();

            let _fill = Fill::new(
                order.id.unwrap_or(0),
                &order.symbol,
                fill_price,
                order.quantity,
                Utc::now(),
            )
            .map_err(|e| BrokerError::Internal(e.to_string()))?;

            // Atualiza ou cria posição.
            let direction = match order.side {
                OrderSide::Buy => Direction::Long,
                OrderSide::Sell => Direction::Short,
            };

            if state.positions.contains_key(&order.symbol) {
                let position = state.positions.get_mut(&order.symbol).unwrap();
                position.quantity += order.quantity;
                position.avg_entry_price = fill_price;
                position.status = PositionStatus::Open;
            } else {
                let position = Position::new(
                    &order.symbol,
                    order.signal_id.unwrap_or(0),
                    direction,
                    order.quantity,
                    fill_price,
                    Decimal::ZERO,
                    "simulated",
                )
                .expect("posição simulada válida");
                state.positions.insert(order.symbol.clone(), position);
            }

            state.cash -= fill_price * order.quantity + commission;
            let position_qty = state.positions.get(&order.symbol).unwrap().quantity;
            state.equity = state.cash + position_qty * fill_price;
        }

        state.orders.insert(id.clone(), order);
        info!(%id, "ordem simulada enviada");
        Ok(id)
    }

    async fn cancel_order(&self, id: &OrderId) -> Result<(), BrokerError> {
        let mut state = self.state.lock().unwrap();
        let order = state
            .orders
            .get_mut(id)
            .ok_or_else(|| BrokerError::OrderNotFound(id.to_string()))?;

        if order.is_filled() {
            return Err(BrokerError::OrderRejected(format!(
                "ordem {id} já preenchida"
            )));
        }

        order.status = OrderStatus::Cancelled;
        order.cancelled_at = Some(Utc::now());
        info!(%id, "ordem simulada cancelada");
        Ok(())
    }

    async fn get_order_status(&self, id: &OrderId) -> Result<OrderStatus, BrokerError> {
        let state = self.state.lock().unwrap();
        state
            .orders
            .get(id)
            .map(|o| o.status)
            .ok_or_else(|| BrokerError::OrderNotFound(id.to_string()))
    }

    async fn get_open_orders(&self) -> Result<Vec<Order>, BrokerError> {
        let state = self.state.lock().unwrap();
        Ok(state
            .orders
            .values()
            .filter(|o| {
                o.status == OrderStatus::Submitted
                    || o.status == OrderStatus::Accepted
                    || o.status == OrderStatus::PartiallyFilled
            })
            .cloned()
            .collect())
    }

    async fn get_position(&self, symbol: &str) -> Result<Option<Position>, BrokerError> {
        let state = self.state.lock().unwrap();
        Ok(state.positions.get(symbol).cloned())
    }

    async fn get_positions(&self) -> Result<Vec<Position>, BrokerError> {
        let state = self.state.lock().unwrap();
        Ok(state.positions.values().cloned().collect())
    }

    async fn get_account_summary(&self) -> Result<AccountSummary, BrokerError> {
        let state = self.state.lock().unwrap();
        Ok(AccountSummary {
            broker: "simulated".to_string(),
            account_id: self.account_id.clone(),
            cash: state.cash,
            equity: state.equity,
            buying_power: state.buying_power,
            daily_pnl: state.daily_pnl,
            timestamp: Utc::now(),
        })
    }

    async fn subscribe_order_events(
        &self,
        _tx: Sender<OrderEvent>,
    ) -> Result<SubscriptionHandle, BrokerError> {
        warn!("subscribe_order_events simulado: não envia eventos");
        Ok(SubscriptionHandle {
            id: "simulated-orders".to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn market_order(symbol: &str, quantity: Decimal) -> Order {
        Order::new(
            symbol,
            OrderSide::Buy,
            OrderType::Market,
            quantity,
            "simulated",
        )
        .unwrap()
    }

    #[tokio::test]
    async fn place_market_order_fills_immediately() {
        let broker = SimulatedBroker::new(Some("DU_SIM".to_string()), Decimal::from(100_000));
        let order = market_order("SPY", Decimal::from(10));

        let id = broker.place_order(order).await.unwrap();
        let status = broker.get_order_status(&id).await.unwrap();

        assert_eq!(status, OrderStatus::Filled);
    }

    #[tokio::test]
    async fn account_summary_reflects_initial_cash() {
        let broker = SimulatedBroker::new(Some("DU_SIM".to_string()), Decimal::from(50_000));
        let summary = broker.get_account_summary().await.unwrap();

        assert_eq!(summary.cash, Decimal::from(50_000));
        assert_eq!(summary.broker, "simulated");
    }
}
