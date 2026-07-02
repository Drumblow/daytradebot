use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::PgPool;

use trader_domain::{Direction, RepositoryError, Trade};

/// Implementação sqlx de repositório de trades.
#[derive(Debug, Clone)]
pub struct SqlxTradeRepository {
    pool: PgPool,
}

impl SqlxTradeRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Salva um trade no banco.
    pub async fn save(&self, trade: &Trade) -> Result<i64, RepositoryError> {
        let asset_id = super::ensure_asset(&self.pool, &trade.symbol).await?;
        let direction = match trade.direction {
            Direction::Long => "long",
            Direction::Short => "short",
        };
        let exit_reason = match trade.exit_reason {
            trader_domain::ExitReason::Target => "target",
            trader_domain::ExitReason::Stop => "stop",
            trader_domain::ExitReason::Time => "time",
            trader_domain::ExitReason::Manual => "manual",
            trader_domain::ExitReason::RiskManager => "risk_manager",
        };
        let correlation_id = uuid::Uuid::parse_str(&trade.correlation_id)
            .map_err(|e| RepositoryError::InvalidData(format!("correlation_id inválido: {e}")))?;

        let id = sqlx::query_scalar!(
            r#"
            INSERT INTO trades (
                asset_id, signal_id, position_id, direction, entry_price, exit_price, quantity,
                entry_time, exit_time, stop_price, target_price, gross_pnl, commissions, fees,
                net_pnl, risk_amount, result_in_r, exit_reason, strategy_id, strategy_version,
                config_hash, journal, correlation_id
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21, $22, $23)
            RETURNING id
            "#,
            asset_id,
            trade.signal_id,
            trade.position_id,
            direction,
            trade.entry_price,
            trade.exit_price,
            trade.quantity,
            trade.entry_time,
            trade.exit_time,
            trade.stop_price,
            trade.target_price,
            trade.gross_pnl,
            trade.commissions,
            trade.fees,
            trade.net_pnl,
            trade.risk_amount,
            trade.result_in_r,
            exit_reason,
            trade.strategy_id,
            trade.strategy_version,
            trade.config_hash,
            trade.journal,
            correlation_id,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| RepositoryError::Query(e.to_string()))?;

        Ok(id)
    }

    /// Busca um trade pelo ID.
    pub async fn get_by_id(&self, id: i64) -> Result<Option<Trade>, RepositoryError> {
        let row = sqlx::query_as!(
            TradeRow,
            r#"
            SELECT
                t.id,
                a.symbol,
                t.signal_id,
                t.position_id,
                t.direction,
                t.entry_price,
                t.exit_price,
                t.quantity,
                t.entry_time,
                t.exit_time,
                t.stop_price,
                t.target_price,
                t.gross_pnl,
                t.commissions,
                t.fees,
                t.net_pnl,
                t.risk_amount,
                t.result_in_r,
                t.exit_reason,
                t.strategy_id,
                t.strategy_version,
                t.config_hash,
                t.journal as "journal!: serde_json::Value",
                t.correlation_id
            FROM trades t
            JOIN assets a ON a.id = t.asset_id
            WHERE t.id = $1
            "#,
            id
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| RepositoryError::Query(e.to_string()))?;

        Ok(row.map(Into::into))
    }

    /// Lista trades recentes de um ativo.
    pub async fn list_by_symbol(
        &self,
        symbol: &str,
        limit: i64,
    ) -> Result<Vec<Trade>, RepositoryError> {
        let rows = sqlx::query_as!(
            TradeRow,
            r#"
            SELECT
                t.id,
                a.symbol,
                t.signal_id,
                t.position_id,
                t.direction,
                t.entry_price,
                t.exit_price,
                t.quantity,
                t.entry_time,
                t.exit_time,
                t.stop_price,
                t.target_price,
                t.gross_pnl,
                t.commissions,
                t.fees,
                t.net_pnl,
                t.risk_amount,
                t.result_in_r,
                t.exit_reason,
                t.strategy_id,
                t.strategy_version,
                t.config_hash,
                t.journal as "journal!: serde_json::Value",
                t.correlation_id
            FROM trades t
            JOIN assets a ON a.id = t.asset_id
            WHERE a.symbol = $1
            ORDER BY t.exit_time DESC
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

    /// Lista trades de hoje.
    pub async fn list_today(&self, symbol: &str) -> Result<Vec<Trade>, RepositoryError> {
        let start = Utc::now().date_naive().and_hms_opt(0, 0, 0).unwrap();
        let start = start.and_utc();

        let rows = sqlx::query_as!(
            TradeRow,
            r#"
            SELECT
                t.id,
                a.symbol,
                t.signal_id,
                t.position_id,
                t.direction,
                t.entry_price,
                t.exit_price,
                t.quantity,
                t.entry_time,
                t.exit_time,
                t.stop_price,
                t.target_price,
                t.gross_pnl,
                t.commissions,
                t.fees,
                t.net_pnl,
                t.risk_amount,
                t.result_in_r,
                t.exit_reason,
                t.strategy_id,
                t.strategy_version,
                t.config_hash,
                t.journal as "journal!: serde_json::Value",
                t.correlation_id
            FROM trades t
            JOIN assets a ON a.id = t.asset_id
            WHERE a.symbol = $1 AND t.exit_time >= $2
            ORDER BY t.exit_time DESC
            "#,
            symbol,
            start
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| RepositoryError::Query(e.to_string()))?;

        Ok(rows.into_iter().map(Into::into).collect())
    }
}

#[derive(Debug, Clone)]
struct TradeRow {
    id: i64,
    symbol: String,
    signal_id: i64,
    position_id: Option<i64>,
    direction: String,
    entry_price: Decimal,
    exit_price: Decimal,
    quantity: Decimal,
    entry_time: DateTime<Utc>,
    exit_time: DateTime<Utc>,
    stop_price: Decimal,
    target_price: Option<Decimal>,
    gross_pnl: Decimal,
    commissions: Decimal,
    fees: Decimal,
    net_pnl: Decimal,
    risk_amount: Decimal,
    result_in_r: Decimal,
    exit_reason: String,
    strategy_id: String,
    strategy_version: String,
    config_hash: String,
    journal: serde_json::Value,
    correlation_id: uuid::Uuid,
}

impl From<TradeRow> for Trade {
    fn from(row: TradeRow) -> Self {
        Self {
            id: Some(row.id),
            symbol: row.symbol,
            signal_id: row.signal_id,
            position_id: row.position_id,
            direction: if row.direction == "short" {
                Direction::Short
            } else {
                Direction::Long
            },
            entry_price: row.entry_price,
            exit_price: row.exit_price,
            quantity: row.quantity,
            entry_time: row.entry_time,
            exit_time: row.exit_time,
            stop_price: row.stop_price,
            target_price: row.target_price,
            gross_pnl: row.gross_pnl,
            commissions: row.commissions,
            fees: row.fees,
            net_pnl: row.net_pnl,
            risk_amount: row.risk_amount,
            result_in_r: row.result_in_r,
            exit_reason: match row.exit_reason.as_str() {
                "stop" => trader_domain::ExitReason::Stop,
                "time" => trader_domain::ExitReason::Time,
                "manual" => trader_domain::ExitReason::Manual,
                "risk_manager" => trader_domain::ExitReason::RiskManager,
                _ => trader_domain::ExitReason::Target,
            },
            strategy_id: row.strategy_id,
            strategy_version: row.strategy_version,
            config_hash: row.config_hash,
            journal: row.journal,
            correlation_id: row.correlation_id.to_string(),
        }
    }
}
