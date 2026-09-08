use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::PgPool;

use trader_domain::RepositoryError;

/// Registro de uma execução de backtest para persistência.
#[derive(Debug, Clone)]
pub struct BacktestRunRecord {
    pub symbol: String,
    pub strategy_id: String,
    pub strategy_version: String,
    pub config_hash: String,
    pub timeframe: String,
    pub period_start: DateTime<Utc>,
    pub period_end: DateTime<Utc>,
    pub initial_capital: Decimal,
    pub final_equity: Decimal,
    pub metrics: serde_json::Value,
    pub label: Option<String>,
}

/// Implementação sqlx de repositório de runs de backtest.
#[derive(Debug, Clone)]
pub struct SqlxBacktestRunRepository {
    pool: PgPool,
}

impl SqlxBacktestRunRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Persiste um run de backtest e retorna o id.
    pub async fn save(&self, run: &BacktestRunRecord) -> Result<i64, RepositoryError> {
        let asset_id = super::ensure_asset(&self.pool, &run.symbol).await?;

        let id = sqlx::query_scalar!(
            r#"
            INSERT INTO backtest_runs (
                asset_id, strategy_id, strategy_version, config_hash, timeframe,
                period_start, period_end, initial_capital, final_equity, metrics, label
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            RETURNING id
            "#,
            asset_id,
            run.strategy_id,
            run.strategy_version,
            run.config_hash,
            run.timeframe,
            run.period_start,
            run.period_end,
            run.initial_capital,
            run.final_equity,
            run.metrics,
            run.label,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| RepositoryError::Query(e.to_string()))?;

        Ok(id)
    }

    /// Run mais recente **não experimental** de uma estratégia num par
    /// específico e com um `config_hash` específico (ADR-019 §3).
    ///
    /// Existe porque `latest_by_strategy` filtra só por `strategy_id` e pega o
    /// mais recente em QUALQUER símbolo e QUALQUER config: a primeira ablação
    /// rodada vira o baseline do gate B, em silêncio. Runs com override são
    /// marcados `experimental` no jsonb `metrics` e ficam de fora.
    ///
    /// Não substitui `latest_by_strategy` de propósito — o método antigo
    /// continua servindo a quem quiser "o último run, qualquer que seja".
    ///
    /// `commission_model` fecha a mesma porta pelo outro lado (§5.6). Em
    /// 08/09/2026 o simulador passou a cobrar a tabela por ação da IBKR, 9,9×
    /// o modelo fixo antigo (medido nos 214 trades OOS); o banco tem os dois. Sem este filtro, o run mais
    /// recente pode ser um `--legacy-cost` rodado para paridade, e o gate B
    /// compararia o paper (que paga comissão real) com um backtest que não
    /// paga — o live pareceria pior por um motivo que não é a estratégia.
    /// Run sem o campo é anterior ao §5.6 e **não casa com nada**: falha
    /// fechado, como o `experimental`.
    ///
    /// `slippage_bps` e `limit_fill_haircut_bps` fecham os dois eixos que
    /// sobravam. Nenhum dos dois marca o run como `experimental` — o
    /// `--slippage-bps` nunca marcou —, e o ADR-018 manda rodar
    /// `--slippage-bps 4` em IJS e SLYV para sensibilidade: esse run entraria
    /// no banco não-experimental, mais recente, e viraria o baseline do gate B
    /// da produção, que roda a 2 bp. Medido sobre os trades reais, 2 bp a mais
    /// derrubam o avg R de IJS de 0,305 para 0,160 — deslocamento maior que a
    /// banda inteira de ±30% do gate B.
    pub async fn latest_for(
        &self,
        strategy_id: &str,
        symbol: &str,
        config_hash: &str,
        commission_model: &str,
        slippage_bps: &str,
        limit_fill_haircut_bps: &str,
    ) -> Result<Option<StoredBacktestRun>, RepositoryError> {
        let row = sqlx::query_as!(
            StoredRunRow,
            r#"
            SELECT
                r.id,
                a.symbol,
                r.strategy_version,
                r.config_hash,
                r.timeframe,
                r.period_start,
                r.period_end,
                r.final_equity,
                r.metrics as "metrics!: serde_json::Value",
                r.label,
                r.created_at
            FROM backtest_runs r
            JOIN assets a ON a.id = r.asset_id
            WHERE r.strategy_id = $1
              AND a.symbol = $2
              AND r.config_hash = $3
              AND COALESCE((r.metrics ->> 'experimental')::boolean, false) = false
              AND r.metrics ->> 'commission_model' = $4
              AND r.metrics ->> 'slippage_bps' = $5
              AND r.metrics ->> 'limit_fill_haircut_bps' = $6
            ORDER BY r.created_at DESC
            LIMIT 1
            "#,
            strategy_id,
            symbol,
            config_hash,
            commission_model,
            slippage_bps,
            limit_fill_haircut_bps
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| RepositoryError::Query(e.to_string()))?;

        Ok(row.map(Into::into))
    }

    /// Retorna o run mais recente de uma estratégia (qualquer label).
    pub async fn latest_by_strategy(
        &self,
        strategy_id: &str,
    ) -> Result<Option<StoredBacktestRun>, RepositoryError> {
        let row = sqlx::query_as!(
            StoredRunRow,
            r#"
            SELECT
                r.id,
                a.symbol,
                r.strategy_version,
                r.config_hash,
                r.timeframe,
                r.period_start,
                r.period_end,
                r.final_equity,
                r.metrics as "metrics!: serde_json::Value",
                r.label,
                r.created_at
            FROM backtest_runs r
            JOIN assets a ON a.id = r.asset_id
            WHERE r.strategy_id = $1
            ORDER BY r.created_at DESC
            LIMIT 1
            "#,
            strategy_id
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| RepositoryError::Query(e.to_string()))?;

        Ok(row.map(Into::into))
    }
}

/// Run de backtest lido do banco.
#[derive(Debug, Clone)]
pub struct StoredBacktestRun {
    pub id: i64,
    pub symbol: String,
    pub strategy_version: String,
    pub config_hash: String,
    pub timeframe: String,
    pub period_start: DateTime<Utc>,
    pub period_end: DateTime<Utc>,
    pub final_equity: Decimal,
    pub metrics: serde_json::Value,
    pub label: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
struct StoredRunRow {
    id: i64,
    symbol: String,
    strategy_version: String,
    config_hash: String,
    timeframe: String,
    period_start: DateTime<Utc>,
    period_end: DateTime<Utc>,
    final_equity: Decimal,
    metrics: serde_json::Value,
    label: Option<String>,
    created_at: DateTime<Utc>,
}

impl From<StoredRunRow> for StoredBacktestRun {
    fn from(row: StoredRunRow) -> Self {
        Self {
            id: row.id,
            symbol: row.symbol,
            strategy_version: row.strategy_version,
            config_hash: row.config_hash,
            timeframe: row.timeframe,
            period_start: row.period_start,
            period_end: row.period_end,
            final_equity: row.final_equity,
            metrics: row.metrics,
            label: row.label,
            created_at: row.created_at,
        }
    }
}
