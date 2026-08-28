//! Dispatch de estratégias por id (`--strategy`).
//!
//! Ponto único de instanciação: paper, backtest e walk-forward carregam a
//! estratégia daqui, garantindo que live e backtest rodam exatamente a mesma
//! lógica (paridade é regra do projeto).

use anyhow::{Context, Result};

use trader_core::execution::time_exit::TimeExitConfig;
use trader_core::strategies::balance_area_breakout_v1::BalanceAreaBreakoutV1;
use trader_core::strategies::breakout_first_pullback_v1::BreakoutFirstPullbackV1;
use trader_core::strategies::failure_test_long_v1::FailureTestLongV1;
use trader_core::strategies::low2_m2s_short_v1::Low2M2sShortV1;
use trader_core::strategies::opening_reversal_v1::OpeningReversalV1;
use trader_core::strategies::pullback_trend_v1::PullbackTrendV1;
use trader_core::strategies::range_extreme_fade_v1::RangeExtremeFadeV1;
use trader_core::strategies::value_area_reentry_v1::ValueAreaReentryV1;
use trader_domain::{Candle, MarketContext, SignalResult, Strategy, StrategyId, StrategyState};

use crate::risk_config::StrategyRiskParams;

/// Estratégia carregada da configuração TOML, pronta para executar.
pub enum LoadedStrategy {
    PullbackTrendV1(PullbackTrendV1),
    FailureTestLongV1(FailureTestLongV1),
    BreakoutFirstPullbackV1(BreakoutFirstPullbackV1),
    OpeningReversalV1(OpeningReversalV1),
    BalanceAreaBreakoutV1(BalanceAreaBreakoutV1),
    RangeExtremeFadeV1(RangeExtremeFadeV1),
    Low2M2sShortV1(Low2M2sShortV1),
    ValueAreaReentryV1(ValueAreaReentryV1),
}

/// Carrega a estratégia pelo id (nome do arquivo em `config/strategies/`).
pub fn load_strategy(strategy_id: &str, toml_str: &str) -> Result<LoadedStrategy> {
    match strategy_id {
        "pullback-trend-v1" => Ok(LoadedStrategy::PullbackTrendV1(
            PullbackTrendV1::from_toml(toml_str)
                .with_context(|| "falha ao fazer parse da configuração TOML da estratégia")?,
        )),
        "failure-test-long-v1" => Ok(LoadedStrategy::FailureTestLongV1(
            FailureTestLongV1::from_toml(toml_str)
                .with_context(|| "falha ao fazer parse da configuração TOML da estratégia")?,
        )),
        "breakout-first-pullback-v1" => Ok(LoadedStrategy::BreakoutFirstPullbackV1(
            BreakoutFirstPullbackV1::from_toml(toml_str)
                .with_context(|| "falha ao fazer parse da configuração TOML da estratégia")?,
        )),
        "opening-reversal-v1" => Ok(LoadedStrategy::OpeningReversalV1(
            OpeningReversalV1::from_toml(toml_str)
                .with_context(|| "falha ao fazer parse da configuração TOML da estratégia")?,
        )),
        "balance-area-breakout-v1" => Ok(LoadedStrategy::BalanceAreaBreakoutV1(
            BalanceAreaBreakoutV1::from_toml(toml_str)
                .with_context(|| "falha ao fazer parse da configuração TOML da estratégia")?,
        )),
        "range-extreme-fade-v1" => Ok(LoadedStrategy::RangeExtremeFadeV1(
            RangeExtremeFadeV1::from_toml(toml_str)
                .with_context(|| "falha ao fazer parse da configuração TOML da estratégia")?,
        )),
        "low2-m2s-short-v1" => Ok(LoadedStrategy::Low2M2sShortV1(
            Low2M2sShortV1::from_toml(toml_str)
                .with_context(|| "falha ao fazer parse da configuração TOML da estratégia")?,
        )),
        "value-area-reentry-v1" => Ok(LoadedStrategy::ValueAreaReentryV1(
            ValueAreaReentryV1::from_toml(toml_str)
                .with_context(|| "falha ao fazer parse da configuração TOML da estratégia")?,
        )),
        other => anyhow::bail!(
            "estratégia desconhecida: {other} (suportadas: pullback-trend-v1, failure-test-long-v1, breakout-first-pullback-v1, opening-reversal-v1, balance-area-breakout-v1, range-extreme-fade-v1, low2-m2s-short-v1, value-area-reentry-v1)"
        ),
    }
}

impl LoadedStrategy {
    /// Candles de validade da entrada stop aguardando o rompimento (ADR-009).
    pub fn entry_validity_candles(&self) -> usize {
        match self {
            Self::PullbackTrendV1(s) => s.parameters().entry_validity_candles,
            Self::FailureTestLongV1(s) => s.parameters().entry_validity_candles,
            Self::BreakoutFirstPullbackV1(s) => s.parameters().entry_validity_candles,
            Self::OpeningReversalV1(s) => s.parameters().entry_validity_candles,
            Self::BalanceAreaBreakoutV1(s) => s.parameters().entry_validity_candles,
            Self::RangeExtremeFadeV1(s) => s.parameters().entry_validity_candles,
            Self::Low2M2sShortV1(s) => s.parameters().entry_validity_candles,
            Self::ValueAreaReentryV1(s) => s.parameters().entry_validity_candles,
        }
    }

