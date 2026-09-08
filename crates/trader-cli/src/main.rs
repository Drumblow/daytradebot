//! `trader-cli` — entrypoint do HumanStyle Trader Bot.

use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};
use tracing::info;

mod alerts;
mod commands;
mod config;
mod dispatch;
mod risk_config;
mod strategy_source;
mod synthetic;

use config::CliConfig;

#[derive(Parser)]
#[command(name = "trader-cli")]
#[command(about = "CLI do HumanStyle Trader Bot")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Verifica conexão com o broker/provedor de dados.
    TestConnection {
        /// Provedor a testar.
        #[arg(long, default_value = "simulated")]
        provider: String,
    },
    /// Exibe resumo da conta.
    Account {
        /// Provedor a consultar.
        #[arg(long, default_value = "simulated")]
        provider: String,
    },
    /// Encerra MANUALMENTE uma posição aberta no broker (conta paper).
    ///
    /// Para posição que nenhuma instância rastreia — o flatten automático de
    /// fim de sessão, de proposito, so fecha o que a propria instancia abriu.
    Flatten {
        /// Símbolo a zerar.
        #[arg(short, long)]
        symbol: String,
        /// Provedor.
        #[arg(long, default_value = "simulated")]
        provider: String,
        /// Sem esta flag o comando só mostra o que faria.
        #[arg(long)]
        confirm: bool,
    },
    /// Cancela todas as ordens abertas de um simbolo no broker (conta paper).
    CancelOrders {
        /// Símbolo.
        #[arg(short, long)]
        symbol: String,
        /// Provedor.
        #[arg(long, default_value = "simulated")]
        provider: String,
        /// Sem esta flag o comando só lista.
        #[arg(long)]
        confirm: bool,
    },
    /// Ingere candles históricos no banco.
    Ingest {
        /// Símbolo do ativo.
        #[arg(short, long)]
        symbol: String,
        /// Timeframe (1m, 5m, 15m, 30m, 1h, 4h, 1d).
        #[arg(short, long, default_value = "15m")]
        timeframe: TimeFrameArg,
        /// Quantidade de dias para trás.
        #[arg(short, long, default_value_t = 30)]
        days: i64,
        /// Provedor de dados.
        #[arg(long, default_value = "simulated")]
        provider: String,
    },
    /// Inicia loop de paper trading.
    Paper {
        /// Símbolo do ativo.
        #[arg(short, long, default_value = "SPY")]
        symbol: String,
        /// Estratégia ativa.
        #[arg(long, default_value = "pullback-trend-v1")]
        strategy: String,
        /// Modo de execução: simulated, replay ou live.
        #[arg(long, default_value = "simulated")]
        mode: String,
        /// Timeframe (1m, 5m, 15m, 30m, 1h, 4h, 1d).
        #[arg(short, long, default_value = "15m")]
        timeframe: TimeFrameArg,
    },
    /// Executa backtest de uma estratégia.
    Backtest {
        /// Símbolo do ativo.
        #[arg(short, long, default_value = "SPY")]
        symbol: String,
        /// Estratégia a testar.
        #[arg(long, default_value = "pullback-trend-v1")]
        strategy: String,
        /// Data de início (YYYY-MM-DD).
        #[arg(long)]
        from: Option<String>,
        /// Data de fim (YYYY-MM-DD).
        #[arg(long)]
        to: Option<String>,
        /// Timeframe (1m, 5m, 15m, 30m, 1h, 4h, 1d).
        #[arg(short, long, default_value = "15m")]
        timeframe: TimeFrameArg,
        /// Permite rodar sobre dados sintéticos se o banco estiver vazio
        /// (smoke test; não é evidência de performance).
        #[arg(long)]
        allow_synthetic: bool,
        /// Exporta o relatório em JSON para o caminho indicado.
        #[arg(short, long)]
        output: Option<String>,
        /// Slippage por execução em pontos-base (1 bp = 0,01%). Padrão: 10 bp.
        #[arg(long)]
        slippage_bps: Option<u32>,
        /// Desliga o flatten de fim de pregão (ADR-018). Só para reproduzir
        /// runs antigos: o resultado carrega posição pela noite, o que o live
        /// nunca faz.
        #[arg(long)]
        no_flatten: bool,
        /// Restaura o custo anterior ao §5.6: US$ 0,35 fixos por perna e alvo
        /// limite que enche sem pagar nada. Só para reproduzir runs de até
        /// 08/09/2026 — a IBKR cobra US$ 0,005 POR AÇÃO (mín. US$ 1,00).
        #[arg(long)]
        legacy_cost: bool,
        /// Desconto no fill do alvo em pontos-base (padrão 2; 0 no modo
        /// legado). É o parâmetro escolhido a mão do modelo de custo, e
        /// responde por mais da metade do efeito — varie para ver quanto do
        /// veredito depende dele.
        #[arg(long)]
        limit_haircut_bps: Option<rust_decimal::Decimal>,
        /// Rótulo do run no banco. Sem ele, grava `backtest-<data>` — nunca
        /// NULL, para o run não ficar impossível de atribuir depois.
        #[arg(long)]
        label: Option<String>,
    },
    /// Validação walk-forward out-of-sample sobre dados reais do banco.
    Walkforward {
        /// Símbolo do ativo.
        #[arg(short, long, default_value = "SPY")]
        symbol: String,
        /// Estratégia a validar.
        #[arg(long, default_value = "pullback-trend-v1")]
        strategy: String,
        /// Data de início (YYYY-MM-DD).
        #[arg(long)]
        from: Option<String>,
        /// Data de fim (YYYY-MM-DD).
        #[arg(long)]
        to: Option<String>,
        /// Timeframe (1m, 5m, 15m, 30m, 1h, 4h, 1d).
        #[arg(short, long, default_value = "15m")]
        timeframe: TimeFrameArg,
        /// Número de janelas out-of-sample.
        #[arg(short, long, default_value_t = 4)]
        windows: usize,
        /// Desliga o flatten de fim de pregão (ADR-018). Só para reproduzir
        /// os runs 413–421 e medir o delta; não vale como gate A.
        #[arg(long)]
        no_flatten: bool,
        /// Restaura o custo anterior ao §5.6: US$ 0,35 fixos por perna e alvo
        /// limite que enche sem pagar nada. Só para reproduzir runs de até
        /// 08/09/2026 — a IBKR cobra US$ 0,005 POR AÇÃO (mín. US$ 1,00).
        #[arg(long)]
        legacy_cost: bool,
        /// Desconto no fill do alvo em pontos-base (padrão 2; 0 no modo
        /// legado). É o parâmetro escolhido a mão do modelo de custo, e
        /// responde por mais da metade do efeito — varie para ver quanto do
        /// veredito depende dele.
        #[arg(long)]
        limit_haircut_bps: Option<rust_decimal::Decimal>,
        /// Exporta o resultado (janelas, métricas, trades do holdout) em JSON.
        #[arg(short, long)]
        output: Option<String>,
        /// Slippage por execução em pontos-base (aceita fração: 2.5).
        #[arg(long)]
        slippage_bps: Option<rust_decimal::Decimal>,
        /// Rótulo do run. Obrigatório com --set/--strategy-config.
        #[arg(long)]
        label: Option<String>,
        /// Bloco final travado (YYYY-MM-DD). Nunca entra em seleção; roda uma
        /// vez por família de hipótese.
        #[arg(long)]
        holdout_from: Option<String>,
        /// TOML alternativo para a mesma struct de parâmetros.
        #[arg(long)]
        strategy_config: Option<String>,
        /// Sobrescreve um parâmetro: --set chave=valor (repetível).
        #[arg(long = "set")]
        set: Vec<String>,
    },
    /// Analisa resultados do live/paper e compara com o backtest mais recente.
    Analyze {
        /// Símbolo do ativo.
        #[arg(short, long, default_value = "SPY")]
        symbol: String,
        /// Estratégia de referência.
        #[arg(long, default_value = "pullback-trend-v1")]
        strategy: String,
    },
    /// Exibe status atual do bot.
    Status,
    /// Exibe diário automático de trades e rejeições.
    Journal {
        /// Data no formato YYYY-MM-DD.
        #[arg(short, long)]
        date: Option<String>,
    },
    /// Diagnóstico cru do feed de candles da IBKR (não persiste nada).
    DebugCandles {
        /// Símbolo do ativo.
        #[arg(short, long, default_value = "IWV")]
        symbol: String,
        /// Timeframe (1m, 5m, 15m, 30m, 1h, 4h, 1d).
        #[arg(short, long, default_value = "15m")]
        timeframe: TimeFrameArg,
        /// Quantidade de dias para trás.
        #[arg(short, long, default_value_t = 1)]
        days: i32,
        /// Quantas barras finais imprimir.
        #[arg(short, long, default_value_t = 30)]
        bars: usize,
        /// Pede MarketDataType::Realtime antes de buscar.
        #[arg(long)]
        realtime: bool,
    },
}

