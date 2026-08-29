//! Consultas do painel — todas somente leitura.
//!
//! Aqui é `sqlx::query_as` em runtime, não os macros `query!` usados em
//! `trader-infra`: o painel não deve exigir um Postgres de pé para COMPILAR
//! (o job de CI dele builda sem serviço de banco), e as consultas são de
//! agregação/leitura, sem os invariantes de escrita que os macros protegem.
//!
//! Enums do Postgres (`order_status` etc.) são sempre lidos com `::text` —
//! o painel não precisa conhecer os tipos, só exibi-los.

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::Serialize;
use sqlx::{FromRow, PgPool};

/// Fuso oficial da operação (janela de pregão, agregações diárias).
pub const ET: &str = "America/New_York";

// ── Overview ────────────────────────────────────────────────────────────────

#[derive(Debug, Default, FromRow, Serialize)]
pub struct TradeTotals {
    pub trades: i64,
    pub wins: i64,
    pub net_pnl: Decimal,
    pub sum_r: Decimal,
}

pub async fn trade_totals(pool: &PgPool) -> sqlx::Result<TradeTotals> {
    sqlx::query_as(
        r#"
        SELECT count(*)                                   AS trades,
               count(*) FILTER (WHERE net_pnl > 0)        AS wins,
               coalesce(sum(net_pnl), 0)                  AS net_pnl,
               coalesce(sum(result_in_r), 0)              AS sum_r
        FROM trades
        "#,
    )
    .fetch_one(pool)
    .await
}

pub async fn trade_totals_today(pool: &PgPool) -> sqlx::Result<TradeTotals> {
    sqlx::query_as(
        r#"
        SELECT count(*)                                   AS trades,
               count(*) FILTER (WHERE net_pnl > 0)        AS wins,
               coalesce(sum(net_pnl), 0)                  AS net_pnl,
               coalesce(sum(result_in_r), 0)              AS sum_r
        FROM trades
        WHERE (exit_time AT TIME ZONE $1)::date = (now() AT TIME ZONE $1)::date
        "#,
    )
    .bind(ET)
    .fetch_one(pool)
    .await
}

#[derive(Debug, Default, FromRow, Serialize)]
pub struct SignalCounts {
    pub accepted: i64,
    pub rejected: i64,
}

pub async fn signal_counts_today(pool: &PgPool) -> sqlx::Result<SignalCounts> {
    sqlx::query_as(
        r#"
        SELECT count(*) FILTER (WHERE status = 'accepted') AS accepted,
               count(*) FILTER (WHERE status = 'rejected') AS rejected
        FROM signals
        WHERE ("timestamp" AT TIME ZONE $1)::date = (now() AT TIME ZONE $1)::date
        "#,
    )
    .bind(ET)
    .fetch_one(pool)
    .await
}

#[derive(Debug, Default, FromRow, Serialize)]
pub struct OpenOrderCounts {
    /// Ordens em status aberto criadas hoje (ET) — podem estar de fato vivas.
    pub today: i64,
    /// Ordens em status aberto de dias anteriores. Como toda ordem do bot é
    /// TIF `day`, isso é sempre artefato de reconciliação (ordem que o bot
    /// nunca confirmou/cancelou no banco), não uma ordem viva no broker.
    pub stale: i64,
}

pub async fn open_order_counts(pool: &PgPool) -> sqlx::Result<OpenOrderCounts> {
    sqlx::query_as(
        r#"
        SELECT count(*) FILTER (WHERE (created_at AT TIME ZONE $1)::date
                                      = (now() AT TIME ZONE $1)::date) AS today,
               count(*) FILTER (WHERE (created_at AT TIME ZONE $1)::date
                                      < (now() AT TIME ZONE $1)::date) AS stale
        FROM orders
        WHERE status::text IN ('pending', 'submitted', 'accepted', 'partially_filled')
        "#,
    )
    .bind(ET)
    .fetch_one(pool)
    .await
}

