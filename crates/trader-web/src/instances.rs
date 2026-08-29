//! Configuração das instâncias exibidas no painel.
//!
//! O banco não tem uma tabela de instâncias — o pareamento símbolo×estratégia
//! vive no compose do app umbrelOS. O default abaixo espelha as 11 instâncias
//! de produção; `TRADER_WEB_INSTANCES` (JSON) sobrescreve sem rebuild quando o
//! compose mudar.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstanceConfig {
    pub name: String,
    pub symbol: String,
    pub strategy: String,
    pub client_id: i32,
}

/// As 11 instâncias do compose do app (client_ids 1–11).
fn default_instances() -> Vec<InstanceConfig> {
    let raw: [(&str, &str, &str, i32); 11] = [
        ("iwm-pullback", "IWM", "pullback-trend-v1", 1),
        ("iwv-pullback", "IWV", "pullback-trend-v1", 2),
        ("iwo-pullback", "IWO", "pullback-trend-v1", 3),
        ("ijs-balance", "IJS", "balance-area-breakout-v1", 4),
        ("vbr-balance", "VBR", "balance-area-breakout-v1", 5),
        ("avuv-balance", "AVUV", "balance-area-breakout-v1", 6),
        ("iwm-openrev", "IWM", "opening-reversal-v1", 7),
        ("iwn-openrev", "IWN", "opening-reversal-v1", 8),
        ("avuv-rangefade", "AVUV", "range-extreme-fade-v1", 9),
        ("slyv-rangefade", "SLYV", "range-extreme-fade-v1", 10),
        ("iwv-rangefade", "IWV", "range-extreme-fade-v1", 11),
    ];
    raw.into_iter()
        .map(|(name, symbol, strategy, client_id)| InstanceConfig {
            name: name.into(),
            symbol: symbol.into(),
            strategy: strategy.into(),
            client_id,
        })
        .collect()
}

pub fn load_from_env() -> Result<Vec<InstanceConfig>> {
    match std::env::var("TRADER_WEB_INSTANCES") {
        Ok(json) => serde_json::from_str(&json)
            .context("TRADER_WEB_INSTANCES não é um JSON válido de instâncias"),
        Err(_) => Ok(default_instances()),
    }
}
