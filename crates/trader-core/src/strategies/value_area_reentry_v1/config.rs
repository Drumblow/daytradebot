//! Configuração da estratégia Value Area Reentry v1.
//!
//! Todos os limiares vêm de `config/strategies/value-area-reentry-v1.toml`
//! (regra de ouro: nenhuma regra hardcoded). Defaults seguem o documento de
//! especificação (`docs/strategies/value-area-reentry-v1.md`).

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Configuração carregável de `config/strategies/value-area-reentry-v1.toml`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ValueAreaReentryV1Config {
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

    // --- value area por proxy TPO (seção 4 do doc; Apêndice 1 do livro) ---
    /// Faixas de preço em que o range do dia anterior é dividido (50).
    pub va_buckets: usize,
    /// Percentual dos TPOs que define a área de valor (70).
    pub va_percent: Decimal,

    // --- filtros obrigatórios do autor (seção 5.3 do doc; Cap. 4) ---
    /// Distância máxima da abertura à borda da VA, em ATR diário (1,0).
    pub max_open_distance_atr: Decimal,
    /// Largura máxima da VA, em ATR diário (0,8) — "narrow value areas are
    /// more easily traversed".
    pub max_va_width_atr: Decimal,
    /// Barras da janela de inclinação da EMA20 para o filtro de direção (12).
    pub trend_lookback: usize,
    /// Inclinação mínima por barra para considerar que existe tendência
    /// contrária, em fração do preço (0,0005 = 0,05%). Abaixo disso o mercado
    /// é considerado sem direção e a travessia é permitida nos dois sentidos.
    pub trend_slope_threshold: Decimal,

    // --- aceitação dentro do valor (seção 5.2 do doc) ---
    /// Fechamentos consecutivos dentro da VA que caracterizam aceitação (2 =
    /// "double TPO prints").
    pub acceptance_bars: usize,

    // --- contexto ---
    /// Dias completos anteriores para o ATR diário médio (14).
    pub daily_atr_period: usize,
    pub atr_period: usize,

    // --- entrada / stop / alvo (seção 6 do doc) ---
    pub entry_order_type: String,
    pub entry_offset_ticks: Decimal,
    pub entry_validity_candles: usize,
    pub min_risk_reward: Decimal,
    /// Stop máximo em múltiplos de ATR14(15min) — acima disso `StopTooWide`.
    pub max_stop_atr: Decimal,
    /// Folga do stop além da borda da VA, em múltiplos de ATR14(15min).
    /// O livro NÃO especifica o stop (seção 3, pergunta 5) — a borda exata
    /// mostrou-se dentro do ruído na calibração de 2026-08-28.
    pub stop_buffer_atr: Decimal,
    /// Risco mínimo em múltiplos de ATR14(15min): abaixo disso o stop está no
    /// ruído e o RR fica artificialmente alto (`StopWithinNoise`).
    pub min_stop_atr: Decimal,

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

impl ValueAreaReentryV1Config {
    /// Calcula o hash SHA256 da configuração para auditoria.
    pub fn config_hash(&self) -> String {
        let json = serde_json::to_string(self).unwrap_or_default();
        let mut hasher = Sha256::new();
        hasher.update(json.as_bytes());
        format!("{:x}", hasher.finalize())[..16].to_string()
    }
}

impl Default for ValueAreaReentryV1Config {
    fn default() -> Self {
        Self {
            strategy: StrategyWithParameters {
                id: "value-area-reentry-v1".to_string(),
                version: "1.0.0".to_string(),
                source:
                    "James Dalton - Mind over Markets, Cap. 4 (The Value-Area Rule) + Apêndice 1 (TPO Value-Area Calculation)"
                        .to_string(),
                parameters: StrategyParameters {
                    operational_timeframe: "15m".to_string(),
                    va_buckets: 50,
                    va_percent: Decimal::from(70),
                    max_open_distance_atr: Decimal::ONE,
                    max_va_width_atr: Decimal::from(8) / Decimal::from(10), // 0,8
                    trend_lookback: 12,
                    trend_slope_threshold: Decimal::from(5) / Decimal::from(10000), // 0,05%
                    acceptance_bars: 2,
                    daily_atr_period: 14,
                    atr_period: 14,
                    entry_order_type: "stop".to_string(),
                    entry_offset_ticks: Decimal::ONE,
                    entry_validity_candles: 2,
                    min_risk_reward: Decimal::from(12) / Decimal::from(10), // 1,2
                    max_stop_atr: Decimal::from(2),
                    stop_buffer_atr: Decimal::from(25) / Decimal::from(100), // 0,25
                    min_stop_atr: Decimal::from(15) / Decimal::from(100),    // 0,15
                    risk_per_trade_pct: None,
                    max_spread_pct: Decimal::from(5) / Decimal::from(100),
                    max_atr_pct: Decimal::from(15) / Decimal::from(10),
                    tick_size: Decimal::from(1) / Decimal::from(100),
                    trading_start_time: "10:00:00".to_string(), // ET
                    trading_end_time: "15:00:00".to_string(),   // ET
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
        let config = ValueAreaReentryV1Config::default();
        assert_eq!(config.config_hash(), config.config_hash());
        assert_eq!(config.config_hash().len(), 16);

        let mut changed = config.clone();
        changed.strategy.parameters.max_va_width_atr = Decimal::from(2);
        assert_ne!(config.config_hash(), changed.config_hash());
    }

    #[test]
    fn parses_project_toml() {
        let toml_str = include_str!("../../../../../config/strategies/value-area-reentry-v1.toml");
        let config: ValueAreaReentryV1Config =
            toml::from_str(toml_str).expect("TOML do projeto deve fazer parse");
        assert_eq!(config.strategy.id, "value-area-reentry-v1");
        assert_eq!(config.strategy.parameters.acceptance_bars, 2);
    }
}
