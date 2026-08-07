//! Comando `walkforward` — validação out-of-sample da estratégia.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use tracing::{info, warn};

use trader_backtest::{run_walk_forward, BacktestConfig};
use trader_domain::{CandleRepository, Strategy, TimeFrame};
use trader_infra::{
    db::create_pool,
    repositories::{BacktestRunRecord, SqlxBacktestRunRepository, SqlxCandleRepository},
};

use crate::config::CliConfig;

/// Argumentos do comando walkforward.
pub struct Args {
    pub symbol: String,
    pub strategy: String,
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
    pub timeframe: TimeFrame,
    /// Número de janelas out-of-sample.
    pub windows: usize,
}

/// Executa análise walk-forward (anchored) sobre dados reais do banco.
///
/// Sem dados reais, FALHA — walk-forward sobre dados sintéticos não é
/// evidência de nada.
pub async fn run(config: &CliConfig, args: Args) -> Result<()> {
    info!(
        symbol = %args.symbol,
        strategy = %args.strategy,
        windows = args.windows,
        "iniciando walk-forward"
    );

    println!("🔬 Iniciando walk-forward");
    println!("   Ativo:     {}", args.symbol);
    println!("   Estratégia: {}", args.strategy);
    println!("   Timeframe: {}", args.timeframe);
    println!("   Janelas:   {}", args.windows);

    let strategy_path = format!("config/strategies/{}.toml", args.strategy);
    let strategy_toml = std::fs::read_to_string(&strategy_path)
        .with_context(|| format!("falha ao ler config da estratégia em {}", strategy_path))?;
    let strategy = crate::dispatch::load_strategy(&args.strategy, &strategy_toml)?;

    let database_url = config
        .app_config
        .database
        .url()
        .map_err(|e| anyhow::anyhow!("DATABASE_URL não configurada: {e}"))?;
    let pool = create_pool(&database_url)
        .await
        .map_err(|e| anyhow::anyhow!("falha ao conectar no banco: {e}"))?;

    let to = args.to.unwrap_or_else(Utc::now);
    let from = args
        .from
        .unwrap_or_else(|| to - chrono::Duration::days(180));

    let repo = SqlxCandleRepository::new(pool.clone());
    let candles = repo
        .get_range(&args.symbol, args.timeframe, from, to)
        .await
        .map_err(|e| anyhow::anyhow!("falha ao buscar candles: {e}"))?;

    if candles.is_empty() {
        anyhow::bail!(
            "nenhum candle no banco para {} no timeframe {}. \
             Rode 'trader-cli ingest' primeiro — walk-forward exige dados reais.",
            args.symbol,
            args.timeframe
        );
    }
    println!(
        "   Candles:   {} ({} → {})\n",
        candles.len(),
        from.date_naive(),
        to.date_naive()
    );

    let backtest_config = BacktestConfig {
        symbol: args.symbol.clone(),
        entry_validity_candles: strategy.entry_validity_candles() as u32,
        time_exit: strategy.time_exit(),
        ..BacktestConfig::default()
    };
    let risk_config =
        crate::risk_config::build_risk_config(&config.app_config.risk, &strategy.risk_params());

    let result = run_walk_forward(
        &strategy,
        &candles,
        args.windows,
        &backtest_config,
        risk_config,
    )
    .await?;

    // Relatório por janela: degradação IS → OOS indica sobreajuste/regime.
    println!("{:-<100}", "");
    println!(
        "{:^7} {:^22} {:^22} {:^8} {:^8} {:^8} {:^8} {:^8}",
        "janela", "período teste", "", "trades", "win%", "PF", "avgR", "netP&L"
    );
    for w in &result.windows {
        println!(
            "{:^7} {} → {}  IS {:^4} OOS {:^4} {:^8} {:^8} {:^8.2} {:^10.2}",
            w.window,
            w.test_start.date_naive(),
            w.test_end.date_naive(),
            w.in_sample.total_trades,
            w.out_of_sample.total_trades,
            format!("{:.1}", w.out_of_sample.win_rate),
            w.out_of_sample.profit_factor_display(),
            w.out_of_sample.avg_r_per_trade,
            w.out_of_sample.net_pnl,
        );
    }
    println!("{:-<100}", "");

    let m = &result.oos_metrics;
    println!("📊 Out-of-sample agregado (a amostra que conta):");
    println!("   Trades:        {}", m.total_trades);
    println!("   Win rate:      {}%", m.win_rate);
    println!("   Profit factor: {}", m.profit_factor_display());
    println!("   Avg R/trade:   {:.3}", m.avg_r_per_trade);
    println!(
        "   Max drawdown:  {} ({}%)",
        m.max_drawdown, m.max_drawdown_pct
    );
    println!("   Net P&L:       {}", m.net_pnl);
    println!();
    println!(
        "   Critérios de aceitação (docs/strategies/{}.md):",
        strategy.id().id
    );
    print_acceptance(m.total_trades, m);

    // Persiste o run agregado OOS para histórico.
    let record = BacktestRunRecord {
        symbol: args.symbol.clone(),
        strategy_id: strategy.id().id,
        strategy_version: strategy.id().version,
        config_hash: strategy.config_hash(),
        timeframe: format!("{:?}", args.timeframe),
        period_start: candles.first().map(|c| c.timestamp).unwrap_or(from),
        period_end: candles.last().map(|c| c.timestamp).unwrap_or(to),
        initial_capital: backtest_config.initial_capital,
        final_equity: backtest_config.initial_capital + m.net_pnl,
        metrics: serde_json::to_value(m).unwrap_or(serde_json::Value::Object(Default::default())),
        label: Some(format!("walkforward-oos-{}w", args.windows)),
    };
    let run_repo = SqlxBacktestRunRepository::new(pool);
    match run_repo.save(&record).await {
        Ok(id) => println!("\n   Run OOS persistido no banco (id={})", id),
        Err(e) => warn!(error = %e, "falha ao persistir run de walk-forward"),
    }

    Ok(())
}

/// Imprime o veredito contra os critérios de aceitação do backtest.
fn print_acceptance(total_trades: usize, m: &trader_backtest::BacktestMetrics) {
    let check =
        |ok: bool, label: String| println!("   [{}] {}", if ok { "OK" } else { "--" }, label);

    check(
        total_trades >= 50,
        format!("≥ 50 trades (atual: {total_trades})"),
    );
    check(
        m.win_rate >= Decimal::from(40),
        format!("win rate ≥ 40% (atual: {:.1}%)", m.win_rate),
    );
    check(
        // Sem perdas no período (None) o PF é infinito: critério atendido.
        m.profit_factor
            .map(|pf| pf >= Decimal::new(13, 1))
            .unwrap_or(m.total_trades > 0),
        format!("profit factor ≥ 1.3 (atual: {})", m.profit_factor_display()),
    );
    check(
        m.max_drawdown_pct <= Decimal::from(10),
        format!("drawdown ≤ 10% (atual: {:.2}%)", m.max_drawdown_pct),
    );
    check(
        m.avg_r_per_trade > Decimal::new(15, 2),
        format!("avg R > 0.15 (atual: {:.3})", m.avg_r_per_trade),
    );
    check(
        m.net_pnl > Decimal::ZERO,
        format!("expectativa positiva (net P&L: {:.2})", m.net_pnl),
    );
}
