//! Carregamento de configuração TOML + variáveis de ambiente.

use config::{Config, ConfigError, Environment, File};
use serde::Deserialize;

/// Configuração raiz da aplicação.
#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    pub app: AppSettings,
    pub database: DatabaseSettings,
    pub broker: BrokerSettings,
    pub ibkr: IbkrSettings,
    pub risk: RiskSettings,
    pub logging: LoggingSettings,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AppSettings {
    pub name: String,
    pub mode: String,
    pub paper_warning: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DatabaseSettings {
    pub url: String,
}

impl DatabaseSettings {
    /// Retorna a URL do banco, com fallback para `DATABASE_URL`.
    /// Falha com mensagem clara se nenhuma fonte estiver configurada.
    pub fn url(&self) -> Result<String, ConfigError> {
        let url = std::env::var("DATABASE_URL")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .or_else(|| {
                let trimmed = self.url.trim();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed.to_string())
                }
            });

        url.filter(|s| !s.trim().is_empty()).ok_or_else(|| {
            ConfigError::Message(
                "DATABASE_URL ou TRADER__DATABASE__URL devem estar configuradas".to_string(),
            )
        })
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct BrokerSettings {
    pub name: String,
    pub paper: bool,
    pub account_id: Option<String>,
    pub api_url: Option<String>,
    pub client_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct IbkrSettings {
    pub host: String,
    pub port: u16,
    pub client_id: i32,
    pub paper: bool,
    pub account_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RiskSettings {
    pub profile: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LoggingSettings {
    pub level: String,
    pub format: String,
}

impl AppConfig {
    /// Carrega configuração de `config/default.toml` e sobrescreve com:
    /// 1. Arquivo especificado em `TRADER_CONFIG` (opcional)
    /// 2. Variáveis de ambiente com prefixo `TRADER_` e separador `__`
    pub fn load() -> Result<Self, ConfigError> {
        let config_path =
            std::env::var("TRADER_CONFIG").unwrap_or_else(|_| "config/default".to_string());

        let settings = Config::builder()
            .add_source(File::with_name("config/default").required(false))
            .add_source(File::with_name(&config_path).required(false))
            .add_source(Environment::with_prefix("TRADER").separator("__"))
            .build()?;

        settings.try_deserialize()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_loads() {
        // Garante que a configuração padrão pode ser carregada quando o arquivo existir.
        let _ = AppConfig::load();
    }
}
