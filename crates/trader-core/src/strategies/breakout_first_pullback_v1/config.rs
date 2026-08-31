//! Configuração da estratégia Breakout — Primeiro Pullback v1.
//!
//! Todos os limiares vêm de `config/strategies/breakout-first-pullback-v1.toml`
//! (regra de ouro: nenhuma regra hardcoded). Os defaults seguem a proposta
//! do documento de especificação (`docs/strategies/breakout-first-pullback-v1.md`).

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Configuração carregável de `config/strategies/breakout-first-pullback-v1.toml`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BreakoutFirstPullbackV1Config {
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

    // --- nível R (seção 4 do doc) ---
    pub level_lookback_candles: usize,
    pub level_min_touches: usize,
    /// Tolerância de toque do nível, em fração (0.001 = 0,10%).
    pub level_touch_tolerance_pct: Decimal,

    // --- breakout (seção 4 do doc) ---
    pub atr_period: usize,
    /// Quanto o fechamento deve superar o nível, em ATRs (0.25).
    pub breakout_close_atr_mult: Decimal,
    /// Expansão mínima de range da barra de breakout vs média (1.5).
    pub breakout_range_mult: Decimal,
    /// Expansão mínima de volume da barra de breakout vs média (1.5).
    pub breakout_volume_mult: Decimal,
    /// Período das médias de range e volume.
    pub avg_period: usize,

    // --- pullback (seção 5 do doc) ---
    pub pullback_min_candles: usize,
    pub pullback_max_candles: usize,
    /// Retração máxima do impulso pós-breakout (0.618).
    pub max_retrace_pct: Decimal,
    /// Janela da base para o pivô pré-breakout (maior mínima).
    pub pivot_lookback_candles: usize,

    // --- entrada / stop / alvo (seção 6 do doc) ---
    /// Tipo da ordem de entrada: "stop" (buy stop no gatilho — default).
    pub entry_order_type: String,
    pub entry_offset_ticks: Decimal,
    /// Candles de validade da entrada stop esperando o rompimento (ADR-009).
    pub entry_validity_candles: usize,
    /// Distância mínima do stop em ranges médios de barra (Cap. 8 do livro).
    pub min_stop_bar_ranges: Decimal,
    pub max_stop_atr_mult: Decimal,
    /// Teto do alvo em múltiplos de R (alvo = min(MMO, teto × risco)).
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

impl BreakoutFirstPullbackV1Config {
    /// Calcula o hash SHA256 da configuração para auditoria.
    pub fn config_hash(&self) -> String {
        let json = serde_json::to_string(self).unwrap_or_default();
        let mut hasher = Sha256::new();
        hasher.update(json.as_bytes());
        format!("{:x}", hasher.finalize())[..16].to_string()
    }
}

impl Default for BreakoutFirstPullbackV1Config {
    fn default() -> Self {
        Self {
            strategy: StrategyWithParameters {
                id: "breakout-first-pullback-v1".to_string(),
                version: "1.0.0".to_string(),
                source: "Adam Grimes - The Art and Science of Technical Analysis, Cap. 6 (Breakout, First Pullback)".to_string(),
                parameters: StrategyParameters {
                    operational_timeframe: "15m".to_string(),
                    level_lookback_candles: 80,
                    level_min_touches: 2,
                    level_touch_tolerance_pct: Decimal::from(1) / Decimal::from(1000), // 0,10%
                    atr_period: 14,
                    breakout_close_atr_mult: Decimal::from(25) / Decimal::from(100), // 0,25
                    // Calibrados na distribuição medida de breakouts 15min em
                    // ETFs (ver TOML): range p75=1,2; volume é confirmação
                    // secundária no livro.
                    breakout_range_mult: Decimal::from(12) / Decimal::from(10),      // 1,2
                    breakout_volume_mult: Decimal::from(11) / Decimal::from(10),     // 1,1
                    avg_period: 20,
                    pullback_min_candles: 2,
                    pullback_max_candles: 6,
                    max_retrace_pct: Decimal::from(618) / Decimal::from(1000), // 0,618
                    pivot_lookback_candles: 20,
                    entry_order_type: "stop".to_string(),
                    entry_offset_ticks: Decimal::ONE,
                    entry_validity_candles: 2,
                    min_stop_bar_ranges: Decimal::ONE,
                    max_stop_atr_mult: Decimal::from(4),
                    target_r_multiple: Decimal::from(2),
                    min_risk_reward: Decimal::from(15) / Decimal::from(10), // 1,5
                    risk_per_trade_pct: None,
                    max_spread_pct: Decimal::from(5) / Decimal::from(100), // 0,05 (percentual)
                    max_atr_pct: Decimal::from(15) / Decimal::from(10),    // 1.5%
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
        let config = BreakoutFirstPullbackV1Config::default();
        assert_eq!(config.config_hash(), config.config_hash());
        assert_eq!(config.config_hash().len(), 16);

        let mut changed = config.clone();
        changed.strategy.parameters.max_retrace_pct = Decimal::from(5) / Decimal::from(10);
        assert_ne!(config.config_hash(), changed.config_hash());
    }

    #[test]
    fn parses_project_toml() {
        let toml_str =
            include_str!("../../../../../config/strategies/breakout-first-pullback-v1.toml");
        let config: BreakoutFirstPullbackV1Config =
            toml::from_str(toml_str).expect("TOML do projeto deve fazer parse");
        assert_eq!(config.strategy.id, "breakout-first-pullback-v1");
        assert_eq!(config.strategy.parameters.level_lookback_candles, 80);
    }
}
