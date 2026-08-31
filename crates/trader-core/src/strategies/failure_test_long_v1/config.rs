//! Configuração da estratégia Failure Test Long v1.
//!
//! Todos os limiares vêm de `config/strategies/failure-test-long-v1.toml`
//! (regra de ouro: nenhuma regra hardcoded). Os defaults seguem a proposta
//! do documento de especificação (`docs/strategies/failure-test-long-v1.md`,
//! seção 14).

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Configuração carregável de `config/strategies/failure-test-long-v1.toml`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FailureTestLongV1Config {
    pub strategy: StrategyWithParameters,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StrategyWithParameters {
    pub id: String,
    pub version: String,
    pub source: String,
    pub parameters: StrategyParameters,
    /// Saída ativa por tempo (validação pós-entrada em R) — seção 6 do doc.
    #[serde(default)]
    pub time_exit: TimeExitParameters,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StrategyParameters {
    pub operational_timeframe: String,

    // --- nível de suporte (seção 5.1 do doc) ---
    pub level_lookback_candles: usize,
    pub level_min_touches: usize,
    /// Tolerância de toque do nível, em fração (0.001 = 0,10%).
    pub level_touch_tolerance_pct: Decimal,
    pub level_min_age_candles: usize,

    // --- contexto de reversão (seção 4 do doc) ---
    pub keltner_ema_period: usize,
    pub keltner_atr_mult: Decimal,
    pub atr_period: usize,
    pub climax_lookback_candles: usize,
    pub overextension_atr_mult: Decimal,
    pub macd_fast_sma: usize,
    pub macd_slow_sma: usize,
    pub macd_signal_sma: usize,
    pub macd_lookback_candles: usize,
    pub climax_bar_atr_mult: Decimal,

    // --- sonda e recuperação (seções 5.2–5.3 do doc) ---
    pub probe_max_bars: usize,
    pub probe_max_atr_mult: Decimal,
    /// Posição mínima do fechamento no range da barra de recuperação (0.5 = metade superior).
    pub signal_close_min_position: Decimal,

    // --- entrada / stop / alvo (seções 6–7 do doc) ---
    /// Tipo da ordem de entrada: "stop" (buy stop no gatilho — default) ou
    /// "market_next_open" (executa na primeira oportunidade, mais fiel ao livro).
    pub entry_order_type: String,
    pub entry_offset_ticks: Decimal,
    /// Candles de validade da entrada stop esperando o rompimento (ADR-009).
    pub entry_validity_candles: usize,
    pub stop_jitter_atr_mult: Decimal,
    /// Distância mínima do stop em ranges médios de barra (Cap. 8 do livro).
    pub min_stop_bar_ranges: Decimal,
    pub max_stop_atr_mult: Decimal,
    pub target_r_multiple: Decimal,
    pub min_risk_reward: Decimal,

    // --- reentrada (seção 8 do doc; v1 só registra) ---
    pub allow_reentry: bool,
    pub reentry_window_candles: usize,

    // --- risco (seção 8 do doc) ---
    /// Override de risco por trade, em pontos percentuais (0.5 = 0,5%).
    /// `None` = usa o `[risk]` global do `default.toml`.
    pub risk_per_trade_pct: Option<Decimal>,
    pub max_spread_pct: Decimal,
    pub max_atr_pct: Decimal,

    // --- precisão e horário ---
    pub tick_size: Decimal,
    pub trading_start_time: String,
    pub trading_end_time: String,
}

/// Parâmetros da saída por tempo (validação em `candles` barras com lucro
/// mínimo de `min_r`). Default ligado para esta estratégia — regra literal do
/// livro ("immediately profitable within one to three bars", Cap. 6).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TimeExitParameters {
    pub enabled: bool,
    pub min_r: Decimal,
    pub candles: u32,
}

impl Default for TimeExitParameters {
    fn default() -> Self {
        Self {
            enabled: true,
            min_r: Decimal::from(5) / Decimal::from(10),
            candles: 3,
        }
    }
}

impl FailureTestLongV1Config {
    /// Calcula o hash SHA256 da configuração para auditoria.
    pub fn config_hash(&self) -> String {
        let json = serde_json::to_string(self).unwrap_or_default();
        let mut hasher = Sha256::new();
        hasher.update(json.as_bytes());
        format!("{:x}", hasher.finalize())[..16].to_string()
    }
}

impl Default for FailureTestLongV1Config {
    fn default() -> Self {
        Self {
            strategy: StrategyWithParameters {
                id: "failure-test-long-v1".to_string(),
                version: "1.0.0".to_string(),
                source:
                    "Adam Grimes - The Art and Science of Technical Analysis, Cap. 6 (Failure Test)"
                        .to_string(),
                parameters: StrategyParameters {
                    operational_timeframe: "15m".to_string(),
                    level_lookback_candles: 60,
                    level_min_touches: 2,
                    level_touch_tolerance_pct: Decimal::from(1) / Decimal::from(1000), // 0,10%
                    level_min_age_candles: 8,
                    keltner_ema_period: 20,
                    keltner_atr_mult: Decimal::from(225) / Decimal::from(100), // 2.25
                    atr_period: 14,
                    climax_lookback_candles: 10,
                    overextension_atr_mult: Decimal::from(2),
                    macd_fast_sma: 3,
                    macd_slow_sma: 10,
                    macd_signal_sma: 16,
                    macd_lookback_candles: 40,
                    climax_bar_atr_mult: Decimal::from(25) / Decimal::from(10), // 2.5
                    probe_max_bars: 2,
                    probe_max_atr_mult: Decimal::ONE,
                    signal_close_min_position: Decimal::from(50) / Decimal::from(100), // 0.50
                    entry_order_type: "stop".to_string(),
                    entry_offset_ticks: Decimal::ONE,
                    entry_validity_candles: 2,
                    stop_jitter_atr_mult: Decimal::from(10) / Decimal::from(100), // 0.10
                    min_stop_bar_ranges: Decimal::ONE,
                    max_stop_atr_mult: Decimal::from(3),
                    target_r_multiple: Decimal::from(15) / Decimal::from(10), // 1.5
                    min_risk_reward: Decimal::from(12) / Decimal::from(10),   // 1.2
                    allow_reentry: false,
                    reentry_window_candles: 3,
                    risk_per_trade_pct: Some(Decimal::from(5) / Decimal::from(10)), // 0,5%
                    max_spread_pct: Decimal::from(5) / Decimal::from(100), // 0.05 (percentual)
                    max_atr_pct: Decimal::from(15) / Decimal::from(10),    // 1.5%
                    tick_size: Decimal::from(1) / Decimal::from(100),
                    trading_start_time: "09:45:00".to_string(),
                    trading_end_time: "15:30:00".to_string(),
                },
                time_exit: TimeExitParameters::default(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_hash_is_stable_and_sensitive() {
        let config = FailureTestLongV1Config::default();
        assert_eq!(config.config_hash(), config.config_hash());
        assert_eq!(config.config_hash().len(), 16);

        let mut changed = config.clone();
        changed.strategy.parameters.target_r_multiple = Decimal::from(2);
        assert_ne!(config.config_hash(), changed.config_hash());
    }

    #[test]
    fn parses_project_toml() {
        let toml_str = include_str!("../../../../../config/strategies/failure-test-long-v1.toml");
        let config: FailureTestLongV1Config =
            toml::from_str(toml_str).expect("TOML do projeto deve fazer parse");
        assert_eq!(config.strategy.id, "failure-test-long-v1");
        assert_eq!(config.strategy.parameters.level_lookback_candles, 60);
        assert!(config.strategy.time_exit.enabled);
    }
}
