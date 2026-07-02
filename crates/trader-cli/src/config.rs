//! Configuração compartilhada do CLI.

use anyhow::{Context, Result};

use trader_adapters::ibkr::IbkrConfig;
use trader_infra::config::AppConfig;

/// Configuração consolidada do CLI.
#[derive(Debug, Clone)]
pub struct CliConfig {
    pub app_config: AppConfig,
    pub provider: String,
}

impl CliConfig {
    /// Carrega configuração da aplicação e define provider padrão.
    pub fn load() -> Result<Self> {
        let app_config = AppConfig::load().context("falha ao carregar configuração")?;

        let provider = std::env::var("TRADER_PROVIDER")
            .ok()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "simulated".to_string());

        Ok(Self {
            app_config,
            provider,
        })
    }

    /// Retorna configuração do IBKR a partir da configuração carregada.
    pub fn ibkr_config(&self) -> Result<IbkrConfig> {
        Ok(IbkrConfig {
            host: self.app_config.ibkr.host.clone(),
            port: self.app_config.ibkr.port,
            client_id: self.app_config.ibkr.client_id,
            account_id: self.app_config.ibkr.account_id.clone(),
            paper: self.app_config.ibkr.paper,
        })
    }
}
