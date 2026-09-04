//! Construção de `RiskConfig` compartilhada entre paper trading e backtest.
//!
//! Garante paridade: o backtest valida sinais com exatamente os mesmos
//! limites que o live/paper usaria — sem isso os resultados não são
//! comparáveis.

use rust_decimal::Decimal;

use trader_core::risk::RiskConfig;
use trader_core::strategies::balance_area_breakout_v1::config::StrategyParameters as BalanceAreaParams;
use trader_core::strategies::breakout_first_pullback_v1::config::StrategyParameters as BreakoutParams;
use trader_core::strategies::failure_test_long_v1::config::StrategyParameters as FailureTestParams;
use trader_core::strategies::low2_m2s_short_v1::config::StrategyParameters as Low2ShortParams;
use trader_core::strategies::opening_reversal_v1::config::StrategyParameters as OpeningReversalParams;
use trader_core::strategies::pullback_trend_v1::config::StrategyParameters as PullbackParams;
use trader_core::strategies::range_extreme_fade_v1::config::StrategyParameters as RangeFadeParams;
use trader_core::strategies::trendline_break_test_v1::config::StrategyParameters as TrendlineBreakParams;
use trader_core::strategies::value_area_reentry_v1::config::StrategyParameters as ValueAreaReentryParams;
use trader_domain::TradingMode;
use trader_infra::config::RiskSettings;

/// Parâmetros da estratégia relevantes para a validação de risco.
///
/// Ponto de integração único: cada estratégia converte seus parâmetros para
/// esta struct, e o `RiskConfig` sai daqui — mesmo para live, paper e
/// backtest.
pub struct StrategyRiskParams {
    pub min_risk_reward: Decimal,
    pub max_spread_pct: Decimal,
    pub max_atr_pct: Decimal,
    pub trading_start_time: String,
    pub trading_end_time: String,
    /// Override de risco por trade da estratégia (pontos percentuais).
    /// `None` = usa o `[risk].risk_per_trade_pct` global do `default.toml`.
    pub risk_per_trade_pct: Option<Decimal>,
}

impl From<&PullbackParams> for StrategyRiskParams {
    fn from(p: &PullbackParams) -> Self {
        Self {
            min_risk_reward: p.min_risk_reward,
            max_spread_pct: p.max_spread_pct,
            max_atr_pct: p.max_atr_pct,
            trading_start_time: p.trading_start_time.clone(),
            trading_end_time: p.trading_end_time.clone(),
            risk_per_trade_pct: None,
        }
    }
}

impl From<&FailureTestParams> for StrategyRiskParams {
    fn from(p: &FailureTestParams) -> Self {
        Self {
            min_risk_reward: p.min_risk_reward,
            max_spread_pct: p.max_spread_pct,
            max_atr_pct: p.max_atr_pct,
            trading_start_time: p.trading_start_time.clone(),
            trading_end_time: p.trading_end_time.clone(),
            risk_per_trade_pct: p.risk_per_trade_pct,
        }
    }
}

impl From<&BalanceAreaParams> for StrategyRiskParams {
    fn from(p: &BalanceAreaParams) -> Self {
        Self {
            min_risk_reward: p.min_risk_reward,
            max_spread_pct: p.max_spread_pct,
            max_atr_pct: p.max_atr_pct,
            trading_start_time: p.trading_start_time.clone(),
            trading_end_time: p.trading_end_time.clone(),
            risk_per_trade_pct: p.risk_per_trade_pct,
        }
    }
}

impl From<&OpeningReversalParams> for StrategyRiskParams {
    fn from(p: &OpeningReversalParams) -> Self {
        Self {
            min_risk_reward: p.min_risk_reward,
            max_spread_pct: p.max_spread_pct,
            max_atr_pct: p.max_atr_pct,
            trading_start_time: p.trading_start_time.clone(),
            trading_end_time: p.trading_end_time.clone(),
            risk_per_trade_pct: p.risk_per_trade_pct,
        }
    }
}

impl From<&Low2ShortParams> for StrategyRiskParams {
    fn from(p: &Low2ShortParams) -> Self {
        Self {
            min_risk_reward: p.min_risk_reward,
            max_spread_pct: p.max_spread_pct,
            max_atr_pct: p.max_atr_pct,
            trading_start_time: p.trading_start_time.clone(),
            trading_end_time: p.trading_end_time.clone(),
            risk_per_trade_pct: p.risk_per_trade_pct,
        }
    }
}

impl From<&RangeFadeParams> for StrategyRiskParams {
    fn from(p: &RangeFadeParams) -> Self {
        Self {
            min_risk_reward: p.min_risk_reward,
            max_spread_pct: p.max_spread_pct,
            max_atr_pct: p.max_atr_pct,
            trading_start_time: p.trading_start_time.clone(),
            trading_end_time: p.trading_end_time.clone(),
            risk_per_trade_pct: p.risk_per_trade_pct,
        }
    }
}

