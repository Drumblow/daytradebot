//! Comando `backtest`.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use tracing::{info, warn};

use trader_backtest::{
    default_backtest_risk_config, BacktestConfig, BacktestEngine, BacktestReport,
};
use trader_core::strategies::pullback_trend_v1::PullbackTrendV1;
use trader_domain::{CandleRepository, TimeFrame};
use trader_infra::{db::create_pool, repositories::SqlxCandleRepository};

use crate::config::CliConfig;

/// Argumentos do comando backtest.
pub struct Args {
    pub symbol: String,
    pub strategy: String,
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
    pub timeframe: TimeFrame,
}

/// Executa um backtest da estratégia solicitada.
///
/// Tenta carregar candles do banco. Se não houver dados suficientes, usa
/// candles sintéticos como fallback.
pub async fn run(config: &CliConfig, args: Args) -> Result<()> {
    info!(
        symbol = %args.symbol,
        strategy = %args.strategy,
        "iniciando backtest"
    );

    println!("📈 Iniciando backtest");
    println!("   Ativo:     {}", args.symbol);
    println!("   Estratégia: {}", args.strategy);
    println!("   Timeframe: {}", args.timeframe);

    // Carrega configuração da estratégia.
    let strategy_path = format!("config/strategies/{}.toml", args.strategy);
    let strategy_toml = std::fs::read_to_string(&strategy_path)
        .with_context(|| format!("falha ao ler config da estratégia em {}", strategy_path))?;

    let strategy = PullbackTrendV1::from_toml(&strategy_toml)
        .with_context(|| "falha ao fazer parse da configuração TOML da estratégia")?;

    let candles = match load_candles(config, &args).await {
        Ok(candles) if !candles.is_empty() => {
            println!("   Fonte:      banco de dados ({} candles)", candles.len());
            candles
        }
        Ok(_) => {
            warn!("nenhum candle no banco; usando série sintética");
            println!("   Fonte:      sintética (fallback)");
            generate_synthetic_series(&args.symbol)
        }
        Err(e) => {
            warn!(error = %e, "falha ao carregar candles do banco; usando série sintética");
            println!("   Fonte:      sintética (fallback)");
            generate_synthetic_series(&args.symbol)
        }
    };

    let backtest_config = BacktestConfig {
        symbol: args.symbol.clone(),
        initial_capital: Decimal::from(100_000),
        commission_per_trade: Decimal::from(35) / Decimal::from(100),
        slippage_pct: Decimal::from(1) / Decimal::from(1000),
    };

    let risk_config = default_backtest_risk_config();
    let mut engine = BacktestEngine::new(backtest_config, risk_config);

    let run = engine.run(&strategy, &candles).await?;
    let report = BacktestReport::from_run(run);

    println!("{}", report);

    Ok(())
}

async fn load_candles(config: &CliConfig, args: &Args) -> Result<Vec<trader_domain::Candle>> {
    let database_url = config
        .app_config
        .database
        .url()
        .map_err(|e| anyhow::anyhow!("DATABASE_URL não configurada: {e}"))?;

    let pool = create_pool(&database_url)
        .await
        .map_err(|e| anyhow::anyhow!("falha ao conectar no banco: {e}"))?;
    let repo = SqlxCandleRepository::new(pool);

    let to = args.to.unwrap_or_else(Utc::now);
    let from = args
        .from
        .unwrap_or_else(|| to - chrono::Duration::days(180));

    repo.get_range(&args.symbol, args.timeframe, from, to)
        .await
        .map_err(|e| anyhow::anyhow!("falha ao buscar candles: {e}"))
}

fn generate_synthetic_series(symbol: &str) -> Vec<trader_domain::Candle> {
    let mut candles = crate::synthetic::generate_synthetic_uptrend(symbol);

    // Adiciona candles de continuação para que o alvo seja atingido.
    for _ in 0..20 {
        if let Some(next) = crate::synthetic::next_candle(symbol, &candles) {
            candles.push(next);
        }
    }

    candles
}
