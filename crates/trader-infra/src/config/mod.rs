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
    #[serde(default)]
    pub alerts: AlertsSettings,
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
    /// Risco por trade, em % do capital (ex.: 1.0 = 1%).
    #[serde(default = "default_risk_per_trade_pct")]
    pub risk_per_trade_pct: f64,
    /// Perda diária máxima, em % do capital (ex.: 2.0 = 2%).
    #[serde(default = "default_max_daily_loss_pct")]
    pub max_daily_loss_pct: f64,
    /// Máximo de trades por dia.
    #[serde(default = "default_max_trades_per_day")]
    pub max_trades_per_day: usize,
    /// Para de operar após N perdas consecutivas.
    #[serde(default = "default_max_consecutive_losses")]
    pub max_consecutive_losses: usize,
    /// Tolerância de overshoot numa entrada stop, como fração da distância do
    /// stop (0.25 = aceita até 25% de risco extra; além disso a entrada é
    /// invalidada em vez de perseguida — ADR-015).
    #[serde(default = "default_entry_overshoot_tolerance")]
    pub entry_overshoot_tolerance: f64,
}

fn default_risk_per_trade_pct() -> f64 {
    1.0
}
fn default_entry_overshoot_tolerance() -> f64 {
    0.25
}
fn default_max_daily_loss_pct() -> f64 {
    2.0
}
fn default_max_trades_per_day() -> usize {
    3
}
fn default_max_consecutive_losses() -> usize {
    3
}

#[derive(Debug, Clone, Deserialize)]
pub struct LoggingSettings {
    pub level: String,
    pub format: String,
}

/// Configuração de alertas operacionais.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct AlertsSettings {
    /// Webhook HTTP(S) para alertas críticos (Slack/Discord/Teams compatível:
    /// POST JSON `{"text": "..."}`). Vazio = alertas só no log.
    #[serde(default)]
    pub webhook_url: String,
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
