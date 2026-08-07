//! Comando `paper`.

use anyhow::{Context, Result};
use rust_decimal::Decimal;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::interval;
use tracing::{info, warn};

use trader_adapters::ibkr::{IbkrBrokerAdapter, IbkrMarketDataProvider};
use trader_adapters::simulated::{SimulatedBroker, SimulatedBrokerConfig};
use trader_core::{
    context::MarketContextAnalyzer,
    execution::time_exit::TimeExitTracker,
    execution::trade_tracker::{classify_exit_reason, FillTracker, TrackerFill},
    execution::ExecutionEngine,
    risk::{RiskManager, RiskState},
};
use trader_domain::{
    Broker, BrokerError, CandleRepository, CandleRequest, Direction, MarketDataProvider, Order,
    OrderEvent, OrderSide, OrderStatus, OrderType, Signal, SignalResult, Strategy, TimeFrame,
    Trade, TradingMode,
};
use trader_infra::{
    db::create_pool,
    repositories::{
        SqlxCandleRepository, SqlxFillRepository, SqlxMarketContextRepository, SqlxOrderRepository,
        SqlxSignalRepository, SqlxSystemEventRepository, SqlxTradeRepository,
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
    Live,
}

impl std::str::FromStr for PaperMode {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "simulated" => Ok(PaperMode::Simulated),
            "replay" => Ok(PaperMode::Replay),
            "live" => Ok(PaperMode::Live),
            other => anyhow::bail!(
                "modo de paper inválido: {}. Use 'simulated', 'replay' ou 'live'",
                other
            ),
        }
    }
}

