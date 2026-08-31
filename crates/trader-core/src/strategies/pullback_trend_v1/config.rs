//! Configuração da estratégia Pullback em Tendência de Alta v1.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Configuração carregável de `config/strategies/pullback-trend-v1.toml`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PullbackTrendV1Config {
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

    pub min_candles_above_ema20: usize,
    pub ema_context_period: usize,
    pub sma_context_period: usize,

    pub max_pullback_candles: usize,
    pub min_signal_body_ratio: Decimal,
    pub signal_close_position: String,

    pub entry_offset_ticks: Decimal,
    pub stop_offset_ticks: Decimal,
    pub reward_multiple: Decimal,

    /// Tipo da ordem de entrada: "stop" (buy stop no gatilho — regra do
    /// livro) ou "limit" (executa imediatamente).
    pub entry_order_type: String,
    /// Quantos candles após o sinal a ordem de entrada stop permanece válida
    /// esperando o rompimento (1 = só o candle seguinte; regra do livro).
    pub entry_validity_candles: usize,

    pub max_spread_pct: Decimal,
    pub max_atr_pct: Decimal,
    pub min_risk_reward: Decimal,

    pub tick_size: Decimal,

    pub trading_start_time: String,
    pub trading_end_time: String,
}

impl PullbackTrendV1Config {
    /// Calcula o hash SHA256 da configuração para auditoria.
    pub fn config_hash(&self) -> String {
        let json = serde_json::to_string(self).unwrap_or_default();
        let mut hasher = Sha256::new();
        hasher.update(json.as_bytes());
        format!("{:x}", hasher.finalize())[..16].to_string()
    }
}

impl Default for PullbackTrendV1Config {
    fn default() -> Self {
        Self {
            strategy: StrategyWithParameters {
                id: "pullback-trend-v1".to_string(),
                version: "1.0.0".to_string(),
                source: "Al Brooks - Trading Price Action Trends".to_string(),
                parameters: StrategyParameters {
                    operational_timeframe: "15m".to_string(),
                    context_timeframe: "1h".to_string(),
                    macro_timeframe: "1d".to_string(),
                    min_candles_above_ema20: 10,
                    ema_context_period: 20,
                    sma_context_period: 200,
                    max_pullback_candles: 5,
                    min_signal_body_ratio: Decimal::from(15) / Decimal::from(10),
                    signal_close_position: "upper_third".to_string(),
                    entry_offset_ticks: Decimal::from(1),
                    stop_offset_ticks: Decimal::from(1),
                    reward_multiple: Decimal::from(2),
                    entry_order_type: "stop".to_string(),
                    entry_validity_candles: 1,
                    max_spread_pct: Decimal::from(5) / Decimal::from(100),
                    max_atr_pct: Decimal::from(15) / Decimal::from(10),
                    min_risk_reward: Decimal::from(2),
                    tick_size: Decimal::from(1) / Decimal::from(100),
                    trading_start_time: "09:30:00".to_string(),
                    trading_end_time: "16:00:00".to_string(),
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
        let config = PullbackTrendV1Config::default();
        assert_eq!(config.config_hash(), config.config_hash());
        assert_eq!(config.config_hash().len(), 16);

        let mut changed = config.clone();
        changed.strategy.parameters.reward_multiple = Decimal::from(3);
        assert_ne!(config.config_hash(), changed.config_hash());
    }

    #[test]
    fn parses_project_toml() {
        let toml_str = include_str!("../../../../../config/strategies/pullback-trend-v1.toml");
        let config: PullbackTrendV1Config =
            toml::from_str(toml_str).expect("TOML do projeto deve fazer parse");
        assert_eq!(config.strategy.id, "pullback-trend-v1");
        assert_eq!(config.strategy.parameters.min_candles_above_ema20, 10);
    }
}
