//! `trader-cli` — entrypoint do HumanStyle Trader Bot.

use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};
use tracing::info;

mod alerts;
mod commands;
mod config;
mod dispatch;
mod risk_config;
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
                },
            )
            .await
        }
        Commands::Status => commands::status::run(&app_config).await,
        Commands::Analyze { symbol, strategy } => {
            commands::analyze::run(&app_config, commands::analyze::Args { symbol, strategy }).await
        }
        Commands::Journal { date } => commands::journal::run(&app_config, date).await,
        Commands::Walkforward {
            symbol,
            strategy,
            from,
            to,
            timeframe,
            windows,
        } => {
            let from = from
                .and_then(|s| chrono::NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok())
                .map(|d| d.and_hms_opt(0, 0, 0).unwrap().and_utc());
            let to = to
                .and_then(|s| chrono::NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok())
                .map(|d| d.and_hms_opt(23, 59, 59).unwrap().and_utc());

            commands::walkforward::run(
                &app_config,
                commands::walkforward::Args {
                    symbol,
                    strategy,
                    from,
                    to,
                    timeframe: timeframe.into(),
                    windows,
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