/// Loop contínuo de paper trading.
///
/// - Modo `simulated`: gera candles sintéticos e opera indefinidamente.
/// - Modo `replay`: carrega candles do banco e opera sobre eles.
/// - Modo `live`: candles reais da IBKR, ordens na conta paper da IBKR.
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
    if args.mode == PaperMode::Live {
        println!("   Aviso:     MODO LIVE — ordens reais serão enviadas à conta PAPER da IBKR");
    }
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

    let strategy = crate::dispatch::load_strategy(&args.strategy, &strategy_toml)?;

    // Saída ativa por tempo (validação pós-entrada em R), quando a estratégia
    // a habilita. Desligada → tracker inerte (no-op).
    let mut time_exit_tracker = TimeExitTracker::new(strategy.time_exit().unwrap_or_default());

    // Setup de banco (usado para persistência e replay). No modo live o banco
    // é OBRIGATÓRIO: sem ele não há auditoria de ordens/trades nem rebuild do
    // estado de risco — falhar fechado em vez de operar às cegas.
    let repos = match setup_repositories(config).await {
        Ok(repos) => Some(repos),
        Err(e) => {
            if args.mode == PaperMode::Live {
                return Err(e).context(
                    "modo live exige banco de dados disponível e migrado (auditoria obrigatória)",
                );
            }
            warn!(error = %e, "banco indisponível; seguindo sem persistência");
            None
        }
    };

    let alerter = crate::alerts::Alerter::new(&config.app_config.alerts.webhook_url);

    let broker = SimulatedBroker::new(SimulatedBrokerConfig {
        account_id: Some("DU_SIM".to_string()),
        initial_cash: Decimal::from(100_000),
        commission_per_trade: Decimal::from(35) / Decimal::from(100),
        slippage_pct: Decimal::from(1) / Decimal::from(1000),
        entry_validity_candles: strategy.entry_validity_candles() as u32,
    });

    // Limites de risco vêm de config/default.toml ([risk]); os filtros de
    // estratégia (RR, spread, ATR, horário) vêm da config da estratégia, que
    // também pode sobrescrever o risco por trade (ex.: 0,5% do failure test).
    // Compartilhado com o backtest para garantir paridade de validação.
    let risk_config =
        crate::risk_config::build_risk_config(&config.app_config.risk, &strategy.risk_params());

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
                    &mut time_exit_tracker,
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
                    &mut time_exit_tracker,
                    repos.as_ref(),
                    idx,
                )
                .await?;
            }
        }
        PaperMode::Live => {
            run_live(
                config,
                &args,
                &strategy,
                &analyzer,
                &engine,
                risk_state,
                time_exit_tracker,
                repos.as_ref(),
                shutdown,
                &alerter,
            )
            .await?;
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
    strategy: &crate::dispatch::LoadedStrategy,
    analyzer: &MarketContextAnalyzer,
    broker: &SimulatedBroker,
    engine: &ExecutionEngine,
    risk_state: &mut RiskState,
    time_exit_tracker: &mut TimeExitTracker,
    repos: Option<&Repositories>,
    _cycle: usize,
) -> Result<()> {
    // Atualiza mercado com o candle completo: stops/alvos avaliados nos
    // extremos intrabar (high/low), não só no fechamento.
    if let Some(last) = candles.last() {
        broker.set_market_candle(symbol, last);

        // Saída ativa por tempo (quando a estratégia habilita): posição que
        // não se validou em R dentro da janela é encerrada a mercado no
        // fechamento — mesma lógica do backtest (paridade).
        match broker.get_position(symbol).await? {
            Some(position) => {
                time_exit_tracker.ensure_tracking(
                    position.avg_entry_price,
                    position.stop_price,
                    position.direction,
                );
                if time_exit_tracker.on_candle_close(last.close)
                    && broker.close_position_at_market(
                        symbol,
                        last.close,
                        trader_domain::ExitReason::Time,
                    )
                {
                    println!(
                        "⏱️  Saída por tempo: posição encerrada a mercado em {}",
                        last.close
                    );
                    time_exit_tracker.reset();
                }
            }
            None => time_exit_tracker.reset(),
        }
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

    // A contagem de trades do dia é feita na entrada (ordem enviada).
    if analyze_and_execute(
        symbol,
        timeframe,
        candles,
        strategy,
        analyzer,
        broker,
        engine,
        risk_state,
        repos,
        "simulated",
    )
    .await?
    .is_some()
    {
        risk_state.daily_trades += 1;
    }

    Ok(())
}

/// Contexto de uma ordem enviada ao broker, para casar fills → trade.
///
/// No modo live, os fills chegam depois (eventos do broker); esta struct
/// carrega os metadados do sinal/ordem necessários para montar o `Trade`
/// quando a posição fechar.
#[derive(Debug, Clone)]
struct PlacedOrderInfo {
    order_db_id: Option<i64>,
    signal_db_id: Option<i64>,
    broker_order_id: String,
    entry_order_type: trader_domain::EntryOrderType,
    /// Candles processados sem o fill da entrada stop (para expiração).
    candles_waited: u32,
    stop_price: Decimal,
    target_price: Option<Decimal>,
    risk_amount: Decimal,
    strategy_id: String,
    strategy_version: String,
    config_hash: String,
}

/// Analisa o candle mais recente e executa o sinal (se houver) via broker genérico.
///
/// Compartilhada pelos três modos. Retorna `Ok(Some(info))` quando uma ordem
/// foi enviada ao broker neste candle, persistindo sinal e ordem.
#[allow(clippy::too_many_arguments)]
async fn analyze_and_execute<B: Broker>(
    symbol: &str,
    timeframe: TimeFrame,
    candles: &[trader_domain::Candle],
    strategy: &crate::dispatch::LoadedStrategy,
    analyzer: &MarketContextAnalyzer,
    broker: &B,
    engine: &ExecutionEngine,
    risk_state: &mut RiskState,
    repos: Option<&Repositories>,
    broker_name: &str,
) -> Result<Option<PlacedOrderInfo>> {
    let summary = broker.get_account_summary().await?;
    let positions = broker.get_positions().await?;

    // Reconciliação simples: se há posição aberta no símbolo, não busca novo sinal.
    if positions.iter().any(|p| p.symbol == symbol) {
        return Ok(None);
    }

    // Computa e persiste contexto de mercado.
    let ctx = match analyzer.analyze(symbol, timeframe, candles) {
        Some(ctx) => ctx,
        None => return Ok(None),
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
                    risk_amount,
                } => {
                    println!(
                        "✅ Ordem enviada: {} | tamanho={} | entrada={} | stop={} | alvo={}",
                        order_id,
                        position_size,
                        signal.entry_price.unwrap_or_default(),
                        signal.stop_price.unwrap_or_default(),
                        signal.target_price.unwrap_or_default()
                    );

                    // Persiste sinal e ordem (trilha de auditoria).
                    let mut signal_db_id = None;
                    let mut order_db_id = None;
                    if let Some(repos) = repos {
                        match repos.signal_repo.save(&signal).await {
                            Ok(id) => signal_db_id = Some(id),
                            Err(e) => warn!(error = %e, "falha ao persistir sinal"),
                        }

                        let order = build_placed_order(
                            &signal,
                            position_size,
                            &order_id,
                            broker_name,
                            signal_db_id,
                        );
                        match repos.order_repo.save(&order).await {
                            Ok(id) => order_db_id = Some(id),
                            Err(e) => warn!(error = %e, "falha ao persistir ordem"),
                        }
                    }

                    return Ok(Some(PlacedOrderInfo {
                        order_db_id,
                        signal_db_id,
                        broker_order_id: order_id.clone(),
                        entry_order_type: signal.entry_order_type,
                        candles_waited: 0,
                        stop_price: signal.stop_price.unwrap_or_default(),
                        target_price: signal.target_price,
                        risk_amount,
                        strategy_id: signal.strategy_id.clone(),
                        strategy_version: signal.strategy_version.clone(),
                        config_hash: signal.config_hash.clone(),
                    }));
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
                    warn!(%error, "ordem rejeitada pelo broker");
                    println!("❌ Ordem rejeitada pelo broker: {}", error);
                }
            }
        }
        SignalResult::Rejected { reason, details } => {
            info!(?reason, ?details, "setup rejeitado");
        }
        _ => {}
    }

    Ok(None)
}

