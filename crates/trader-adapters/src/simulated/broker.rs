//! Broker simulado para testes, desenvolvimento e backtest.

use async_trait::async_trait;
use chrono::Utc;
use rust_decimal::Decimal;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc::Sender;
use tracing::{info, warn};

use trader_domain::market::{OrderEvent, SubscriptionHandle};
use trader_domain::{
    AccountSummary, Broker, BrokerError, Direction, ExitReason, Fill, Order, OrderId, OrderSide,
    OrderStatus, OrderType, Position, Trade,
};

/// Saídas pendentes associadas a uma posição aberta.
#[derive(Debug, Clone)]
struct PendingExit {
    stop_price: Decimal,
    target_price: Decimal,
}

/// Estado interno do broker simulado.
#[derive(Debug, Clone)]
struct SimulatedState {
    next_order_id: i64,
    orders: HashMap<OrderId, Order>,
    positions: HashMap<String, Position>,
    pending_exits: HashMap<String, PendingExit>,
    market_prices: HashMap<String, Decimal>,
    cash: Decimal,
    equity: Decimal,
    buying_power: Decimal,
    daily_pnl: Decimal,
    closed_trades: Vec<Trade>,
}

impl Default for SimulatedState {
    fn default() -> Self {
        Self {
            next_order_id: 0,
            orders: HashMap::new(),
            positions: HashMap::new(),
            pending_exits: HashMap::new(),
            market_prices: HashMap::new(),
            cash: Decimal::ZERO,
            equity: Decimal::ZERO,
            buying_power: Decimal::ZERO,
            daily_pnl: Decimal::ZERO,
            closed_trades: Vec::new(),
        }
    }
}

/// Configuração do broker simulado.
#[derive(Debug, Clone)]
pub struct SimulatedBrokerConfig {
    pub account_id: Option<String>,
    pub initial_cash: Decimal,
    pub commission_per_trade: Decimal,
    pub slippage_pct: Decimal,
}

impl Default for SimulatedBrokerConfig {
    fn default() -> Self {
        Self {
            account_id: Some("DU_SIM".to_string()),
            initial_cash: Decimal::from(100_000),
            commission_per_trade: Decimal::from(35) / Decimal::from(100), // $0.35
            slippage_pct: Decimal::from(1) / Decimal::from(1000),         // 0.1%
        }
    }
}

/// Broker em memória que imita respostas da Interactive Brokers.
///
/// - Ordens market geram fill imediato.
/// - Ordens bracket criam posição, stop loss e take profit pendentes.
/// - `set_market_price` executa stop/alvo quando o preço é alcançado.
/// - Rejeita nova posição no mesmo ativo se já existir posição aberta.
#[derive(Debug, Clone)]
pub struct SimulatedBroker {
    state: Arc<Mutex<SimulatedState>>,
    config: SimulatedBrokerConfig,
}

impl SimulatedBroker {
    pub fn new(config: SimulatedBrokerConfig) -> Self {
        let initial_cash = config.initial_cash;
        Self {
            state: Arc::new(Mutex::new(SimulatedState {
                cash: initial_cash,
                equity: initial_cash,
                buying_power: initial_cash,
                ..Default::default()
            })),
            config,
        }
    }

    /// Cria um broker simulado com configuração padrão.
    pub fn default_simulated() -> Self {
        Self::new(SimulatedBrokerConfig::default())
    }

