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
    OrderEvent, OrderSide, OrderStatus, OrderType, RejectionReason, Signal, SignalResult, Strategy,
    TimeFrame, Trade, TradingMode,
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

    // Limites de risco vêm de config/default.toml ([risk]); os filtros de
    // estratégia (RR, spread, ATR, horário) vêm da config da estratégia, que
    // também pode sobrescrever o risco por trade (ex.: 0,5% do failure test).
    // Compartilhado com o backtest para garantir paridade de validação.
    let risk_config =
        crate::risk_config::build_risk_config(&config.app_config.risk, &strategy.risk_params())?;

    let broker = SimulatedBroker::new(SimulatedBrokerConfig {
        account_id: Some("DU_SIM".to_string()),
        initial_cash: Decimal::from(100_000),
        commission_per_trade: Decimal::from(35) / Decimal::from(100),
        slippage_pct: Decimal::from(1) / Decimal::from(1000),
        entry_validity_candles: strategy.entry_validity_candles() as u32,
        // Paridade com live/backtest: mesma tolerância de overshoot (ADR-015).
        entry_overshoot_tolerance: risk_config.entry_overshoot_tolerance,
    });

    let risk_manager = RiskManager::new(risk_config);
    let engine = ExecutionEngine::new(risk_manager.clone());
    let mut risk_state = RiskState::default();
    let analyzer =
        MarketContextAnalyzer::new(trader_core::context::ContextAnalyzerConfig::default());

    // Flag de shutdown gracioso.
    //
    // O flag sozinho não basta: o laço do live espera até 30s no tick antes de
    // olhar para ele, e o `docker stop` manda SIGKILL depois de 10s. Em
    // 04/09/2026, já com o SIGTERM tratado, só 2 das 11 instâncias
    // conseguiram registrar `live_stopped` — as outras 9 morreram esperando o
    // tick virar. O `Notify` acorda a espera na hora.
    let shutdown = Arc::new(AtomicBool::new(false));
    let shutdown_notify = Arc::new(tokio::sync::Notify::new());
    let shutdown_clone = shutdown.clone();
    let notify_clone = shutdown_notify.clone();
    tokio::spawn(async move {
        wait_for_stop_signal().await;
        println!("\n🛑 Sinal de parada recebido. Encerrando paper trading...");
        shutdown_clone.store(true, Ordering::SeqCst);
        notify_clone.notify_waiters();
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
                    &config.app_config.risk,
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
                    &config.app_config.risk,
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
                shutdown_notify,
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
    risk_settings: &trader_infra::config::RiskSettings,
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
        candles.last().map(|c| c.close),
        risk_settings,
    )
    .await?
    .placed
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
    // Preço mais fresco conhecido no momento do envio, para a guarda de
    // overshoot (ADR-015). No live é o close da barra em formação do último
    // fetch; em simulated/replay, o close da barra do sinal.
    reference_price: Option<Decimal>,
    // Limites da conta inteira (C2): precisam da config crua, não da
    // RiskConfig por estratégia.
    risk_settings: &trader_infra::config::RiskSettings,
) -> Result<ExecOutcome> {
    let summary = broker.get_account_summary().await?;
    let positions = broker.get_positions().await?;

    // Reconciliação simples: se há posição aberta no símbolo, não busca novo sinal.
    if positions.iter().any(|p| p.symbol == symbol) {
        return Ok(ExecOutcome::default());
    }

    // Limite de risco da CONTA INTEIRA (C2 da auditoria de 30/08/2026). Todo o
    // controle de risco do projeto é por processo: cada instância valida
    // contra o próprio estado e só soma trades do próprio símbolo. Com 11
    // instâncias na mesma conta, a perda diária efetiva era 11× o limite
    // configurado e o cap de notional valia por processo — a exposição
    // agregada podia passar de várias vezes a equity, em small-caps
    // correlacionados onde as perdas chegam juntas.
    //
    // As duas fontes são autoritativas e não exigem tabela nova: o BROKER diz
    // a exposição real da conta, o BANCO diz o P&L realizado do dia.
    if let Some(motivo) =
        portfolio_limit_hit(&positions, summary.equity, repos, risk_settings).await
    {
        info!(%symbol, %motivo, "limite de portfólio atingido; sem novo sinal");
        record_event(
            repos,
            "warning",
            "risk",
            "portfolio_limit",
            &format!("{symbol}: entrada bloqueada pelo limite da conta - {motivo}"),
        )
        .await;
        return Ok(ExecOutcome {
            blocked: Some(BlockReason::Portfolio(motivo)),
            ..Default::default()
        });
    }

    // Computa e persiste contexto de mercado.
    let ctx = match analyzer.analyze(symbol, timeframe, candles) {
        Some(ctx) => ctx,
        None => return Ok(ExecOutcome::default()),
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
                .process_signal(
                    broker,
                    &signal,
                    &ctx,
                    None,
                    reference_price,
                    risk_state,
                    capital,
                )
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

                    return Ok(ExecOutcome::placed(PlacedOrderInfo {
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

                    // Recusa que PARA a operação do dia merece alerta; recusa
                    // de rotina (não há setup, contexto ruim) não — seria
                    // ruído a cada candle.
                    if halts_trading(reason) {
                        return Ok(ExecOutcome {
                            blocked: Some(BlockReason::Risk(reason, detail)),
                            ..Default::default()
                        });
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

    Ok(ExecOutcome::default())
}

/// O que aconteceu numa tentativa de execução.
///
/// Antes era só `Option<PlacedOrderInfo>`: o chamador sabia que nada foi
/// enviado, mas não POR QUÊ. Sem isso não dá para avisar que o bot parou de
/// operar — que é justamente o evento que alguém precisa saber (A9 da
/// auditoria).
#[derive(Default)]
struct ExecOutcome {
    placed: Option<PlacedOrderInfo>,
    /// Preenchido só quando a recusa PARA a operação, não em recusa de rotina.
    blocked: Option<BlockReason>,
}

impl ExecOutcome {
    fn placed(info: PlacedOrderInfo) -> Self {
        Self {
            placed: Some(info),
            blocked: None,
        }
    }
}

enum BlockReason {
    Risk(RejectionReason, String),
    Portfolio(String),
}

impl BlockReason {
    /// Chave estável para não repetir o mesmo alerta a cada candle.
    fn key(&self) -> String {
        match self {
            Self::Risk(reason, _) => format!("risk:{reason:?}"),
            Self::Portfolio(_) => "portfolio".to_string(),
        }
    }

    fn message(&self, symbol: &str) -> String {
        match self {
            Self::Risk(reason, detail) => {
                format!("{symbol}: operacao interrompida pelo risco - {reason:?}: {detail}")
            }
            Self::Portfolio(motivo) => {
                format!("{symbol}: entrada bloqueada pelo limite da CONTA - {motivo}")
            }
        }
    }
}

/// Recusas que significam "o bot parou de operar hoje", e não "este candle não
/// tinha setup". Só estas viram alerta.
fn halts_trading(reason: RejectionReason) -> bool {
    matches!(
        reason,
        RejectionReason::DailyLossLimitReached
            | RejectionReason::MaxTradesReached
            | RejectionReason::ConsecutiveLosses
            | RejectionReason::StopMissing
    )
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
/// 600 candles ≈ 23 pregões de 15min. A janela precisa cobrir o warmup das
/// estratégias com contexto multi-dia: a range-extreme-fade-v1 calcula
/// ATR DIÁRIO de 14 dias completos anteriores (~15 pregões × 26 barras ≈
/// 390 barras) — com 200 (≈8 dias) ela rejeitava TUDO com IncompleteSetup
/// ("série sem dias anteriores suficientes para o ATR diário"), ficando
/// inerte no live embora aprovada no backtest (que roda com a série cheia).
/// Para as demais (SMA200 de 15min), 600 mantém o contexto completo e
/// melhora a paridade live × backtest.
const LIVE_MAX_CANDLES: usize = 600;

/// Barras degeneradas consecutivas (1 print, high==low) que disparam alerta
/// crítico + evento em `system_events`. Abaixo disso, apenas log: barras
/// espúrias isoladas não devem acordar ninguém.
const DEGENERATE_BAR_ALERT_THRESHOLD: u32 = 5;

/// Polls de 30s esperando uma barra estabilizar/consolidar antes de desistir
/// (30 polls ≈ 15 min; consolidação medida em ~3–4 min em 2026-08-17).
const MAX_BAR_SETTLE_POLLS: u32 = 30;

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
    shutdown_notify: Arc<tokio::sync::Notify>,
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
    // A conexão do broker (ordens, execuções, account info) usa um client id
    // SEPARADO do market data. Compartilhar o mesmo id fazia o gateway
    // recusar a segunda conexão simultânea com erro 326 ("client id already
    // in use") — a corrida entre as duas conexões da mesma instância gerou a
    // tempestade de early eof → circuit breaker → restart de 2026-08-19
    // (66 CBs no dia). +100 fica fora do range 1-11 das instâncias e do 99
    // reservado a diagnósticos manuais.
    let mut broker_config = ibkr_config.clone();
    broker_config.client_id += 100;
    info!(
        market_data_client_id = ibkr_config.client_id,
        broker_client_id = broker_config.client_id,
        "client ids IBKR separados (market data × broker)"
    );
    let broker = IbkrBrokerAdapter::new(broker_config);

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
    // Barras degeneradas consecutivas que desistimos de esperar (nunca
    // consolidaram) — alimenta o alerta de sequência.
    let mut degenerate_streak: u32 = 0;
    // Barra aguardando estabilizar: o feed sem dados em tempo real entrega a
    // barra recém-fechada incompleta (1 print ou parcial) e ela consolida
    // alguns minutos depois — o cursor não avança até os valores pararem de
    // mudar entre polls (ou até desistir, ver guarda no loop).
    let mut pending_bar: Option<PendingBar> = None;
    // Dia (calendário de NY) em que o flatten de fim de sessão já rodou.
    let mut flattened_on: Option<chrono::NaiveDate> = None;
    // Motivos de bloqueio já alertados hoje — o alerta é por motivo, não por
    // candle. Limpo na virada do dia, junto com o estado de risco.
    let mut blocked_alerted: std::collections::HashSet<String> = std::collections::HashSet::new();

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
        // Espera interrompível: sem isto o encerramento gracioso depende de
        // cair no fim de um tick de 30s, e o SIGKILL do docker chega antes.
        tokio::select! {
            _ = tick.tick() => {}
            _ = shutdown_notify.notified() => {}
        }
        if shutdown.load(Ordering::SeqCst) {
            break;
        }

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
            blocked_alerted.clear();
            info!("novo dia UTC: estado de risco diário reconstruído do banco");
        }

        // Flatten obrigatório de fim de sessão (C1): as pernas do bracket vão
        // com TIF Day e expiram no sino — nenhuma posição atravessa o
        // fechamento sem proteção. Roda POR TICK, não por candle novo: depois
        // das 16h ET não chega mais candle fechado para disparar nada.
        let now_utc = chrono::Utc::now();
        if in_flatten_window(now_utc) {
            let ny_day = now_utc
                .with_timezone(&chrono_tz::America::New_York)
                .date_naive();
            if flattened_on != Some(ny_day) {
                match flatten_session(&broker, &args.symbol, &mut live_fills, repos, alerter).await
                {
                    Ok(had_position) => {
                        flattened_on = Some(ny_day);
                        if had_position {
                            println!("🔔 Flatten de fim de sessão: posição encerrada a mercado");
                        }
                    }
                    // Sem marcar o dia: os ticks seguintes dentro da janela
                    // tentam de novo (a janela vai até 16h10 ET).
                    Err(e) => {
                        warn!(error = %e, "falha no flatten de fim de sessão; retentando no próximo tick")
                    }
                }
            }
        }

        // Persistência degradada (C3): religa as entradas assim que o banco
        // voltar a aceitar escrita.
        if live_fills.persistence_degraded && persistence_recovered(repos).await {
            live_fills.persistence_degraded = false;
            let message = format!(
                "persistência restabelecida em {}; entradas liberadas",
                args.symbol
            );
            info!(%message);
            alerter.info(&message);
        }

        // P&L diário e perdas consecutivas vêm de trades reais (rebuild no
        // boot + sync a cada trade fechado) — não há mais aproximação por
        // equity, que mascarava o estado de risco após restart.

        // Janela de candles reais: últimos 30 dias (~20 pregões) para cobrir
        // o warmup de estratégias com contexto multi-dia (ATR diário de 14
        // dias da range-extreme-fade-v1) mesmo com fins de semana/feriados.
        // O corte final é pelo LIVE_MAX_CANDLES (600 barras ≈ 23 pregões).
        let now = chrono::Utc::now();
        let request = CandleRequest {
            symbol: args.symbol.clone(),
            timeframe: args.timeframe,
            from: now - chrono::Duration::days(30),
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

            // Guarda de qualidade de dados — espera a barra ESTABILIZAR.
            // O feed sem dados em tempo real entrega a barra recém-fechada
            // incompleta: primeiro como 1 print (high==low, volume 0), depois
            // parcial (poucos minutos de dados), e só ~3–4 min após o
            // fechamento ela consolida (medido em 2026-08-17). Regras:
            //  - barra degenerada (flat): nunca é operada; espera consolidar
            //    e desiste após MAX_BAR_SETTLE_POLLS (alerta em sequência);
            //  - barra não-degenerada: só é operada quando os valores param
            //    de mudar entre duas polls (estável) — senão espera;
            //  - o upsert do repositório repara a linha no banco quando a
            //    versão consolidada chega.
            // Com feed em tempo real a barra já chega consolidada e o custo
            // é de 1 poll (~30s) por barra.
            // Ver docs/reports/validacao-live-vs-backtest-2026-08-07_a_08-14.md.
            let bar = &candles[i];
            let same_as_pending = matches!(&pending_bar, Some(p)
                if p.timestamp == bar.timestamp
                    && p.high == bar.high
                    && p.low == bar.low
                    && p.close == bar.close
                    && p.volume == bar.volume);
            let pending_polls = match &pending_bar {
                Some(p) if p.timestamp == bar.timestamp => p.polls + 1,
                _ => 0,
            };

            if bar.is_degenerate() {
                if pending_polls > MAX_BAR_SETTLE_POLLS {
                    // Nunca consolidou: desiste da barra (fica flat no banco,
                    // auditável) e segue em frente. Alerta em sequência.
                    degenerate_streak += 1;
                    pending_bar = None;
                    warn!(
                        symbol = %args.symbol,
                        ts = %bar.timestamp,
                        degenerate_streak,
                        "barra degenerada não consolidou; barra abandonada"
                    );
                    // Cada barra abandonada vira evento: o cursor avança e ela
                    // NUNCA é avaliada. Sem este registro a perda é invisível —
                    // só dá para descobrir comparando `candles` com
                    // `market_contexts` (foi assim que as três barras puladas
                    // em 01–02/09/2026 apareceram).
                    record_event(
                        repos,
                        "warn",
                        "data_quality",
                        "bar_abandoned",
                        &format!(
                            "{}: barra {} abandonada sem consolidar; setup deste fechamento nao foi avaliado",
                            args.symbol, bar.timestamp
                        ),
                    )
                    .await;
                    if degenerate_streak == DEGENERATE_BAR_ALERT_THRESHOLD {
                        record_event(
                            repos,
                            "warn",
                            "data_quality",
                            "degenerate_bar_streak",
                            &format!(
                                "{}: {} barras degeneradas consecutivas sem consolidar; sinais suspensos",
                                args.symbol, degenerate_streak
                            ),
                        )
                        .await;
                        alerter
                            .critical_await(&format!(
                                "⚠️ {}: {} barras degeneradas seguidas sem consolidar (feed sem high/low); sinais suspensos até o feed normalizar",
                                args.symbol, degenerate_streak
                            ))
                            .await;
                    }
                    last_processed = Some(bar.timestamp);
                    continue;
                }
                if pending_polls == 0 || pending_polls % 10 == 0 {
                    warn!(
                        symbol = %args.symbol,
                        ts = %bar.timestamp,
                        pending_polls,
                        "barra degenerada (1 print); aguardando consolidação do feed"
                    );
                }
                pending_bar = Some(PendingBar::from_candle(bar, pending_polls));
                // Não avança o cursor nem processa barras mais novas fora de
                // ordem: a próxima poll reavalia a mesma barra.
                break;
            }

            if !same_as_pending && pending_polls <= MAX_BAR_SETTLE_POLLS {
                // Barra ainda mudando entre polls (parcial): espera estabilizar.
                info!(
                    symbol = %args.symbol,
                    ts = %bar.timestamp,
                    pending_polls,
                    "aguardando barra estabilizar antes de avaliar"
                );
                pending_bar = Some(PendingBar::from_candle(bar, pending_polls));
                break;
            }
            if !same_as_pending {
                warn!(
                    symbol = %args.symbol,
                    ts = %bar.timestamp,
                    "barra não estabilizou após ~15min; avaliando com o melhor dado disponível"
                );
            }
            if degenerate_streak >= DEGENERATE_BAR_ALERT_THRESHOLD {
                info!(
                    symbol = %args.symbol,
                    degenerate_streak, "feed normalizou após sequência de barras degeneradas"
                );
            }
            pending_bar = None;
            degenerate_streak = 0;

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
            // O snapshot de posição da IBKR devolve `stop_price = 0` por
            // construção — o stop real vive na ordem rastreada. Armar o
            // tracker com o zero fazia o risco por unidade virar o preço
            // inteiro: o lucro em R ficava ~0, o trade nunca validava e era
            // SEMPRE encerrado a mercado após N candles, mesmo a favor
            // (A1 da auditoria de 30/08/2026).
            let tracked_stop = live_fills.open_order.as_ref().map(|o| o.stop_price);
            match broker.get_position(&args.symbol).await {
                Ok(Some(position)) => {
                    let mut closed_by_time = false;
                    if live_fills.tracker.is_open() {
                        match tracked_stop {
                            Some(stop) => live_fills.time_exit.ensure_tracking(
                                position.avg_entry_price,
                                stop,
                                position.direction,
                            ),
                            None => warn!(
                                symbol = %args.symbol,
                                "posição aberta sem ordem rastreada; saída por tempo não armada"
                            ),
                        }
                        if live_fills.time_exit.on_candle_close(candles[i].close) {
                            match close_position_at_market(&broker, &args.symbol, &position).await {
                                Ok(()) => {
                                    println!(
                                        "⏱️  Saída por tempo: posição encerrada a mercado em {}",
                                        candles[i].close
                                    );
                                    live_fills.time_exit_triggered = true;
                                    live_fills.time_exit.reset();
                                    closed_by_time = true;
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
                    } else {
                        live_fills.time_exit.reset();
                    }

                    // Watchdog do C1: posição aberta ⇒ stop trabalhando. Roda
                    // mesmo sem ordem rastreada — o caso mais perigoso é
                    // justamente a posição herdada de outra sessão, que a
                    // recuperação enxerga (e usa para bloquear entradas) mas
                    // nunca reprotege.
                    if !closed_by_time {
                        ensure_stop_protection(
                            &broker,
                            &args.symbol,
                            &position,
                            tracked_stop,
                            &mut live_fills,
                            repos,
                            alerter,
                        )
                        .await;
                    }
                }
                Ok(None) => {
                    live_fills.time_exit.reset();
                    live_fills.unprotected_streak = 0;
                }
                Err(e) => {
                    consecutive_failures += 1;
                    warn!(error = %e, consecutive_failures, "falha ao consultar posição do símbolo");
                    check_circuit_breaker(
                        consecutive_failures,
                        "falha ao consultar posição do símbolo",
                        repos,
                        alerter,
                    )
                    .await?;
                }
            }

            // Reconciliação: posição aberta OU ordem pendente no símbolo impede
            // novo sinal. A checagem de ordens abertas evita entradas duplicadas
            // enquanto a limit de entrada do bracket não é preenchida.
            match find_exposure(&broker, &args.symbol).await {
                Ok(Exposure::None) => {
                    live_fills.untracked_exposure_streak = 0;
                    // Rastreamos uma entrada pendente, mas o broker não conhece
                    // ordem nem posição no símbolo: a ordem NÃO está
                    // trabalhando. Foi assim que a ordem 17 (VBR, 28/08/2026)
                    // morreu — o mercado atravessou o gatilho por 35 centavos,
                    // nada encheu, e o bot a deu por "expirada" dois candles
                    // depois, em silêncio.
                    if live_fills.pending_entry_at_broker() {
                        live_fills.missing_order_streak += 1;
                        if live_fills.missing_order_streak == MISSING_ORDER_ALERT_STREAK {
                            let id = live_fills
                                .open_order
                                .as_ref()
                                .map(|o| o.broker_order_id.clone())
                                .unwrap_or_default();
                            let message = format!(
                                "ordem {id} de {} NAO esta no broker (sem ordem aberta e sem posicao) - provavel rejeicao nao detectada; a entrada nao vai executar",
                                args.symbol
                            );
                            record_event(repos, "critical", "live", "order_missing", &message)
                                .await;
                            alerter.critical_await(&message).await;
                        }
                    } else {
                        live_fills.missing_order_streak = 0;
                    }
                }
                Ok(exposure) => {
                    info!(symbol = %args.symbol, %exposure, "exposição existente; sem novo sinal");
                    // Exposição que o bot NÃO rastreia trava o símbolo para
                    // sempre — e em silêncio: este `break` acontece antes de
                    // `analyze_and_execute`, que é quem grava o contexto de
                    // mercado. Foi o que aconteceu com as duas instâncias de
                    // IWM entre 07/08 e 03/09/2026: 18 pregões sem avaliar
                    // nada, sem nenhum sinal disso no painel.
                    if live_fills.tracking_nothing() {
                        live_fills.untracked_exposure_streak += 1;
                        if live_fills.untracked_exposure_streak == UNTRACKED_EXPOSURE_ALERT_STREAK {
                            let message = format!(
                                "{} BLOQUEADO: exposicao no broker que o bot nao rastreia ({exposure}) - nenhum setup sera avaliado ate limpar",
                                args.symbol
                            );
                            record_event(repos, "critical", "live", "untracked_exposure", &message)
                                .await;
                            alerter.critical_await(&message).await;
                        }
                    } else {
                        live_fills.untracked_exposure_streak = 0;
                    }
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
            }

            // C3: sem persistência confiável o estado de risco congela —
            // nenhuma entrada nova até o banco voltar.
            if live_fills.persistence_degraded {
                info!(
                    symbol = %args.symbol,
                    "persistência degradada; entrada suspensa neste candle"
                );
                last_processed = Some(candles[i].timestamp);
                continue;
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
                // O fetch inclui a barra em formação (além de `closed`): o
                // close dela é o preço mais fresco que o live conhece — é o
                // que pega o cenário do trade 12 (preço já correu além do
                // gatilho entre o fechamento da barra do sinal e o envio).
                candles.last().map(|c| c.close),
                &config.app_config.risk,
            )
            .await
            {
                Ok(outcome) => {
                    // Recusa que interrompe a operação vira alerta, uma vez
                    // por motivo por dia — a cada candle seria ruído.
                    if let Some(motivo) = &outcome.blocked {
                        let chave = motivo.key();
                        if blocked_alerted.insert(chave) {
                            let message = motivo.message(&args.symbol);
                            warn!(%message, "operação bloqueada");
                            record_event(repos, "warning", "risk", "trading_halted", &message)
                                .await;
                            alerter.critical_await(&message).await;
                        }
                    }
                    let Some(placed) = outcome.placed else {
                        last_processed = Some(candles[i].timestamp);
                        continue;
                    };
                    // Cada execução conta para o limite de trades do dia.
                    risk_state.daily_trades += 1;
                    if placed.order_db_id.is_none() {
                        // Sem id de banco os fills dessa ordem não são
                        // persistidos (drain_order_events desiste) e o trade
                        // nunca fecha: mesma família de dano do C3.
                        live_fills.persistence_degraded = true;
                        let message = format!(
                            "ordem de {} enviada mas NAO persistida - entradas SUSPENSAS ate o banco voltar",
                            args.symbol
                        );
                        record_event(repos, "critical", "live", "persistence_failure", &message)
                            .await;
                        alerter.critical_await(&message).await;
                    }
                    // A ordem passa a ser rastreada: fills dela (e das filhas
                    // do bracket) fecharão o trade via drain_order_events.
                    live_fills.open_order = Some(placed);
                }
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

/// Snapshot da barra em espera de estabilização (guarda de qualidade do feed).
/// A barra recém-fechada só é avaliada quando os valores param de mudar
/// entre polls — ou quando o cap de espera estoura.
struct PendingBar {
    timestamp: chrono::DateTime<chrono::Utc>,
    high: Decimal,
    low: Decimal,
    close: Decimal,
    volume: Decimal,
    polls: u32,
}

impl PendingBar {
    fn from_candle(candle: &trader_domain::Candle, polls: u32) -> Self {
        Self {
            timestamp: candle.timestamp,
            high: candle.high,
            low: candle.low,
            close: candle.close,
            volume: candle.volume,
            polls,
        }
    }
}

/// Verifica os limites de risco da CONTA INTEIRA antes de abrir posição.
///
/// Devolve `Some(motivo)` quando alguma trava está ativa. Três travas, todas
/// somando as 11 instâncias:
///
/// 1. **perda diária agregada** — P&L realizado de hoje, todos os símbolos;
/// 2. **posições simultâneas** — quantas posições a conta já carrega;
/// 3. **notional agregado** — soma do valor de mercado das posições abertas.
///
/// Falha FECHADO: sem banco não dá para saber a perda do dia, e nesse caso a
/// entrada é bloqueada. É o mesmo princípio do "live não sobe sem banco".
async fn portfolio_limit_hit(
    positions: &[trader_domain::Position],
    equity: Decimal,
    repos: Option<&Repositories>,
    risk: &trader_infra::config::RiskSettings,
) -> Option<String> {
    // 1 e 2: exposição, medida no broker.
    if let Some(motivo) = exposure_limit_hit(
        positions,
        equity,
        risk.max_concurrent_positions,
        risk.max_portfolio_notional_pct,
    ) {
        return Some(motivo);
    }

    // 3. Perda diária agregada.
    let Some(repos) = repos else {
        return Some("sem banco: nao da para medir a perda do dia da conta".to_string());
    };
    match repos.trade_repo.list_today_account().await {
        Ok(trades) => {
            let pnl: Decimal = trades
                .iter()
                .filter(|t| !t.is_latency_artifact())
                .map(|t| t.net_pnl)
                .sum();
            let limite = -(equity * pct_to_fraction(risk.max_portfolio_daily_loss_pct));
            if pnl <= limite {
                return Some(format!(
                    "perda do dia na conta {pnl:.2} atingiu o limite {limite:.2} ({}% do capital)",
                    risk.max_portfolio_daily_loss_pct
                ));
            }
            None
        }
        Err(e) => Some(format!("falha ao medir a perda do dia da conta: {e}")),
    }
}

/// Parte PURA da trava de portfólio: posições simultâneas e notional
/// agregado, ambos medidos no broker. Separada para poder ser testada — um
/// guarda de risco sem teste é um guarda que ninguém sabe se funciona.
fn exposure_limit_hit(
    positions: &[trader_domain::Position],
    equity: Decimal,
    max_positions: usize,
    max_notional_pct: f64,
) -> Option<String> {
    if positions.len() >= max_positions {
        return Some(format!(
            "{} posicoes abertas na conta (maximo {max_positions})",
            positions.len()
        ));
    }

    if equity > Decimal::ZERO {
        let notional: Decimal = positions
            .iter()
            .map(|p| (p.avg_entry_price * p.quantity).abs())
            .sum();
        let teto = equity * pct_to_fraction(max_notional_pct);
        if notional >= teto {
            return Some(format!(
                "notional agregado {notional:.0} >= teto {teto:.0} ({max_notional_pct}% do capital)"
            ));
        }
    }

    None
}

/// Converte "2.0" (por cento) em 0.02 (fração), sem passar por f64 no cálculo.
fn pct_to_fraction(pct: f64) -> Decimal {
    Decimal::from_f64_retain(pct)
        .unwrap_or_default()
        .checked_div(Decimal::from(100))
        .unwrap_or_default()
}

/// Espera por um sinal de parada do sistema operacional.
///
/// Escutava só `ctrl_c()` — ou seja, SIGINT. Em produção quem para as
/// instâncias é `docker stop`, que manda **SIGTERM**: ninguém tratava, o
/// processo era morto à força depois da carência e o caminho de encerramento
/// nunca rodava. O banco confirma: 384 eventos `live_started` e **zero**
/// `live_stopped` em toda a vida do projeto (análise de 03/09/2026).
async fn wait_for_stop_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        match signal(SignalKind::terminate()) {
            Ok(mut sigterm) => {
                tokio::select! {
                    _ = tokio::signal::ctrl_c() => {}
                    _ = sigterm.recv() => {}
                }
            }
            Err(e) => {
                warn!(error = %e, "não foi possível escutar SIGTERM; só Ctrl+C encerra");
                let _ = tokio::signal::ctrl_c().await;
            }
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
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
    /// Marca que o fechamento em andamento é o flatten de fim de sessão.
    flatten_triggered: bool,
    /// Checagens consecutivas em que a posição apareceu SEM stop trabalhando.
    /// A recolocação só acontece na segunda detecção seguida: `open_orders`
    /// pode não listar as pernas logo após um restart, e recolocar sobre um
    /// stop que existe deixaria uma ordem órfã capaz de abrir posição
    /// invertida — pior do que o problema.
    unprotected_streak: u32,
    /// Persistência crítica (fill, ordem ou trade) falhou: novas entradas
    /// ficam suspensas até o banco voltar. Sem isso o bot seguia operando com
    /// perda diária e perdas consecutivas congeladas (C3 da auditoria).
    persistence_degraded: bool,
    /// Ciclos seguidos bloqueados por exposição que o bot não rastreia.
    untracked_exposure_streak: u32,
    /// Ciclos seguidos com entrada rastreada que o broker não conhece.
    missing_order_streak: u32,
}

/// Ciclos de exposição não rastreada antes de gritar. Dois, e não um, porque
/// logo depois de enviar um bracket a listagem do broker pode ainda não
/// refletir o estado — mas dois candles seguidos já são anomalia.
const UNTRACKED_EXPOSURE_ALERT_STREAK: u32 = 2;
/// Idem para a entrada rastreada que sumiu do broker.
const MISSING_ORDER_ALERT_STREAK: u32 = 2;

impl LiveFillState {
    /// `true` quando o bot não tem nem posição nem ordem própria em jogo —
    /// então qualquer exposição no broker é órfã, de outra sessão ou de um
    /// bracket que ficou para trás.
    fn tracking_nothing(&self) -> bool {
        !self.tracker.is_open() && self.open_order.is_none()
    }

    /// `true` quando há uma entrada rastreada que deveria estar trabalhando no
    /// broker (ordem enviada, ainda sem fill).
    fn pending_entry_at_broker(&self) -> bool {
        !self.tracker.is_open()
            && self
                .open_order
                .as_ref()
                .is_some_and(|o| !o.broker_order_id.is_empty())
    }
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
            trader_domain::stop_entry_expired(pending.candles_waited, validity)
                && !pending.broker_order_id.is_empty(),
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
                    // C3 da auditoria: o fill é DESCARTADO e nunca reemitido
                    // (o dedupe do adapter é em memória). O trade não fecha no
                    // tracker e o risk_state congela — perda diária e perdas
                    // consecutivas param no tempo. Suspender entradas é melhor
                    // do que seguir operando com os freios desatualizados.
                    warn!(error = %e, "falha ao persistir fill; entradas suspensas");
                    state.persistence_degraded = true;
                    let message = format!(
                        "falha ao persistir fill de {symbol}: {e} - entradas SUSPENSAS ate o banco voltar"
                    );
                    record_event(
                        Some(repos),
                        "critical",
                        "live",
                        "persistence_failure",
                        &message,
                    )
                    .await;
                    alerter.critical_await(&message).await;
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
    let was_flatten = state.flatten_triggered;
    let exit_reason = if state.time_exit_triggered {
        trader_domain::ExitReason::Time
    } else if was_flatten {
        // O domínio não tem variante EndOfDay; o flatten é uma saída ativa,
        // como qualquer fechamento a mercado fora de stop/alvo. O journal
        // guarda a origem exata.
        trader_domain::ExitReason::Manual
    } else {
        classify_exit_reason(
            closed.direction,
            closed.exit_price,
            ctx.stop_price,
            ctx.target_price,
        )
    };
    state.time_exit_triggered = false;
    state.flatten_triggered = false;
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
        journal: if was_flatten {
            serde_json::json!({ "source": "live_fills", "forced_exit": "session_flatten" })
        } else {
            serde_json::json!({ "source": "live_fills" })
        },
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
            warn!(error = %e, "falha ao persistir trade do live; entradas suspensas");
            state.persistence_degraded = true;
            let message = format!(
                "falha ao persistir trade de {symbol}: {e} - entradas SUSPENSAS ate o banco voltar"
            );
            record_event(
                Some(repos),
                "critical",
                "live",
                "persistence_failure",
                &message,
            )
            .await;
            alerter.critical_await(&message).await;
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
/// O que o broker reporta para o símbolo. Diferente de um `bool`, isto diz
/// *o quê* está bloqueando — sem essa informação o operador não consegue agir
/// sobre o alerta.
enum Exposure {
    None,
    Position {
        quantity: Decimal,
        direction: Direction,
    },
    OpenOrders(Vec<String>),
}

impl std::fmt::Display for Exposure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::None => write!(f, "nenhuma"),
            Self::Position {
                quantity,
                direction,
            } => write!(f, "posicao {direction:?} de {quantity}"),
            Self::OpenOrders(ids) => write!(f, "ordens abertas: {}", ids.join(", ")),
        }
    }
}

/// Posição aberta ou ordem pendente no símbolo, com o detalhe do que é.
async fn find_exposure<B: Broker>(broker: &B, symbol: &str) -> Result<Exposure, BrokerError> {
    if let Some(position) = broker.get_position(symbol).await? {
        return Ok(Exposure::Position {
            quantity: position.quantity,
            direction: position.direction,
        });
    }
    let ids: Vec<String> = broker
        .get_open_orders()
        .await?
        .iter()
        .filter(|o| o.symbol == symbol)
        .map(|o| o.broker_order_id.clone().unwrap_or_else(|| "?".to_string()))
        .collect();
    if ids.is_empty() {
        Ok(Exposure::None)
    } else {
        Ok(Exposure::OpenOrders(ids))
    }
}

/// Tentativas do envio da ordem de fechamento a mercado antes de desistir.
const CLOSE_ATTEMPTS: u32 = 3;
/// Espera entre as tentativas de fechamento — segundos, não o próximo candle.
const CLOSE_RETRY_DELAY: Duration = Duration::from_secs(2);

/// Encerra uma posição aberta a mercado (saída ativa — saída por tempo,
/// flatten de fim de sessão).
///
/// ORDEM DAS OPERAÇÕES (C1 da auditoria de 30/08/2026): FECHA primeiro,
/// cancela as pernas de proteção depois. A versão anterior cancelava antes de
/// enviar o fechamento — se o envio falhasse, a posição ficava NUA e a
/// retentativa só vinha no próximo candle, 15 minutos depois. Aqui o stop
/// server-side continua trabalhando durante o envio, e a janela em que as
/// duas ordens coexistem é de segundos.
async fn close_position_at_market<B: Broker>(
    broker: &B,
    symbol: &str,
    position: &trader_domain::Position,
) -> Result<()> {
    let side = match position.direction {
        Direction::Long => OrderSide::Sell,
        Direction::Short => OrderSide::Buy,
    };

    let mut last_error: Option<String> = None;
    let mut ordem_de_fechamento: Option<String> = None;
    for attempt in 1..=CLOSE_ATTEMPTS {
        if attempt > 1 {
            tokio::time::sleep(CLOSE_RETRY_DELAY).await;
            // Repetir ordem a mercado às cegas duplica posição. Só a ausência
            // COMPROVADA de ordem de saída no broker autoriza reenviar.
            match broker.get_open_orders().await {
                Ok(orders) => {
                    if orders.iter().any(|o| o.symbol == symbol && o.side == side) {
                        warn!(%symbol, "tentativa anterior chegou ao broker; fechamento não reenviado");
                        return Ok(());
                    }
                }
                Err(e) => {
                    warn!(%symbol, error = %e, "não deu para conferir se o fechamento anterior foi enviado; não reenviando");
                    return Err(anyhow::anyhow!(
                        "fechamento de {symbol} em estado incerto: {e}"
                    ));
                }
            }
        }

        let order = Order::new(symbol, side, OrderType::Market, position.quantity, "ibkr")
            .map_err(|e| anyhow::anyhow!("ordem de fechamento inválida: {e}"))?;
        match broker.place_order(order).await {
            Ok(id) => {
                last_error = None;
                ordem_de_fechamento = Some(id.0.clone());
                info!(%symbol, quantity = %position.quantity, attempt, %id,
                    "ordem de fechamento a mercado enviada (saída ativa)");
                break;
            }
            Err(e) => {
                warn!(%symbol, attempt, error = %e, "falha ao enviar fechamento a mercado");
                last_error = Some(e.to_string());
            }
        }
    }
    if let Some(e) = last_error {
        return Err(anyhow::anyhow!(
            "falha ao enviar ordem de fechamento a mercado após {CLOSE_ATTEMPTS} tentativas: {e}"
        ));
    }

    // Só agora as pernas de proteção saem. Uma perna órfã que execute depois
    // abre posição invertida, que o bot ignora como "fill sem ordem
    // rastreada" — por isso a falha de cancelamento é ruidosa.
    cancel_protection_legs(broker, symbol, side, ordem_de_fechamento.as_deref()).await;
    Ok(())
}

/// Cancela as pernas de PROTEÇÃO do símbolo — as ordens do lado da saída
/// (stop e alvo de uma posição long são vendas; de uma short, compras),
/// preservando a ordem de fechamento recém-enviada (`manter`).
///
/// Filtrar pelo lado importa porque três símbolos rodam com duas instâncias
/// (IWM, IWV e AVUV). Cancelar tudo do símbolo derrubava a entrada pendente da
/// instância vizinha — que é de outra estratégia e não tem nada a ver com esta
/// posição.
async fn cancel_protection_legs<B: Broker>(
    broker: &B,
    symbol: &str,
    exit_side: OrderSide,
    manter: Option<&str>,
) {
    let open_orders = match broker.get_open_orders().await {
        Ok(orders) => orders,
        Err(e) => {
            warn!(%symbol, error = %e, "falha ao listar ordens abertas para cancelar");
            return;
        }
    };
    for order in open_orders
        .iter()
        .filter(|o| o.symbol == symbol && o.side == exit_side)
    {
        if let Some(broker_order_id) = &order.broker_order_id {
            // A ordem de fechamento é, ela mesma, do lado da saída: sem esta
            // exceção o cancelamento derruba exatamente o que acabou de ser
            // enviado (aconteceu em 03/09/2026 com a ordem 4 de IWM).
            if manter == Some(broker_order_id.as_str()) {
                continue;
            }
            let id = trader_domain::OrderId::from(broker_order_id.clone());
            if let Err(e) = broker.cancel_order(&id).await {
                warn!(order_id = %id, error = %e, "falha ao cancelar ordem aberta na saída ativa");
            }
        }
    }
}

/// Janela (ET) do encerramento forçado da sessão.
///
/// O pregão fecha às 16h00 ET e os timers do host param os containers às
/// 16h10 ET; 15h55 dá cinco minutos de folga para o fechamento a mercado
/// resolver, com retentativas nos ticks seguintes dentro da janela.
const FLATTEN_START_MINUTES: u32 = 15 * 60 + 55;
const FLATTEN_END_MINUTES: u32 = 16 * 60 + 10;

/// `true` quando o relógio de Nova York está na janela de flatten.
fn in_flatten_window(now: chrono::DateTime<chrono::Utc>) -> bool {
    use chrono::Timelike;
    let ny = now.with_timezone(&chrono_tz::America::New_York);
    let minutes = ny.hour() * 60 + ny.minute();
    (FLATTEN_START_MINUTES..FLATTEN_END_MINUTES).contains(&minutes)
}

/// Encerra a sessão: cancela entrada pendente e fecha a posição aberta a
/// mercado. Retorna `Ok(true)` quando havia posição para fechar.
///
/// Por que existe (C1 da auditoria): as pernas do bracket vão com TIF Day e
/// EXPIRAM no fechamento do pregão. Uma posição que atravessa o sino fica
/// overnight SEM stop, e a recuperação do dia seguinte vê a posição, bloqueia
/// novas entradas e NÃO recoloca a proteção. Nenhuma posição atravessa o
/// fechamento.
async fn flatten_session<B: Broker>(
    broker: &B,
    symbol: &str,
    state: &mut LiveFillState,
    repos: Option<&Repositories>,
    alerter: &crate::alerts::Alerter,
) -> Result<bool> {
    // Entrada stop ainda trabalhando: cancela antes de tudo — abrir posição a
    // cinco minutos do sino é o oposto do que queremos.
    if !state.tracker.is_open() {
        if let Some(pending) = state.open_order.as_ref() {
            if !pending.broker_order_id.is_empty() {
                let id = trader_domain::OrderId::from(pending.broker_order_id.clone());
                match broker.cancel_order(&id).await {
                    Ok(()) => {
                        info!(order_id = %id, "entrada pendente cancelada no fim da sessão");
                        state.open_order = None;
                    }
                    Err(e) => {
                        warn!(order_id = %id, error = %e, "falha ao cancelar entrada pendente no fim da sessão")
                    }
                }
            }
        }
    }

    let Some(position) = broker.get_position(symbol).await? else {
        return Ok(false);
    };

    // NUNCA fechar posição que esta instância não abriu. Três símbolos rodam
    // com DUAS instâncias (IWM, IWV e AVUV): se as duas fizessem flatten da
    // mesma posição, a primeira zeraria e a segunda abriria uma posição
    // INVERTIDA do mesmo tamanho, a mercado, cinco minutos antes do sino.
    //
    // Posição que ninguém rastreia é problema de operação, não de automação:
    // vira alerta crítico e espera decisão humana. É o caso do IWM, que
    // carrega 827 ações órfãs desde 07/08/2026.
    if !state.tracker.is_open() {
        let message = format!(
            "{symbol}: posicao de {} no broker que esta instancia NAO rastreia - flatten de fim de sessao NAO executado, intervencao manual necessaria",
            position.quantity
        );
        record_event(repos, "critical", "live", "untracked_position", &message).await;
        alerter.critical_await(&message).await;
        return Ok(false);
    }

    state.flatten_triggered = true;
    if let Err(e) = close_position_at_market(broker, symbol, &position).await {
        state.flatten_triggered = false;
        return Err(e);
    }
    state.time_exit.reset();

    let message = format!(
        "flatten de fim de sessão: {} {} encerrada a mercado (pernas do bracket expiram no sino)",
        position.quantity, symbol
    );
    record_event(repos, "warning", "live", "session_flatten", &message).await;
    alerter.info(&message);
    Ok(true)
}

/// Watchdog "posição aberta ⇒ stop trabalhando" (C1 da auditoria).
///
/// A regra nº 1 do projeto ("nunca opera sem stop") valia só no envio da
/// ordem e não era vigiada depois: uma perna de stop rejeitada de forma
/// assíncrona pela IBKR (tick inválido, margem) deixava a entrada cheia sem
/// proteção e nada detectava.
///
/// Na PRIMEIRA detecção só alerta — `open_orders` pode não listar as pernas
/// logo após um restart, e recolocar sobre um stop que existe deixaria ordem
/// órfã capaz de abrir posição invertida. Na segunda seguida, recoloca.
#[allow(clippy::too_many_arguments)]
async fn ensure_stop_protection<B: Broker>(
    broker: &B,
    symbol: &str,
    position: &trader_domain::Position,
    known_stop: Option<Decimal>,
    state: &mut LiveFillState,
    repos: Option<&Repositories>,
    alerter: &crate::alerts::Alerter,
) {
    let exit_side = match position.direction {
        Direction::Long => OrderSide::Sell,
        Direction::Short => OrderSide::Buy,
    };

    let orders = match broker.get_open_orders().await {
        Ok(orders) => orders,
        Err(e) => {
            warn!(%symbol, error = %e, "falha ao verificar o stop da posição aberta");
            return;
        }
    };

    // A perna de proteção é uma STP do lado CONTRÁRIO à posição. O lado
    // distingue a proteção do parent de entrada stop, que é do mesmo lado da
    // posição e pode continuar trabalhando num fill parcial.
    let protecao = orders
        .iter()
        .find(|o| o.symbol == symbol && o.order_type == OrderType::Stop && o.side == exit_side);

    if let Some(stop_leg) = protecao {
        state.unprotected_streak = 0;

        // FILL PARCIAL. As pernas do bracket saem com a quantidade CHEIA da
        // ordem. Se a entrada encher só em parte, o stop protege mais ações do
        // que a conta tem: quando ele dispara, vende o que existe E abre
        // posição invertida no resto. O contrário — stop cobrindo menos que a
        // posição — deixa o excedente nu.
        //
        // Não dá para corrigir isso aqui sem arriscar cancelar a proteção que
        // está funcionando, então a regra é gritar: quantidade divergente é
        // intervenção humana, com o número exato dos dois lados.
        if stop_leg.quantity != position.quantity {
            let message = format!(
                "{symbol}: PROTECAO COM QUANTIDADE ERRADA - posicao {} x stop {} (provavel fill parcial); disparo do stop deixaria {} acoes descobertas ou invertidas",
                position.quantity,
                stop_leg.quantity,
                (stop_leg.quantity - position.quantity).abs()
            );
            record_event(
                repos,
                "critical",
                "live",
                "protection_quantity_mismatch",
                &message,
            )
            .await;
            alerter.critical_await(&message).await;
        }
        return;
    }

    state.unprotected_streak += 1;
    let message = format!(
        "POSICAO SEM STOP: {} {} aberta sem perna de protecao trabalhando (deteccao {})",
        position.quantity, symbol, state.unprotected_streak
    );
    record_event(repos, "critical", "live", "position_unprotected", &message).await;
    alerter.critical_await(&message).await;

    if state.unprotected_streak < 2 {
        return;
    }

    let Some(stop_price) = known_stop.filter(|p| *p > Decimal::ZERO) else {
        let message = format!(
            "{symbol} SEM STOP e sem stop conhecido para recolocar - intervencao manual necessaria"
        );
        record_event(repos, "critical", "live", "position_unprotected", &message).await;
        alerter.critical_await(&message).await;
        return;
    };

    let mut order = match Order::new(
        symbol,
        exit_side,
        OrderType::Stop,
        position.quantity,
        "ibkr",
    ) {
        Ok(order) => order,
        Err(e) => {
            warn!(%symbol, error = %e, "stop de proteção inválido; não recolocado");
            return;
        }
    };
    order.stop_price = Some(stop_price);

    let message = match broker.place_order(order).await {
        Ok(id) => {
            state.unprotected_streak = 0;
            format!("{symbol} estava sem stop; stop recolocado em {stop_price} (ordem {id})")
        }
        Err(e) => format!("{symbol} SEM STOP: falha ao recolocar stop em {stop_price}: {e}"),
    };
    record_event(repos, "critical", "live", "stop_replaced", &message).await;
    alerter.critical_await(&message).await;
}

/// Sonda de persistência: grava um evento leve para saber se o banco voltou.
///
/// Usada só enquanto o live está com a persistência degradada (C3), para
/// religar as entradas assim que o Postgres responder de novo.
async fn persistence_recovered(repos: Option<&Repositories>) -> bool {
    let Some(repos) = repos else {
        return false;
    };
    repos
        .event_repo
        .record(
            "info",
            "live",
            "persistence_probe",
            "sonda de persistência após degradação",
            None,
        )
        .await
        .is_ok()
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

#[cfg(test)]
mod tests {
    use super::{exposure_limit_hit, in_flatten_window};
    use chrono::{DateTime, Utc};
    use rust_decimal::Decimal;

    fn posicao(symbol: &str, preco: i64, qtd: i64) -> trader_domain::Position {
        trader_domain::Position::new(
            symbol,
            1,
            trader_domain::Direction::Long,
            Decimal::from(qtd),
            Decimal::from(preco),
            Decimal::from(preco - 5),
            "ibkr",
        )
        .expect("posição válida")
    }

    /// C2 da auditoria: o risco era medido por processo. Onze instâncias na
    /// mesma conta multiplicavam por onze a exposição permitida.
    #[test]
    fn trava_no_numero_de_posicoes_simultaneas() {
        let equity = Decimal::from(250_000);
        let duas = vec![posicao("IWM", 300, 100), posicao("IWV", 400, 100)];
        assert!(
            exposure_limit_hit(&duas, equity, 3, 200.0).is_none(),
            "duas posições com teto de três deve passar"
        );

        let tres = vec![
            posicao("IWM", 300, 100),
            posicao("IWV", 400, 100),
            posicao("IWO", 370, 100),
        ];
        let motivo = exposure_limit_hit(&tres, equity, 3, 200.0).expect("deve travar");
        assert!(motivo.contains("posicoes abertas"), "motivo: {motivo}");
    }

    #[test]
    fn trava_no_notional_agregado() {
        let equity = Decimal::from(100_000);
        // Duas posições de 90k = 180k, contra teto de 150% (150k).
        let posicoes = vec![posicao("IWM", 900, 100), posicao("IWV", 900, 100)];
        let motivo = exposure_limit_hit(&posicoes, equity, 10, 150.0).expect("deve travar");
        assert!(motivo.contains("notional agregado"), "motivo: {motivo}");

        // Com teto de 200% (200k) a mesma exposição passa.
        assert!(exposure_limit_hit(&posicoes, equity, 10, 200.0).is_none());
    }

    /// Só recusa que PARA a operação vira alerta. Recusa de rotina (não há
    /// setup, contexto ruim, fora de horário) acontece a cada candle e viraria
    /// ruído — o alerta perderia o sentido.
    #[test]
    fn so_recusa_que_para_a_operacao_alerta() {
        use trader_domain::RejectionReason as R;
        for r in [
            R::DailyLossLimitReached,
            R::MaxTradesReached,
            R::ConsecutiveLosses,
            R::StopMissing,
        ] {
            assert!(super::halts_trading(r), "{r:?} deveria alertar");
        }
        for r in [
            R::NoContext,
            R::OutsideTradingHours,
            R::PoorRiskReward,
            R::HighVolatility,
            R::HighSpread,
            R::SetupInvalidated,
        ] {
            assert!(!super::halts_trading(r), "{r:?} NAO deveria alertar");
        }
    }

    #[test]
    fn conta_vazia_nao_trava() {
        assert!(exposure_limit_hit(&[], Decimal::from(250_000), 3, 200.0).is_none());
    }

    fn utc(s: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(s).unwrap().with_timezone(&Utc)
    }

    /// A janela é definida em horário de NY, não em UTC fixo: no horário de
    /// verão (EDT, UTC-4) 15h55 ET é 19h55 UTC.
    #[test]
    fn janela_de_flatten_no_horario_de_verao() {
        assert!(!in_flatten_window(utc("2026-08-31T19:54:00Z")));
        assert!(in_flatten_window(utc("2026-08-31T19:55:00Z")));
        assert!(in_flatten_window(utc("2026-08-31T20:09:00Z")));
        assert!(!in_flatten_window(utc("2026-08-31T20:10:00Z")));
    }

    /// Depois da virada do DST (EST, UTC-5) a mesma janela é 20h55–21h10 UTC.
    /// É exatamente o deslocamento que o A2 aponta nas janelas de negociação.
    #[test]
    fn janela_de_flatten_apos_a_virada_do_dst() {
        assert!(!in_flatten_window(utc("2026-11-02T19:55:00Z")));
        assert!(in_flatten_window(utc("2026-11-02T20:55:00Z")));
        assert!(in_flatten_window(utc("2026-11-02T21:09:00Z")));
        assert!(!in_flatten_window(utc("2026-11-02T21:10:00Z")));
    }
}
