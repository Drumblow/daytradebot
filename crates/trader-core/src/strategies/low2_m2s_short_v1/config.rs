//! Configuração da estratégia Low 2 / M2S Short v1.
//!
//! Espelho da `pullback-trend-v1` para tendências de baixa. Todos os limiares
//! vêm de `config/strategies/low2-m2s-short-v1.toml` (regra de ouro: nenhuma
//! regra hardcoded). Especificação: `docs/strategies/low2-m2s-short-v1.md`.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Configuração carregável de `config/strategies/low2-m2s-short-v1.toml`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Low2M2sShortV1Config {
    pub strategy: StrategyWithParameters,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StrategyWithParameters {
    pub id: String,
    pub version: String,
    pub source: String,
    pub parameters: StrategyParameters,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StrategyParameters {
    pub operational_timeframe: String,
    pub context_timeframe: String,
    pub macro_timeframe: String,

    /// Candles consecutivos fechando ABAIXO da EMA de contexto (espelho do
    /// min_candles_above_ema20 da irmã long).
    pub min_candles_below_ema20: usize,
    pub ema_context_period: usize,
    pub sma_context_period: usize,

    pub max_pullback_candles: usize,
    /// Sombra superior / corpo mínima na barra de sinal bear (espelho do
    /// min_signal_body_ratio da irmã).
    pub min_signal_body_ratio: Decimal,
    /// Fechamento exigido na barra de sinal: "lower_third" (default) ou
    /// "lower_half".
    pub signal_close_position: String,

    pub entry_offset_ticks: Decimal,
    pub stop_offset_ticks: Decimal,
    pub reward_multiple: Decimal,

    /// Tipo da ordem de entrada: "stop" (sell stop no gatilho — regra do
    /// livro) ou "limit".
    pub entry_order_type: String,
    /// Candles de validade da entrada stop esperando o rompimento (ADR-009).
    pub entry_validity_candles: usize,

    pub max_spread_pct: Decimal,
    pub max_atr_pct: Decimal,
    pub min_risk_reward: Decimal,

    /// Override de risco por trade, em pontos percentuais. `None` = global.
    pub risk_per_trade_pct: Option<Decimal>,

    pub tick_size: Decimal,

    pub trading_start_time: String,
    pub trading_end_time: String,
}

impl Low2M2sShortV1Config {
    /// Calcula o hash SHA256 da configuração para auditoria.
    pub fn config_hash(&self) -> String {
        let json = serde_json::to_string(self).unwrap_or_default();
        let mut hasher = Sha256::new();
        hasher.update(json.as_bytes());
        format!("{:x}", hasher.finalize())[..16].to_string()
    }
}

impl Default for Low2M2sShortV1Config {
    fn default() -> Self {
        Self {
            strategy: StrategyWithParameters {
                id: "low2-m2s-short-v1".to_string(),
                version: "1.0.0".to_string(),
                source: "Al Brooks - Reading Price Charts Bar by Bar, Cap. 4 (Low 2 / M2S)"
                    .to_string(),
                parameters: StrategyParameters {
                    operational_timeframe: "15m".to_string(),
                    context_timeframe: "1h".to_string(),
                    macro_timeframe: "1d".to_string(),
                    min_candles_below_ema20: 10,
                    ema_context_period: 20,
                    sma_context_period: 200,
                    max_pullback_candles: 5,
                    min_signal_body_ratio: Decimal::from(15) / Decimal::from(10), // 1,5
                    signal_close_position: "lower_third".to_string(),
                    entry_offset_ticks: Decimal::ONE,
                    stop_offset_ticks: Decimal::ONE,
                    reward_multiple: Decimal::from(2),
                    entry_order_type: "stop".to_string(),
                    entry_validity_candles: 2,
                    max_spread_pct: Decimal::from(5) / Decimal::from(100),
                    max_atr_pct: Decimal::from(15) / Decimal::from(10),
                    min_risk_reward: Decimal::from(2),
                    risk_per_trade_pct: None,
                    tick_size: Decimal::from(1) / Decimal::from(100),
                    trading_start_time: "09:45:00".to_string(), // ET
                    trading_end_time: "15:15:00".to_string(),   // ET
                },
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_hash_is_stable_and_sensitive() {
        let config = Low2M2sShortV1Config::default();
        assert_eq!(config.config_hash(), config.config_hash());
        assert_eq!(config.config_hash().len(), 16);

        let mut changed = config.clone();
        changed.strategy.parameters.reward_multiple = Decimal::from(3);
        assert_ne!(config.config_hash(), changed.config_hash());
    }

    #[test]
    fn parses_project_toml() {
        let toml_str = include_str!("../../../../../config/strategies/low2-m2s-short-v1.toml");
        let config: Low2M2sShortV1Config =
            toml::from_str(toml_str).expect("TOML do projeto deve fazer parse");
        assert_eq!(config.strategy.id, "low2-m2s-short-v1");
        assert_eq!(config.strategy.parameters.min_candles_below_ema20, 10);
    }
}
