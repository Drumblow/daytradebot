//! Configuração da estratégia Range Extreme Fade v1.
//!
//! Todos os limiares vêm de `config/strategies/range-extreme-fade-v1.toml`
//! (regra de ouro: nenhuma regra hardcoded). Defaults seguem o documento de
//! especificação (`docs/strategies/range-extreme-fade-v1.md`), já com a
//! calibração do teste de frequência de 2026-08-17 (§12.1 do doc).

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Configuração carregável de `config/strategies/range-extreme-fade-v1.toml`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RangeExtremeFadeV1Config {
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

    // --- detector de dia de range (seção 4.1 do doc) ---
    /// Barras na janela de estrutura/inclinação (12).
    pub structure_lookback: usize,
    /// Inclinação máxima da EMA20 por barra, em fração (0.0005 = 0,05%).
    pub max_ema_slope_pct_per_bar: Decimal,
    /// Dias completos anteriores para o ATR diário médio (14).
    pub daily_atr_period: usize,
    /// Range do dia até o momento, máximo em múltiplos do ATR diário (1,5).
    pub max_day_range_atr_mult: Decimal,

    // --- extremo rompido (seção 4.2 do doc) ---
    /// Extensão máxima do rompimento do extremo do dia, em múltiplos de
    /// ATR14(15min) — 0.5 na calibração de frequência (0.3 era raro demais).
    pub max_extension_atr_mult: Decimal,

    // --- regra da EMA (seção 4.3 do doc; Cap. 5, contexto Barb Wire) ---
    /// Se a regra da EMA se aplica a todo fade (false = só vetos de Barb
    /// Wire; default da calibração de frequência — variante C do §12.1).
    pub use_ema_side_rule: bool,
    /// Closes considerados na regra da EMA (8).
    pub ema_side_lookback: usize,

    // --- veto meio do dia + meio do range (seção 4.4 do doc; literal Cap. 5) ---
    /// Início do veto (11:30 ET = 15:30 UTC no horário de verão).
    pub midday_start_time: String,
    /// Fim do veto (14:00 ET = 18:00 UTC no horário de verão).
    pub midday_end_time: String,

    // --- veto Barb Wire (seção 4.5 do doc; literal Cap. 5) ---
    /// Barras sobrepostas mínimas para caracterizar Barb Wire (3).
    pub barb_wire_bars: usize,
    /// Sobreposição mínima entre barras consecutivas, fração do range médio (0,5).
    pub barb_wire_overlap_pct: Decimal,
    /// Corpo máximo de um doji, fração do range da barra (0,3).
    pub barb_wire_doji_body_pct: Decimal,

    // --- barra de sinal (seção 5.1 do doc; Cap. 1/9) ---
    /// Corpo mínimo em fração do range da barra (0.30).
    pub signal_body_min_pct: Decimal,
    /// Sombra do lado do extremo testado, mínima em fração do range (≈1/3).
    pub signal_wick_min_pct: Decimal,

    // --- entrada / stop / alvo (seção 6 do doc) ---
    pub atr_period: usize,
    pub entry_order_type: String,
    pub entry_offset_ticks: Decimal,
    pub entry_validity_candles: usize,
    /// Alvo fixo em R (1,5 — adaptação do scalp ao bracket de TP único).
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

impl RangeExtremeFadeV1Config {
    /// Calcula o hash SHA256 da configuração para auditoria.
    pub fn config_hash(&self) -> String {
        let json = serde_json::to_string(self).unwrap_or_default();
        let mut hasher = Sha256::new();
        hasher.update(json.as_bytes());
        format!("{:x}", hasher.finalize())[..16].to_string()
    }
}

impl Default for RangeExtremeFadeV1Config {
    fn default() -> Self {
        Self {
            strategy: StrategyWithParameters {
                id: "range-extreme-fade-v1".to_string(),
                version: "1.0.0".to_string(),
                source:
                    "Al Brooks - Reading Price Charts Bar by Bar, Cap. 9 (Failed HH/LL Breakouts) + Cap. 5 (Barb Wire)"
                        .to_string(),
                parameters: StrategyParameters {
                    operational_timeframe: "15m".to_string(),
                    structure_lookback: 12,
                    max_ema_slope_pct_per_bar: Decimal::from(5) / Decimal::from(10000), // 0,05%
                    daily_atr_period: 14,
                    max_day_range_atr_mult: Decimal::from(15) / Decimal::from(10), // 1,5
                    max_extension_atr_mult: Decimal::from(5) / Decimal::from(10), // 0,5 (calibrado)
                    use_ema_side_rule: false, // calibrado: regra vale para Barb Wire (Cap. 5)
                    ema_side_lookback: 8,
                    midday_start_time: "15:30:00".to_string(), // 11:30 ET (DST)
                    midday_end_time: "18:00:00".to_string(),   // 14:00 ET (DST)
                    barb_wire_bars: 3,
                    barb_wire_overlap_pct: Decimal::from(5) / Decimal::from(10), // 0,5
                    barb_wire_doji_body_pct: Decimal::from(3) / Decimal::from(10), // 0,3
                    signal_body_min_pct: Decimal::from(30) / Decimal::from(100), // 0,30
                    signal_wick_min_pct: Decimal::from(334) / Decimal::from(1000), // ~1/3
                    atr_period: 14,
                    entry_order_type: "stop".to_string(),
                    entry_offset_ticks: Decimal::ONE,
                    entry_validity_candles: 2,
                    target_r_multiple: Decimal::from(15) / Decimal::from(10), // 1,5
                    min_risk_reward: Decimal::from(12) / Decimal::from(10), // 1,2
                    risk_per_trade_pct: None,
                    max_spread_pct: Decimal::from(5) / Decimal::from(100),
                    max_atr_pct: Decimal::from(15) / Decimal::from(10),
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
        let config = RangeExtremeFadeV1Config::default();
        assert_eq!(config.config_hash(), config.config_hash());
        assert_eq!(config.config_hash().len(), 16);

        let mut changed = config.clone();
        changed.strategy.parameters.max_extension_atr_mult = Decimal::from(3) / Decimal::from(10);
        assert_ne!(config.config_hash(), changed.config_hash());
    }

    #[test]
    fn parses_project_toml() {
        let toml_str = include_str!("../../../../../config/strategies/range-extreme-fade-v1.toml");
        let config: RangeExtremeFadeV1Config =
            toml::from_str(toml_str).expect("TOML do projeto deve fazer parse");
        assert_eq!(config.strategy.id, "range-extreme-fade-v1");
        assert_eq!(config.strategy.parameters.structure_lookback, 12);
    }
}