    /// Define o preço de mercado atual para um ativo e executa stops/alvos.
    pub fn set_market_price(&self, symbol: &str, price: Decimal) {
        let mut state = match self.state.lock() {
            Ok(guard) => guard,
            Err(e) => {
                warn!(error = %e, "lock envenenado no broker simulado");
                return;
            }
        };

        state.market_prices.insert(symbol.to_string(), price);

        if let Some(position) = state.positions.get_mut(symbol) {
            let direction_multiplier = match position.direction {
                Direction::Long => Decimal::ONE,
                Direction::Short => -Decimal::ONE,
            };
            let quantity = position.quantity;
            position.unrealized_pnl =
                (price - position.avg_entry_price) * quantity * direction_multiplier;
            state.equity = state.cash + price * quantity * direction_multiplier;
        }

        if let Some(exit) = state.pending_exits.get(symbol).cloned() {
            let reason = if price <= exit.stop_price {
                Some(ExitReason::Stop)
            } else if price >= exit.target_price {
                Some(ExitReason::Target)
            } else {
                None
            };

            if let Some(reason) = reason {
                if let Some(position) = state.positions.remove(symbol) {
                    state.pending_exits.remove(symbol);
                    if let Some(trade) =
                        close_position_to_trade(&position, price, reason, &self.config)
                    {
                        // Recebe o valor da venda menos comissão de saída.
                        let exit_commission = trade.commissions / Decimal::from(2);
                        state.cash += price * position.quantity - exit_commission;
                        state.daily_pnl += trade.net_pnl;
                        state.equity = state.cash;
                        state.closed_trades.push(trade);
                    }
                }
            }
        }
    }

    /// Retorna as operações fechadas até o momento.
    pub fn get_closed_trades(&self) -> Vec<Trade> {
        match self.state.lock() {
            Ok(state) => state.closed_trades.clone(),
            Err(e) => {
                warn!(error = %e, "falha ao ler trades fechados");
                Vec::new()
            }
        }
    }

    /// Limpa o histórico de trades fechados.
    pub fn clear_closed_trades(&self) {
        if let Ok(mut state) = self.state.lock() {
            state.closed_trades.clear();
        }
    }

    fn next_id(state: &mut SimulatedState) -> OrderId {
        let id = OrderId::from(format!(
            "sim-{}-{}",
            Utc::now().timestamp_nanos_opt().unwrap_or(0),
            state.next_order_id
        ));
        state.next_order_id += 1;
        id
    }
}

#[async_trait]
impl Broker for SimulatedBroker {
    async fn place_order(&self, mut order: Order) -> Result<OrderId, BrokerError> {
        let mut state = self.state.lock().map_err(|e| {
            BrokerError::Internal(format!("erro ao adquirir lock do estado simulado: {e}"))
        })?;

        // Regra de segurança financeira: não abrir nova posição se já existir
        // posição aberta no mesmo ativo.
        if state.positions.contains_key(&order.symbol) {
            return Err(BrokerError::OrderRejected(format!(
                "já existe posição aberta em {}; não é permitido sobrepor posições",
                order.symbol
            )));
        }

        let id = Self::next_id(&mut state);

        order.broker_order_id = Some(id.to_string());
        order.status = OrderStatus::Submitted;
        order.submitted_at = Some(Utc::now());

        let base_price = order
            .price
            .or_else(|| state.market_prices.get(&order.symbol).copied())
            .unwrap_or_else(|| Decimal::from(100));

        let direction = match order.side {
            OrderSide::Buy => Direction::Long,
            OrderSide::Sell => Direction::Short,
        };

        // Aplica slippage contra o trader: entrada pior para long, melhor para short.
        let slippage_factor = Decimal::ONE + self.config.slippage_pct / Decimal::from(100);
        let fill_price = match direction {
            Direction::Long => base_price * slippage_factor,
            Direction::Short => base_price / slippage_factor,
        };

        let commission = self.config.commission_per_trade;

        match order.order_type {
            OrderType::Market | OrderType::Limit => {
                Fill::new(
                    order.id.unwrap_or(0),
                    &order.symbol,
                    fill_price,
                    order.quantity,
                    Utc::now(),
                )
                .map_err(|e| BrokerError::Internal(e.to_string()))?;

                let position = Position::new(
                    &order.symbol,
                    order.signal_id.unwrap_or(0),
                    direction,
                    order.quantity,
                    fill_price,
                    order.stop_price.unwrap_or(Decimal::ZERO),
                    "simulated",
                )
                .map_err(|e| BrokerError::Internal(e.to_string()))?;

                state.positions.insert(order.symbol.clone(), position);

                state.cash -= fill_price * order.quantity + commission;
                state.equity = state.cash + order.quantity * fill_price;

                order.status = OrderStatus::Filled;
                order.filled_quantity = order.quantity;
                order.avg_fill_price = Some(fill_price);
                order.filled_at = Some(Utc::now());
            }
            OrderType::Bracket => {
                let stop_price = order.stop_price.ok_or_else(|| {
                    BrokerError::OrderRejected("bracket order sem stop".to_string())
                })?;
                let target_price = order.target_price.ok_or_else(|| {
                    BrokerError::OrderRejected("bracket order sem alvo".to_string())
                })?;

                let position = Position::new(
                    &order.symbol,
                    order.signal_id.unwrap_or(0),
                    direction,
                    order.quantity,
                    fill_price,
                    stop_price,
                    "simulated",
                )
                .map_err(|e| BrokerError::Internal(e.to_string()))?;

                state.positions.insert(order.symbol.clone(), position);

                let pending_exit = PendingExit {
                    stop_price,
                    target_price,
                };
                state
                    .pending_exits
                    .insert(order.symbol.clone(), pending_exit);

                state.cash -= fill_price * order.quantity + commission;
                state.equity = state.cash + order.quantity * fill_price;

                order.status = OrderStatus::Filled;
                order.filled_quantity = order.quantity;
                order.avg_fill_price = Some(fill_price);
                order.filled_at = Some(Utc::now());
            }
            OrderType::Stop | OrderType::StopLimit => {
                return Err(BrokerError::OrderRejected(
                    "ordens stop/stop-limit isoladas não suportadas no simulador; use bracket"
                        .to_string(),
                ));
            }
        }

        state.orders.insert(id.clone(), order);
        info!(%id, "ordem simulada enviada");
        Ok(id)
    }