/// Monta a `Order` de domínio de uma ordem recém-aceita pelo broker, para
/// persistência imediata (fills chegam depois, via eventos).
fn build_placed_order(
    signal: &Signal,
    position_size: Decimal,
    broker_order_id: &str,
    broker_name: &str,
    signal_db_id: Option<i64>,
) -> Order {
    let side = match signal.direction {
        Direction::Long => OrderSide::Buy,
        Direction::Short => OrderSide::Sell,
    };

    let mut order = Order::new(
        &signal.symbol,
        side,
        OrderType::Bracket,
        position_size,
        broker_name,
    )
    .unwrap_or_else(|_| {
        // position_size > 0 é garantido pelo RiskManager; fallback defensivo.
        Order::new(
            &signal.symbol,
            side,
            OrderType::Bracket,
            Decimal::ONE,
            broker_name,
        )
        .expect("ordem com quantidade 1 é sempre válida")
    });

    order.signal_id = signal_db_id;
    order.broker_order_id = Some(broker_order_id.to_string());
    order.entry_order_type = signal.entry_order_type;
    order.price = signal.entry_price;
    order.stop_price = signal.stop_price;
    order.target_price = signal.target_price;
    order.status = OrderStatus::Submitted;
    order.submitted_at = Some(chrono::Utc::now());
    order.metadata = serde_json::json!({
        "entry_order_type": signal.entry_order_type,
    });
    order
}

/// Intervalo de poll do modo live.
const LIVE_POLL_SECS: u64 = 30;

/// Número máximo de candles mantidos na janela de análise do modo live.
/// 200 candles ≈ 8 dias de 15min — aproxima o contexto do live do histórico
/// completo do backtest (com 80, EMA20/sequência de tendência divergiam nas
/// bordas e setups marginais sumiam; divergência observada em 2026-08-05).
const LIVE_MAX_CANDLES: usize = 200;

/// Falhas consecutivas de infra (dados/reconciliação) que disparam o
/// circuit breaker: o live encerra com erro em vez de ficar "morto" em
/// silêncio.
const LIVE_MAX_CONSECUTIVE_FAILURES: u32 = 10;

/// Circuit breaker: estoura o limite → alerta crítico + evento + erro fatal.
async fn check_circuit_breaker(
    consecutive_failures: u32,
    context: &str,
    repos: Option<&Repositories>,
    alerter: &crate::alerts::Alerter,
) -> Result<()> {
    if consecutive_failures < LIVE_MAX_CONSECUTIVE_FAILURES {
        return Ok(());
    }

    let message = format!("circuit breaker: {context} ({consecutive_failures} seguidas)");
    record_event(repos, "critical", "live", "circuit_breaker", &message).await;
    // Aguarda a entrega: o processo encerra logo em seguida e o
    // fire-and-forget perderia o alerta mais importante.
    alerter.critical_await(&message).await;
    anyhow::bail!("{message}");
}

/// Registra um evento de sistema (melhor esforço).
async fn record_event(
    repos: Option<&Repositories>,
    level: &str,
    component: &str,
    event_type: &str,
    message: &str,
) {
    if let Some(repos) = repos {
        if let Err(e) = repos
            .event_repo
            .record(level, component, event_type, message, None)
            .await
        {
            warn!(error = %e, "falha ao registrar evento de sistema");
        }
    }
}

