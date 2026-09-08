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
/// Monta a `RiskConfig` a partir da config da aplicação e da estratégia.
///
/// FALHA FECHADO. Antes, valor inválido virava o padrão em silêncio: um `NaN`
/// no percentual de risco ou um typo no horário (`"9:30"`, `"09:3O"`) fazia o
/// bot subir com a janela de negociação errada e ninguém ficava sabendo. Um
/// limite de risco que o operador acha que configurou, mas que não está
/// valendo, é pior do que não ter limite nenhum.
pub fn build_risk_config(
    risk: &RiskSettings,
    params: &StrategyRiskParams,
) -> anyhow::Result<RiskConfig> {
    let pct = |nome: &str, v: f64| -> anyhow::Result<Decimal> {
        Decimal::from_f64_retain(v).ok_or_else(|| anyhow::anyhow!("[risk].{nome} inválido: {v}"))
    };

    let risk_per_trade_pct = match params.risk_per_trade_pct {
        Some(v) => v,
        None => pct("risk_per_trade_pct", risk.risk_per_trade_pct)?,
    };

    let hora = |nome: &str, v: &str| -> anyhow::Result<(u32, u32, u32)> {
        parse_time(v).ok_or_else(|| {
            anyhow::anyhow!("{nome} inválido: {v:?} (esperado \"HH:MM:SS\" em horário de NY)")
        })
    };

    Ok(RiskConfig {
        trading_mode: TradingMode::Paper,
        risk_per_trade_pct,
        max_daily_loss_pct: pct("max_daily_loss_pct", risk.max_daily_loss_pct)?,
        max_trades_per_day: risk.max_trades_per_day,
        max_consecutive_losses: risk.max_consecutive_losses,
        min_risk_reward: params.min_risk_reward,
        max_spread_pct: params.max_spread_pct,
        max_atr_pct: params.max_atr_pct,
        // Horário de NOVA YORK (A2): os TOMLs declaram a janela em ET.
        trading_start_time_et: hora("trading_start_time", &params.trading_start_time)?,
        trading_end_time_et: hora("trading_end_time", &params.trading_end_time)?,
        entry_overshoot_tolerance: pct(
            "entry_overshoot_tolerance",
            risk.entry_overshoot_tolerance,
        )?,

        // --- ADR-020 ---
        //
        // Falha fechado, como o resto desta função: valor fora de faixa
        // ABORTA a subida. Um teto de tamanho que o operador acha que
        // configurou, mas que virou o default em silêncio, é o mesmo defeito
        // que o `risk_per_trade_pct` já tinha.
        max_notional_multiple: {
            let v = pct("max_notional_multiple", risk.max_notional_multiple)?;
            if v < Decimal::ONE {
                anyhow::bail!(
                    "[risk].max_notional_multiple = {v}: abaixo de 1 seria um teto MENOR \
                     que a fatia de capital, que é o que `capital_fraction` já faz — use \
                     a fração, ou um `max_notional_usd`."
                );
            }
            v
        },
        max_notional_usd: match risk.max_notional_usd {
            None => None,
            Some(v) => {
                let d = pct("max_notional_usd", v)?;
                if d <= Decimal::ZERO {
                    anyhow::bail!("[risk].max_notional_usd = {d}: teto tem de ser positivo");
                }
                Some(d)
            }
        },
        capital_fraction: {
            let v = pct("capital_fraction", risk.capital_fraction)?;
            if v <= Decimal::ZERO || v > Decimal::ONE {
                anyhow::bail!(
                    "[risk].capital_fraction = {v}: fora de (0, 1]. Fração é a fatia da \
                     conta que esta instância pode ocupar; acima de 1 seria alavancagem \
                     por outro nome (use max_notional_multiple)."
                );
            }
            v
        },
        max_pct_of_median_bar_notional: match risk.max_pct_of_median_bar_notional {
            None => None,
            Some(v) => {
                let d = pct("max_pct_of_median_bar_notional", v)?;
                if d <= Decimal::ZERO || d > Decimal::from(100) {
                    anyhow::bail!("[risk].max_pct_of_median_bar_notional = {d}: fora de (0, 100]");
                }
                Some(d)
            }
        },
        liquidity_lookback_bars: {
            if risk.liquidity_lookback_bars == 0 {
                anyhow::bail!(
                    "[risk].liquidity_lookback_bars = 0: sem janela não há mediana, e com \
                     o cap de liquidez ligado isso recusaria todo sinal."
                );
            }
            risk.liquidity_lookback_bars
        },
    })
}

