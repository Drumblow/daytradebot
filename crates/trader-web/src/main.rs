//! `trader-web` — painel de status do HumanStyle Trader Bot.
//!
//! Servidor HTTP read-only: lê o mesmo PostgreSQL das instâncias e serve um
//! dashboard estático embutido no binário. Não conhece o broker, não envia
//! ordens e não escreve no banco — a sessão do Postgres é aberta com
//! `default_transaction_read_only=on`, então qualquer escrita acidental falha.
//!
//! Configuração via ambiente:
//! - `DATABASE_URL`               (obrigatória)
//! - `TRADER_WEB_BIND`            (default `0.0.0.0:8551`)
//! - `TRADER_WEB_GATEWAY_ADDR`    (default `127.0.0.1:4002`, probe TCP de liveness)
//! - `TRADER_WEB_INSTANCES`       (JSON; default = as 11 instâncias do app umbrelOS)

mod api;
mod instances;
mod queries;

use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::{Context, Result};
use axum::{routing::get, Router};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::PgPool;
use tracing::info;

use instances::InstanceConfig;

/// Estado compartilhado dos handlers.
pub struct AppState {
    pub pool: PgPool,
    pub instances: Vec<InstanceConfig>,
    pub gateway_addr: String,
    /// host:porta/banco, sem credencial — exibido no rodapé do painel.
    pub db_target: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let database_url = std::env::var("DATABASE_URL").context("DATABASE_URL não definida")?;
    let bind = std::env::var("TRADER_WEB_BIND").unwrap_or_else(|_| "0.0.0.0:8551".into());
    let gateway_addr =
        std::env::var("TRADER_WEB_GATEWAY_ADDR").unwrap_or_else(|_| "127.0.0.1:4002".into());

    // Sessão read-only: o painel nunca escreve; se um dia uma query tentar,
    // o Postgres recusa em vez de corromper dados de produção.
    let connect_options: PgConnectOptions = database_url
        .parse::<PgConnectOptions>()
        .context("DATABASE_URL inválida")?
        .options([("default_transaction_read_only", "on")]);

    let db_target = format!(
        "{}:{}/{}",
        connect_options.get_host(),
        connect_options.get_port(),
        connect_options.get_database().unwrap_or("?"),
    );

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect_with(connect_options)
        .await
        .context("falha ao conectar no banco")?;

    let state = Arc::new(AppState {
        pool,
        instances: instances::load_from_env()?,
        gateway_addr,
        db_target,
    });

    let app = Router::new()
        .route("/", get(api::index_html))
        .route("/app.js", get(api::app_js))
        .route("/style.css", get(api::style_css))
        .route("/api/health", get(api::health))
        .route("/api/overview", get(api::overview))
        .route("/api/instances", get(api::instances))
        .route("/api/equity-curve", get(api::equity_curve))
        .route("/api/pnl-daily", get(api::pnl_daily))
        .route("/api/trades", get(api::trades))
        .route("/api/signals", get(api::signals))
        .route("/api/orders", get(api::orders))
        .route("/api/events", get(api::events))
        .route("/api/strategies", get(api::strategies))
        .route("/api/backtests", get(api::backtests))
        .route("/api/candles", get(api::candles))
        .with_state(state);

    let addr: SocketAddr = bind.parse().context("TRADER_WEB_BIND inválido")?;
    info!("painel escutando em http://{addr}");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
