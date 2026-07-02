use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::PgPool;

use trader_domain::{Order, OrderSide, OrderStatus, OrderType, RepositoryError, TimeInForce};

/// Implementação sqlx de repositório de ordens.
#[derive(Debug, Clone)]
pub struct SqlxOrderRepository {
    pool: PgPool,
}

impl SqlxOrderRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Salva uma ordem no banco.
    pub async fn save(&self, order: &Order) -> Result<i64, RepositoryError> {
        let asset_id = super::ensure_asset(&self.pool, &order.symbol).await?;
        let side = match order.side {
            OrderSide::Buy => "buy",
            OrderSide::Sell => "sell",
        };
        let order_type = match order.order_type {
            OrderType::Market => "market",
            OrderType::Limit => "limit",
            OrderType::Stop => "stop",
            OrderType::StopLimit => "stop_limit",
            OrderType::Bracket => "bracket",
        };
        let status = match order.status {
            OrderStatus::Pending => "pending",
            OrderStatus::Submitted => "submitted",
            OrderStatus::Accepted => "accepted",
            OrderStatus::PartiallyFilled => "partially_filled",
            OrderStatus::Filled => "filled",
            OrderStatus::Cancelled => "cancelled",
            OrderStatus::Rejected => "rejected",
            OrderStatus::Expired => "expired",
        };
        let tif = match order.time_in_force {
            TimeInForce::Day => "day",
            TimeInForce::Gtc => "gtc",
            TimeInForce::Ioc => "ioc",
            TimeInForce::Fok => "fok",
        };
        let correlation_id = uuid::Uuid::parse_str(&order.correlation_id)
            .map_err(|e| RepositoryError::InvalidData(format!("correlation_id inválido: {e}")))?;

        let id = sqlx::query_scalar!(
            r#"
            INSERT INTO orders (
                signal_id, asset_id, broker_order_id, parent_order_id, side, order_type, status,
                time_in_force, quantity, filled_quantity, price, stop_price, target_price, avg_fill_price,
                broker, error_message, metadata, correlation_id, created_at, updated_at,
                submitted_at, filled_at, cancelled_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21, $22, $23)
            RETURNING id
            "#,
            order.signal_id,
            asset_id,
            order.broker_order_id,
            order.parent_order_id,
            side,
            order_type,
            status,
            tif,
            order.quantity,
            order.filled_quantity,
            order.price,
            order.stop_price,
            order.target_price,
            order.avg_fill_price,
            order.broker,
            order.error_message,
            order.metadata,
            correlation_id,
            order.created_at,
            order.updated_at,
            order.submitted_at,
            order.filled_at,
            order.cancelled_at,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| RepositoryError::Query(e.to_string()))?;

        Ok(id)
    }

    /// Busca uma ordem pelo ID interno.
    pub async fn get_by_id(&self, id: i64) -> Result<Option<Order>, RepositoryError> {
        let row = sqlx::query_as!(
            OrderRow,
            r#"
            SELECT
                o.id,
                a.symbol,
                o.signal_id,
                o.broker_order_id,
                o.parent_order_id,
                o.side,
                o.order_type,
                o.status,
                o.time_in_force,
                o.quantity,
                o.filled_quantity,
                o.price,
                o.stop_price,
                o.target_price,
                o.avg_fill_price,
                o.broker,
                o.error_message,
                o.metadata as "metadata!: serde_json::Value",
                o.correlation_id,
                o.created_at,
                o.updated_at,
                o.submitted_at,
                o.filled_at,
                o.cancelled_at
            FROM orders o
            JOIN assets a ON a.id = o.asset_id
            WHERE o.id = $1
            "#,
            id
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| RepositoryError::Query(e.to_string()))?;

        Ok(row.map(Into::into))
    }

    /// Lista ordens abertas (não preenchidas totalmente nem canceladas).
    pub async fn list_open(&self) -> Result<Vec<Order>, RepositoryError> {
        let rows = sqlx::query_as!(
            OrderRow,
            r#"
            SELECT
                o.id,
                a.symbol,
                o.signal_id,
                o.broker_order_id,
                o.parent_order_id,
                o.side,
                o.order_type,
                o.status,
                o.time_in_force,
                o.quantity,
                o.filled_quantity,
                o.price,
                o.stop_price,
                o.target_price,
                o.avg_fill_price,
                o.broker,
                o.error_message,
                o.metadata as "metadata!: serde_json::Value",
                o.correlation_id,
                o.created_at,
                o.updated_at,
                o.submitted_at,
                o.filled_at,
                o.cancelled_at
            FROM orders o
            JOIN assets a ON a.id = o.asset_id
            WHERE o.status IN ('pending', 'submitted', 'accepted', 'partially_filled')
            ORDER BY o.created_at DESC
            "#
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| RepositoryError::Query(e.to_string()))?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    /// Lista ordens de um sinal.
    pub async fn list_by_signal(&self, signal_id: i64) -> Result<Vec<Order>, RepositoryError> {
        let rows = sqlx::query_as!(
            OrderRow,
            r#"
            SELECT
                o.id,
                a.symbol,
                o.signal_id,
                o.broker_order_id,
                o.parent_order_id,
                o.side,
                o.order_type,
                o.status,
                o.time_in_force,
                o.quantity,
                o.filled_quantity,
                o.price,
                o.stop_price,
                o.target_price,
                o.avg_fill_price,
                o.broker,
                o.error_message,
                o.metadata as "metadata!: serde_json::Value",
                o.correlation_id,
                o.created_at,
                o.updated_at,
                o.submitted_at,
                o.filled_at,
                o.cancelled_at
            FROM orders o
            JOIN assets a ON a.id = o.asset_id
            WHERE o.signal_id = $1
            ORDER BY o.created_at DESC
            "#,
            signal_id
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| RepositoryError::Query(e.to_string()))?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    /// Atualiza o status de uma ordem.
    pub async fn update_status(
        &self,
        id: i64,
        status: OrderStatus,
        filled_quantity: Option<Decimal>,
        avg_fill_price: Option<Decimal>,
    ) -> Result<(), RepositoryError> {
        let status_str = match status {
            OrderStatus::Pending => "pending",
            OrderStatus::Submitted => "submitted",
            OrderStatus::Accepted => "accepted",
            OrderStatus::PartiallyFilled => "partially_filled",
            OrderStatus::Filled => "filled",
            OrderStatus::Cancelled => "cancelled",
            OrderStatus::Rejected => "rejected",
            OrderStatus::Expired => "expired",
        };

        let now = Utc::now();

        sqlx::query!(
            r#"
            UPDATE orders
            SET status = $1,
                filled_quantity = COALESCE($2, filled_quantity),
                avg_fill_price = COALESCE($3, avg_fill_price),
                updated_at = $4,
                filled_at = CASE WHEN $1 = 'filled' THEN $4 ELSE filled_at END
            WHERE id = $5
            "#,
            status_str,
            filled_quantity,
            avg_fill_price,
            now,
            id
        )
        .execute(&self.pool)
        .await
        .map_err(|e| RepositoryError::Query(e.to_string()))?;

        Ok(())
    }
}

#[derive(Debug, Clone)]
struct OrderRow {
    id: i64,
    symbol: String,
    signal_id: Option<i64>,
    broker_order_id: Option<String>,
    parent_order_id: Option<i64>,
    side: String,
    order_type: String,
    status: String,
    time_in_force: String,
    quantity: Decimal,
    filled_quantity: Decimal,
    price: Option<Decimal>,
    stop_price: Option<Decimal>,
    target_price: Option<Decimal>,
    avg_fill_price: Option<Decimal>,
    broker: String,
    error_message: Option<String>,
    metadata: serde_json::Value,
    correlation_id: uuid::Uuid,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    submitted_at: Option<DateTime<Utc>>,
    filled_at: Option<DateTime<Utc>>,
    cancelled_at: Option<DateTime<Utc>>,
}

impl From<OrderRow> for Order {
    fn from(row: OrderRow) -> Self {
        Self {
            id: Some(row.id),
            symbol: row.symbol,
            signal_id: row.signal_id,
            broker_order_id: row.broker_order_id,
            parent_order_id: row.parent_order_id,
            side: if row.side == "sell" {
                OrderSide::Sell
            } else {
                OrderSide::Buy
            },
            order_type: match row.order_type.as_str() {
                "limit" => OrderType::Limit,
                "stop" => OrderType::Stop,
                "stop_limit" => OrderType::StopLimit,
                "bracket" => OrderType::Bracket,
                _ => OrderType::Market,
            },
            status: match row.status.as_str() {
                "submitted" => OrderStatus::Submitted,
                "accepted" => OrderStatus::Accepted,
                "partially_filled" => OrderStatus::PartiallyFilled,
                "filled" => OrderStatus::Filled,
                "cancelled" => OrderStatus::Cancelled,
                "rejected" => OrderStatus::Rejected,
                "expired" => OrderStatus::Expired,
                _ => OrderStatus::Pending,
            },
            time_in_force: match row.time_in_force.as_str() {
                "gtc" => TimeInForce::Gtc,
                "ioc" => TimeInForce::Ioc,
                "fok" => TimeInForce::Fok,
                _ => TimeInForce::Day,
            },
            quantity: row.quantity,
            filled_quantity: row.filled_quantity,
            price: row.price,
            stop_price: row.stop_price,
            target_price: row.target_price,
            avg_fill_price: row.avg_fill_price,
            broker: row.broker,
            error_message: row.error_message,
            metadata: row.metadata,
            correlation_id: row.correlation_id.to_string(),
            created_at: row.created_at,
            updated_at: row.updated_at,
            submitted_at: row.submitted_at,
            filled_at: row.filled_at,
            cancelled_at: row.cancelled_at,
        }
    }
}
