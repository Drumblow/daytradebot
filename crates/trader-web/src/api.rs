//! Handlers HTTP do painel.

use std::sync::Arc;
use std::time::Duration;

use axum::extract::{Query, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use chrono::{Datelike, NaiveTime, Utc};
use chrono_tz::America::New_York;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use tracing::error;

use crate::{queries, AppState};

type Shared = State<Arc<AppState>>;

/// Erro de API: loga o detalhe e devolve 500 genérico.
pub struct ApiError(anyhow::Error);

impl<E: Into<anyhow::Error>> From<E> for ApiError {
    fn from(err: E) -> Self {
        Self(err.into())
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        error!("erro na API do painel: {:#}", self.0);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": "erro interno; veja o log do container" })),
        )
            .into_response()
    }
}

type ApiResult<T> = Result<Json<T>, ApiError>;

// ── Estáticos (embutidos no binário) ────────────────────────────────────────

pub async fn index_html() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        include_str!("../assets/index.html"),
    )
}

pub async fn app_js() -> impl IntoResponse {
    (
        [(
            header::CONTENT_TYPE,
            "application/javascript; charset=utf-8",
        )],
        include_str!("../assets/app.js"),
    )
}

pub async fn style_css() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/css; charset=utf-8")],
        include_str!("../assets/style.css"),
    )
}

// ── Health ──────────────────────────────────────────────────────────────────

pub async fn health(State(state): Shared) -> ApiResult<serde_json::Value> {
    sqlx::query("SELECT 1").execute(&state.pool).await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

// ── Overview ────────────────────────────────────────────────────────────────

/// Fase do mercado segundo a janela operacional do bot (horário de NY).
#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MarketPhase {
    /// Fim de semana.
    Weekend,
    /// Dia útil antes das 09:25 ET.
    PreWindow,
    /// Dentro da janela 09:25–16:10 ET.
    Open,
    /// Dia útil depois das 16:10 ET.
    AfterWindow,
}

fn market_phase_now() -> (MarketPhase, String) {
    let now_et = Utc::now().with_timezone(&New_York);
    let phase = if matches!(
        now_et.weekday(),
        chrono::Weekday::Sat | chrono::Weekday::Sun
    ) {
        MarketPhase::Weekend
    } else {
        // Janela operacional das instâncias (mesma do scheduler do app).
        let start = NaiveTime::from_hms_opt(9, 25, 0).unwrap();
        let end = NaiveTime::from_hms_opt(16, 10, 0).unwrap();
        let t = now_et.time();
        if t < start {
            MarketPhase::PreWindow
        } else if t <= end {
            MarketPhase::Open
        } else {
            MarketPhase::AfterWindow
        }
    };
    (phase, now_et.format("%Y-%m-%d %H:%M:%S").to_string())
}

async fn gateway_port_open(addr: &str) -> bool {
    matches!(
        tokio::time::timeout(
            Duration::from_millis(800),
            tokio::net::TcpStream::connect(addr)
        )
        .await,
        Ok(Ok(_))
    )
}

#[derive(Debug, Serialize)]
pub struct Overview {
    pub now_et: String,
    pub market_phase: MarketPhase,
    pub gateway_port_open: bool,
    /// Host:porta/banco a que o painel está conectado — evita confundir um
    /// snapshot de desenvolvimento com o banco de produção.
    pub db_target: String,
    pub open_orders: queries::OpenOrderCounts,
    pub last_candle_at: Option<chrono::DateTime<Utc>>,
    pub last_error: Option<queries::EventRow>,
    pub today: queries::TradeTotals,
    pub total: queries::TradeTotals,
    pub signals_today: queries::SignalCounts,
}

pub async fn overview(State(state): Shared) -> ApiResult<Overview> {
    let (market_phase, now_et) = market_phase_now();
    let (total, today, signals_today, open_orders, last_candle_at, last_error) = tokio::try_join!(
        queries::trade_totals(&state.pool),
        queries::trade_totals_today(&state.pool),
        queries::signal_counts_today(&state.pool),
        queries::open_order_counts(&state.pool),
        queries::last_candle_at(&state.pool),
        queries::last_error_event(&state.pool),
    )?;
    let gateway_port_open = gateway_port_open(&state.gateway_addr).await;

    Ok(Json(Overview {
        now_et,
        market_phase,
        gateway_port_open,
        db_target: state.db_target.clone(),
        open_orders,
        last_candle_at,
        last_error,
        today,
        total,
        signals_today,
    }))
}

