use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::PgPool;

use trader_domain::{Candle, RepositoryError, TimeFrame};

use crate::ports::CandleRepository;

use super::{ensure_asset, parse_source, parse_timeframe};

/// Implementação sqlx de `CandleRepository`.
#[derive(Debug, Clone)]
pub struct SqlxCandleRepository {
    pool: PgPool,
}

impl SqlxCandleRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl CandleRepository for SqlxCandleRepository {
    async fn save(&self, candles: &[Candle]) -> Result<usize, RepositoryError> {
        let mut inserted = 0usize;

        for candle in candles {
            let asset_id = ensure_asset(&self.pool, &candle.symbol).await?;
            let timeframe = candle.timeframe.to_string();
            let source = match candle.source {
                trader_domain::DataSource::Ibkr => "ibkr",
                trader_domain::DataSource::Polygon => "polygon",
                trader_domain::DataSource::Manual => "manual",
                trader_domain::DataSource::Simulated => "simulated",
            };

            let result = sqlx::query!(
                r#"
                INSERT INTO candles (
                    asset_id, timeframe, timestamp, open, high, low, close, volume, vwap, source, is_complete
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
                ON CONFLICT (asset_id, timeframe, timestamp) DO NOTHING
                "#,
                asset_id,
                timeframe,
                candle.timestamp,
                candle.open,
                candle.high,
                candle.low,
                candle.close,
                candle.volume,
                candle.vwap,
                source,
                candle.is_complete,
            )
            .execute(&self.pool)
            .await
            .map_err(|e| RepositoryError::Query(e.to_string()))?;

            inserted += result.rows_affected() as usize;
        }

        Ok(inserted)
    }

    async fn get_range(
        &self,
        symbol: &str,
        timeframe: TimeFrame,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<Vec<Candle>, RepositoryError> {
        let rows = sqlx::query!(
            r#"
            SELECT
                a.symbol as symbol,
                c.timeframe as timeframe,
                c.timestamp as timestamp,
                c.open as open,
                c.high as high,
                c.low as low,
                c.close as close,
                c.volume as volume,
                c.vwap as vwap,
                c.source as source,
                c.is_complete as is_complete
            FROM candles c
            JOIN assets a ON a.id = c.asset_id
            WHERE a.symbol = $1 AND c.timeframe = $2 AND c.timestamp >= $3 AND c.timestamp <= $4
            ORDER BY c.timestamp ASC
            "#,
            symbol,
            timeframe.to_string(),
            from,
            to
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| RepositoryError::Query(e.to_string()))?;

        let mut candles = Vec::with_capacity(rows.len());
        for row in rows {
            candles.push(super::candle_from_row(
                row.symbol,
                parse_timeframe(&row.timeframe)?,
                row.timestamp,
                row.open,
                row.high,
                row.low,
                row.close,
                row.volume,
                row.vwap,
                parse_source(&row.source)?,
                row.is_complete,
            ));
        }

        Ok(candles)
    }

    async fn exists(
        &self,
        symbol: &str,
        timeframe: TimeFrame,
        timestamp: DateTime<Utc>,
    ) -> Result<bool, RepositoryError> {
        let row = sqlx::query!(
            r#"
            SELECT 1 as exists
            FROM candles c
            JOIN assets a ON a.id = c.asset_id
            WHERE a.symbol = $1 AND c.timeframe = $2 AND c.timestamp = $3
            "#,
            symbol,
            timeframe.to_string(),
            timestamp
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| RepositoryError::Query(e.to_string()))?;

        Ok(row.is_some())
    }
}
