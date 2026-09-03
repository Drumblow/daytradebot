//! Comando `account`.

use anyhow::Result;
use tracing::info;

use trader_adapters::{ibkr::IbkrBrokerAdapter, simulated::SimulatedBroker};
use trader_domain::Broker;

use crate::config::CliConfig;

/// Exibe resumo da conta no broker escolhido, mais **posições e ordens
/// abertas**.
///
/// As duas listas existem por um motivo concreto: posição ou ordem que o bot
/// não rastreia trava o símbolo inteiro na reconciliação do live — e em
/// silêncio, porque o ciclo é interrompido antes de gravar o contexto de
/// mercado. Foi o que deixou as duas instâncias de IWM sem avaliar nada entre
/// 07/08 e 03/09/2026. Este comando é como se enxerga isso de fora: o painel
/// lê só o banco, nunca o broker.
pub async fn run(config: &CliConfig) -> Result<()> {
    info!(provider = %config.provider, "consultando conta");

    match config.provider.as_str() {
        "ibkr" => {
            let broker = IbkrBrokerAdapter::new(config.ibkr_config()?);
            print_summary(&broker.get_account_summary().await?);
            print_exposure(&broker).await?;
        }
        "simulated" => {
            let broker = SimulatedBroker::new(trader_adapters::simulated::SimulatedBrokerConfig {
                account_id: Some("DU_SIM".to_string()),
                initial_cash: rust_decimal::Decimal::from(100_000),
                commission_per_trade: rust_decimal::Decimal::from(35)
                    / rust_decimal::Decimal::from(100),
                slippage_pct: rust_decimal::Decimal::from(1) / rust_decimal::Decimal::from(1000),
                entry_validity_candles: 1,
                entry_overshoot_tolerance: rust_decimal::Decimal::from(25)
                    / rust_decimal::Decimal::from(100),
            });
            print_summary(&broker.get_account_summary().await?);
            print_exposure(&broker).await?;
        }
        other => anyhow::bail!("provedor desconhecido: {}", other),
    }

    Ok(())
}

fn print_summary(summary: &trader_domain::AccountSummary) {
    println!("Broker:        {}", summary.broker);
    println!(
        "Account ID:    {}",
        summary.account_id.as_deref().unwrap_or("N/A")
    );
    println!("Cash:          {}", summary.cash);
    println!("Equity:        {}", summary.equity);
    println!("Buying Power:  {}", summary.buying_power);
    println!("Daily PnL:     {}", summary.daily_pnl);
}

async fn print_exposure<B: Broker>(broker: &B) -> Result<()> {
    let positions = broker.get_positions().await?;
    println!("\n=== POSICOES ABERTAS ({}) ===", positions.len());
    if positions.is_empty() {
        println!("  (nenhuma)");
    } else {
        for p in &positions {
            println!(
                "  {:<6} {:?} qty={} entrada={} stop={}",
                p.symbol, p.direction, p.quantity, p.avg_entry_price, p.stop_price
            );
        }
    }

    let orders = broker.get_open_orders().await?;
    println!("\n=== ORDENS ABERTAS ({}) ===", orders.len());
    if orders.is_empty() {
        println!("  (nenhuma)");
    } else {
        for o in &orders {
            println!(
                "  id={:<8} {:<6} {:?} {:?} qty={} preco={:?} stop={:?}",
                o.broker_order_id.as_deref().unwrap_or("?"),
                o.symbol,
                o.side,
                o.order_type,
                o.quantity,
                o.price,
                o.stop_price
            );
        }
    }

    if !positions.is_empty() || !orders.is_empty() {
        println!(
            "\n[!] Posicao ou ordem listada acima BLOQUEIA o simbolo no live: a\n\
             reconciliacao recusa novos setups enquanto houver exposicao."
        );
    }
    Ok(())
}