/// Loop do modo `live`: paper trading em tempo real contra a conta paper da IBKR.
///
/// - Candles reais via `IbkrMarketDataProvider::get_historical_candles`.
/// - Ordens (bracket: entrada + stop + alvo) via `IbkrBrokerAdapter` — stop e
///   alvo ficam server-side na IBKR, sem simulação local de stops.
/// - A cada ciclo processa apenas candles fechados e ainda não processados
///   (dedupe pelo timestamp do último candle processado).
#[allow(clippy::too_many_arguments)]
async fn run_live(
    config: &CliConfig,
    args: &Args,
    strategy: &crate::dispatch::LoadedStrategy,
    analyzer: &MarketContextAnalyzer,
    engine: &ExecutionEngine,
    mut risk_state: RiskState,
    time_exit_tracker: TimeExitTracker,
    repos: Option<&Repositories>,
    shutdown: Arc<AtomicBool>,
    alerter: &crate::alerts::Alerter,
) -> Result<()> {
    let ibkr_config = config.ibkr_config()?;

    // Guardas de segurança contra operar conta real "por engano": além do
    // check de `app.mode`, exigimos ibkr.paper=true e uma porta de paper.
    if !ibkr_config.paper {
        anyhow::bail!(
            "ibkr.paper=false na configuração — o modo live só pode apontar para ambiente paper"
        );
    }
    if matches!(ibkr_config.port, 7496 | 4001) {
        anyhow::bail!(
            "porta {} é de conta REAL (7496=TWS, 4001=Gateway). Use 7497 (TWS paper) ou 4002 (Gateway paper)",
            ibkr_config.port
        );
    }

    let connection = ibkr_config.connection_string();
    let market_data = IbkrMarketDataProvider::new(ibkr_config.clone());
    let broker = IbkrBrokerAdapter::new(ibkr_config);

    warn!(
        gateway = %connection,
        "MODO LIVE: ordens reais serão enviadas à conta PAPER da IBKR; stop e alvo server-side (bracket)"
    );
    println!(
        "   Live IBKR: gateway {} | poll a cada {}s\n",
        connection, LIVE_POLL_SECS
    );

    // Stream de eventos de ordem (polling de execuções do dia na IBKR).
    // Os fills alimentam o FillTracker, que monta trades fechados →
    // persistência + atualização do RiskState (perdas consecutivas, P&L).
    let (event_tx, mut event_rx) = tokio::sync::mpsc::channel::<OrderEvent>(256);
    if let Err(e) = broker.subscribe_order_events(event_tx).await {
        warn!(error = %e, "falha ao assinar eventos de ordem; fills não serão rastreados nesta sessão");
    }

    let mut live_fills = LiveFillState {
        time_exit: time_exit_tracker,
        ..Default::default()
    };

    // Recupera ordem em aberto de uma sessão anterior (restart com posição):
    // religa o tracker aos fills já persistidos para não perder o trade.
    if let Some(repos) = repos {
        match recover_open_order(repos, &args.symbol, &mut live_fills).await {
            Ok(true) => info!("ordem em aberto recuperada do banco; tracker de fills religado"),
            Ok(false) => {}
            Err(e) => warn!(error = %e, "falha ao recuperar ordem em aberto do banco"),
        }

        // Reconstrói o estado de risco do dia a partir do banco: um restart no
        // meio do pregão não pode zerar perda diária nem perdas consecutivas.
        risk_state = rebuild_risk_state(repos, &args.symbol).await;
        info!(
            daily_pnl = %risk_state.daily_pnl,
            daily_trades = risk_state.daily_trades,
            consecutive_losses = risk_state.consecutive_losses,
            "estado de risco reconstruído do banco"
        );
    }

    let mut tick = interval(Duration::from_secs(LIVE_POLL_SECS));
    let mut last_processed: Option<chrono::DateTime<chrono::Utc>> = None;
    let mut current_day = chrono::Utc::now().date_naive();
    let mut consecutive_failures: u32 = 0;

    record_event(
        repos,
        "info",
        "live",
        "live_started",
        &format!("live iniciado: {} @ {}", args.symbol, connection),
    )
    .await;
    alerter.info(&format!("Live iniciado: {} @ {}", args.symbol, connection));

    while !shutdown.load(Ordering::SeqCst) {
        tick.tick().await;

        // Processa fills pendentes (monta trades, atualiza risco).
        drain_order_events(
            &mut event_rx,
            &mut live_fills,
            &args.symbol,
            repos,
            engine,
            &mut risk_state,
            alerter,
        )
        .await;

        // Rollover diário (UTC): reconstrói os contadores do novo dia a
        // partir do banco (normalmente zerados, a menos que já haja trades
        // persistidos hoje).
        let today = chrono::Utc::now().date_naive();
        if today != current_day {
            current_day = today;
            risk_state = match repos {
                Some(repos) => rebuild_risk_state(repos, &args.symbol).await,
                None => RiskState::default(),
            };
            info!("novo dia UTC: estado de risco diário reconstruído do banco");
        }

        // P&L diário e perdas consecutivas vêm de trades reais (rebuild no
        // boot + sync a cada trade fechado) — não há mais aproximação por
        // equity, que mascarava o estado de risco após restart.

        // Janela de candles reais: últimos 10 dias (enche os 200 candles de
        // contexto mesmo com fins de semana/feriados no meio).
        let now = chrono::Utc::now();
        let request = CandleRequest {
            symbol: args.symbol.clone(),
            timeframe: args.timeframe,
            from: now - chrono::Duration::days(10),
            to: now,
        };
        let mut candles = match market_data.get_historical_candles(request).await {
            Ok(candles) => {
                consecutive_failures = 0;
                candles
            }
            Err(e) => {
                consecutive_failures += 1;
                warn!(error = %e, consecutive_failures, "falha ao buscar candles na IBKR; tentando no próximo ciclo");
                check_circuit_breaker(
                    consecutive_failures,
                    "falhas consecutivas ao buscar candles na IBKR",
                    repos,
                    alerter,
                )
                .await?;
                continue;
            }
        };
        if candles.len() > LIVE_MAX_CANDLES {
            candles.drain(0..candles.len() - LIVE_MAX_CANDLES);
        }

        // Candle fechado = cujo fechamento nominal (timestamp + timeframe) já
        // passou. A heurística anterior ("último da série está incompleto")
        // falhava entre sessões: o último candle de ONTEM era tratado como
        // "em formação", e o candle anterior a ele era processado como novo —
        // risco de operar setup velho no startup.
        let tf_dur = args.timeframe.duration();
        let closed = candles
            .iter()
            .filter(|c| c.timestamp + tf_dur <= now)
            .count();

        // No primeiro ciclo, apenas sincroniza o cursor: evita operar setups de
        // candles antigos (possivelmente de horas atrás) logo no startup.
        if last_processed.is_none() {
            if let Some(ts) = candles.get(closed.saturating_sub(1)).map(|c| c.timestamp) {
                info!(%ts, "cursor sincronizado; aguardando próximo candle fechado");
                last_processed = Some(ts);
            }
            continue;
        }

        let new_indices: Vec<usize> = (0..closed)
            .filter(|&i| last_processed.is_none_or(|ts| candles[i].timestamp > ts))
            .collect();

        for i in new_indices {
            if shutdown.load(Ordering::SeqCst) {
                break;
            }

            // Persiste o candle fechado processado (melhor esforço; dedupe
            // por (symbol, timeframe, timestamp) torna idempotente). Sem
            // isso, candles do dia só entravam no banco via ingest manual.
            if let Some(repos) = repos {
                if let Err(e) = repos.candle_repo.save(&[candles[i].clone()]).await {
                    warn!(error = %e, "falha ao persistir candle do live");
                }
            }

            // Expiração da entrada stop (regra do livro): se o rompimento não
            // aconteceu em `entry_validity_candles` candles, cancela a ordem
            // que está trabalhando e libera para o próximo setup.
            if expire_stale_stop_entry(
                &broker,
                &mut live_fills,
                strategy.entry_validity_candles() as u32,
                repos,
            )
            .await?
            {
                println!("⏳ Entrada stop expirada sem rompimento; ordem cancelada");
                last_processed = Some(candles[i].timestamp);
                continue;
            }

            // Saída ativa por tempo (quando a estratégia habilita): posição
            // aberta que não se validou em R dentro da janela é encerrada a
            // mercado no fechamento — mesma lógica do backtest (paridade).
            if live_fills.tracker.is_open() {
                match broker.get_position(&args.symbol).await {
                    Ok(Some(position)) => {
                        live_fills.time_exit.ensure_tracking(
                            position.avg_entry_price,
                            position.stop_price,
                            position.direction,
                        );
                        if live_fills.time_exit.on_candle_close(candles[i].close) {
                            match close_position_at_market(&broker, &args.symbol, &position).await {
                                Ok(()) => {
                                    println!(
                                        "⏱️  Saída por tempo: posição encerrada a mercado em {}",
                                        candles[i].close
                                    );
                                    live_fills.time_exit_triggered = true;
                                    live_fills.time_exit.reset();
                                    // O fill da ordem de fechamento chega pelo
                                    // stream de eventos e fecha o trade no
                                    // FillTracker (finalize_live_trade).
                                }
                                Err(e) => {
                                    consecutive_failures += 1;
                                    warn!(error = %e, consecutive_failures, "falha ao encerrar posição na saída por tempo");
                                    check_circuit_breaker(
                                        consecutive_failures,
                                        "falha ao encerrar posição na saída por tempo",
                                        repos,
                                        alerter,
                                    )
                                    .await?;
                                }
                            }
                        }
                    }
                    Ok(None) => live_fills.time_exit.reset(),
                    Err(e) => {
                        consecutive_failures += 1;
                        warn!(error = %e, consecutive_failures, "falha ao consultar posição para saída por tempo");
                        check_circuit_breaker(
                            consecutive_failures,
                            "falha ao consultar posição para saída por tempo",
                            repos,
                            alerter,
                        )
                        .await?;
                    }
                }
            } else {
                live_fills.time_exit.reset();
            }

            // Reconciliação: posição aberta OU ordem pendente no símbolo impede
            // novo sinal. A checagem de ordens abertas evita entradas duplicadas
            // enquanto a limit de entrada do bracket não é preenchida.
            match has_exposure(&broker, &args.symbol).await {
                Ok(true) => {
                    info!(symbol = %args.symbol, "exposição existente (posição ou ordem aberta); sem novo sinal");
                    last_processed = Some(candles[i].timestamp);
                    break;
                }
                Err(e) => {
                    consecutive_failures += 1;
                    warn!(error = %e, consecutive_failures, "falha na reconciliação com o broker; ciclo ignorado por segurança");
                    check_circuit_breaker(
                        consecutive_failures,
                        "falhas consecutivas na reconciliação com o broker",
                        repos,
                        alerter,
                    )
                    .await?;
                    break;
                }
                Ok(false) => {}
            }

            match analyze_and_execute(
                &args.symbol,
                args.timeframe,
                &candles[..=i],
                strategy,
                analyzer,
                &broker,
                engine,
                &mut risk_state,
                repos,
                "ibkr",
            )
            .await
            {
                Ok(Some(placed)) => {
                    // Cada execução conta para o limite de trades do dia.
                    risk_state.daily_trades += 1;
                    // A ordem passa a ser rastreada: fills dela (e das filhas
                    // do bracket) fecharão o trade via drain_order_events.
                    live_fills.open_order = Some(placed);
                }
                Ok(None) => {}
                Err(e) => {
                    warn!(error = %e, "falha ao processar candle; próximo ciclo tentará novamente")
                }
            }
            last_processed = Some(candles[i].timestamp);
        }
    }

    record_event(repos, "info", "live", "live_stopped", "live encerrado").await;
    // Aguarda a entrega: o processo encerra ao retornar.
    alerter.info_await("Live encerrado").await;
    Ok(())
}

