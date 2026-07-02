//! Comando `test-connection`.

use anyhow::Result;
use tracing::{error, info};

use trader_adapters::{ibkr::IbkrMarketDataProvider, simulated::SimulatedMarketDataProvider};
use trader_infra::ports::MarketDataProvider;

use crate::config::CliConfig;

/// Verifica conexão com o provedor de dados escolhido.
pub async fn run(config: &CliConfig) -> Result<()> {
    info!(provider = %config.provider, "testando conexão");

    let health = match config.provider.as_str() {
        "ibkr" => {
            let ibkr_config = config.ibkr_config()?;
            IbkrMarketDataProvider::new(ibkr_config)
                .health_check()
                .await?
        }
        "simulated" => {
            SimulatedMarketDataProvider::new("SPY")
                .health_check()
                .await?
        }
        other => anyhow::bail!("provedor desconhecido: {}", other),
    };

    match health {
        trader_domain::ProviderHealth::Healthy => {
            info!("conexão saudável");
            println!("✅ Conexão saudável");
        }
        trader_domain::ProviderHealth::Degraded => {
            info!("conexão degradada");
            println!("⚠️  Conexão degradada");
        }
        trader_domain::ProviderHealth::Unhealthy => {
            error!("conexão insalubre");
            println!("❌ Conexão insalubre");
            if config.provider == "ibkr" {
                println!("   Verifique se o IB Gateway/TWS está aberto e configurado para aceitar conexões em 127.0.0.1:7497.");
            }
        }
    }

    Ok(())
}
