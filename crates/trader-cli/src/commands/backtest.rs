//! Comando `backtest`.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use tracing::{info, warn};

use trader_backtest::{BacktestConfig, BacktestEngine, BacktestReport};
use trader_domain::{CandleRepository, Strategy, TimeFrame};
use trader_infra::{
    db::create_pool,
    repositories::{BacktestRunRecord, SqlxBacktestRunRepository, SqlxCandleRepository},
};

use crate::config::CliConfig;

/// Argumentos do comando backtest.
pub struct Args {
    pub symbol: String,
    pub strategy: String,
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
    pub timeframe: TimeFrame,
    /// Permite rodar sobre série sintética quando não há dados no banco.
    /// Sem esta flag, backtest sem dados reais FALHA — um backtest sobre
    /// dados fabricados não é evidência de nada.
    pub allow_synthetic: bool,
    /// Caminho opcional para exportar o relatório em JSON.
    pub output: Option<String>,
    /// Slippage por execução, em pontos-base do preço (1 bp = 0,01%).
    ///
    /// Existe para CALIBRAR, não para embelezar: o custo de execução destas
    /// estratégias é da mesma ordem de grandeza do risco por trade (stops de
    /// ~0,13% do preço), então o resultado é muito sensível a ele. Varie e
    /// veja onde o PF cruza 1 antes de acreditar em qualquer backtest.
    pub slippage_bps: Option<u32>,
}

/// Executa um backtest da estratégia solicitada.
///
/// Usa o mesmo `RiskConfig` do live/paper (paridade de validação) e persiste
/// o run no banco (`backtest_runs`) para comparação futura.
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

    let strategy = crate::dispatch::load_strategy(&args.strategy, &strategy_toml)?;

    let pool = match config.app_config.database.url() {
        Ok(url) => match create_pool(&url).await {
            Ok(pool) => Some(pool),
            Err(e) => {
                warn!(error = %e, "falha ao conectar no banco");
                None
            }
        },
        Err(e) => {
            warn!(error = %e, "DATABASE_URL não configurada");
            None
        }
    };

    let candles = match &pool {
        Some(pool) => {
            let loaded = load_candles(pool, &args).await?;
            if loaded.is_empty() {
                if !args.allow_synthetic {
                    anyhow::bail!(
                        "nenhum candle no banco para {} no timeframe {}. \
                         Rode 'trader-cli ingest' primeiro, ou use --allow-synthetic \
                         para um smoke test com dados fabricados.",
                        args.symbol,
                        args.timeframe
                    );
                }
                warn!("nenhum candle no banco; usando série sintética (--allow-synthetic)");
                println!("   Fonte:      sintética (--allow-synthetic)");
                generate_synthetic_series(&args.symbol)
            } else {
                println!("   Fonte:      banco de dados ({} candles)", loaded.len());
                loaded
            }
        }
        None => {
            if !args.allow_synthetic {
                anyhow::bail!(
                    "sem banco de dados disponível. Configure DATABASE_URL e rode \
                     'trader-cli ingest', ou use --allow-synthetic para um smoke test."
                );
            }
            warn!("sem banco; usando série sintética (--allow-synthetic)");
            println!("   Fonte:      sintética (--allow-synthetic)");
            generate_synthetic_series(&args.symbol)
        }
    };

    let backtest_config = BacktestConfig {
        symbol: args.symbol.clone(),
        initial_capital: Decimal::from(100_000),
        commission_per_trade: Decimal::from(35) / Decimal::from(100),
        slippage_pct: match args.slippage_bps {
            Some(bps) => Decimal::from(bps) / Decimal::from(10_000),
            // 2 bp — ver a justificativa da calibracao em
            // trader-backtest/src/engine.rs.
            None => Decimal::from(2) / Decimal::from(10_000),
        },
        entry_validity_candles: strategy.entry_validity_candles() as u32,
        time_exit: strategy.time_exit(),
    };

    // Paridade com o live: mesmos limites de risco e horário da estratégia.
    let risk_config =
        crate::risk_config::build_risk_config(&config.app_config.risk, &strategy.risk_params())?;
    let mut engine = BacktestEngine::new(backtest_config, risk_config);

    let run = engine.run(&strategy, &candles).await?;
    let report = BacktestReport::from_run(run);

    println!("{}", report);

    // Exporta o relatório em JSON, se solicitado.
    if let Some(path) = &args.output {
        let json = report.to_json()?;
        std::fs::write(path, json)
            .with_context(|| format!("falha ao escrever relatório em {}", path))?;
        println!("   Relatório exportado para {}", path);
    }

    // Persiste o run no banco (melhor esforço: backtest já foi executado).
    if let Some(pool) = &pool {
        let record = BacktestRunRecord {
            symbol: args.symbol.clone(),
            strategy_id: strategy.id().id,
            strategy_version: strategy.id().version,
            config_hash: strategy.config_hash(),
            timeframe: format!("{:?}", args.timeframe),
            period_start: report.start_time,
            period_end: report.end_time,
            initial_capital: report.initial_capital,
            final_equity: report.final_equity,
            metrics: serde_json::to_value(&report.metrics)
                .unwrap_or(serde_json::Value::Object(Default::default())),
            label: None,
        };
        let repo = SqlxBacktestRunRepository::new(pool.clone());
        match repo.save(&record).await {
            Ok(id) => println!("   Run persistido no banco (id={})", id),
            Err(e) => warn!(error = %e, "falha ao persistir run de backtest"),
        }
    }

    Ok(())
}

async fn load_candles(pool: &sqlx::PgPool, args: &Args) -> Result<Vec<trader_domain::Candle>> {
    let repo = SqlxCandleRepository::new(pool.clone());

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