/// Estado do rastreamento de fills do modo live.
#[derive(Default)]
struct LiveFillState {
    tracker: FillTracker,
    open_order: Option<PlacedOrderInfo>,
    /// Saída ativa por tempo (validação pós-entrada em R) da posição aberta.
    time_exit: TimeExitTracker,
    /// Marca que o fechamento em andamento foi disparado pela saída por
    /// tempo — o trade fechado deve sair com `ExitReason::Time`.
    time_exit_triggered: bool,
}

/// Expira uma entrada stop que não rompeu em `validity` candles.
///
/// Retorna `Ok(true)` quando a ordem foi cancelada (chamador deve liberar o
/// ciclo para novos setups). Se o cancelamento falhar (ex.: a ordem acabou de
/// ser preenchida e o fill ainda não chegou), mantém o rastreamento — o fill
/// será tratado normalmente quando chegar.
async fn expire_stale_stop_entry<B: Broker>(
    broker: &B,
    state: &mut LiveFillState,
    validity: u32,
    repos: Option<&Repositories>,
) -> Result<bool> {
    let (should_cancel, broker_order_id) = {
        if state.tracker.is_open() {
            return Ok(false);
        }
        let Some(pending) = state.open_order.as_mut() else {
            return Ok(false);
        };
        if pending.entry_order_type != trader_domain::EntryOrderType::Stop {
            return Ok(false);
        }
        pending.candles_waited += 1;
        (
            pending.candles_waited >= validity && !pending.broker_order_id.is_empty(),
            pending.broker_order_id.clone(),
        )
    };

    if !should_cancel {
        return Ok(false);
    }

    let id = trader_domain::OrderId::from(broker_order_id);
    if let Err(e) = broker.cancel_order(&id).await {
        warn!(error = %e, "falha ao cancelar entrada stop expirada; mantendo rastreamento");
        return Ok(false);
    }

    info!(order_id = %id, "entrada stop expirada sem rompimento; cancelada");
    if let (Some(repos), Some(pending)) = (repos, state.open_order.as_ref()) {
        if let Some(db_id) = pending.order_db_id {
            if let Err(e) = repos
                .order_repo
                .update_status(db_id, OrderStatus::Cancelled, None, None)
                .await
            {
                warn!(error = %e, "falha ao marcar ordem expirada como cancelada");
            }
        }
    }
    state.open_order = None;
    Ok(true)
}

