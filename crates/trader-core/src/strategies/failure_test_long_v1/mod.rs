//! Estratégia: Failure Test Long v1 (spring de Wyckoff / 2B de Sperandeo).
//!
//! Baseada em *The Art and Science of Technical Analysis*, de Adam Grimes
//! (Cap. 6 — Failure Test). O mercado sonda abaixo de um suporte claramente
//! definido e a violação FALHA: o preço fecha de volta acima do nível na
//! mesma barra ou na seguinte. Entramos comprados contra essa falha, com
//! stop abaixo do extremo da sonda e alvo de 1,5R.
//!
//! Especificação completa: `docs/strategies/failure-test-long-v1.md`.

pub mod config;
pub mod context;
pub mod entry;
pub mod setup;

use tracing::{debug, info};

use crate::context::MarketContextAnalyzer;
use crate::execution::time_exit::TimeExitConfig;
use crate::strategies::failure_test_long_v1::config::FailureTestLongV1Config;
use crate::strategies::failure_test_long_v1::context::check_context;
use crate::strategies::failure_test_long_v1::entry::build_signal;
use crate::strategies::failure_test_long_v1::setup::SetupResult;
use trader_domain::{
    Candle, MarketContext, RejectionReason, SignalResult, Strategy as StrategyTrait, StrategyId,
    StrategyState, TimeFrame,
};

/// Estratégia failure test de compra em suporte.
#[derive(Debug, Clone)]
pub struct FailureTestLongV1 {
    config: FailureTestLongV1Config,
    analyzer: MarketContextAnalyzer,
}

impl FailureTestLongV1 {
    pub fn new(config: FailureTestLongV1Config) -> Self {
        // O analyzer é usado apenas para a fase de mercado (horário) e o
        // snapshot de contexto; a lógica do setup usa os indicadores próprios.
        let analyzer = MarketContextAnalyzer::new(crate::context::ContextAnalyzerConfig::default());
        Self { config, analyzer }
    }

    /// Carrega a configuração a partir de uma string TOML.
    pub fn from_toml(toml_str: &str) -> Result<Self, toml::de::Error> {
        let config: FailureTestLongV1Config = toml::from_str(toml_str)?;
        Ok(Self::new(config))
    }

    /// Retorna referência para os parâmetros da estratégia.
    pub fn parameters(&self) -> &config::StrategyParameters {
        &self.config.strategy.parameters
    }

    /// Configuração da saída ativa por tempo (validação em R), se habilitada.
    pub fn time_exit(&self) -> Option<TimeExitConfig> {
        let te = &self.config.strategy.time_exit;
        te.enabled.then_some(TimeExitConfig {
            enabled: te.enabled,
            min_r: te.min_r,
            candles: te.candles,
        })
    }

    /// Hash de auditoria da configuração carregada.
    pub fn config_hash(&self) -> String {
        self.config.config_hash()
    }

    /// Analisa uma série de candles e retorna sinal ou rejeição.
    pub fn analyze_candles(&self, symbol: &str, candles: &[Candle]) -> SignalResult {
        let timeframe = match self
            .config
            .strategy
            .parameters
            .operational_timeframe
            .parse::<TimeFrame>()
        {
            Ok(tf) => tf,
            Err(_) => {
                return SignalResult::Rejected {
                    reason: RejectionReason::IncompleteSetup,
                    details: Some(serde_json::json!({ "reason": "invalid operational timeframe" })),
                }
            }
        };

        let ctx = match self.analyzer.analyze(symbol, timeframe, candles) {
            Some(ctx) => ctx,
            None => {
                return SignalResult::Rejected {
                    reason: RejectionReason::IncompleteSetup,
                    details: Some(
                        serde_json::json!({ "reason": "unable to compute market context" }),
                    ),
                }
            }
        };

        let context = match check_context(candles, &ctx, &self.config.strategy.parameters) {
            context::ContextCheck::Rejected(reason, details) => {
                debug!(?reason, "contexto rejeitado");
                return SignalResult::Rejected {
                    reason,
                    details: Some(details),
                };
            }
            context::ContextCheck::Approved(data) => *data,
        };

        match setup::detect_setup(candles, &context, &self.config.strategy.parameters) {
            SetupResult::Found(setup) => {
                let params = &self.config.strategy.parameters;
                let entry_order_type = parse_entry_order_type(&params.entry_order_type);

                // Guard anti-latência (mesmo do pullback-trend-v1): se o candle
                // mais recente já fechou além do gatilho, o rompimento aconteceu
                // sem a nossa ordem trabalhando. Só se aplica à entrada stop —
                // a "market_next_open" executa na primeira oportunidade por definição.
                if entry_order_type == trader_domain::EntryOrderType::Stop {
                    let last_close = candles.last().map(|c| c.close).unwrap_or_default();
                    if last_close >= setup.entry_price {
                        debug!(
                            %last_close,
                            entry = %setup.entry_price,
                            "setup invalidado: preço já além do gatilho"
                        );
                        return SignalResult::Rejected {
                            reason: RejectionReason::SetupInvalidated,
                            details: Some(serde_json::json!({
                                "reason": "price already beyond entry trigger",
                                "last_close": last_close,
                                "entry_price": setup.entry_price,
                            })),
                        };
                    }
                }

                info!(
                    entry = %setup.entry_price,
                    stop = %setup.stop_price,
                    target = %setup.target_price,
                    level = %setup.level.price,
                    "failure test detectado"
                );

                let signal = build_signal(
                    symbol,
                    timeframe,
                    &setup,
                    &context,
                    &ctx,
                    &self.config.strategy.id,
                    &self.config.strategy.version,
                    self.config.config_hash(),
                    entry_order_type,
                    params,
                );

                SignalResult::Signal(signal)
            }
            SetupResult::NotFound(reason, details) => SignalResult::Rejected {
                reason,
                details: Some(details),
            },
        }
    }
}

impl StrategyTrait for FailureTestLongV1 {
    fn id(&self) -> StrategyId {
        StrategyId::new(&self.config.strategy.id, &self.config.strategy.version)
    }

    fn name(&self) -> &'static str {
        "Failure Test Long v1"
    }

    fn source(&self) -> &'static str {
        "Adam Grimes - The Art and Science of Technical Analysis"
    }

    fn version(&self) -> &'static str {
        "1.0.0"
    }

    fn analyze(
        &self,
        ctx: &MarketContext,
        _state: &StrategyState,
        candles: &[Candle],
    ) -> SignalResult {
        if candles.is_empty() {
            return SignalResult::Rejected {
                reason: RejectionReason::IncompleteSetup,
                details: Some(serde_json::json!({ "reason": "empty candle series" })),
            };
        }

        self.analyze_candles(&ctx.symbol, candles)
    }
}

/// Faz parse do parâmetro `entry_order_type` da config ("stop" | "market_next_open").
/// Qualquer valor desconhecido cai no default do doc: stop.
fn parse_entry_order_type(raw: &str) -> trader_domain::EntryOrderType {
    match raw.trim().to_lowercase().as_str() {
        // Fill imediato (aproximação da "entrada no fechamento" do livro).
        "market_next_open" => trader_domain::EntryOrderType::Limit,
        _ => trader_domain::EntryOrderType::Stop,
    }
}

#[cfg(test)]
mod tests;