/// Avisa quando a fração de capital e o teto de notional da conta não fecham.
///
/// Não é erro: `capital_fraction × max_concurrent_positions` maior que o teto
/// agregado significa que as N posições cheias NÃO cabem — a trava do ADR-017
/// vai recusar a última, que é justamente o cluster que o ADR-020 quer
/// preservar. É config coerente, só não faz o que quem a escreveu queria.
pub fn aviso_de_fracao(risk: &RiskSettings) -> Option<String> {
    let ocupacao = risk.capital_fraction * risk.max_concurrent_positions as f64 * 100.0;
    if ocupacao > risk.max_portfolio_notional_pct {
        return Some(format!(
            "capital_fraction {:.4} × {} posições = {:.1}% de notional, acima do teto da \
             conta ({:.1}%): a última posição do cluster será recusada pelo ADR-017",
            risk.capital_fraction,
            risk.max_concurrent_positions,
            ocupacao,
            risk.max_portfolio_notional_pct
        ));
    }
    None
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
            max_notional_multiple: 1.0,
            max_notional_usd: None,
            capital_fraction: 1.0,
            max_pct_of_median_bar_notional: None,
            liquidity_lookback_bars: 600,
            max_portfolio_notional_pct: 200.0,
            entry_overshoot_tolerance: 0.25,
        }
    }

    /// Config inválida tem que DERRUBAR o boot, não virar o padrão. Um limite
    /// que o operador acha que configurou, mas que não está valendo, é pior do
    /// que não ter limite.
    #[test]
    fn config_invalida_falha_fechado() {
        let params = PullbackTrendV1Config::default().strategy.parameters;

        let mut horario_errado = StrategyRiskParams::from(&params);
        horario_errado.trading_start_time = "9:3O:00".to_string(); // letra O no lugar do zero
        let erro = build_risk_config(&risk_settings(), &horario_errado)
            .expect_err("horário inválido deveria falhar");
        assert!(
            erro.to_string().contains("trading_start_time"),
            "erro deveria nomear o campo: {erro}"
        );

        let mut risco_nan = risk_settings();
        risco_nan.max_daily_loss_pct = f64::NAN;
        let erro = build_risk_config(&risco_nan, &StrategyRiskParams::from(&params))
            .expect_err("NaN deveria falhar");
        assert!(
            erro.to_string().contains("max_daily_loss_pct"),
            "erro deveria nomear o campo: {erro}"
        );
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
        let config = build_risk_config(&risk_settings(), &StrategyRiskParams::from(&params))
            .expect("config válida");
        assert_eq!(config.risk_per_trade_pct, Decimal::ONE);
    }

    #[test]
    fn failure_test_overrides_risk_per_trade_with_half_percent() {
        let params = FailureTestLongV1Config::default().strategy.parameters;
        let config = build_risk_config(&risk_settings(), &StrategyRiskParams::from(&params))
            .expect("config válida");
        assert_eq!(
            config.risk_per_trade_pct,
            Decimal::from(5) / Decimal::from(10)
        );
        // Demais limites continuam vindo do [risk] global.
        assert_eq!(config.max_daily_loss_pct, Decimal::from(2));
        assert_eq!(config.max_trades_per_day, 3);
    }

    /// Falha FECHADO nos campos do ADR-020: valor fora de faixa aborta a
    /// subida em vez de virar o default em silêncio. Um teto de tamanho que o
    /// operador acha que configurou, mas que não está valendo, é pior do que
    /// não ter teto — foi o argumento que criou esta função.
    #[test]
    fn dimensionamento_invalido_aborta_a_subida() {
        let params = StrategyRiskParams {
            min_risk_reward: Decimal::from(2),
            max_spread_pct: Decimal::new(5, 2),
            max_atr_pct: Decimal::new(15, 1),
            trading_start_time: "09:45:00".to_string(),
            trading_end_time: "15:30:00".to_string(),
            risk_per_trade_pct: None,
        };

        let caso = |ajuste: &dyn Fn(&mut RiskSettings)| {
            let mut r = risk_settings();
            ajuste(&mut r);
            build_risk_config(&r, &params)
        };

        assert!(caso(&|r| r.capital_fraction = 1.5).is_err(), "fração > 1");
        assert!(caso(&|r| r.capital_fraction = 0.0).is_err(), "fração zero");
        assert!(
            caso(&|r| r.capital_fraction = -0.5).is_err(),
            "fração negativa"
        );
        assert!(
            caso(&|r| r.max_notional_multiple = 0.5).is_err(),
            "multiplicador < 1"
        );
        assert!(
            caso(&|r| r.max_notional_usd = Some(0.0)).is_err(),
            "teto zero"
        );
        assert!(
            caso(&|r| r.max_pct_of_median_bar_notional = Some(101.0)).is_err(),
            "mais que a barra inteira"
        );
        assert!(
            caso(&|r| r.liquidity_lookback_bars = 0).is_err(),
            "janela vazia"
        );

        // E o caminho feliz: a fração do ADR-020 e o cap de 1/3 da barra.
        let ok = caso(&|r| {
            r.capital_fraction = 1.0 / 3.0;
            r.max_pct_of_median_bar_notional = Some(100.0 / 3.0);
        })
        .expect("config válida");
        assert_eq!(
            ok.capital_efetivo(Decimal::from(240_000)),
            Decimal::from(80_000)
        );
    }

    /// Os defaults reproduzem o sizing anterior ao ADR-020 — se isto quebrar,
    /// todo run gravado até 08/09/2026 deixou de ser reproduzível.
    #[test]
    fn defaults_do_adr020_nao_mudam_o_sizing_de_ninguem() {
        let params = StrategyRiskParams {
            min_risk_reward: Decimal::from(2),
            max_spread_pct: Decimal::new(5, 2),
            max_atr_pct: Decimal::new(15, 1),
            trading_start_time: "09:45:00".to_string(),
            trading_end_time: "15:30:00".to_string(),
            risk_per_trade_pct: None,
        };
        let config = build_risk_config(&risk_settings(), &params).expect("config válida");

        assert_eq!(config.capital_fraction, Decimal::ONE);
        assert_eq!(config.max_notional_multiple, Decimal::ONE);
        assert_eq!(config.max_notional_usd, None);
        assert_eq!(config.max_pct_of_median_bar_notional, None);
        assert_eq!(
            config.capital_efetivo(Decimal::from(238_000)),
            Decimal::from(238_000)
        );
    }
}