    /// Hash de auditoria da configuração carregada.
    pub fn config_hash(&self) -> String {
        match self {
            Self::PullbackTrendV1(s) => s.config_hash(),
            Self::FailureTestLongV1(s) => s.config_hash(),
            Self::BreakoutFirstPullbackV1(s) => s.config_hash(),
            Self::OpeningReversalV1(s) => s.config_hash(),
            Self::BalanceAreaBreakoutV1(s) => s.config_hash(),
            Self::RangeExtremeFadeV1(s) => s.config_hash(),
            Self::Low2M2sShortV1(s) => s.config_hash(),
            Self::ValueAreaReentryV1(s) => s.config_hash(),
        }
    }

    /// Parâmetros de risco da estratégia para o `RiskConfig`.
    pub fn risk_params(&self) -> StrategyRiskParams {
        match self {
            Self::PullbackTrendV1(s) => StrategyRiskParams::from(s.parameters()),
            Self::FailureTestLongV1(s) => StrategyRiskParams::from(s.parameters()),
            Self::BreakoutFirstPullbackV1(s) => StrategyRiskParams::from(s.parameters()),
            Self::OpeningReversalV1(s) => StrategyRiskParams::from(s.parameters()),
            Self::BalanceAreaBreakoutV1(s) => StrategyRiskParams::from(s.parameters()),
            Self::RangeExtremeFadeV1(s) => StrategyRiskParams::from(s.parameters()),
            Self::Low2M2sShortV1(s) => StrategyRiskParams::from(s.parameters()),
            Self::ValueAreaReentryV1(s) => StrategyRiskParams::from(s.parameters()),
        }
    }

    /// Saída ativa por tempo (validação pós-entrada em R), se a estratégia
    /// a habilitar. Desligada por default na pullback-trend-v1.
    pub fn time_exit(&self) -> Option<TimeExitConfig> {
        match self {
            Self::PullbackTrendV1(_) => None,
            Self::FailureTestLongV1(s) => s.time_exit(),
            Self::BreakoutFirstPullbackV1(_) => None, // sem saída por tempo na v1 (doc §6)
            Self::OpeningReversalV1(_) => None,       // sem saída por tempo na v1 (doc §6)
            Self::BalanceAreaBreakoutV1(_) => None,   // sem saída por tempo na v1 (doc §6)
            Self::RangeExtremeFadeV1(_) => None,      // sem saída por tempo na v1 (doc §6)
            Self::Low2M2sShortV1(_) => None,          // sem saída por tempo na v1 (doc §6)
            Self::ValueAreaReentryV1(_) => None,      // sem saída por tempo na v1 (doc §6)
        }
    }
}

impl Strategy for LoadedStrategy {
    fn id(&self) -> StrategyId {
        match self {
            Self::PullbackTrendV1(s) => s.id(),
            Self::FailureTestLongV1(s) => s.id(),
            Self::BreakoutFirstPullbackV1(s) => s.id(),
            Self::OpeningReversalV1(s) => s.id(),
            Self::BalanceAreaBreakoutV1(s) => s.id(),
            Self::RangeExtremeFadeV1(s) => s.id(),
            Self::Low2M2sShortV1(s) => s.id(),
            Self::ValueAreaReentryV1(s) => s.id(),
        }
    }

    fn name(&self) -> &'static str {
        match self {
            Self::PullbackTrendV1(s) => s.name(),
            Self::FailureTestLongV1(s) => s.name(),
            Self::BreakoutFirstPullbackV1(s) => s.name(),
            Self::OpeningReversalV1(s) => s.name(),
            Self::BalanceAreaBreakoutV1(s) => s.name(),
            Self::RangeExtremeFadeV1(s) => s.name(),
            Self::Low2M2sShortV1(s) => s.name(),
            Self::ValueAreaReentryV1(s) => s.name(),
        }
    }

    fn source(&self) -> &'static str {
        match self {
            Self::PullbackTrendV1(s) => s.source(),
            Self::FailureTestLongV1(s) => s.source(),
            Self::BreakoutFirstPullbackV1(s) => s.source(),
            Self::OpeningReversalV1(s) => s.source(),
            Self::BalanceAreaBreakoutV1(s) => s.source(),
            Self::RangeExtremeFadeV1(s) => s.source(),
            Self::Low2M2sShortV1(s) => s.source(),
            Self::ValueAreaReentryV1(s) => s.source(),
        }
    }

    fn version(&self) -> &'static str {
        match self {
            Self::PullbackTrendV1(s) => s.version(),
            Self::FailureTestLongV1(s) => s.version(),
            Self::BreakoutFirstPullbackV1(s) => s.version(),
            Self::OpeningReversalV1(s) => s.version(),
            Self::BalanceAreaBreakoutV1(s) => s.version(),
            Self::RangeExtremeFadeV1(s) => s.version(),
            Self::Low2M2sShortV1(s) => s.version(),
            Self::ValueAreaReentryV1(s) => s.version(),
        }
    }

    fn analyze(
        &self,
        ctx: &MarketContext,
        state: &StrategyState,
        candles: &[Candle],
    ) -> SignalResult {
        match self {
            Self::PullbackTrendV1(s) => s.analyze(ctx, state, candles),
            Self::FailureTestLongV1(s) => s.analyze(ctx, state, candles),
            Self::BreakoutFirstPullbackV1(s) => s.analyze(ctx, state, candles),
            Self::OpeningReversalV1(s) => s.analyze(ctx, state, candles),
            Self::BalanceAreaBreakoutV1(s) => s.analyze(ctx, state, candles),
            Self::RangeExtremeFadeV1(s) => s.analyze(ctx, state, candles),
            Self::Low2M2sShortV1(s) => s.analyze(ctx, state, candles),
            Self::ValueAreaReentryV1(s) => s.analyze(ctx, state, candles),
        }
    }
}
