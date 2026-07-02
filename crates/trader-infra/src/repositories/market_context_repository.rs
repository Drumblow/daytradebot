use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::PgPool;

use trader_domain::{MarketContext, RepositoryError, TimeFrame};

/// Implementação sqlx de repositório de contextos de mercado.
#[derive(Debug, Clone)]
pub struct SqlxMarketContextRepository {
    pool: PgPool,
}

impl SqlxMarketContextRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Salva um contexto de mercado no banco.
    pub async fn save(&self, ctx: &MarketContext) -> Result<i64, RepositoryError> {
        let asset_id = super::ensure_asset(&self.pool, &ctx.symbol).await?;
        let trend_state = format!("{:?}", ctx.trend_state).to_lowercase();
        let volatility_regime = format!("{:?}", ctx.volatility_regime).to_lowercase();
        let market_phase = format!("{:?}", ctx.market_phase).to_lowercase();

        let id = sqlx::query_scalar!(
            r#"
            INSERT INTO market_contexts (
                asset_id, timeframe, timestamp, trend_state, volatility_regime, market_phase,
                ema_20, ema_50, sma_200, atr_14, atr_percent_14, volume_relative,
                hh_hl_count, lh_ll_count, range_percent, is_tradeable, raw_values
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17)
            ON CONFLICT (asset_id, timeframe, timestamp) DO UPDATE SET
                trend_state = EXCLUDED.trend_state,
                volatility_regime = EXCLUDED.volatility_regime,
                market_phase = EXCLUDED.market_phase,
                ema_20 = EXCLUDED.ema_20,
                ema_50 = EXCLUDED.ema_50,
                sma_200 = EXCLUDED.sma_200,
                atr_14 = EXCLUDED.atr_14,
                atr_percent_14 = EXCLUDED.atr_percent_14,
                volume_relative = EXCLUDED.volume_relative,
                hh_hl_count = EXCLUDED.hh_hl_count,
                lh_ll_count = EXCLUDED.lh_ll_count,
                range_percent = EXCLUDED.range_percent,
                is_tradeable = EXCLUDED.is_tradeable,
                raw_values = EXCLUDED.raw_values,
                created_at = NOW()
            RETURNING id
            "#,
            asset_id,
            ctx.timeframe.to_string(),
            ctx.timestamp,
            trend_state,
            volatility_regime,
            market_phase,
            ctx.ema_20,
            ctx.ema_50,
            ctx.sma_200,
            ctx.atr_14,
            ctx.atr_percent_14,
            ctx.volume_relative,
            ctx.hh_hl_count,
            ctx.lh_ll_count,
            ctx.range_percent,
            ctx.is_tradeable,
            ctx.raw_values,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| RepositoryError::Query(e.to_string()))?;

        Ok(id)
    }

    /// Retorna o contexto mais recente para um ativo/timeframe.
    pub async fn get_latest(
        &self,
        symbol: &str,
        timeframe: TimeFrame,
    ) -> Result<Option<MarketContext>, RepositoryError> {
        let row = sqlx::query_as!(
            MarketContextRow,
            r#"
            SELECT
                mc.asset_id,
                a.symbol,
                mc.timeframe,
                mc.timestamp,
                mc.trend_state,
                mc.volatility_regime,
                mc.market_phase,
                mc.ema_20,
                mc.ema_50,
                mc.sma_200,
                mc.atr_14,
                mc.atr_percent_14,
                mc.volume_relative,
                mc.hh_hl_count,
                mc.lh_ll_count,
                mc.range_percent,
                mc.is_tradeable,
                mc.raw_values as "raw_values!: serde_json::Value"
            FROM market_contexts mc
            JOIN assets a ON a.id = mc.asset_id
            WHERE a.symbol = $1 AND mc.timeframe = $2
            ORDER BY mc.timestamp DESC
            LIMIT 1
            "#,
            symbol,
            timeframe.to_string(),
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| RepositoryError::Query(e.to_string()))?;

        Ok(row.map(Into::into))
    }

    /// Retorna contextos em um intervalo de tempo.
    pub async fn get_range(
        &self,
        symbol: &str,
        timeframe: TimeFrame,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<Vec<MarketContext>, RepositoryError> {
        let rows = sqlx::query_as!(
            MarketContextRow,
            r#"
            SELECT
                mc.asset_id,
                a.symbol,
                mc.timeframe,
                mc.timestamp,
                mc.trend_state,
                mc.volatility_regime,
                mc.market_phase,
                mc.ema_20,
                mc.ema_50,
                mc.sma_200,
                mc.atr_14,
                mc.atr_percent_14,
                mc.volume_relative,
                mc.hh_hl_count,
                mc.lh_ll_count,
                mc.range_percent,
                mc.is_tradeable,
                mc.raw_values as "raw_values!: serde_json::Value"
            FROM market_contexts mc
            JOIN assets a ON a.id = mc.asset_id
            WHERE a.symbol = $1 AND mc.timeframe = $2 AND mc.timestamp >= $3 AND mc.timestamp <= $4
            ORDER BY mc.timestamp ASC
            "#,
            symbol,
            timeframe.to_string(),
            from,
            to,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| RepositoryError::Query(e.to_string()))?;

        Ok(rows.into_iter().map(Into::into).collect())
    }
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct MarketContextRow {
    asset_id: i32,
    symbol: String,
    timeframe: String,
    timestamp: DateTime<Utc>,
    trend_state: String,
    volatility_regime: String,
    market_phase: String,
    ema_20: Option<Decimal>,
    ema_50: Option<Decimal>,
    sma_200: Option<Decimal>,
    atr_14: Option<Decimal>,
    atr_percent_14: Option<Decimal>,
    volume_relative: Option<Decimal>,
    hh_hl_count: Option<i32>,
    lh_ll_count: Option<i32>,
    range_percent: Option<Decimal>,
    is_tradeable: bool,
    raw_values: serde_json::Value,
}

impl From<MarketContextRow> for MarketContext {
    fn from(row: MarketContextRow) -> Self {
        Self {
            symbol: row.symbol,
            timeframe: row
                .timeframe
                .parse()
                .unwrap_or(trader_domain::TimeFrame::M15),
            timestamp: row.timestamp,
            candle_timestamp: Some(row.timestamp),
            trend_state: parse_trend_state(&row.trend_state),
            volatility_regime: parse_volatility_regime(&row.volatility_regime),
            market_phase: parse_market_phase(&row.market_phase),
            ema_20: row.ema_20,
            ema_50: row.ema_50,
            sma_200: row.sma_200,
            atr_14: row.atr_14,
            atr_percent_14: row.atr_percent_14,
            volume_relative: row.volume_relative,
            hh_hl_count: row.hh_hl_count,
            lh_ll_count: row.lh_ll_count,
            range_percent: row.range_percent,
            is_tradeable: row.is_tradeable,
            raw_values: row.raw_values,
        }
    }
}

fn parse_trend_state(value: &str) -> trader_domain::TrendState {
    match value {
        "uptrend" => trader_domain::TrendState::Uptrend,
        "downtrend" => trader_domain::TrendState::Downtrend,
        "neutral" => trader_domain::TrendState::Neutral,
        _ => trader_domain::TrendState::Unknown,
    }
}

fn parse_volatility_regime(value: &str) -> trader_domain::VolatilityRegime {
    match value {
        "high" => trader_domain::VolatilityRegime::High,
        "normal" => trader_domain::VolatilityRegime::Normal,
        "low" => trader_domain::VolatilityRegime::Low,
        _ => trader_domain::VolatilityRegime::Unknown,
    }
}

fn parse_market_phase(value: &str) -> trader_domain::MarketPhase {
    match value {
        "pre_market" => trader_domain::MarketPhase::PreMarket,
        "regular" => trader_domain::MarketPhase::Regular,
        "after_hours" => trader_domain::MarketPhase::AfterHours,
        _ => trader_domain::MarketPhase::Unknown,
    }
}
