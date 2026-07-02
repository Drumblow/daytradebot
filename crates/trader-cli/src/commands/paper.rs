//! Comando `paper`.

use anyhow::{Context, Result};
use rust_decimal::Decimal;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::interval;
use tracing::{info, warn};

use trader_adapters::simulated::{SimulatedBroker, SimulatedBrokerConfig};
use trader_core::{
    context::MarketContextAnalyzer,
    execution::ExecutionEngine,
    risk::{RiskConfig, RiskManager, RiskState},
    strategies::pullback_trend_v1::PullbackTrendV1,
};
use trader_domain::{Broker, CandleRepository, SignalResult, Strategy, TimeFrame, TradingMode};
use trader_infra::{
    db::create_pool,
    repositories::{
        SqlxCandleRepository, SqlxMarketContextRepository, SqlxOrderRepository,
        SqlxSignalRepository, SqlxTradeRepository,
    },
};

use crate::config::CliConfig;

/// Argumentos do comando paper.
pub struct Args {
    pub symbol: String,
    pub strategy: String,
    pub mode: PaperMode,
    pub timeframe: TimeFrame,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaperMode {
    Simulated,
    Replay,
}

impl std::str::FromStr for PaperMode {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "simulated" => Ok(PaperMode::Simulated),
            "replay" => Ok(PaperMode::Replay),
            other => anyhow::bail!(
                "modo de paper inválido: {}. Use 'simulated' ou 'replay'",
                other
            ),
        }
    }
}

