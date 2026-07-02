//! Comando `journal`.

use anyhow::Result;
use chrono::{Local, NaiveDate};
use tracing::info;

use trader_domain::SignalStatus;
use trader_infra::{
    db::create_pool,
    repositories::{SqlxSignalRepository, SqlxTradeRepository},
};

use crate::config::CliConfig;

/// Exibe diário automático de trades e rejeições para uma data.
pub async fn run(config: &CliConfig, date: Option<String>) -> Result<()> {
    let target_date = match date {
        Some(d) => NaiveDate::parse_from_str(&d, "%Y-%m-%d")?,
        None => Local::now().date_naive(),
    };

    info!(date = %target_date, "consultando journal");

    println!("📓 Journal de {}\n", target_date.format("%d/%m/%Y"));

    let database_url = config
        .app_config
        .database
        .url()
        .map_err(|e| anyhow::anyhow!("DATABASE_URL não configurada: {e}"))?;
    let pool = create_pool(&database_url)
        .await
        .map_err(|e| anyhow::anyhow!("falha ao conectar no banco: {e}"))?;

    let symbol = std::env::var("TRADER_SYMBOL").unwrap_or_else(|_| "SPY".to_string());

    let trade_repo = SqlxTradeRepository::new(pool.clone());
    let signal_repo = SqlxSignalRepository::new(pool.clone());

    // Trades do dia.
    match trade_repo.list_today(&symbol).await {
        Ok(trades) => {
            if trades.is_empty() {
                println!("Nenhum trade fechado hoje.");
            } else {
                println!("Trades:");
                let mut total_pnl = rust_decimal::Decimal::ZERO;
                for t in trades {
                    total_pnl += t.net_pnl;
                    println!(
                        "  {} {:?} | entry={} exit={} | net_pnl={} | result={}R | exit_reason={:?}",
                        t.exit_time.format("%H:%M"),
                        t.direction,
                        t.entry_price,
                        t.exit_price,
                        t.net_pnl,
                        t.result_in_r,
                        t.exit_reason
                    );
                }
                println!("\nTotal P&L do dia: {}", total_pnl);
            }
        }
        Err(e) => println!("Erro ao carregar trades: {e}"),
    }

    // Sinais rejeitados do dia.
    match signal_repo.list_today(&symbol).await {
        Ok(signals) => {
            let rejected: Vec<_> = signals
                .into_iter()
                .filter(|s| s.status == SignalStatus::Rejected)
                .collect();

            if rejected.is_empty() {
                println!("\nNenhum sinal rejeitado hoje.");
            } else {
                println!("\nSinais rejeitados:");
                for s in rejected {
                    println!(
                        "  {} | reason={:?} | details={:?}",
                        s.timestamp.format("%H:%M"),
                        s.rejection_reason,
                        s.rejection_details
                    );
                }
            }
        }
        Err(e) => println!("Erro ao carregar sinais: {e}"),
    }

    Ok(())
}
