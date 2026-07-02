use sqlx::PgPool;

use trader_domain::{Order, OrderSide, OrderStatus, OrderType, TimeInForce};

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
    pub async fn save(&self, order: &Order) -> Result<i64, trader_domain::RepositoryError> {
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
        let correlation_id =
            uuid::Uuid::parse_str(&order.correlation_id).unwrap_or_else(|_| uuid::Uuid::new_v4());

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
        .map_err(|e| trader_domain::RepositoryError::Query(e.to_string()))?;

        Ok(id)
    }
}
