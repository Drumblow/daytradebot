//! Comando `account`.

use anyhow::Result;
use tracing::info;

use trader_adapters::{ibkr::IbkrBrokerAdapter, simulated::SimulatedBroker};
use trader_infra::ports::Broker;

use crate::config::CliConfig;

/// Exibe resumo da conta no broker escolhido.
pub async fn run(config: &CliConfig) -> Result<()> {
    info!(provider = %config.provider, "consultando conta");

    let summary = match config.provider.as_str() {
        "ibkr" => {
            let ibkr_config = config.ibkr_config()?;
            IbkrBrokerAdapter::new(ibkr_config)
                .get_account_summary()
                .await?
        }
        "simulated" => {
            SimulatedBroker::new(
                Some("DU_SIM".to_string()),
                rust_decimal::Decimal::from(100_000),
            )
            .get_account_summary()
            .await?
        }
        other => anyhow::bail!("provedor desconhecido: {}", other),
    };

    println!("Broker:        {}", summary.broker);
    println!(
        "Account ID:    {}",
        summary.account_id.as_deref().unwrap_or("N/A")
    );
    println!("Cash:          {}", summary.cash);
    println!("Equity:        {}", summary.equity);
    println!("Buying Power:  {}", summary.buying_power);
    println!("Daily PnL:     {}", summary.daily_pnl);

    Ok(())
}
