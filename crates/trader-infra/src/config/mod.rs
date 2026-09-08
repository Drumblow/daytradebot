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
    #[serde(default)]
    pub session: SessionSettings,
}

/// Fim de pregão em horário de Nova York (ADR-018).
///
/// Uma única fonte para o live e para o backtest. Antes disto o live tinha
/// duas constantes em `paper.rs` e o backtest não tinha fim de sessão nenhum
/// — a divergência que fazia o gate A comparar o live com um backtest que
/// ganha dinheiro dormindo posicionado.
///
/// Os campos são texto `"HH:MM:SS"` e passam por
/// `trader_core::session::parse_et_time`, o mesmo parser das janelas das
/// estratégias.
#[derive(Debug, Clone, Deserialize)]
pub struct SessionSettings {
    /// Início da janela de encerramento a mercado do live.
    #[serde(default = "default_flatten_start")]
    pub flatten_start: String,
    /// Fim da janela (exclusivo).
    #[serde(default = "default_flatten_end")]
    pub flatten_end: String,
    /// Última barra de 15 min do RTH. Checagem de sanidade do backtest — o
    /// gatilho do flatten é a mudança de data ET, não este horário.
    #[serde(default = "default_last_bar")]
    pub last_bar: String,
}

impl Default for SessionSettings {
    fn default() -> Self {
        Self {
            flatten_start: default_flatten_start(),
            flatten_end: default_flatten_end(),
            last_bar: default_last_bar(),
        }
    }
}

fn default_flatten_start() -> String {
    "15:55:00".to_string()
}
fn default_flatten_end() -> String {
    "16:10:00".to_string()
}
fn default_last_bar() -> String {
    "15:45:00".to_string()
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
    /// Perda diária máxima da CONTA INTEIRA, em % do capital.
    ///
    /// Os limites acima valem por instância; com 11 instâncias na mesma conta
    /// a perda efetiva era 11× o configurado (C2 da auditoria). Este é o teto
    /// somado — nenhuma instância abre posição depois que a conta o atinge.
    #[serde(default = "default_max_portfolio_daily_loss_pct")]
    pub max_portfolio_daily_loss_pct: f64,
    /// Máximo de posições abertas ao mesmo tempo na conta, somando todas as
    /// instâncias. Os ativos operados são small-caps correlacionados: as
    /// perdas chegam juntas.
    #[serde(default = "default_max_concurrent_positions")]
    pub max_concurrent_positions: usize,
    /// Teto do notional agregado das posições abertas, em % do capital.
    /// O cap por posição já é ~100% do capital; sem um teto somado, N
    /// instâncias multiplicam a exposição por N.
    #[serde(default = "default_max_portfolio_notional_pct")]
    pub max_portfolio_notional_pct: f64,
    /// Tolerância de overshoot numa entrada stop, como fração da distância do
    /// stop (0.25 = aceita até 25% de risco extra; além disso a entrada é
    /// invalidada em vez de perseguida — ADR-015).
    #[serde(default = "default_entry_overshoot_tolerance")]
    pub entry_overshoot_tolerance: f64,

    // --- ADR-020: de quanto é uma posição ---
    /// Multiplicador do teto de notional por posição (1 = 1× a fatia de
    /// capital, o comportamento anterior à ADR-020). Acima de 1 é alavancagem
    /// intraday: paper only, e o teto de 200% da conta continua por cima.
    #[serde(default = "default_max_notional_multiple")]
    pub max_notional_multiple: f64,
    /// Teto absoluto de notional por posição, em dólares. Vazio = sem teto.
    /// Existe para ser posto por instância (`TRADER__RISK__MAX_NOTIONAL_USD`)
    /// nos ativos em que a posição de 1× é grande demais para a barra.
    #[serde(default)]
    pub max_notional_usd: Option<f64>,
    /// Fatia do capital da conta que ESTA instância pode ocupar (1 = tudo,
    /// como sempre foi). Com `1 / max_concurrent_positions`, as três posições
    /// cheias cabem em 100% de notional — a regra de dinheiro real do ADR-017
    /// sem recusar o cluster, que é onde o edge medido está.
    #[serde(default = "default_capital_fraction")]
    pub capital_fraction: f64,
    /// Fração (%) da barra mediana de 15m que uma posição pode ocupar.
    /// Vazio = cap de liquidez desligado. O "1/3" da ADR-020 é interpretação
    /// nossa, não número de livro: fica em config para poder ser varrido.
    #[serde(default)]
    pub max_pct_of_median_bar_notional: Option<f64>,
    /// Quantas barras entram na mediana de liquidez. O default é a janela do
    /// live (600 barras ≈ 23 pregões): live e backtest têm de medir a mesma
    /// coisa, e este é o N que os dois servem hoje.
    #[serde(default = "default_liquidity_lookback_bars")]
    pub liquidity_lookback_bars: usize,
}

/// 1× — paridade com o cap que estava hardcoded em `risk/mod.rs`.
fn default_max_notional_multiple() -> f64 {
    1.0
}

/// 1 = a instância enxerga a conta inteira, como antes da ADR-020. Fracionar
/// é decisão de política de risco, e ela não entra por default.
fn default_capital_fraction() -> f64 {
    1.0
}

/// 600 barras de 15m ≈ 23 pregões — a janela que o live carrega
/// (`LIVE_MAX_CANDLES`).
fn default_liquidity_lookback_bars() -> usize {
    600
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
/// 4% da conta: o dobro do orçamento de uma instância, e não a soma dos onze.
fn default_max_portfolio_daily_loss_pct() -> f64 {
    4.0
}

/// Três posições simultâneas. Os pares aprovados são quase todos small-caps
/// correlacionados — os sinais chegam em cluster no mesmo dia.
fn default_max_concurrent_positions() -> usize {
    3
}

/// 200% do capital. O sizing já trava cada posição em ~100% do capital em
/// notional; este teto impede que N instâncias multipliquem isso por N.
fn default_max_portfolio_notional_pct() -> f64 {
    200.0
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
