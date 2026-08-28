//! Configuração da estratégia Trendline Break Test v1.
//!
//! Todos os limiares vêm de `config/strategies/trendline-break-test-v1.toml`
//! (regra de ouro: nenhuma regra hardcoded). Defaults seguem o documento de
//! especificação (`docs/strategies/trendline-break-test-v1.md`).

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Configuração carregável de `config/strategies/trendline-break-test-v1.toml`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrendlineBreakTestV1Config {
    pub strategy: StrategyWithParameters,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StrategyWithParameters {
    pub id: String,
    pub version: String,
    pub source: String,
    pub parameters: StrategyParameters,
    #[serde(default)]
    pub time_exit: TimeExitParams,
}

/// Saída por tempo: o trade precisa se validar (lucro flutuante `min_r`)
/// dentro de `candles` barras após o fill.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TimeExitParams {
    pub enabled: bool,
    pub min_r: Decimal,
    pub candles: u32,
}

impl Default for TimeExitParams {
    fn default() -> Self {
        Self {
            enabled: true,
            min_r: Decimal::from(5) / Decimal::from(10), // 0,5R
            candles: 12,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StrategyParameters {
    pub operational_timeframe: String,

    // --- contexto: tendência estabelecida (seção 4 do doc) ---
    /// Barras da janela de estrutura (12 ≈ 3 h de 15m = "algumas horas").
    pub trend_lookback: usize,
    /// Barras de cada lado que confirmam um pivô de swing (2).
    pub pivot_bars: usize,

    // --- rompimento da trendline (seção 5 do doc; Cap. 8) ---
    /// Barras mínimas da perna contrária (3).
    pub break_min_bars: usize,
    /// Fechamentos além da EMA20 exigidos na perna contrária (2).
    pub break_min_closes_beyond_ema: usize,
    /// Idade máxima do rompimento, em barras (20).
    pub break_max_age: usize,

    // --- teste do extremo (seção 6 do doc) ---
    /// Proximidade máxima do extremo antigo que caracteriza teste, em % (0,3).
    pub test_tolerance_pct: Decimal,
    /// Overshoot máximo além do extremo antigo, em ATR (0,5). Acima disso a
    /// reversão é anulada (Cap. 8).
    pub max_overshoot_atr: Decimal,

    // --- barra de sinal (seção 7 do doc; Cap. 1) ---
    pub signal_body_min_pct: Decimal,
    pub signal_wick_min_pct: Decimal,

    // --- contexto numérico ---
    pub atr_period: usize,

    // --- entrada / stop / alvo (seção 8 do doc) ---
    pub entry_order_type: String,
    pub entry_offset_ticks: Decimal,
    pub entry_validity_candles: usize,
    /// Alvo em múltiplos de R (2,0 — expectativa de duas pernas, Cap. 15).
    pub target_r_multiple: Decimal,
    pub min_risk_reward: Decimal,
    pub max_stop_atr: Decimal,
    /// Risco mínimo em ATR: abaixo disso o stop está no ruído. Lição direta da
    /// `value-area-reentry-v1` (§16 daquele doc).
    pub min_stop_atr: Decimal,

    // --- risco ---
    pub risk_per_trade_pct: Option<Decimal>,
    pub max_spread_pct: Decimal,
    pub max_atr_pct: Decimal,

    // --- precisão e horário ---
    pub tick_size: Decimal,
    pub trading_start_time: String,
    pub trading_end_time: String,
}

impl TrendlineBreakTestV1Config {
    /// Calcula o hash SHA256 da configuração para auditoria.
    pub fn config_hash(&self) -> String {
        let json = serde_json::to_string(self).unwrap_or_default();
        let mut hasher = Sha256::new();
        hasher.update(json.as_bytes());
        format!("{:x}", hasher.finalize())[..16].to_string()
    }
}

impl Default for TrendlineBreakTestV1Config {
    fn default() -> Self {
        Self {
            strategy: StrategyWithParameters {
                id: "trendline-break-test-v1".to_string(),
                version: "1.0.0".to_string(),
                source:
                    "Al Brooks - Reading Price Charts Bar by Bar, Cap. 15 (Major Reversals) + Cap. 8 (Trendline Break)"
                        .to_string(),
                parameters: StrategyParameters {
                    operational_timeframe: "15m".to_string(),
                    trend_lookback: 12,
                    pivot_bars: 2,
                    break_min_bars: 3,
                    break_min_closes_beyond_ema: 2,
                    break_max_age: 20,
                    test_tolerance_pct: Decimal::from(3) / Decimal::from(10), // 0,3%
                    max_overshoot_atr: Decimal::from(5) / Decimal::from(10),  // 0,5
                    signal_body_min_pct: Decimal::from(30) / Decimal::from(100),
                    signal_wick_min_pct: Decimal::from(334) / Decimal::from(1000),
                    atr_period: 14,
                    entry_order_type: "stop".to_string(),
                    entry_offset_ticks: Decimal::ONE,
                    entry_validity_candles: 2,
                    target_r_multiple: Decimal::from(2),
                    min_risk_reward: Decimal::from(15) / Decimal::from(10), // 1,5
                    max_stop_atr: Decimal::from(2),
                    min_stop_atr: Decimal::from(15) / Decimal::from(100), // 0,15
                    risk_per_trade_pct: None,
                    max_spread_pct: Decimal::from(5) / Decimal::from(100),
                    max_atr_pct: Decimal::from(15) / Decimal::from(10),
                    tick_size: Decimal::from(1) / Decimal::from(100),
                    trading_start_time: "14:00:00".to_string(), // 10:00 ET (DST)
                    trading_end_time: "19:15:00".to_string(),   // 15:15 ET (DST)
                },
                time_exit: TimeExitParams::default(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_hash_is_stable_and_sensitive() {
        let config = TrendlineBreakTestV1Config::default();
        assert_eq!(config.config_hash(), config.config_hash());
        assert_eq!(config.config_hash().len(), 16);

        let mut changed = config.clone();
        changed.strategy.parameters.max_overshoot_atr = Decimal::from(2);
        assert_ne!(config.config_hash(), changed.config_hash());
    }

    #[test]
    fn parses_project_toml() {
        let toml_str =
            include_str!("../../../../../config/strategies/trendline-break-test-v1.toml");
        let config: TrendlineBreakTestV1Config =
            toml::from_str(toml_str).expect("TOML do projeto deve fazer parse");
        assert_eq!(config.strategy.id, "trendline-break-test-v1");
        assert_eq!(config.strategy.parameters.trend_lookback, 12);
        assert!(config.strategy.time_exit.enabled);
    }
}
