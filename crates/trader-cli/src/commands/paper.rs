//! Comando `paper`.

use anyhow::Result;
use std::time::Duration;
use tracing::{info, warn};

use trader_adapters::simulated::SimulatedBroker;
use trader_infra::ports::Broker;

use crate::config::CliConfig;

/// Argumentos do comando paper.
pub struct Args {
    pub symbol: String,
    pub strategy: String,
}

/// Loop mínimo de paper trading (placeholder até a Fase 5).
///
/// Conecta ao broker simulado e imprime status a cada ciclo.
/// Não toma decisões de trading — isso será implementado no `trader-core`.
pub async fn run(_config: &CliConfig, args: Args) -> Result<()> {
    info!(symbol = %args.symbol, strategy = %args.strategy, "iniciando paper trading (modo simulado)");

    println!("🚀 Iniciando paper trading simulado");
    println!("   Ativo:     {}", args.symbol);
    println!("   Estratégia: {}", args.strategy);
    println!("   Modo:      SIMULADO (nenhuma ordem real é enviada)");

    let broker = SimulatedBroker::new(
        Some("DU_SIM".to_string()),
        rust_decimal::Decimal::from(100_000),
    );

    for i in 1..=5 {
        let summary = broker.get_account_summary().await?;
        println!(
            "[ciclo {i}] cash={} equity={} posições={}",
            summary.cash,
            summary.equity,
            broker.get_positions().await?.len()
        );
        tokio::time::sleep(Duration::from_millis(500)).await;
    }

    warn!("paper trading simulado encerrado após ciclos iniciais");
    println!("\n⏹️  Paper trading simulado encerrado.");
    println!("   O loop completo com sinais será implementado na Fase 5.");

    Ok(())
}
