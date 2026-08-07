use sqlx::PgPool;

use trader_domain::{Fill, OrderSide, RepositoryError};

/// Implementação sqlx de repositório de fills.
#[derive(Debug, Clone)]
pub struct SqlxFillRepository {
    pool: PgPool,
}

impl SqlxFillRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Salva um fill no banco.
    ///
    /// Idempotente por `broker_fill_id`: se o fill já existir (replay de
    /// execuções do dia, restart do processo), retorna `Ok(None)` sem
    /// inserir. O chamador usa isso para não contar P&L em dobro.
    pub async fn save(&self, fill: &Fill) -> Result<Option<i64>, RepositoryError> {
        let asset_id = super::ensure_asset(&self.pool, &fill.symbol).await?;
        let side = match fill.side {
            OrderSide::Buy => "buy",
            OrderSide::Sell => "sell",
        };

        let id = sqlx::query_scalar!(
            r#"
            INSERT INTO fills (
                order_id, asset_id, side, fill_price, quantity, commission, fees,
                broker_fill_id, timestamp
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            ON CONFLICT DO NOTHING
            RETURNING id
            "#,
            fill.order_id,
            asset_id,
            side,
            fill.fill_price,
            fill.quantity,
            fill.commission,
            fill.fees,
            fill.broker_fill_id,
            fill.timestamp,
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| RepositoryError::Query(e.to_string()))?;

        Ok(id)
    }

    /// Lista fills de uma ordem, em ordem cronológica.
    ///
    /// Usado na recuperação de sessão (restart do live) para reconstruir o
    /// estado do tracker de fills.
    pub async fn list_by_order(&self, order_id: i64) -> Result<Vec<Fill>, RepositoryError> {
        let rows = sqlx::query_as!(
            FillRow,
            r#"
            SELECT
                f.id,
                f.order_id,
                a.symbol,
                f.side,
                f.fill_price,
                f.quantity,
                f.commission,
                f.fees,
                f.broker_fill_id,
                f.timestamp
            FROM fills f
            JOIN assets a ON a.id = f.asset_id
            WHERE f.order_id = $1
            ORDER BY f.timestamp ASC
            "#,
            order_id
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| RepositoryError::Query(e.to_string()))?;

        Ok(rows.into_iter().map(Into::into).collect())
    }
}

#[derive(Debug, Clone)]
struct FillRow {
    id: i64,
    order_id: i64,
    symbol: String,
    side: String,
    fill_price: rust_decimal::Decimal,
    quantity: rust_decimal::Decimal,
    commission: rust_decimal::Decimal,
    fees: rust_decimal::Decimal,
    broker_fill_id: Option<String>,
    timestamp: chrono::DateTime<chrono::Utc>,
}

impl From<FillRow> for Fill {
    fn from(row: FillRow) -> Self {
        Self {
            id: Some(row.id),
            order_id: row.order_id,
            symbol: row.symbol,
            side: if row.side == "sell" {
                OrderSide::Sell
            } else {
                OrderSide::Buy
            },
            fill_price: row.fill_price,
            quantity: row.quantity,
            commission: row.commission,
            fees: row.fees,
            broker_fill_id: row.broker_fill_id,
            timestamp: row.timestamp,
        }
    }
}
