//! Implementações sqlx dos repositories de domínio.

pub mod asset_repository;
pub mod candle_repository;
pub mod market_context_repository;
pub mod order_repository;
pub mod signal_repository;
pub mod trade_repository;

pub use asset_repository::SqlxAssetRepository;
pub use candle_repository::SqlxCandleRepository;
pub use market_context_repository::SqlxMarketContextRepository;
pub use order_repository::SqlxOrderRepository;
pub use signal_repository::SqlxSignalRepository;
pub use trade_repository::SqlxTradeRepository;

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use trader_domain::{Candle, DataSource, TimeFrame};

/// Converte uma string de timeframe para o enum `TimeFrame`.
fn parse_timeframe(value: &str) -> Result<TimeFrame, trader_domain::RepositoryError> {
    value
        .parse()
        .map_err(|_| trader_domain::RepositoryError::Query(format!("timeframe inválido: {value}")))
}

/// Converte uma string de fonte para o enum `DataSource`.
fn parse_source(value: &str) -> Result<DataSource, trader_domain::RepositoryError> {
    match value {
        "ibkr" => Ok(DataSource::Ibkr),
        "polygon" => Ok(DataSource::Polygon),
        "manual" => Ok(DataSource::Manual),
        "simulated" => Ok(DataSource::Simulated),
        _ => Err(trader_domain::RepositoryError::Query(format!(
            "fonte inválida: {value}"
        ))),
    }
}

/// Busca o ID de um ativo pelo símbolo.
async fn get_asset_id(
    pool: &sqlx::PgPool,
    symbol: &str,
) -> Result<i32, trader_domain::RepositoryError> {
    let row = sqlx::query!("SELECT id FROM assets WHERE symbol = $1", symbol)
        .fetch_optional(pool)
        .await
        .map_err(|e| trader_domain::RepositoryError::Query(e.to_string()))?;

    row.map(|r| r.id)
        .ok_or(trader_domain::RepositoryError::NotFound)
}

/// Insere ou retorna o ID de um ativo pelo símbolo.
async fn ensure_asset(
    pool: &sqlx::PgPool,
    symbol: &str,
) -> Result<i32, trader_domain::RepositoryError> {
    if let Ok(id) = get_asset_id(pool, symbol).await {
        return Ok(id);
    }

    let tick_size = Decimal::from_f64_retain(0.01).unwrap_or(Decimal::from(1) / Decimal::from(100));

    let id = sqlx::query_scalar!(
        r#"
        INSERT INTO assets (symbol, name, asset_type, exchange, currency, tick_size, lot_size, sector, is_active)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
        RETURNING id
        "#,
        symbol,
        None::<String>,
        "stock",
        None::<String>,
        "USD",
        tick_size,
        Decimal::ONE,
        None::<String>,
        true,
    )
    .fetch_one(pool)
    .await
    .map_err(|e| trader_domain::RepositoryError::Query(e.to_string()))?;

    Ok(id)
}

/// Monta um `Candle` a partir de uma linha do banco.
#[allow(clippy::too_many_arguments)]
fn candle_from_row(
    symbol: String,
    timeframe: TimeFrame,
    timestamp: DateTime<Utc>,
    open: Decimal,
    high: Decimal,
    low: Decimal,
    close: Decimal,
    volume: Decimal,
    vwap: Option<Decimal>,
    source: DataSource,
    is_complete: bool,
) -> Candle {
    Candle {
        symbol,
        timeframe,
        timestamp,
        open,
        high,
        low,
        close,
        volume,
        vwap,
        source,
        is_complete,
    }
}