#[derive(Debug, Clone, ValueEnum)]
enum TimeFrameArg {
    #[value(name = "1m")]
    M1,
    #[value(name = "5m")]
    M5,
    #[value(name = "15m")]
    M15,
    #[value(name = "30m")]
    M30,
    #[value(name = "1h")]
    H1,
    #[value(name = "4h")]
    H4,
    #[value(name = "1d")]
    D1,
}

impl From<TimeFrameArg> for trader_domain::TimeFrame {
    fn from(arg: TimeFrameArg) -> Self {
        match arg {
            TimeFrameArg::M1 => trader_domain::TimeFrame::M1,
            TimeFrameArg::M5 => trader_domain::TimeFrame::M5,
            TimeFrameArg::M15 => trader_domain::TimeFrame::M15,
            TimeFrameArg::M30 => trader_domain::TimeFrame::M30,
            TimeFrameArg::H1 => trader_domain::TimeFrame::H1,
            TimeFrameArg::H4 => trader_domain::TimeFrame::H4,
            TimeFrameArg::D1 => trader_domain::TimeFrame::D1,
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Carrega variáveis de ambiente do .env (se existir) antes de ler a config.
    let _ = dotenvy::dotenv();

    let cli = Cli::parse();

    // Inicializa logging com base na config.
    let app_config = CliConfig::load()?;
    trader_infra::logging::init_logging(
        &app_config.app_config.logging.level,
        &app_config.app_config.logging.format,
    );

    info!(command = ?cli.command, "comando recebido");

    match cli.command {
        Commands::TestConnection { provider } => {
            let config = config_with_provider(app_config, provider);
            commands::test_connection::run(&config).await
        }
        Commands::Account { provider } => {
            let config = config_with_provider(app_config, provider);
            commands::account::run(&config).await
        }
        Commands::Flatten {
            symbol,
            provider,
            confirm,
        } => {
            let config = config_with_provider(app_config, provider);
            commands::flatten::run(&config, &symbol, confirm).await
        }
        Commands::CancelOrders {
            symbol,
            provider,
            confirm,
        } => {
            let config = config_with_provider(app_config, provider);
            commands::flatten::cancel_orders(&config, &symbol, confirm).await
        }
        Commands::Ingest {
            symbol,
            timeframe,
            days,
            provider,
        } => {
            let config = config_with_provider(app_config, provider);
            commands::ingest::run(
                &config,
                commands::ingest::Args {
                    symbol,
                    timeframe: timeframe.into(),
                    days,
                },
            )
            .await
        }
        Commands::Paper {
            symbol,
            strategy,
            mode,
            timeframe,
        } => {
            let mode = mode.parse::<commands::paper::PaperMode>()?;
            commands::paper::run(
                &app_config,
                commands::paper::Args {
                    symbol,
                    strategy,
                    mode,
                    timeframe: timeframe.into(),
                },
            )
            .await
        }
        Commands::Backtest {
            symbol,
            strategy,
            from,
            to,
            timeframe,
            allow_synthetic,
            output,
            slippage_bps,
            no_flatten,
            legacy_cost,
            limit_haircut_bps,
            label,
        } => {
            let from = from
                .and_then(|s| chrono::NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok())
                .map(|d| d.and_hms_opt(0, 0, 0).unwrap().and_utc());
            let to = to
                .and_then(|s| chrono::NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok())
                .map(|d| d.and_hms_opt(23, 59, 59).unwrap().and_utc());

            commands::backtest::run(
                &app_config,
                commands::backtest::Args {
                    symbol,
                    strategy,
                    from,
                    to,
                    timeframe: timeframe.into(),
                    allow_synthetic,
                    output,
                    slippage_bps,
                    no_flatten,
                    legacy_cost,
                    limit_haircut_bps,
                    label,
                },
            )
            .await
        }
        Commands::Status => commands::status::run(&app_config).await,
        Commands::Analyze { symbol, strategy } => {
            commands::analyze::run(&app_config, commands::analyze::Args { symbol, strategy }).await
        }
        Commands::Journal { date } => commands::journal::run(&app_config, date).await,
        Commands::DebugCandles {
            symbol,
            timeframe,
            days,
            bars,
            realtime,
        } => {
            commands::debug_candles::run(
                &app_config,
                commands::debug_candles::Args {
                    symbol,
                    timeframe: timeframe.into(),
                    days,
                    bars,
                    realtime,
                },
            )
            .await
        }
        Commands::Walkforward {
            symbol,
            strategy,
            from,
            to,
            timeframe,
            windows,
            no_flatten,
            output,
            slippage_bps,
            label,
            holdout_from,
            strategy_config,
            set,
            legacy_cost,
            limit_haircut_bps,
        } => {
            let from = from
                .and_then(|s| chrono::NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok())
                .map(|d| d.and_hms_opt(0, 0, 0).unwrap().and_utc());
            let to = to
                .and_then(|s| chrono::NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok())
                .map(|d| d.and_hms_opt(23, 59, 59).unwrap().and_utc());
            // Data inválida aqui NÃO pode virar `None`: o padrão `.ok()` usado
            // em `--from`/`--to` desligaria o holdout em silêncio, e um run sem
            // holdout parece exatamente com um run com holdout que não cortou
            // nada (ADR-019, riscos).
            let holdout_from = holdout_from
                .map(|s| {
                    chrono::NaiveDate::parse_from_str(&s, "%Y-%m-%d")
                        .map(|d| d.and_hms_opt(0, 0, 0).unwrap().and_utc())
                        .map_err(|e| {
                            anyhow::anyhow!("--holdout-from inválido ({s}): {e}. Use YYYY-MM-DD.")
                        })
                })
                .transpose()?;

            commands::walkforward::run(
                &app_config,
                commands::walkforward::Args {
                    symbol,
                    strategy,
                    from,
                    to,
                    timeframe: timeframe.into(),
                    windows,
                    no_flatten,
                    output,
                    slippage_bps,
                    label,
                    holdout_from,
                    strategy_config,
                    set,
                    legacy_cost,
                    limit_haircut_bps,
                },
            )
            .await
        }
    }
}

fn config_with_provider(mut config: CliConfig, provider: String) -> CliConfig {
    config.provider = provider;
    config
}
