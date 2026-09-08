//! Snapshots da conta — equity, caixa e poder de compra (ADR-020 §5).
//!
//! A tabela `account_snapshots` existe desde a migração inicial
//! (`0001_initial_schema.sql:278`) e **nada nunca escreveu nela**. O número
//! que dimensiona todas as posições do projeto — a equity da conta paper,
//! ≈ US$ 237.892 — só existia numa linha do `HANDOFF.md`, escrita à mão em
//! algum dia de agosto. O sizing lê a equity do broker a cada sinal e a
//! joga fora; o painel (ADR-014) não tem de onde tirar patrimônio.
//!
//! `buying_power` é lido do broker desde sempre e **não entra no sizing** —
//! aqui ele é registrado, não usado: numa conta paper com margem ele é um
//! múltiplo da equity e não diz nada sobre a liquidez do ativo (ADR-020,
//! "alternativas rejeitadas").

use rust_decimal::Decimal;
use sqlx::PgPool;

use trader_domain::RepositoryError;

/// Implementação sqlx do registro de snapshots da conta.
#[derive(Debug, Clone)]
pub struct SqlxAccountSnapshotRepository {
    pool: PgPool,
}

impl SqlxAccountSnapshotRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Grava um snapshot. Idempotente por (broker, conta, timestamp): a
    /// tabela tem UNIQUE nessa tripla, e um restart no mesmo segundo não
    /// pode derrubar o registro do dia.
    ///
    /// `metadata` carrega o que a tabela não tem coluna para guardar — a
    /// **moeda** da conta, em primeiro lugar. O cap de notional trata
    /// `NET_LIQUIDATION` como dólares; se a conta paper for em CAD, o teto de
    /// 1× está ≈ 1,37× errado desde sempre, e hoje ninguém tem como saber
    /// porque o adapter descarta a moeda.
    pub async fn save(&self, snapshot: &AccountSnapshotRecord) -> Result<(), RepositoryError> {
        sqlx::query!(
            r#"
            INSERT INTO account_snapshots
                (broker, account_id, timestamp, cash, equity, buying_power, daily_pnl, metadata)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            ON CONFLICT (broker, account_id, timestamp) DO NOTHING
            "#,
            snapshot.broker,
            snapshot.account_id,
            snapshot.timestamp,
            snapshot.cash,
            snapshot.equity,
            snapshot.buying_power,
            snapshot.daily_pnl,
            snapshot.metadata,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| RepositoryError::Query(e.to_string()))?;

        Ok(())
    }

    /// Último snapshot conhecido de um broker. Serve ao painel e ao
    /// diagnóstico: "de que equity o bot estava dimensionando naquele dia?".
    pub async fn latest(
        &self,
        broker: &str,
    ) -> Result<Option<AccountSnapshotRecord>, RepositoryError> {
        let row = sqlx::query!(
            r#"
            SELECT broker, account_id, timestamp, cash, equity, buying_power,
                   daily_pnl, metadata
            FROM account_snapshots
            WHERE broker = $1
            ORDER BY timestamp DESC
            LIMIT 1
            "#,
            broker
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| RepositoryError::Query(e.to_string()))?;

        Ok(row.map(|r| AccountSnapshotRecord {
            broker: r.broker,
            account_id: r.account_id,
            timestamp: r.timestamp,
            cash: r.cash,
            equity: r.equity,
            buying_power: r.buying_power,
            daily_pnl: r.daily_pnl,
            metadata: r.metadata,
        }))
    }
}

/// Uma linha de `account_snapshots`.
#[derive(Debug, Clone)]
pub struct AccountSnapshotRecord {
    pub broker: String,
    pub account_id: Option<String>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub cash: Decimal,
    pub equity: Decimal,
    pub buying_power: Decimal,
    pub daily_pnl: Decimal,
    pub metadata: serde_json::Value,
}
