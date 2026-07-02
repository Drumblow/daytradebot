//! Comando `ingest`.

use anyhow::Result;
use chrono::{Duration, Utc};
use tracing::{info, warn};

use trader_adapters::{ibkr::IbkrMarketDataProvider, simulated::SimulatedMarketDataProvider};
use trader_domain::{CandleRepository, DataSource, MarketDataProvider, TimeFrame};
use trader_infra::{db::create_pool, repositories::SqlxCandleRepository};

use crate::config::CliConfig;

/// Argumentos do comando ingest.
pub struct Args {
    pub symbol: String,
    pub timeframe: TimeFrame,
    pub days: i64,
}

/// Ingere candles históricos no banco de dados.
pub async fn run(config: &CliConfig, args: Args) -> Result<()> {
    info!(
        symbol = %args.symbol,
        timeframe = %args.timeframe,
        days = args.days,
        provider = %config.provider,
        "iniciando ingestão"
    );

    let provider: Box<dyn MarketDataProvider> = match config.provider.as_str() {
        "ibkr" => {
            let ibkr_config = config.ibkr_config()?;
            Box::new(IbkrMarketDataProvider::new(ibkr_config))
        }
        "simulated" => {
            warn!("provedor simulado não retorna dados históricos reais");
            Box::new(SimulatedMarketDataProvider::new(&args.symbol))
        }
        other => anyhow::bail!("provedor desconhecido: {}", other),
    };

    let to = Utc::now();
    let from = to - Duration::days(args.days);
    let request = trader_domain::CandleRequest {
        symbol: args.symbol.clone(),
        timeframe: args.timeframe,
        from,
        to,
    };

    let candles = provider.get_historical_candles(request).await?;
    info!(count = candles.len(), "candles recebidos");

    if candles.is_empty() {
        println!("Nenhum candle retornado.");
        return Ok(());
    }

    // Persiste no banco.
    let database_url = config.app_config.database.url()?;
    let pool = create_pool(&database_url).await?;
    let repo = SqlxCandleRepository::new(pool);

    let enriched: Vec<trader_domain::Candle> = candles
        .into_iter()
        .map(|mut c| {
            c.source = match config.provider.as_str() {
                "ibkr" => DataSource::Ibkr,
                _ => DataSource::Simulated,
            };
            c
        })
        .collect();

    let inserted = repo.save(&enriched).await?;
    info!(inserted, "candles persistidos");

    println!("✅ Ingestão concluída: {} candles inseridos", inserted);

    Ok(())
}