    async fn cancel_order(&self, id: &OrderId) -> Result<(), BrokerError> {
        let mut state = self.state.lock().map_err(|e| {
            BrokerError::Internal(format!("erro ao adquirir lock do estado simulado: {e}"))
        })?;

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
        let state = self.state.lock().map_err(|e| {
            BrokerError::Internal(format!("erro ao adquirir lock do estado simulado: {e}"))
        })?;

        state
            .orders
            .get(id)
            .map(|o| o.status)
            .ok_or_else(|| BrokerError::OrderNotFound(id.to_string()))
    }

    async fn get_open_orders(&self) -> Result<Vec<Order>, BrokerError> {
        let state = self.state.lock().map_err(|e| {
            BrokerError::Internal(format!("erro ao adquirir lock do estado simulado: {e}"))
        })?;

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
        let state = self.state.lock().map_err(|e| {
            BrokerError::Internal(format!("erro ao adquirir lock do estado simulado: {e}"))
        })?;

        Ok(state.positions.get(symbol).cloned())
    }

    async fn get_positions(&self) -> Result<Vec<Position>, BrokerError> {
        let state = self.state.lock().map_err(|e| {
            BrokerError::Internal(format!("erro ao adquirir lock do estado simulado: {e}"))
        })?;

        Ok(state.positions.values().cloned().collect())
    }