// ── Instâncias ──────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct InstanceStatus {
    pub name: String,
    pub symbol: String,
    pub strategy: String,
    pub client_id: i32,
    pub last_candle_at: Option<chrono::DateTime<Utc>>,
    pub last_signal_at: Option<chrono::DateTime<Utc>>,
    pub last_signal_status: Option<String>,
    pub last_trade_at: Option<chrono::DateTime<Utc>>,
    pub last_trade_pnl: Option<Decimal>,
}

pub async fn instances(State(state): Shared) -> ApiResult<Vec<InstanceStatus>> {
    let (candles, signals, trades) = tokio::try_join!(
        queries::last_candle_by_symbol(&state.pool),
        queries::last_signal_by_pair(&state.pool),
        queries::last_trade_by_pair(&state.pool),
    )?;

    let result = state
        .instances
        .iter()
        .map(|inst| {
            let candle = candles.iter().find(|c| c.symbol == inst.symbol);
            let signal = signals
                .iter()
                .find(|s| s.symbol == inst.symbol && s.strategy_id == inst.strategy);
            let trade = trades
                .iter()
                .find(|t| t.symbol == inst.symbol && t.strategy_id == inst.strategy);
            InstanceStatus {
                name: inst.name.clone(),
                symbol: inst.symbol.clone(),
                strategy: inst.strategy.clone(),
                client_id: inst.client_id,
                last_candle_at: candle.map(|c| c.last_candle_at),
                last_signal_at: signal.map(|s| s.timestamp),
                last_signal_status: signal.map(|s| s.status.clone()),
                last_trade_at: trade.map(|t| t.exit_time),
                last_trade_pnl: trade.map(|t| t.net_pnl),
            }
        })
        .collect();

    Ok(Json(result))
}

// ── Gráficos ────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct EquityCurvePoint {
    pub t: chrono::DateTime<Utc>,
    pub pnl: Decimal,
    pub cum: Decimal,
    pub symbol: String,
    pub strategy_id: String,
}

pub async fn equity_curve(State(state): Shared) -> ApiResult<Vec<EquityCurvePoint>> {
    let points = queries::equity_points(&state.pool).await?;
    let mut cum = Decimal::ZERO;
    let curve = points
        .into_iter()
        .map(|p| {
            cum += p.net_pnl;
            EquityCurvePoint {
                t: p.exit_time,
                pnl: p.net_pnl,
                cum,
                symbol: p.symbol,
                strategy_id: p.strategy_id,
            }
        })
        .collect();
    Ok(Json(curve))
}

#[derive(Debug, Deserialize)]
pub struct DaysParam {
    #[serde(default = "default_days")]
    pub days: i64,
}

fn default_days() -> i64 {
    90
}

pub async fn pnl_daily(
    State(state): Shared,
    Query(params): Query<DaysParam>,
) -> ApiResult<Vec<queries::DailyPnl>> {
    let days = params.days.clamp(1, 365);
    Ok(Json(queries::pnl_daily(&state.pool, days).await?))
}

// ── Listagens ───────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct LimitParam {
    #[serde(default = "default_limit")]
    pub limit: i64,
}

fn default_limit() -> i64 {
    50
}

impl LimitParam {
    fn clamped(&self) -> i64 {
        self.limit.clamp(1, 500)
    }
}

pub async fn trades(
    State(state): Shared,
    Query(p): Query<LimitParam>,
) -> ApiResult<Vec<queries::TradeRow>> {
    Ok(Json(queries::trades(&state.pool, p.clamped()).await?))
}

pub async fn signals(
    State(state): Shared,
    Query(p): Query<LimitParam>,
) -> ApiResult<Vec<queries::SignalRow>> {
    Ok(Json(queries::signals(&state.pool, p.clamped()).await?))
}

#[derive(Debug, Serialize)]
pub struct OrdersAndFills {
    pub orders: Vec<queries::OrderRow>,
    pub fills: Vec<queries::FillRow>,
}

