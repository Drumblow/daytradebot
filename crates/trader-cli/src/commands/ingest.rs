//! Comando `ingest`.

use anyhow::Result;
use chrono::{Duration, Utc};
use tracing::{info, warn};

use trader_adapters::{ibkr::IbkrMarketDataProvider, simulated::SimulatedMarketDataProvider};
use trader_core::data_quality::count_gaps;
use trader_domain::{CandleRepository, DataSource, MarketDataProvider, TimeFrame};
use trader_infra::{
    db::create_pool,
    repositories::{IngestionRecord, SqlxCandleRepository, SqlxIngestionRepository},
};

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

    // Qualidade do feed: barras degeneradas (1 print, high==low) indicam
    // feed sem subscrição de dados em tempo real — ingerir isso corrompe
    // backtests (ver docs/reports/validacao-live-vs-backtest-2026-08-07_a_08-14.md).
    let degenerate = candles.iter().filter(|c| c.is_degenerate()).count();
    if degenerate > 0 {
        warn!(
            degenerate,
            total = candles.len(),
            "barras degeneradas na série recebida — verifique a subscrição de dados do feed"
        );
        println!(
            "⚠️  {degenerate}/{} barras degeneradas (high==low) recebidas do feed",
            candles.len()
        );
    }

    // Persiste no banco.
    let database_url = config.app_config.database.url()?;
    let pool = create_pool(&database_url).await?;
    let repo = SqlxCandleRepository::new(pool.clone());

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

    let gaps = count_gaps(&enriched, args.timeframe);
    let start_time = enriched.first().map(|c| c.timestamp).unwrap_or(from);
    let end_time = enriched.last().map(|c| c.timestamp).unwrap_or(to);

    let inserted = repo.save(&enriched).await?;
    info!(inserted, gaps, "candles persistidos");

    // Registra a ingestão para rastreabilidade de qualidade de dados.
    let ingestion_repo = SqlxIngestionRepository::new(pool);
    let record = IngestionRecord {
        symbol: args.symbol.clone(),
        timeframe: args.timeframe.to_string(),
        source: config.provider.clone(),
        start_time,
        end_time,
        candles_inserted: inserted as i32,
        gaps_detected: gaps as i32,
        status: "completed".to_string(),
        error_message: None,
    };
    if let Err(e) = ingestion_repo.save(&record).await {
        warn!(error = %e, "falha ao registrar ingestão");
    }

    if gaps > 0 {
        println!("⚠️  {} gap(s) intraday detectado(s) na série", gaps);
    }
    println!("✅ Ingestão concluída: {} candles inseridos", inserted);

    Ok(())
}
