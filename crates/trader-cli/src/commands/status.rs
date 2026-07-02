//! Comando `status`.

use anyhow::Result;
use tracing::info;

use trader_domain::{Broker, TradingMode};
use trader_infra::{
    db::create_pool,
    repositories::{SqlxSignalRepository, SqlxTradeRepository},
};

use crate::config::CliConfig;

/// Exibe status atual do bot: modo, posição, P&L do dia e sinais recentes.
pub async fn run(config: &CliConfig) -> Result<()> {
    info!("consultando status do bot");

    println!("🤖 Status do HumanStyle Trader Bot");
    println!("   Modo:        {}", config.app_config.app.mode);
    println!(
        "   Paper warning: {}",
        if config.app_config.app.paper_warning {
            "ATIVO"
        } else {
            "inativo"
        }
    );

    let is_paper = config
        .app_config
        .app
        .mode
        .parse::<TradingMode>()
        .map(|m| m.is_paper())
        .unwrap_or(false);

    if is_paper {
        println!("   ⚠️  PAPER TRADING — NÃO OPERANDO DINHEIRO REAL");
    }

    // Status do broker simulado (fallback simples).
    let broker = trader_adapters::simulated::SimulatedBroker::default_simulated();
    let summary = broker.get_account_summary().await?;
    let positions = broker.get_positions().await?;

    println!("\n💰 Conta (simulada)");
    println!("   Cash:   {}", summary.cash);
    println!("   Equity: {}", summary.equity);
    println!("   P&L dia: {}", summary.daily_pnl);

    if positions.is_empty() {
        println!("   Posição: nenhuma posição aberta");
    } else {
        for pos in positions {
            println!(
                "   Posição: {} {} @ {} | unrealized P&L: {} | stop: {} | alvo: {:?}",
                pos.symbol,
                pos.quantity,
                pos.avg_entry_price,
                pos.unrealized_pnl,
                pos.stop_price,
                pos.target_price
            );
        }
    }

    // Sinais e trades recentes do banco.
    if let Ok(database_url) = config.app_config.database.url() {
        if let Ok(pool) = create_pool(&database_url).await {
            let signal_repo = SqlxSignalRepository::new(pool.clone());
            let trade_repo = SqlxTradeRepository::new(pool.clone());

            // Usa SPY como exemplo padrão; em produção, viria da config ativa.
            let symbol = std::env::var("TRADER_SYMBOL").unwrap_or_else(|_| "SPY".to_string());

            match signal_repo.list_by_symbol(&symbol, 5).await {
                Ok(signals) => {
                    println!("\n📡 Sinais recentes ({symbol})");
                    for s in signals {
                        let status = format!("{:?}", s.status);
                        let direction = format!("{:?}", s.direction);
                        println!(
                            "   [{}] {} {} @ {} | entry={:?} stop={:?} target={:?}",
                            s.timestamp.format("%Y-%m-%d %H:%M"),
                            status,
                            direction,
                            s.symbol,
                            s.entry_price,
                            s.stop_price,
                            s.target_price
                        );
                    }
                }
                Err(e) => println!("   Erro ao carregar sinais: {e}"),
            }

            match trade_repo.list_by_symbol(&symbol, 5).await {
                Ok(trades) => {
                    println!("\n📊 Trades recentes ({symbol})");
                    for t in trades {
                        println!(
                            "   [{}] {:?} {} | entry={} exit={} | net_pnl={} | R={}",
                            t.exit_time.format("%Y-%m-%d %H:%M"),
                            t.direction,
                            t.symbol,
                            t.entry_price,
                            t.exit_price,
                            t.net_pnl,
                            t.result_in_r
                        );
                    }
                }
                Err(e) => println!("   Erro ao carregar trades: {e}"),
            }
        }
    }

    Ok(())
}
