//! Configuração da estratégia Opening Reversal v1.
//!
//! Todos os limiares vêm de `config/strategies/opening-reversal-v1.toml`
//! (regra de ouro: nenhuma regra hardcoded). Defaults seguem o documento de
//! especificação (`docs/strategies/opening-reversal-v1.md`).

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Configuração carregável de `config/strategies/opening-reversal-v1.toml`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OpeningReversalV1Config {
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

    // --- níveis e zona (seção 4 do doc) ---
    /// Banda da "zona de teste" do nível de ontem, em fração (0.003 = 0,3%).
    pub level_zone_pct: Decimal,

    // --- barra de sinal (seção 5.1 do doc) ---
    /// Corpo mínimo em fração do range da barra (0.30).
    pub signal_body_min_pct: Decimal,
    /// Sombra do lado da reversão, mínima em fração do range (1/3 ≈ 0.334).
    pub signal_wick_min_pct: Decimal,

    // --- vetos de momentum (seção 4 do doc) ---
    /// Corpo mínimo para considerar a barra uma "trend bar" (0.60).
    pub momentum_body_pct: Decimal,
    /// Barras fortes consecutivas além da zona que vetam o fade.
    pub momentum_bars: usize,
    /// Trend bars contra a direção pretendida, na janela, que vetam o fade.
    pub counter_trend_bars: usize,
    pub counter_window: usize,

    // --- entrada / stop / alvo (seção 6 do doc) ---
    pub atr_period: usize,
    /// Risco acima deste múltiplo de ATR dispara o stop monetário (60% da barra).
    pub stop_atr_mult: Decimal,
    /// Stop monetário: fração do range da barra de sinal (0.60 — Fig. 10.19).
    pub monetary_stop_range_pct: Decimal,
    pub entry_order_type: String,
    pub entry_offset_ticks: Decimal,
    pub entry_validity_candles: usize,
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

impl OpeningReversalV1Config {
    /// Calcula o hash SHA256 da configuração para auditoria.
    pub fn config_hash(&self) -> String {
        let json = serde_json::to_string(self).unwrap_or_default();
        let mut hasher = Sha256::new();
        hasher.update(json.as_bytes());
        format!("{:x}", hasher.finalize())[..16].to_string()
    }
}

impl Default for OpeningReversalV1Config {
    fn default() -> Self {
        Self {
            strategy: StrategyWithParameters {
                id: "opening-reversal-v1".to_string(),
                version: "1.0.0".to_string(),
                source: "Al Brooks - Reading Price Charts Bar by Bar, Cap. 11 (Opening Reversals)"
                    .to_string(),
                parameters: StrategyParameters {
                    operational_timeframe: "15m".to_string(),
                    level_zone_pct: Decimal::from(3) / Decimal::from(1000), // 0,3%
                    signal_body_min_pct: Decimal::from(30) / Decimal::from(100), // 0,30
                    signal_wick_min_pct: Decimal::from(334) / Decimal::from(1000), // ~1/3
                    momentum_body_pct: Decimal::from(60) / Decimal::from(100), // 0,60
                    momentum_bars: 2,
                    counter_trend_bars: 4,
                    counter_window: 6,
                    atr_period: 14,
                    stop_atr_mult: Decimal::from(15) / Decimal::from(10), // 1,5
                    monetary_stop_range_pct: Decimal::from(60) / Decimal::from(100), // 0,60
                    entry_order_type: "stop".to_string(),
                    entry_offset_ticks: Decimal::ONE,
                    entry_validity_candles: 2,
                    target_r_multiple: Decimal::from(2),
                    min_risk_reward: Decimal::from(15) / Decimal::from(10), // 1,5
                    risk_per_trade_pct: None,
                    max_spread_pct: Decimal::from(5) / Decimal::from(100),
                    max_atr_pct: Decimal::from(15) / Decimal::from(10),
                    tick_size: Decimal::from(1) / Decimal::from(100),
                    trading_start_time: "13:30:00".to_string(),
                    trading_end_time: "14:30:00".to_string(),
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
        let config = OpeningReversalV1Config::default();
        assert_eq!(config.config_hash(), config.config_hash());
        assert_eq!(config.config_hash().len(), 16);

        let mut changed = config.clone();
        changed.strategy.parameters.level_zone_pct = Decimal::from(5) / Decimal::from(1000);
        assert_ne!(config.config_hash(), changed.config_hash());
    }

    #[test]
    fn parses_project_toml() {
        let toml_str = include_str!("../../../../../config/strategies/opening-reversal-v1.toml");
        let config: OpeningReversalV1Config =
            toml::from_str(toml_str).expect("TOML do projeto deve fazer parse");
        assert_eq!(config.strategy.id, "opening-reversal-v1");
        assert_eq!(config.strategy.parameters.counter_trend_bars, 4);
    }
}
