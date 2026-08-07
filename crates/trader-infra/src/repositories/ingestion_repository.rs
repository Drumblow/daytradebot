//! Implementação sqlx de repositório de ingestões.

use chrono::{DateTime, Utc};
use sqlx::PgPool;

use trader_domain::RepositoryError;

/// Registro de uma execução de ingestão de candles.
#[derive(Debug, Clone)]
pub struct IngestionRecord {
    pub symbol: String,
    pub timeframe: String,
    pub source: String,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub candles_inserted: i32,
    pub gaps_detected: i32,
    pub status: String,
    pub error_message: Option<String>,
}

/// Implementação sqlx de repositório de ingestões.
#[derive(Debug, Clone)]
pub struct SqlxIngestionRepository {
    pool: PgPool,
}

impl SqlxIngestionRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Registra uma ingestão concluída (ou falha) e retorna o id.
    pub async fn save(&self, rec: &IngestionRecord) -> Result<i64, RepositoryError> {
        let asset_id = super::ensure_asset(&self.pool, &rec.symbol).await?;

        let id = sqlx::query_scalar!(
            r#"
            INSERT INTO ingestions (
                asset_id, timeframe, source, start_time, end_time,
                candles_inserted, gaps_detected, status, error_message, finished_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, NOW())
            RETURNING id
            "#,
            asset_id,
            rec.timeframe,
            rec.source,
            rec.start_time,
            rec.end_time,
            rec.candles_inserted,
            rec.gaps_detected,
            rec.status,
            rec.error_message,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| RepositoryError::Query(e.to_string()))?;

        Ok(id)
    }
}
