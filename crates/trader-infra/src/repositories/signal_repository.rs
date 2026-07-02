use sqlx::PgPool;

use trader_domain::{Direction, Signal, SignalStatus};

/// Implementação sqlx de repositório de sinais.
#[derive(Debug, Clone)]
pub struct SqlxSignalRepository {
    pool: PgPool,
}

impl SqlxSignalRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Salva um sinal no banco.
    pub async fn save(&self, signal: &Signal) -> Result<i64, trader_domain::RepositoryError> {
        let asset_id = super::ensure_asset(&self.pool, &signal.symbol).await?;
        let direction = match signal.direction {
            Direction::Long => "long",
            Direction::Short => "short",
        };
        let status = match signal.status {
            SignalStatus::Accepted => "accepted",
            SignalStatus::Rejected => "rejected",
            SignalStatus::Pending => "pending",
            SignalStatus::Expired => "expired",
        };
        let rejection_reason = signal.rejection_reason.map(|r| format!("{:?}", r));
        let correlation_id =
            uuid::Uuid::parse_str(&signal.correlation_id).unwrap_or_else(|_| uuid::Uuid::new_v4());

        let id = sqlx::query_scalar!(
            r#"
            INSERT INTO signals (
                asset_id, strategy_id, strategy_version, config_hash, timeframe, timestamp,
                direction, status, entry_price, stop_price, target_price, risk_reward_ratio,
                risk_amount, risk_percent, position_size, entry_reason, rejection_reason,
                rejection_details, market_snapshot, correlation_id
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20)
            RETURNING id
            "#,
            asset_id,
            signal.strategy_id,
            signal.strategy_version,
            signal.config_hash,
            signal.timeframe.to_string(),
            signal.timestamp,
            direction,
            status,
            signal.entry_price,
            signal.stop_price,
            signal.target_price,
            signal.risk_reward_ratio,
            signal.risk_amount,
            signal.risk_percent,
            signal.position_size,
            signal.entry_reason,
            rejection_reason,
            signal.rejection_details,
            signal.market_snapshot,
            correlation_id,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| trader_domain::RepositoryError::Query(e.to_string()))?;

        Ok(id)
    }
}
