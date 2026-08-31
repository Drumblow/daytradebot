//! Configuração da estratégia Balance-Area Breakout v1.
//!
//! Todos os limiares vêm de `config/strategies/balance-area-breakout-v1.toml`
//! (regra de ouro: nenhuma regra hardcoded). Defaults seguem o documento de
//! especificação (`docs/strategies/balance-area-breakout-v1.md`).

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Configuração carregável de `config/strategies/balance-area-breakout-v1.toml`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BalanceAreaBreakoutV1Config {
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

    // --- área de balanceamento (seção 4 do doc) ---
    /// Janela da área (78 candles ≈ 3 dias de 15min).
    pub balance_lookback_candles: usize,
    /// Largura máxima da área em fração do preço médio (0.02 = 2%).
    pub balance_max_width_pct: Decimal,
    /// Largura máxima da área em múltiplos de ATR14 (10.0 — solta; ATR de
    /// 15min é escala errada para área de 3 dias, o filtro real é o % acima).
    pub balance_max_width_atr_mult: Decimal,

    // --- entrada / stop / alvo (seções 5–6 do doc) ---
    pub atr_period: usize,
    pub entry_order_type: String,
    pub entry_offset_ticks: Decimal,
    /// Candles de validade da entrada stop esperando o rompimento (ADR-009).
    pub entry_validity_candles: usize,
    /// Distância do stop para dentro da área, em ATRs (0.3).
    pub stop_buffer_atr_mult: Decimal,
    pub max_stop_atr_mult: Decimal,
    pub target_r_multiple: Decimal,
    pub min_risk_reward: Decimal,

    // --- risco ---
    /// Override de risco por trade, em pontos percentuais. `None` = global.
    pub risk_per_trade_pct: Option<Decimal>,
    pub max_spread_pct: Decimal,
    pub max_atr_pct: Decimal,

    // --- precisão e horário ---
    pub tick_size: Decimal,
    pub trading_start_time: String,
    pub trading_end_time: String,
}

impl BalanceAreaBreakoutV1Config {
    /// Calcula o hash SHA256 da configuração para auditoria.
    pub fn config_hash(&self) -> String {
        let json = serde_json::to_string(self).unwrap_or_default();
        let mut hasher = Sha256::new();
        hasher.update(json.as_bytes());
        format!("{:x}", hasher.finalize())[..16].to_string()
    }
}

impl Default for BalanceAreaBreakoutV1Config {
    fn default() -> Self {
        Self {
            strategy: StrategyWithParameters {
                id: "balance-area-breakout-v1".to_string(),
                version: "1.0.0".to_string(),
                source: "James Dalton - Mind over Markets, Cap. 4 (Balance-Area Break-outs)"
                    .to_string(),
                parameters: StrategyParameters {
                    operational_timeframe: "15m".to_string(),
                    balance_lookback_candles: 78,
                    balance_max_width_pct: Decimal::from(2) / Decimal::from(100), // 2%
                    balance_max_width_atr_mult: Decimal::from(10),
                    atr_period: 14,
                    entry_order_type: "stop".to_string(),
                    entry_offset_ticks: Decimal::ONE,
                    entry_validity_candles: 2,
                    stop_buffer_atr_mult: Decimal::from(30) / Decimal::from(100), // 0,3
                    max_stop_atr_mult: Decimal::from(3),
                    target_r_multiple: Decimal::from(2),
                    min_risk_reward: Decimal::from(15) / Decimal::from(10), // 1,5
                    risk_per_trade_pct: None,
                    max_spread_pct: Decimal::from(5) / Decimal::from(100),
                    max_atr_pct: Decimal::from(15) / Decimal::from(10),
                    tick_size: Decimal::from(1) / Decimal::from(100),
                    trading_start_time: "09:45:00".to_string(),
                    trading_end_time: "15:30:00".to_string(),
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
        let config = BalanceAreaBreakoutV1Config::default();
        assert_eq!(config.config_hash(), config.config_hash());
        assert_eq!(config.config_hash().len(), 16);

        let mut changed = config.clone();
        changed.strategy.parameters.balance_max_width_pct = Decimal::from(3) / Decimal::from(100);
        assert_ne!(config.config_hash(), changed.config_hash());
    }

    #[test]
    fn parses_project_toml() {
        let toml_str =
            include_str!("../../../../../config/strategies/balance-area-breakout-v1.toml");
        let config: BalanceAreaBreakoutV1Config =
            toml::from_str(toml_str).expect("TOML do projeto deve fazer parse");
        assert_eq!(config.strategy.id, "balance-area-breakout-v1");
        assert_eq!(config.strategy.parameters.balance_lookback_candles, 78);
    }
}