/// Drena eventos de ordem pendentes: persiste fills novos, monta o trade
/// quando a posição zera e sincroniza o estado de risco.
///
/// Idempotência: fills já presentes no banco (replay do dia, restart) são
/// ignorados — nunca contam P&L em dobro.
async fn drain_order_events(
    rx: &mut tokio::sync::mpsc::Receiver<OrderEvent>,
    state: &mut LiveFillState,
    symbol: &str,
    repos: Option<&Repositories>,
    engine: &ExecutionEngine,
    risk_state: &mut RiskState,
    alerter: &crate::alerts::Alerter,
) {
    while let Ok(event) = rx.try_recv() {
        let OrderEvent::Fill { order_id, fill } = event else {
            // StatusUpdate não é emitido pela implementação atual (polling de
            // execuções); se um dia for, tratamos aqui.
            continue;
        };

        if fill.symbol != symbol {
            warn!(
                symbol = %fill.symbol,
                "fill de outro símbolo ignorado (operação fora do bot?)"
            );
            continue;
        }

        let Some(ctx) = state.open_order.clone() else {
            warn!(
                broker_order_id = %order_id,
                "fill sem ordem rastreada (bot reiniciado ou operação manual); ignorado"
            );
            continue;
        };

        // Persiste o fill atrelado à ordem pai do bracket. Fills de saída
        // (stop/alvo) pertencem a ordens filhas que não existem no banco;
        // o lado do fill está na própria linha (coluna `side`).
        let mut fill = fill;
        let Some(order_db_id) = ctx.order_db_id else {
            warn!("ordem rastreada sem id de banco; fill não persistido");
            continue;
        };
        fill.order_id = order_db_id;

        let inserted = match repos {
            Some(repos) => match repos.fill_repo.save(&fill).await {
                Ok(id) => id.is_some(),
                Err(e) => {
                    warn!(error = %e, "falha ao persistir fill; ignorado");
                    continue;
                }
            },
            // Sem banco, não há como auditar nem deduplicar: ignora.
            None => {
                warn!("sem banco de dados; fill descartado");
                continue;
            }
        };

        if !inserted {
            continue; // replay de execução já processada
        }

        let closed = state.tracker.on_fill(TrackerFill {
            side: fill.side,
            price: fill.fill_price,
            quantity: fill.quantity,
            commission: fill.commission,
            timestamp: fill.timestamp,
        });

        if let Some(closed) = closed {
            finalize_live_trade(
                state, &ctx, &closed, symbol, repos, engine, risk_state, alerter,
            )
            .await;
        }
    }
}