impl From<&TrendlineBreakParams> for StrategyRiskParams {
    fn from(p: &TrendlineBreakParams) -> Self {
        Self {
            min_risk_reward: p.min_risk_reward,
            max_spread_pct: p.max_spread_pct,
            max_atr_pct: p.max_atr_pct,
            trading_start_time: p.trading_start_time.clone(),
            trading_end_time: p.trading_end_time.clone(),
            risk_per_trade_pct: p.risk_per_trade_pct,
        }
    }
}

impl From<&ValueAreaReentryParams> for StrategyRiskParams {
    fn from(p: &ValueAreaReentryParams) -> Self {
        Self {
            min_risk_reward: p.min_risk_reward,
            max_spread_pct: p.max_spread_pct,
            max_atr_pct: p.max_atr_pct,
            trading_start_time: p.trading_start_time.clone(),
            trading_end_time: p.trading_end_time.clone(),
            risk_per_trade_pct: p.risk_per_trade_pct,
        }
    }
}

impl From<&BreakoutParams> for StrategyRiskParams {
    fn from(p: &BreakoutParams) -> Self {
        Self {
            min_risk_reward: p.min_risk_reward,
            max_spread_pct: p.max_spread_pct,
            max_atr_pct: p.max_atr_pct,
            trading_start_time: p.trading_start_time.clone(),
            trading_end_time: p.trading_end_time.clone(),
            risk_per_trade_pct: p.risk_per_trade_pct,
        }
    }
}

/// Monta o `RiskConfig` a partir da configuração da aplicação (`[risk]`) e
/// dos parâmetros da estratégia (RR, spread, ATR e horário vêm dela; o risco
/// por trade pode ser sobrescrito por ela — ex.: 0,5% do failure test).
pub fn build_risk_config(risk: &RiskSettings, params: &StrategyRiskParams) -> RiskConfig {
    RiskConfig {
        trading_mode: TradingMode::Paper,
        risk_per_trade_pct: params
            .risk_per_trade_pct
            .or_else(|| Decimal::from_f64_retain(risk.risk_per_trade_pct))
            .unwrap_or(Decimal::ONE),
        max_daily_loss_pct: Decimal::from_f64_retain(risk.max_daily_loss_pct)
            .unwrap_or(Decimal::from(2)),
        max_trades_per_day: risk.max_trades_per_day,
        max_consecutive_losses: risk.max_consecutive_losses,
        min_risk_reward: params.min_risk_reward,
        max_spread_pct: params.max_spread_pct,
        max_atr_pct: params.max_atr_pct,
        // Horário de NOVA YORK (A2): os TOMLs declaram a janela em ET.
        trading_start_time_et: parse_time(&params.trading_start_time).unwrap_or((9, 30, 0)),
        trading_end_time_et: parse_time(&params.trading_end_time).unwrap_or((16, 0, 0)),
        entry_overshoot_tolerance: Decimal::from_f64_retain(risk.entry_overshoot_tolerance)
            .unwrap_or_else(|| Decimal::from(25) / Decimal::from(100)),
    }
}

/// Faz parse de "HH:MM:SS" para uma tupla (h, m, s).
pub fn parse_time(time_str: &str) -> Option<(u32, u32, u32)> {
    let parts: Vec<&str> = time_str.split(':').collect();
    if parts.len() != 3 {
        return None;
    }
    Some((
        parts[0].parse().ok()?,
        parts[1].parse().ok()?,
        parts[2].parse().ok()?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use trader_core::strategies::failure_test_long_v1::config::FailureTestLongV1Config;
    use trader_core::strategies::pullback_trend_v1::config::PullbackTrendV1Config;

    fn risk_settings() -> RiskSettings {
        RiskSettings {
            profile: "conservative".to_string(),
            risk_per_trade_pct: 1.0,
            max_daily_loss_pct: 2.0,
            max_trades_per_day: 3,
            max_consecutive_losses: 3,
            max_portfolio_daily_loss_pct: 4.0,
            max_concurrent_positions: 3,
            max_portfolio_notional_pct: 200.0,
            entry_overshoot_tolerance: 0.25,
        }
    }

    #[test]
    fn parses_hh_mm_ss() {
        assert_eq!(parse_time("09:30:00"), Some((9, 30, 0)));
        assert_eq!(parse_time("16:00:00"), Some((16, 0, 0)));
        assert_eq!(parse_time("invalid"), None);
    }

    #[test]
    fn pullback_uses_global_risk_per_trade() {
        let params = PullbackTrendV1Config::default().strategy.parameters;
        let config = build_risk_config(&risk_settings(), &StrategyRiskParams::from(&params));
        assert_eq!(config.risk_per_trade_pct, Decimal::ONE);
    }

    #[test]
    fn failure_test_overrides_risk_per_trade_with_half_percent() {
        let params = FailureTestLongV1Config::default().strategy.parameters;
        let config = build_risk_config(&risk_settings(), &StrategyRiskParams::from(&params));
        assert_eq!(
            config.risk_per_trade_pct,
            Decimal::from(5) / Decimal::from(10)
        );
        // Demais limites continuam vindo do [risk] global.
        assert_eq!(config.max_daily_loss_pct, Decimal::from(2));
        assert_eq!(config.max_trades_per_day, 3);
    }
}
