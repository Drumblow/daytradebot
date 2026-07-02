use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::PgPool;

use trader_domain::{Direction, RepositoryError, Signal, SignalStatus};

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
    pub async fn save(&self, signal: &Signal) -> Result<i64, RepositoryError> {
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
        let correlation_id = uuid::Uuid::parse_str(&signal.correlation_id)
            .map_err(|e| RepositoryError::InvalidData(format!("correlation_id inválido: {e}")))?;

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
        .map_err(|e| RepositoryError::Query(e.to_string()))?;

        Ok(id)
    }

    /// Busca um sinal pelo ID.
    pub async fn get_by_id(&self, id: i64) -> Result<Option<Signal>, RepositoryError> {
        let row = sqlx::query_as!(
            SignalRow,
            r#"
            SELECT
                s.id,
                a.symbol,
                s.strategy_id,
                s.strategy_version,
                s.config_hash,
                s.timeframe,
                s.timestamp,
                s.direction,
                s.status,
                s.entry_price,
                s.stop_price,
                s.target_price,
                s.risk_reward_ratio,
                s.risk_amount,
                s.risk_percent,
                s.position_size,
                s.entry_reason,
                s.rejection_reason,
                s.rejection_details as "rejection_details!: serde_json::Value",
                s.market_snapshot as "market_snapshot!: serde_json::Value",
                s.correlation_id
            FROM signals s
            JOIN assets a ON a.id = s.asset_id
            WHERE s.id = $1
            "#,
            id
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| RepositoryError::Query(e.to_string()))?;

        Ok(row.map(Into::into))
    }

    /// Lista sinais recentes de um ativo.
    pub async fn list_by_symbol(
        &self,
        symbol: &str,
        limit: i64,
    ) -> Result<Vec<Signal>, RepositoryError> {
        let rows = sqlx::query_as!(
            SignalRow,
            r#"
            SELECT
                s.id,
                a.symbol,
                s.strategy_id,
                s.strategy_version,
                s.config_hash,
                s.timeframe,
                s.timestamp,
                s.direction,
                s.status,
                s.entry_price,
                s.stop_price,
                s.target_price,
                s.risk_reward_ratio,
                s.risk_amount,
                s.risk_percent,
                s.position_size,
                s.entry_reason,
                s.rejection_reason,
                s.rejection_details as "rejection_details!: serde_json::Value",
                s.market_snapshot as "market_snapshot!: serde_json::Value",
                s.correlation_id
            FROM signals s
            JOIN assets a ON a.id = s.asset_id
            WHERE a.symbol = $1
            ORDER BY s.timestamp DESC
            LIMIT $2
            "#,
            symbol,
            limit
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| RepositoryError::Query(e.to_string()))?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    /// Lista sinais por status.
    pub async fn list_by_status(
        &self,
        status: SignalStatus,
        limit: i64,
    ) -> Result<Vec<Signal>, RepositoryError> {
        let status_str = match status {
            SignalStatus::Accepted => "accepted",
            SignalStatus::Rejected => "rejected",
            SignalStatus::Pending => "pending",
            SignalStatus::Expired => "expired",
        };

        let rows = sqlx::query_as!(
            SignalRow,
            r#"
            SELECT
                s.id,
                a.symbol,
                s.strategy_id,
                s.strategy_version,
                s.config_hash,
                s.timeframe,
                s.timestamp,
                s.direction,
                s.status,
                s.entry_price,
                s.stop_price,
                s.target_price,
                s.risk_reward_ratio,
                s.risk_amount,
                s.risk_percent,
                s.position_size,
                s.entry_reason,
                s.rejection_reason,
                s.rejection_details as "rejection_details!: serde_json::Value",
                s.market_snapshot as "market_snapshot!: serde_json::Value",
                s.correlation_id
            FROM signals s
            JOIN assets a ON a.id = s.asset_id
            WHERE s.status = $1
            ORDER BY s.timestamp DESC
            LIMIT $2
            "#,
            status_str,
            limit
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| RepositoryError::Query(e.to_string()))?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    /// Lista sinais de hoje.
    pub async fn list_today(&self, symbol: &str) -> Result<Vec<Signal>, RepositoryError> {
        let start = Utc::now().date_naive().and_hms_opt(0, 0, 0).unwrap();
        let start = start.and_utc();

        let rows = sqlx::query_as!(
            SignalRow,
            r#"
            SELECT
                s.id,
                a.symbol,
                s.strategy_id,
                s.strategy_version,
                s.config_hash,
                s.timeframe,
                s.timestamp,
                s.direction,
                s.status,
                s.entry_price,
                s.stop_price,
                s.target_price,
                s.risk_reward_ratio,
                s.risk_amount,
                s.risk_percent,
                s.position_size,
                s.entry_reason,
                s.rejection_reason,
                s.rejection_details as "rejection_details!: serde_json::Value",
                s.market_snapshot as "market_snapshot!: serde_json::Value",
                s.correlation_id
            FROM signals s
            JOIN assets a ON a.id = s.asset_id
            WHERE a.symbol = $1 AND s.timestamp >= $2
            ORDER BY s.timestamp DESC
            "#,
            symbol,
            start
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| RepositoryError::Query(e.to_string()))?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    /// Atualiza o status de um sinal.
    pub async fn update_status(
        &self,
        id: i64,
        status: SignalStatus,
    ) -> Result<(), RepositoryError> {
        let status_str = match status {
            SignalStatus::Accepted => "accepted",
            SignalStatus::Rejected => "rejected",
            SignalStatus::Pending => "pending",
            SignalStatus::Expired => "expired",
        };

        sqlx::query!(
            "UPDATE signals SET status = $1 WHERE id = $2",
            status_str,
            id
        )
        .execute(&self.pool)
        .await
        .map_err(|e| RepositoryError::Query(e.to_string()))?;

        Ok(())
    }
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct SignalRow {
    id: i64,
    symbol: String,
    strategy_id: String,
    strategy_version: String,
    config_hash: String,
    timeframe: String,
    timestamp: DateTime<Utc>,
    direction: Option<String>,
    status: String,
    entry_price: Option<Decimal>,
    stop_price: Option<Decimal>,
    target_price: Option<Decimal>,
    risk_reward_ratio: Option<Decimal>,
    risk_amount: Option<Decimal>,
    risk_percent: Option<Decimal>,
    position_size: Option<Decimal>,
    entry_reason: Option<String>,
    rejection_reason: Option<String>,
    rejection_details: serde_json::Value,
    market_snapshot: serde_json::Value,
    correlation_id: uuid::Uuid,
}

impl From<SignalRow> for Signal {
    fn from(row: SignalRow) -> Self {
        Self {
            symbol: row.symbol,
            strategy_id: row.strategy_id,
            strategy_version: row.strategy_version,
            config_hash: row.config_hash,
            timeframe: row
                .timeframe
                .parse()
                .unwrap_or(trader_domain::TimeFrame::M15),
            timestamp: row.timestamp,
            direction: if row.direction.as_deref() == Some("short") {
                trader_domain::Direction::Short
            } else {
                trader_domain::Direction::Long
            },
            status: match row.status.as_str() {
                "accepted" => SignalStatus::Accepted,
                "rejected" => SignalStatus::Rejected,
                "pending" => SignalStatus::Pending,
                _ => SignalStatus::Expired,
            },
            entry_price: row.entry_price,
            stop_price: row.stop_price,
            target_price: row.target_price,
            risk_reward_ratio: row.risk_reward_ratio,
            risk_amount: row.risk_amount,
            risk_percent: row.risk_percent,
            position_size: row.position_size,
            entry_reason: row.entry_reason,
            rejection_reason: row
                .rejection_reason
                .and_then(|r| serde_json::from_str(&format!("\"{r}\"")).ok()),
            rejection_details: Some(row.rejection_details),
            market_snapshot: row.market_snapshot,
            correlation_id: row.correlation_id.to_string(),
        }
    }
}