    async fn get_account_summary(&self) -> Result<AccountSummary, BrokerError> {
        let state = self.state.lock().map_err(|e| {
            BrokerError::Internal(format!("erro ao adquirir lock do estado simulado: {e}"))
        })?;

        Ok(AccountSummary {
            broker: "simulated".to_string(),
            account_id: self.config.account_id.clone(),
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

/// Fecha uma posição e gera o `Trade` correspondente.
fn close_position_to_trade(
    position: &Position,
    exit_price: Decimal,
    exit_reason: ExitReason,
    config: &SimulatedBrokerConfig,
) -> Option<Trade> {
    let direction_multiplier = match position.direction {
        Direction::Long => Decimal::ONE,
        Direction::Short => -Decimal::ONE,
    };

    let gross_pnl =
        (exit_price - position.avg_entry_price) * position.quantity * direction_multiplier;
    let commissions = config.commission_per_trade * Decimal::from(2); // entrada + saída
    let net_pnl = gross_pnl - commissions;

    let risk_amount = (position.avg_entry_price - position.stop_price).abs() * position.quantity;
    let result_in_r = if risk_amount.is_zero() {
        Decimal::ZERO
    } else {
        net_pnl / risk_amount
    };

    let strategy_id = position
        .metadata
        .get("strategy_id")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();
    let strategy_version = position
        .metadata
        .get("strategy_version")
        .and_then(|v| v.as_str())
        .unwrap_or("0.0.0")
        .to_string();
    let config_hash = position
        .metadata
        .get("config_hash")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();

    Some(Trade {
        id: None,
        symbol: position.symbol.clone(),
        signal_id: position.signal_id,
        position_id: position.id,
        direction: position.direction,
        entry_price: position.avg_entry_price,
        exit_price,
        quantity: position.quantity,
        entry_time: position.entry_time,
        exit_time: Utc::now(),
        stop_price: position.stop_price,
        target_price: position.target_price,
        gross_pnl,
        commissions,
        fees: Decimal::ZERO,
        net_pnl,
        risk_amount,
        result_in_r,
        exit_reason,
        strategy_id,
        strategy_version,
        config_hash,
        journal: serde_json::Value::Object(Default::default()),
        correlation_id: position.correlation_id.clone(),
    })
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

    fn bracket_order(symbol: &str, quantity: Decimal, stop: Decimal, target: Decimal) -> Order {
        let mut order = Order::new(
            symbol,
            OrderSide::Buy,
            OrderType::Bracket,
            quantity,
            "simulated",
        )
        .unwrap();
        order.price = Some(Decimal::from(100));
        order.stop_price = Some(stop);
        order.target_price = Some(target);
        order
    }

    #[tokio::test]
    async fn place_market_order_fills_immediately() {
        let broker = SimulatedBroker::default_simulated();
        let order = market_order("SPY", Decimal::from(10));

        let id = broker.place_order(order).await.unwrap();
        let status = broker.get_order_status(&id).await.unwrap();

        assert_eq!(status, OrderStatus::Filled);
    }

    #[tokio::test]
    async fn account_summary_reflects_initial_cash() {
        let broker = SimulatedBroker::new(SimulatedBrokerConfig {
            account_id: Some("DU_SIM".to_string()),
            initial_cash: Decimal::from(50_000),
            commission_per_trade: Decimal::ZERO,
            slippage_pct: Decimal::ZERO,
        });
        let summary = broker.get_account_summary().await.unwrap();

        assert_eq!(summary.cash, Decimal::from(50_000));
        assert_eq!(summary.broker, "simulated");
    }

    #[tokio::test]
    async fn rejects_second_position_in_same_symbol() {
        let broker = SimulatedBroker::default_simulated();
        let first = market_order("SPY", Decimal::from(10));
        broker.place_order(first).await.unwrap();
        // slippage default não afeta o teste.

        let second = market_order("SPY", Decimal::from(5));
        let result = broker.place_order(second).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn bracket_order_hits_stop_loss() {
        let broker = SimulatedBroker::new(SimulatedBrokerConfig {
            account_id: Some("DU_SIM".to_string()),
            initial_cash: Decimal::from(100_000),
            commission_per_trade: Decimal::ZERO,
            slippage_pct: Decimal::ZERO,
        });

        let order = bracket_order(
            "SPY",
            Decimal::from(10),
            Decimal::from(95),
            Decimal::from(110),
        );
        broker.place_order(order).await.unwrap();

        assert!(broker.get_position("SPY").await.unwrap().is_some());

        broker.set_market_price("SPY", Decimal::from(94));

        assert!(broker.get_position("SPY").await.unwrap().is_none());
        let trades = broker.get_closed_trades();
        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].exit_reason, ExitReason::Stop);
    }

    #[tokio::test]
    async fn bracket_order_hits_take_profit() {
        let broker = SimulatedBroker::new(SimulatedBrokerConfig {
            account_id: Some("DU_SIM".to_string()),
            initial_cash: Decimal::from(100_000),
            commission_per_trade: Decimal::ZERO,
            slippage_pct: Decimal::ZERO,
        });

        let order = bracket_order(
            "SPY",
            Decimal::from(10),
            Decimal::from(95),
            Decimal::from(110),
        );
        broker.place_order(order).await.unwrap();

        broker.set_market_price("SPY", Decimal::from(111));

        let trades = broker.get_closed_trades();
        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].exit_reason, ExitReason::Target);
        assert!(trades[0].net_pnl > Decimal::ZERO);
    }
}