/// Persiste o trade fechado, atualiza a ordem e sincroniza o estado de risco.
#[allow(clippy::too_many_arguments)]
async fn finalize_live_trade(
    state: &mut LiveFillState,
    ctx: &PlacedOrderInfo,
    closed: &trader_core::execution::trade_tracker::ClosedTrade,
    symbol: &str,
    repos: Option<&Repositories>,
    engine: &ExecutionEngine,
    risk_state: &mut RiskState,
    alerter: &crate::alerts::Alerter,
) {
    let exit_reason = if state.time_exit_triggered {
        trader_domain::ExitReason::Time
    } else {
        classify_exit_reason(closed.exit_price, ctx.stop_price, ctx.target_price)
    };
    state.time_exit_triggered = false;
    let result_in_r = if ctx.risk_amount > Decimal::ZERO {
        closed.net_pnl / ctx.risk_amount
    } else {
        Decimal::ZERO
    };

    let trade = Trade {
        id: None,
        symbol: symbol.to_string(),
        signal_id: ctx.signal_db_id.unwrap_or_default(),
        position_id: None,
        direction: closed.direction,
        entry_price: closed.entry_price,
        exit_price: closed.exit_price,
        quantity: closed.quantity,
        entry_time: closed.entry_time,
        exit_time: closed.exit_time,
        stop_price: ctx.stop_price,
        target_price: ctx.target_price,
        gross_pnl: closed.gross_pnl,
        commissions: closed.commissions,
        fees: Decimal::ZERO,
        net_pnl: closed.net_pnl,
        risk_amount: ctx.risk_amount,
        result_in_r,
        exit_reason,
        strategy_id: ctx.strategy_id.clone(),
        strategy_version: ctx.strategy_version.clone(),
        config_hash: ctx.config_hash.clone(),
        journal: serde_json::json!({ "source": "live_fills" }),
        correlation_id: uuid::Uuid::new_v4().to_string(),
    };

    println!(
        "🏁 Trade fechado: {:?} {} | entrada={} saída={} | P&L={} ({:?})",
        closed.direction,
        closed.quantity,
        closed.entry_price,
        closed.exit_price,
        closed.net_pnl,
        exit_reason
    );
    info!(
        net_pnl = %closed.net_pnl,
        ?exit_reason,
        "trade do live fechado"
    );

    if let Some(repos) = repos {
        if let Err(e) = repos.trade_repo.save(&trade).await {
            warn!(error = %e, "falha ao persistir trade do live");
        }
        if let Some(order_db_id) = ctx.order_db_id {
            if let Err(e) = repos
                .order_repo
                .update_status(
                    order_db_id,
                    OrderStatus::Filled,
                    Some(closed.quantity),
                    Some(closed.entry_price),
                )
                .await
            {
                warn!(error = %e, "falha ao atualizar status da ordem");
            }
        }
    }

    // Atualiza perdas consecutivas e P&L diário com o resultado real.
    engine.sync_risk_state(risk_state, &[closed.net_pnl]);
    state.open_order = None;

    alerter.info(&format!(
        "Trade fechado: {} {} @ {} → {} | P&L {} ({:?})",
        closed.quantity, symbol, closed.entry_price, closed.exit_price, closed.net_pnl, exit_reason
    ));

    // Limites de risco atingidos merecem alerta crítico.
    if risk_state.consecutive_losses > 0 {
        record_event(
            repos,
            "warning",
            "live",
            "trade_closed",
            &format!(
                "trade fechado com P&L {} ({:?})",
                closed.net_pnl, exit_reason
            ),
        )
        .await;
    }
}

/// Recupera uma ordem em aberto de sessão anterior e religa o tracker aos
/// fills já persistidos dela. Retorna `Ok(true)` se havia ordem a recuperar.
async fn recover_open_order(
    repos: &Repositories,
    symbol: &str,
    state: &mut LiveFillState,
) -> Result<bool> {
    let open = repos
        .order_repo
        .list_open()
        .await
        .map_err(|e| anyhow::anyhow!("falha ao listar ordens abertas: {e}"))?;

    let Some(order) = open.into_iter().find(|o| o.symbol == symbol) else {
        return Ok(false);
    };

    let (Some(order_db_id), Some(signal_db_id)) = (order.id, order.signal_id) else {
        warn!(?order.broker_order_id, "ordem aberta sem id/sinal; não recuperável");
        return Ok(false);
    };

    let signal = repos
        .signal_repo
        .get_by_id(signal_db_id)
        .await
        .map_err(|e| anyhow::anyhow!("falha ao carregar sinal da ordem aberta: {e}"))?
        .ok_or_else(|| anyhow::anyhow!("sinal {signal_db_id} da ordem aberta não encontrado"))?;

    // Replay dos fills já persistidos para reconstruir o estado do tracker.
    let fills = repos
        .fill_repo
        .list_by_order(order_db_id)
        .await
        .map_err(|e| anyhow::anyhow!("falha ao carregar fills da ordem aberta: {e}"))?;
    for f in &fills {
        state.tracker.on_fill(TrackerFill {
            side: f.side,
            price: f.fill_price,
            quantity: f.quantity,
            commission: f.commission,
            timestamp: f.timestamp,
        });
    }

    let risk_amount =
        signal
            .risk_amount
            .unwrap_or_else(|| match (signal.entry_price, signal.stop_price) {
                (Some(entry), Some(stop)) => (entry - stop).abs() * order.quantity,
                _ => Decimal::ZERO,
            });

    state.open_order = Some(PlacedOrderInfo {
        order_db_id: Some(order_db_id),
        signal_db_id: Some(signal_db_id),
        broker_order_id: order.broker_order_id.clone().unwrap_or_default(),
        entry_order_type: order.entry_order_type,
        candles_waited: 0,
        stop_price: order.stop_price.unwrap_or_default(),
        target_price: order.target_price,
        risk_amount,
        strategy_id: signal.strategy_id,
        strategy_version: signal.strategy_version,
        config_hash: signal.config_hash,
    });

    Ok(true)
}