pub async fn orders(
    State(state): Shared,
    Query(p): Query<LimitParam>,
) -> ApiResult<OrdersAndFills> {
    let limit = p.clamped();
    let (orders, fills) = tokio::try_join!(
        queries::orders(&state.pool, limit),
        queries::fills(&state.pool, limit),
    )?;
    Ok(Json(OrdersAndFills { orders, fills }))
}

pub async fn events(
    State(state): Shared,
    Query(p): Query<LimitParam>,
) -> ApiResult<Vec<queries::EventRow>> {
    Ok(Json(queries::events(&state.pool, p.clamped()).await?))
}

// ── Estratégias ─────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct StrategySummary {
    pub strategy_id: String,
    pub trades: i64,
    pub wins: i64,
    pub net_pnl: Decimal,
    pub sum_r: Decimal,
    pub accepted_signals: i64,
    pub rejected_signals: i64,
    pub last_trade_at: Option<chrono::DateTime<Utc>>,
    pub last_signal_at: Option<chrono::DateTime<Utc>>,
}

pub async fn strategies(State(state): Shared) -> ApiResult<Vec<StrategySummary>> {
    let (trade_summaries, signal_summaries) = tokio::try_join!(
        queries::strategy_trade_summaries(&state.pool),
        queries::strategy_signal_summaries(&state.pool),
    )?;

    // Une pelos dois lados: estratégias com sinal mas sem trade também aparecem.
    let mut ids: Vec<String> = trade_summaries
        .iter()
        .map(|t| t.strategy_id.clone())
        .chain(signal_summaries.iter().map(|s| s.strategy_id.clone()))
        .collect();
    ids.sort();
    ids.dedup();

    let result = ids
        .into_iter()
        .map(|id| {
            let t = trade_summaries.iter().find(|t| t.strategy_id == id);
            let s = signal_summaries.iter().find(|s| s.strategy_id == id);
            StrategySummary {
                strategy_id: id,
                trades: t.map_or(0, |t| t.trades),
                wins: t.map_or(0, |t| t.wins),
                net_pnl: t.map_or(Decimal::ZERO, |t| t.net_pnl),
                sum_r: t.map_or(Decimal::ZERO, |t| t.sum_r),
                accepted_signals: s.map_or(0, |s| s.accepted),
                rejected_signals: s.map_or(0, |s| s.rejected),
                last_trade_at: t.and_then(|t| t.last_trade_at),
                last_signal_at: s.and_then(|s| s.last_signal_at),
            }
        })
        .collect();

    Ok(Json(result))
}

pub async fn backtests(
    State(state): Shared,
    Query(p): Query<LimitParam>,
) -> ApiResult<Vec<queries::BacktestRow>> {
    Ok(Json(queries::backtests(&state.pool, p.clamped()).await?))
}

// ── Candles (sparkline) ─────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct CandlesParams {
    pub symbol: String,
    #[serde(default = "default_timeframe")]
    pub timeframe: String,
    #[serde(default = "default_candles_limit")]
    pub limit: i64,
}

fn default_timeframe() -> String {
    "15m".into()
}

fn default_candles_limit() -> i64 {
    54 // ~2 pregões de candles de 15m
}

// Devolve `Response` pronto (e não um Result): o clippy do CI recusa um
// `Err`-variant do tamanho de um `Response` (result_large_err).
pub async fn candles(State(state): Shared, Query(p): Query<CandlesParams>) -> Response {
    let symbol_ok = !p.symbol.is_empty()
        && p.symbol.len() <= 8
        && p.symbol.chars().all(|c| c.is_ascii_alphanumeric());
    let timeframe_ok = matches!(p.timeframe.as_str(), "5m" | "15m" | "1h" | "1d");
    if !symbol_ok || !timeframe_ok {
        return (StatusCode::BAD_REQUEST, "symbol/timeframe inválido").into_response();
    }

    let limit = p.limit.clamp(1, 500);
    match queries::recent_candles(&state.pool, &p.symbol, &p.timeframe, limit).await {
        Ok(rows) => Json(rows).into_response(),
        Err(e) => ApiError::from(e).into_response(),
    }
}
