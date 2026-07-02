use sqlx::PgPool;

use trader_domain::{Direction, Trade};

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
    pub async fn save(&self, trade: &Trade) -> Result<i64, trader_domain::RepositoryError> {
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
        let correlation_id =
            uuid::Uuid::parse_str(&trade.correlation_id).unwrap_or_else(|_| uuid::Uuid::new_v4());

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
        .map_err(|e| trader_domain::RepositoryError::Query(e.to_string()))?;

        Ok(id)
    }
}