/// Reconstrói o `RiskState` do dia a partir do banco: P&L e perdas
/// consecutivas dos trades fechados hoje, e contagem de sinais executados.
///
/// Garante que um restart do processo não zere os limites diários no meio do
/// pregão (regra de segurança financeira).
async fn rebuild_risk_state(repos: &Repositories, symbol: &str) -> RiskState {
    let mut state = RiskState::default();

    match repos.trade_repo.list_today(symbol).await {
        Ok(trades) => {
            // Artefatos operacionais (bug já corrigido) não contam para o
            // estado de risco — não são perdas da estratégia.
            let trades: Vec<_> = trades
                .into_iter()
                .filter(|t| !t.is_latency_artifact())
                .collect();
            state.daily_pnl = trades.iter().map(|t| t.net_pnl).sum();
            // list_today retorna em ordem DESC de saída: as perdas no topo da
            // lista são exatamente a sequência atual de perdas consecutivas.
            state.consecutive_losses = trades
                .iter()
                .take_while(|t| t.net_pnl < Decimal::ZERO)
                .count();
        }
        Err(e) => warn!(error = %e, "falha ao reconstruir P&L do dia; começando zerado"),
    }

    // Trades do dia = sinais executados (não rejeitados) hoje. Contar na
    // entrada evita que um trade aberto o dia inteiro fure o limite diário.
    match repos.signal_repo.list_today(symbol).await {
        Ok(signals) => {
            state.daily_trades = signals
                .iter()
                .filter(|s| s.status != trader_domain::SignalStatus::Rejected)
                .count();
        }
        Err(e) => warn!(error = %e, "falha ao reconstruir contagem de trades do dia"),
    }

    state
}

/// Verifica se já existe exposição no símbolo: posição aberta ou ordem pendente.
async fn has_exposure<B: Broker>(broker: &B, symbol: &str) -> Result<bool, BrokerError> {
    if broker.get_position(symbol).await?.is_some() {
        return Ok(true);
    }
    let open_orders = broker.get_open_orders().await?;
    Ok(open_orders.iter().any(|o| o.symbol == symbol))
}

/// Encerra uma posição aberta a mercado (saída ativa — ex.: saída por tempo).
///
/// Cancela antes as ordens abertas do símbolo (pernas stop/alvo do bracket
/// server-side) para não deixar ordem órfã trabalhando após o fechamento.
async fn close_position_at_market<B: Broker>(
    broker: &B,
    symbol: &str,
    position: &trader_domain::Position,
) -> Result<()> {
    let open_orders = broker.get_open_orders().await?;
    for order in open_orders.iter().filter(|o| o.symbol == symbol) {
        if let Some(broker_order_id) = &order.broker_order_id {
            let id = trader_domain::OrderId::from(broker_order_id.clone());
            if let Err(e) = broker.cancel_order(&id).await {
                warn!(order_id = %id, error = %e, "falha ao cancelar perna do bracket na saída ativa");
            }
        }
    }

    let side = match position.direction {
        Direction::Long => OrderSide::Sell,
        Direction::Short => OrderSide::Buy,
    };
    let order = Order::new(symbol, side, OrderType::Market, position.quantity, "ibkr")
        .map_err(|e| anyhow::anyhow!("ordem de fechamento inválida: {e}"))?;
    broker
        .place_order(order)
        .await
        .map_err(|e| anyhow::anyhow!("falha ao enviar ordem de fechamento a mercado: {e}"))?;
    info!(%symbol, quantity = %position.quantity, "ordem de fechamento a mercado enviada (saída ativa)");
    Ok(())
}

struct Repositories {
    #[allow(dead_code)]
    candle_repo: SqlxCandleRepository,
    signal_repo: SqlxSignalRepository,
    order_repo: SqlxOrderRepository,
    fill_repo: SqlxFillRepository,
    trade_repo: SqlxTradeRepository,
    context_repo: SqlxMarketContextRepository,
    event_repo: SqlxSystemEventRepository,
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

    // Migrações sobem no startup: o schema nunca fica para trás.
    trader_infra::db::run_migrations(&pool)
        .await
        .map_err(|e| anyhow::anyhow!("falha ao rodar migrações do banco: {e}"))?;

    Ok(Repositories {
        candle_repo: SqlxCandleRepository::new(pool.clone()),
        signal_repo: SqlxSignalRepository::new(pool.clone()),
        order_repo: SqlxOrderRepository::new(pool.clone()),
        fill_repo: SqlxFillRepository::new(pool.clone()),
        trade_repo: SqlxTradeRepository::new(pool.clone()),
        context_repo: SqlxMarketContextRepository::new(pool.clone()),
        event_repo: SqlxSystemEventRepository::new(pool.clone()),
    })
}
