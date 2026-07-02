use rust_decimal::Decimal;
use sqlx::PgPool;

use trader_domain::{Asset, RepositoryError};

/// Implementação sqlx de repositório de ativos.
#[derive(Debug, Clone)]
pub struct SqlxAssetRepository {
    pool: PgPool,
}

impl SqlxAssetRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Busca um ativo pelo símbolo.
    pub async fn get_by_symbol(&self, symbol: &str) -> Result<Option<Asset>, RepositoryError> {
        let row = sqlx::query_as!(
            AssetRow,
            r#"
            SELECT id, symbol, name, asset_type, exchange, currency, tick_size, lot_size, sector, is_active
            FROM assets
            WHERE symbol = $1
            "#,
            symbol
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| RepositoryError::Query(e.to_string()))?;

        Ok(row.map(Into::into))
    }

    /// Insere ou atualiza um ativo.
    pub async fn save(&self, asset: &Asset) -> Result<i32, RepositoryError> {
        let id = sqlx::query_scalar!(
            r#"
            INSERT INTO assets (symbol, name, asset_type, exchange, currency, tick_size, lot_size, sector, is_active)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            ON CONFLICT (symbol) DO UPDATE SET
                name = EXCLUDED.name,
                asset_type = EXCLUDED.asset_type,
                exchange = EXCLUDED.exchange,
                currency = EXCLUDED.currency,
                tick_size = EXCLUDED.tick_size,
                lot_size = EXCLUDED.lot_size,
                sector = EXCLUDED.sector,
                is_active = EXCLUDED.is_active,
                updated_at = NOW()
            RETURNING id
            "#,
            asset.symbol,
            asset.name,
            asset.asset_type,
            asset.exchange,
            asset.currency,
            asset.tick_size,
            asset.lot_size,
            asset.sector,
            asset.is_active,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| RepositoryError::Query(e.to_string()))?;

        Ok(id)
    }
}

#[derive(Debug, Clone)]
struct AssetRow {
    id: i32,
    symbol: String,
    name: Option<String>,
    asset_type: String,
    exchange: Option<String>,
    currency: String,
    tick_size: Decimal,
    lot_size: Decimal,
    sector: Option<String>,
    is_active: bool,
}

impl From<AssetRow> for Asset {
    fn from(row: AssetRow) -> Self {
        Self {
            id: Some(row.id),
            symbol: row.symbol,
            name: row.name,
            asset_type: row.asset_type,
            exchange: row.exchange,
            currency: row.currency,
            tick_size: row.tick_size,
            lot_size: row.lot_size,
            sector: row.sector,
            is_active: row.is_active,
        }
    }
}