/// Loop contínuo de paper trading.
///
/// - Modo `simulated`: gera candles sintéticos e opera indefinidamente.
/// - Modo `replay`: carrega candles do banco e opera sobre eles.
///
/// Persiste sinais, ordens, trades e contexto no PostgreSQL.
pub async fn run(config: &CliConfig, args: Args) -> Result<()> {
    info!(
        symbol = %args.symbol,
        strategy = %args.strategy,
        mode = ?args.mode,
        "iniciando paper trading"
    );

    println!("🚀 Iniciando paper trading");
    println!("   Ativo:     {}", args.symbol);
    println!("   Estratégia: {}", args.strategy);
    println!("   Timeframe: {}", args.timeframe);
    println!("   Modo:      {:?}", args.mode);
    println!("   Aviso:     PAPER TRADING — NÃO OPERANDO DINHEIRO REAL\n");

    // Hard check de segurança: só permite paper.
    let trading_mode = config
        .app_config
        .app
        .mode
        .parse::<TradingMode>()
        .unwrap_or(TradingMode::Paper);

    if trading_mode.is_real() {
        anyhow::bail!(
            "modo de operação real não é permitido no MVP. \
             Configure TRADER__APP__MODE=paper ou [app].mode='paper'"
        );
    }

    // Carrega configuração da estratégia.
    let strategy_path = format!("config/strategies/{}.toml", args.strategy);
    let strategy_toml = std::fs::read_to_string(&strategy_path)
        .with_context(|| format!("falha ao ler config da estratégia em {}", strategy_path))?;

    let strategy = PullbackTrendV1::from_toml(&strategy_toml)
        .with_context(|| "falha ao fazer parse da configuração TOML da estratégia")?;

    // Setup de banco (usado para persistência e replay).
    let repos = setup_repositories(config).await.ok();

    let broker = SimulatedBroker::new(SimulatedBrokerConfig {
        account_id: Some("DU_SIM".to_string()),
        initial_cash: Decimal::from(100_000),
        commission_per_trade: Decimal::from(35) / Decimal::from(100),
        slippage_pct: Decimal::from(1) / Decimal::from(1000),
    });

    let risk_config = RiskConfig {
        trading_mode: TradingMode::Paper,
        risk_per_trade_pct: Decimal::from(1),
        max_daily_loss_pct: Decimal::from(2),
        max_trades_per_day: 3,
        max_consecutive_losses: 3,
        min_risk_reward: strategy.parameters().min_risk_reward,
        max_spread_pct: strategy.parameters().max_spread_pct,
        max_atr_pct: strategy.parameters().max_atr_pct,
        trading_start_time_utc: parse_time(&strategy.parameters().trading_start_time)
            .unwrap_or((14, 30, 0)),
        trading_end_time_utc: parse_time(&strategy.parameters().trading_end_time)
            .unwrap_or((21, 0, 0)),
    };

    let risk_manager = RiskManager::new(risk_config);
    let engine = ExecutionEngine::new(risk_manager.clone());
    let mut risk_state = RiskState::default();
    let analyzer =
        MarketContextAnalyzer::new(trader_core::context::ContextAnalyzerConfig::default());

    // Flag de shutdown gracioso.
    let shutdown = Arc::new(AtomicBool::new(false));
    let shutdown_clone = shutdown.clone();
    tokio::spawn(async move {
        tokio::signal::ctrl_c()
            .await
            .expect("falha ao escutar Ctrl+C");
        println!("\n🛑 Sinal de parada recebido. Encerrando paper trading...");
        shutdown_clone.store(true, Ordering::SeqCst);
    });

    match args.mode {
        PaperMode::Simulated => {
            let mut candles = crate::synthetic::generate_synthetic_uptrend(&args.symbol);
            let mut tick = interval(Duration::from_millis(500));
            let mut cycle: usize = 0;

            while !shutdown.load(Ordering::SeqCst) {
                cycle += 1;
                tick.tick().await;

                if let Some(next) = crate::synthetic::next_candle(&args.symbol, &candles) {
                    candles.push(next);
                }
                if candles.len() > 80 {
                    candles.drain(0..candles.len() - 80);
                }

                process_candle(
                    &args.symbol,
                    args.timeframe,
                    &candles,
                    &strategy,
                    &analyzer,
                    &broker,
                    &engine,
                    &mut risk_state,
                    repos.as_ref(),
                    cycle,
                )
                .await?;

                // Demo: para automaticamente após 100 ciclos para não rodar eternamente.
                if cycle >= 100 {
                    println!("\n🏁 Limite de ciclos de demonstração atingido. Encerrando.");
                    break;
                }
            }
        }
        PaperMode::Replay => {
            let repo = repos
                .as_ref()
                .map(|r| &r.candle_repo)
                .context("modo replay requer conexão com o banco")?;

            let end = chrono::Utc::now();
            let start = end - chrono::Duration::days(180);
            let candles = repo
                .get_range(&args.symbol, args.timeframe, start, end)
                .await
                .map_err(|e| anyhow::anyhow!("falha ao carregar candles do banco: {e}"))?;

            if candles.is_empty() {
                anyhow::bail!(
                    "nenhum candle encontrado no banco para {} no timeframe {}. \
                     Use 'trader-cli ingest' primeiro ou rode em modo 'simulated'.",
                    args.symbol,
                    args.timeframe
                );
            }

            println!("   Replay de {} candles do banco", candles.len());

            let mut tick = interval(Duration::from_millis(100));
            for (idx, _candle) in candles.iter().enumerate() {
                if shutdown.load(Ordering::SeqCst) {
                    break;
                }
                tick.tick().await;

                // Série até o candle atual (inclusive), simulando tempo real.
                let history = &candles[..=idx];
                process_candle(
                    &args.symbol,
                    args.timeframe,
                    history,
                    &strategy,
                    &analyzer,
                    &broker,
                    &engine,
                    &mut risk_state,
                    repos.as_ref(),
                    idx,
                )
                .await?;
            }
        }
    }

    println!("\n⏹️  Paper trading encerrado.");
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn process_candle(
    symbol: &str,
    timeframe: TimeFrame,
    candles: &[trader_domain::Candle],
    strategy: &PullbackTrendV1,
    analyzer: &MarketContextAnalyzer,
    broker: &SimulatedBroker,
    engine: &ExecutionEngine,
    risk_state: &mut RiskState,
    repos: Option<&Repositories>,
    _cycle: usize,
) -> Result<()> {
    let summary = broker.get_account_summary().await?;
    let positions = broker.get_positions().await?;

    // Atualiza preço de mercado para execução de stops/alvos.
    if let Some(last) = candles.last() {
        broker.set_market_price(symbol, last.close);
    }

    // Sincroniza trades fechados com o estado de risco.
    let closed_trades = broker.get_closed_trades();
    let closed_pnls: Vec<Decimal> = closed_trades.iter().map(|t| t.net_pnl).collect();
    if !closed_pnls.is_empty() {
        engine.sync_risk_state(risk_state, &closed_pnls);
        broker.clear_closed_trades();

        // Persiste trades fechados.
        if let Some(repos) = repos {
            for trade in &closed_trades {
                if let Err(e) = repos.trade_repo.save(trade).await {
                    warn!(error = %e, "falha ao persistir trade");
                }
            }
        }
    }

    // Reconciliação simples: se há posição aberta, não busca novo sinal.
    if !positions.is_empty() {
        return Ok(());
    }

    // Computa e persiste contexto de mercado.
    let ctx = match analyzer.analyze(symbol, timeframe, candles) {
        Some(ctx) => ctx,
        None => return Ok(()),
    };

    if let Some(repos) = repos {
        if let Err(e) = repos.context_repo.save(&ctx).await {
            warn!(error = %e, "falha ao persistir contexto de mercado");
        }
    }

    // Executa estratégia.
    match strategy.analyze(&ctx, &Default::default(), candles) {
        SignalResult::Signal(signal) => {
            info!(
                entry = ?signal.entry_price,
                stop = ?signal.stop_price,
                target = ?signal.target_price,
                "sinal detectado"
            );

            let capital = summary.equity;

            match engine
                .process_signal(broker, &signal, &ctx, None, risk_state, capital)
                .await
            {
                trader_core::execution::ExecutionResult::Executed {
                    order_id,
                    position_size,
                    ..
                } => {
                    println!(
                        "✅ Ordem enviada: {} | tamanho={} | entrada={} | stop={} | alvo={}",
                        order_id,
                        position_size,
                        signal.entry_price.unwrap_or_default(),
                        signal.stop_price.unwrap_or_default(),
                        signal.target_price.unwrap_or_default()
                    );

                    // Persiste sinal e ordem.
                    if let Some(repos) = repos {
                        if let Err(e) = repos.signal_repo.save(&signal).await {
                            warn!(error = %e, "falha ao persistir sinal");
                        }
                        // A ordem foi enviada ao broker simulado; não temos o objeto aqui.
                        // O broker simulado mantém internamente. Em produção, o listener de
                        // eventos do broker persistiria a ordem.
                    }
                }
                trader_core::execution::ExecutionResult::RejectedByRisk { reason, detail } => {
                    warn!(?reason, %detail, "sinal rejeitado pelo risk manager");
                    println!("🚫 Sinal rejeitado: {:?} — {}", reason, detail);

                    let mut rejected = signal;
                    rejected.status = trader_domain::SignalStatus::Rejected;
                    rejected.rejection_reason = Some(reason);
                    rejected.rejection_details = Some(serde_json::json!({ "detail": detail }));

                    if let Some(repos) = repos {
                        if let Err(e) = repos.signal_repo.save(&rejected).await {
                            warn!(error = %e, "falha ao persistir sinal rejeitado");
                        }
                    }
                }
                trader_core::execution::ExecutionResult::RejectedByBroker { error } => {
                    warn!(%error, "ordem rejeitada pelo broker simulado");
                    println!("❌ Ordem rejeitada pelo broker: {}", error);
                }
            }
        }
        SignalResult::Rejected { reason, details } => {
            info!(?reason, ?details, "setup rejeitado");
        }
        _ => {}
    }

    Ok(())
}

struct Repositories {
    #[allow(dead_code)]
    candle_repo: SqlxCandleRepository,
    signal_repo: SqlxSignalRepository,
    #[allow(dead_code)]
    order_repo: SqlxOrderRepository,
    trade_repo: SqlxTradeRepository,
    context_repo: SqlxMarketContextRepository,
}

async fn setup_repositories(config: &CliConfig) -> Result<Repositories> {
    let database_url = config
        .app_config
        .database
        .url()
        .map_err(|e| anyhow::anyhow!("DATABASE_URL não configurada: {e}"))?;

    let pool = create_pool(&database_url)
        .await
        .map_err(|e| anyhow::anyhow!("falha ao conectar no banco: {e}"))?;

    Ok(Repositories {
        candle_repo: SqlxCandleRepository::new(pool.clone()),
        signal_repo: SqlxSignalRepository::new(pool.clone()),
        order_repo: SqlxOrderRepository::new(pool.clone()),
        trade_repo: SqlxTradeRepository::new(pool.clone()),
        context_repo: SqlxMarketContextRepository::new(pool.clone()),
    })
}

fn parse_time(time_str: &str) -> Option<(u32, u32, u32)> {
    let parts: Vec<&str> = time_str.split(':').collect();
    if parts.len() != 3 {
        return None;
    }
    Some((
        parts[0].parse().ok()?,
        parts[1].parse().ok()?,
        parts[2].parse().ok()?,
    ))
}