pub async fn last_candle_at(pool: &PgPool) -> sqlx::Result<Option<DateTime<Utc>>> {
    sqlx::query_scalar(r#"SELECT max("timestamp") FROM candles"#)
        .fetch_one(pool)
        .await
}

#[derive(Debug, FromRow, Serialize)]
pub struct EventRow {
    pub id: i64,
    pub timestamp: DateTime<Utc>,
    pub level: String,
    pub component: String,
    pub event_type: String,
    pub message: String,
}

pub async fn last_error_event(pool: &PgPool) -> sqlx::Result<Option<EventRow>> {
    sqlx::query_as(
        r#"
        SELECT id, "timestamp", level::text AS level, component, event_type, message
        FROM system_events
        WHERE level::text IN ('error', 'critical')
        ORDER BY "timestamp" DESC
        LIMIT 1
        "#,
    )
    .fetch_optional(pool)
    .await
}

// ── Séries para gráficos ────────────────────────────────────────────────────

#[derive(Debug, FromRow, Serialize)]
pub struct EquityPoint {
    pub exit_time: DateTime<Utc>,
    pub net_pnl: Decimal,
    pub symbol: String,
    pub strategy_id: String,
}

pub async fn equity_points(pool: &PgPool) -> sqlx::Result<Vec<EquityPoint>> {
    sqlx::query_as(
        r#"
        SELECT t.exit_time, t.net_pnl, a.symbol, t.strategy_id
        FROM trades t
        JOIN assets a ON a.id = t.asset_id
        ORDER BY t.exit_time
        "#,
    )
    .fetch_all(pool)
    .await
}

#[derive(Debug, FromRow, Serialize)]
pub struct DailyPnl {
    pub day: NaiveDate,
    pub net_pnl: Decimal,
    pub trades: i64,
    pub wins: i64,
}

pub async fn pnl_daily(pool: &PgPool, days: i64) -> sqlx::Result<Vec<DailyPnl>> {
    sqlx::query_as(
        r#"
        SELECT (exit_time AT TIME ZONE $1)::date            AS day,
               sum(net_pnl)                                 AS net_pnl,
               count(*)                                     AS trades,
               count(*) FILTER (WHERE net_pnl > 0)          AS wins
        FROM trades
        GROUP BY 1
        ORDER BY 1 DESC
        LIMIT $2
        "#,
    )
    .bind(ET)
    .bind(days)
    .fetch_all(pool)
    .await
}

// ── Listagens ───────────────────────────────────────────────────────────────

#[derive(Debug, FromRow, Serialize)]
pub struct TradeRow {
    pub id: i64,
    pub symbol: String,
    pub strategy_id: String,
    pub direction: String,
    pub entry_time: DateTime<Utc>,
    pub exit_time: DateTime<Utc>,
    pub entry_price: Decimal,
    pub exit_price: Decimal,
    pub quantity: Decimal,
    pub net_pnl: Decimal,
    /// R gravado pelo bot no fechamento. Trades anteriores a 2026-08-29
    /// usavam o ORÇAMENTO de risco como denominador (diluído pelo cap de
    /// notional); a partir do fix do risk_amount, coincide com o R real.
    pub result_in_r: Decimal,
    /// R contra o risco REAL da posição (distância do stop × quantidade):
    /// um stop cheio ≈ −1R aqui, um alvo 2R ≈ +2R. É a métrica que confere
    /// com o desenho da estratégia.
    pub real_r: Option<Decimal>,
    pub exit_reason: String,
    /// Marcado no journal: trade-artefato do dia 1 (latência) — fora da
    /// amostra de validação do gate B.
    pub latency_artifact: bool,
    /// Marcado no journal: sinal calculado sobre dados degradados (semana
    /// 07–14/08) — fora da amostra de validação do gate B.
    pub data_quality_suspect: bool,
}

pub async fn trades(pool: &PgPool, limit: i64) -> sqlx::Result<Vec<TradeRow>> {
    sqlx::query_as(
        r#"
        SELECT t.id, a.symbol, t.strategy_id, t.direction::text AS direction,
               t.entry_time, t.exit_time, t.entry_price, t.exit_price,
               t.quantity, t.net_pnl, t.result_in_r,
               t.net_pnl / nullif(abs(t.entry_price - t.stop_price) * t.quantity, 0) AS real_r,
               t.exit_reason,
               coalesce((t.journal->>'latency_artifact')::boolean, false) AS latency_artifact,
               coalesce((t.journal->>'data_quality_suspect')::boolean, false) AS data_quality_suspect
        FROM trades t
        JOIN assets a ON a.id = t.asset_id
        ORDER BY t.exit_time DESC
        LIMIT $1
        "#,
    )
    .bind(limit)
    .fetch_all(pool)
    .await
}

#[derive(Debug, FromRow, Serialize)]
pub struct SignalRow {
    pub id: i64,
    pub timestamp: DateTime<Utc>,
    pub symbol: String,
    pub strategy_id: String,
    pub timeframe: String,
    pub direction: Option<String>,
    pub status: String,
    pub entry_price: Option<Decimal>,
    pub stop_price: Option<Decimal>,
    pub target_price: Option<Decimal>,
    pub entry_reason: Option<String>,
    pub rejection_reason: Option<String>,
}

pub async fn signals(pool: &PgPool, limit: i64) -> sqlx::Result<Vec<SignalRow>> {
    sqlx::query_as(
        r#"
        SELECT s.id, s."timestamp", a.symbol, s.strategy_id, s.timeframe,
               s.direction::text AS direction, s.status::text AS status,
               s.entry_price, s.stop_price, s.target_price,
               s.entry_reason, s.rejection_reason
        FROM signals s
        JOIN assets a ON a.id = s.asset_id
        ORDER BY s."timestamp" DESC
        LIMIT $1
        "#,
    )
    .bind(limit)
    .fetch_all(pool)
    .await
}

#[derive(Debug, FromRow, Serialize)]
pub struct OrderRow {
    pub id: i64,
    pub created_at: DateTime<Utc>,
    pub symbol: String,
    pub side: String,
    pub order_type: String,
    pub status: String,
    pub quantity: Decimal,
    pub filled_quantity: Decimal,
    pub price: Option<Decimal>,
    pub stop_price: Option<Decimal>,
    pub avg_fill_price: Option<Decimal>,
    pub broker: String,
    pub error_message: Option<String>,
}

pub async fn orders(pool: &PgPool, limit: i64) -> sqlx::Result<Vec<OrderRow>> {
    sqlx::query_as(
        r#"
        SELECT o.id, o.created_at, a.symbol, o.side::text AS side,
               o.order_type::text AS order_type, o.status::text AS status,
               o.quantity, o.filled_quantity, o.price, o.stop_price,
               o.avg_fill_price, o.broker, o.error_message
        FROM orders o
        JOIN assets a ON a.id = o.asset_id
        ORDER BY o.created_at DESC
        LIMIT $1
        "#,
    )
    .bind(limit)
    .fetch_all(pool)
    .await
}

#[derive(Debug, FromRow, Serialize)]
pub struct FillRow {
    pub id: i64,
    pub timestamp: DateTime<Utc>,
    pub symbol: String,
    pub side: String,
    pub fill_price: Decimal,
    pub quantity: Decimal,
    pub commission: Decimal,
}

pub async fn fills(pool: &PgPool, limit: i64) -> sqlx::Result<Vec<FillRow>> {
    sqlx::query_as(
        r#"
        SELECT f.id, f."timestamp", a.symbol, f.side, f.fill_price,
               f.quantity, f.commission
        FROM fills f
        JOIN assets a ON a.id = f.asset_id
        ORDER BY f."timestamp" DESC
        LIMIT $1
        "#,
    )
    .bind(limit)
    .fetch_all(pool)
    .await
}

pub async fn events(pool: &PgPool, limit: i64) -> sqlx::Result<Vec<EventRow>> {
    sqlx::query_as(
        r#"
        SELECT id, "timestamp", level::text AS level, component, event_type, message
        FROM system_events
        ORDER BY "timestamp" DESC
        LIMIT $1
        "#,
    )
    .bind(limit)
    .fetch_all(pool)
    .await
}

// ── Resumo por estratégia ───────────────────────────────────────────────────

#[derive(Debug, FromRow, Serialize)]
pub struct StrategyTradeSummary {
    pub strategy_id: String,
    pub trades: i64,
    pub wins: i64,
    pub net_pnl: Decimal,
    pub sum_r: Decimal,
    pub last_trade_at: Option<DateTime<Utc>>,
}

pub async fn strategy_trade_summaries(pool: &PgPool) -> sqlx::Result<Vec<StrategyTradeSummary>> {
    sqlx::query_as(
        r#"
        SELECT strategy_id,
               count(*)                              AS trades,
               count(*) FILTER (WHERE net_pnl > 0)   AS wins,
               coalesce(sum(net_pnl), 0)             AS net_pnl,
               coalesce(sum(result_in_r), 0)         AS sum_r,
               max(exit_time)                        AS last_trade_at
        FROM trades
        GROUP BY strategy_id
        ORDER BY strategy_id
        "#,
    )
    .fetch_all(pool)
    .await
}

#[derive(Debug, FromRow, Serialize)]
pub struct StrategySignalSummary {
    pub strategy_id: String,
    pub accepted: i64,
    pub rejected: i64,
    pub last_signal_at: Option<DateTime<Utc>>,
}

pub async fn strategy_signal_summaries(pool: &PgPool) -> sqlx::Result<Vec<StrategySignalSummary>> {
    sqlx::query_as(
        r#"
        SELECT strategy_id,
               count(*) FILTER (WHERE status = 'accepted') AS accepted,
               count(*) FILTER (WHERE status = 'rejected') AS rejected,
               max("timestamp")                            AS last_signal_at
        FROM signals
        GROUP BY strategy_id
        ORDER BY strategy_id
        "#,
    )
    .fetch_all(pool)
    .await
}

// ── Instâncias ──────────────────────────────────────────────────────────────

#[derive(Debug, FromRow, Serialize)]
pub struct SymbolLastCandle {
    pub symbol: String,
    pub last_candle_at: DateTime<Utc>,
}

pub async fn last_candle_by_symbol(pool: &PgPool) -> sqlx::Result<Vec<SymbolLastCandle>> {
    sqlx::query_as(
        r#"
        SELECT a.symbol, max(c."timestamp") AS last_candle_at
        FROM candles c
        JOIN assets a ON a.id = c.asset_id
        GROUP BY a.symbol
        "#,
    )
    .fetch_all(pool)
    .await
}

#[derive(Debug, FromRow, Serialize)]
pub struct PairLastSignal {
    pub symbol: String,
    pub strategy_id: String,
    pub timestamp: DateTime<Utc>,
    pub status: String,
    pub direction: Option<String>,
}

pub async fn last_signal_by_pair(pool: &PgPool) -> sqlx::Result<Vec<PairLastSignal>> {
    sqlx::query_as(
        r#"
        SELECT DISTINCT ON (a.symbol, s.strategy_id)
               a.symbol, s.strategy_id, s."timestamp",
               s.status::text AS status, s.direction::text AS direction
        FROM signals s
        JOIN assets a ON a.id = s.asset_id
        ORDER BY a.symbol, s.strategy_id, s."timestamp" DESC
        "#,
    )
    .fetch_all(pool)
    .await
}

#[derive(Debug, FromRow, Serialize)]
pub struct PairLastTrade {
    pub symbol: String,
    pub strategy_id: String,
    pub exit_time: DateTime<Utc>,
    pub net_pnl: Decimal,
}

pub async fn last_trade_by_pair(pool: &PgPool) -> sqlx::Result<Vec<PairLastTrade>> {
    sqlx::query_as(
        r#"
        SELECT DISTINCT ON (a.symbol, t.strategy_id)
               a.symbol, t.strategy_id, t.exit_time, t.net_pnl
        FROM trades t
        JOIN assets a ON a.id = t.asset_id
        ORDER BY a.symbol, t.strategy_id, t.exit_time DESC
        "#,
    )
    .fetch_all(pool)
    .await
}

// ── Backtests e candles ─────────────────────────────────────────────────────

#[derive(Debug, FromRow, Serialize)]
pub struct BacktestRow {
    pub id: i64,
    pub created_at: DateTime<Utc>,
    pub symbol: String,
    pub strategy_id: String,
    pub timeframe: String,
    pub period_start: DateTime<Utc>,
    pub period_end: DateTime<Utc>,
    pub initial_capital: Decimal,
    pub final_equity: Decimal,
    pub label: Option<String>,
    pub metrics: serde_json::Value,
}

pub async fn backtests(pool: &PgPool, limit: i64) -> sqlx::Result<Vec<BacktestRow>> {
    sqlx::query_as(
        r#"
        SELECT b.id, b.created_at, a.symbol, b.strategy_id, b.timeframe,
               b.period_start, b.period_end, b.initial_capital, b.final_equity,
               b.label, b.metrics
        FROM backtest_runs b
        JOIN assets a ON a.id = b.asset_id
        ORDER BY b.created_at DESC
        LIMIT $1
        "#,
    )
    .bind(limit)
    .fetch_all(pool)
    .await
}

#[derive(Debug, FromRow, Serialize)]
pub struct CandlePoint {
    pub timestamp: DateTime<Utc>,
    pub close: Decimal,
}

pub async fn recent_candles(
    pool: &PgPool,
    symbol: &str,
    timeframe: &str,
    limit: i64,
) -> sqlx::Result<Vec<CandlePoint>> {
    let mut rows: Vec<CandlePoint> = sqlx::query_as(
        r#"
        SELECT c."timestamp", c.close
        FROM candles c
        JOIN assets a ON a.id = c.asset_id
        WHERE a.symbol = $1 AND c.timeframe = $2
        ORDER BY c."timestamp" DESC
        LIMIT $3
        "#,
    )
    .bind(symbol)
    .bind(timeframe)
    .bind(limit)
    .fetch_all(pool)
    .await?;
    rows.reverse();
    Ok(rows)
}
