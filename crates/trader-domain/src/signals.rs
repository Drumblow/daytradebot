use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// Direção de uma operação ou posição.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    Long,
    Short,
}

impl Direction {
    pub fn opposite(&self) -> Self {
        match self {
            Direction::Long => Direction::Short,
            Direction::Short => Direction::Long,
        }
    }
}

/// Status de um sinal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignalStatus {
    /// Sinal aceito e pronto para execução.
    Accepted,
    /// Sinal rejeitado por alguma regra.
    Rejected,
    /// Sinal pendente de confirmação (ex: próximo candle).
    Pending,
    /// Sinal expirado antes da execução.
    Expired,
}

/// Motivo de rejeição de um sinal ou ordem.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RejectionReason {
    NoContext,
    MarketLateral,
    HighVolatility,
    LowVolatility,
    PoorRiskReward,
    HighSpread,
    OutsideTradingHours,
    DailyLossLimitReached,
    MaxTradesReached,
    ConsecutiveLosses,
    IncompleteSetup,
    /// O preço já passou do gatilho de entrada antes da ordem ser enviada —
    /// o rompimento aconteceu sem a nossa ordem estar trabalhando (latência
    /// entre o fechamento do candle e o envio).
    SetupInvalidated,
    WeakConfirmation,
    PositionAlreadyOpen,
    // --- Failure Test (docs/strategies/failure-test-long-v1.md, seção 11) ---
    /// Nenhuma condição de sobreextensão presente (mercado não "primed").
    NotOverextended,
    /// Clímax de venda em andamento (impulso de baixa fresco e extremo).
    ClimaxInProgress,
    /// Nenhum pivô de mínima atende aos critérios de nível significativo.
    SupportLevelNotFound,
    /// Nível com menos toques que o mínimo configurado.
    SupportNotTestedEnough,
    /// Fechamento prévio abaixo do nível (suporte já rompido).
    SupportAlreadyBroken,
    /// Nível formado há menos candles que a idade mínima.
    LevelTooRecent,
    /// Sem penetração do suporte (não é failure test).
    NoProbe,
    /// Sonda mais profunda que o máximo em ATR (sugere rompimento real).
    ProbeTooDeep,
    /// Sonda excedeu o máximo de barras consecutivas sem recuperação.
    ProbeTooLong,
    /// Sem fechamento de volta acima do suporte.
    NoRecoveryClose,
    /// Barra de recuperação fechou abaixo da posição mínima do range.
    WeakRecoveryBar,
    /// Ordem stop de entrada expirou sem rompimento do gatilho.
    EntryExpired,
    /// Stop mais próximo que 1x o range médio de barra (dentro do ruído).
    StopWithinNoise,
    /// Stop mais distante que o máximo em ATR (RR ruim para alvo intraday).
    StopTooWide,
    /// Segunda falha detectada com reentrada desabilitada (v1 só registra).
    ReentryDisabled,
    /// Resistência sem o mínimo de toques no lookback (breakout-pullback).
    ResistanceLevelNotFound,
    /// Breakout sem expansão de range ou de volume (breakout-pullback).
    WeakBreakout,
    /// Segundo breakout do mesmo nível (só a primeira tentativa vale).
    BreakoutAlreadyTaken,
    /// Pullback retraiu mais que o máximo do impulso pós-breakout.
    PullbackTooDeep,
    /// Pullback passou do máximo de candles sem gatilho.
    PullbackTooLong,
    /// Pullback fechou abaixo do pivô pré-breakout (breakout falho).
    BreakoutFailed,
    /// Candle não tocou a zona do nível de ontem (opening reversal).
    YesterdayLevelNotTested,
    /// Momentum contra demais para o fade (opening reversal).
    MomentumAgainst,
    /// Últimos candles não formam área de balanceamento (balance breakout).
    NoBalanceArea,
    StopMissing,
    InvalidQuantity,
    InsufficientBuyingPower,
    NotInPaperMode,
    BrokerError,
    Unknown,
}

/// Resultado da análise de uma estratégia.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[allow(clippy::large_enum_variant)]
pub enum SignalResult {
    None,
    Signal(Signal),
    Rejected {
        reason: RejectionReason,
        details: Option<serde_json::Value>,
    },
}

/// Sinal de entrada gerado por uma estratégia.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Signal {
    pub symbol: String,
    pub strategy_id: String,
    pub strategy_version: String,
    pub config_hash: String,
    pub timeframe: crate::TimeFrame,
    pub timestamp: DateTime<Utc>,
    pub direction: Direction,
    pub status: SignalStatus,
    /// Como a entrada deve ser trabalhada (stop no gatilho ou limit imediata).
    #[serde(default)]
    pub entry_order_type: crate::EntryOrderType,

    pub entry_price: Option<Decimal>,
    pub stop_price: Option<Decimal>,
    pub target_price: Option<Decimal>,
    pub risk_reward_ratio: Option<Decimal>,

    pub risk_amount: Option<Decimal>,
    pub risk_percent: Option<Decimal>,
    pub position_size: Option<Decimal>,

    pub entry_reason: Option<String>,
    pub rejection_reason: Option<RejectionReason>,
    pub rejection_details: Option<serde_json::Value>,

    pub market_snapshot: serde_json::Value,
    pub correlation_id: String,
}

impl Signal {
    /// Cria um sinal aceito.
    pub fn accepted(
        symbol: impl Into<String>,
        strategy_id: impl Into<String>,
        strategy_version: impl Into<String>,
        config_hash: impl Into<String>,
        timeframe: crate::TimeFrame,
        timestamp: DateTime<Utc>,
        direction: Direction,
    ) -> Self {
        Self {
            symbol: symbol.into(),
            strategy_id: strategy_id.into(),
            strategy_version: strategy_version.into(),
            config_hash: config_hash.into(),
            timeframe,
            timestamp,
            direction,
            status: SignalStatus::Accepted,
            entry_order_type: crate::EntryOrderType::default(),
            entry_price: None,
            stop_price: None,
            target_price: None,
            risk_reward_ratio: None,
            risk_amount: None,
            risk_percent: None,
            position_size: None,
            entry_reason: None,
            rejection_reason: None,
            rejection_details: None,
            market_snapshot: serde_json::Value::Object(Default::default()),
            correlation_id: uuid::Uuid::new_v4().to_string(),
        }
    }

    /// Cria um sinal rejeitado.
    #[allow(clippy::too_many_arguments)]
    pub fn rejected(
        symbol: impl Into<String>,
        strategy_id: impl Into<String>,
        strategy_version: impl Into<String>,
        config_hash: impl Into<String>,
        timeframe: crate::TimeFrame,
        timestamp: DateTime<Utc>,
        direction: Option<Direction>,
        reason: RejectionReason,
        details: Option<serde_json::Value>,
    ) -> Self {
        Self {
            symbol: symbol.into(),
            strategy_id: strategy_id.into(),
            strategy_version: strategy_version.into(),
            config_hash: config_hash.into(),
            timeframe,
            timestamp,
            direction: direction.unwrap_or(Direction::Long),
            status: SignalStatus::Rejected,
            entry_order_type: crate::EntryOrderType::default(),
            entry_price: None,
            stop_price: None,
            target_price: None,
            risk_reward_ratio: None,
            risk_amount: None,
            risk_percent: None,
            position_size: None,
            entry_reason: None,
            rejection_reason: Some(reason),
            rejection_details: details,
            market_snapshot: serde_json::Value::Object(Default::default()),
            correlation_id: uuid::Uuid::new_v4().to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Toda variante serializa em snake_case e faz o parse de volta
    /// (round-trip), formato usado na persistência de `signals`.
    #[test]
    fn rejection_reason_serde_round_trip_snake_case() {
        let cases = [
            (RejectionReason::NoContext, "no_context"),
            (RejectionReason::SetupInvalidated, "setup_invalidated"),
            (RejectionReason::NotOverextended, "not_overextended"),
            (RejectionReason::ClimaxInProgress, "climax_in_progress"),
            (
                RejectionReason::SupportLevelNotFound,
                "support_level_not_found",
            ),
            (
                RejectionReason::SupportNotTestedEnough,
                "support_not_tested_enough",
            ),
            (
                RejectionReason::SupportAlreadyBroken,
                "support_already_broken",
            ),
            (RejectionReason::LevelTooRecent, "level_too_recent"),
            (RejectionReason::NoProbe, "no_probe"),
            (RejectionReason::ProbeTooDeep, "probe_too_deep"),
            (RejectionReason::ProbeTooLong, "probe_too_long"),
            (RejectionReason::NoRecoveryClose, "no_recovery_close"),
            (RejectionReason::WeakRecoveryBar, "weak_recovery_bar"),
            (RejectionReason::EntryExpired, "entry_expired"),
            (RejectionReason::StopWithinNoise, "stop_within_noise"),
            (RejectionReason::StopTooWide, "stop_too_wide"),
            (RejectionReason::ReentryDisabled, "reentry_disabled"),
            (
                RejectionReason::ResistanceLevelNotFound,
                "resistance_level_not_found",
            ),
            (RejectionReason::WeakBreakout, "weak_breakout"),
            (
                RejectionReason::BreakoutAlreadyTaken,
                "breakout_already_taken",
            ),
            (RejectionReason::PullbackTooDeep, "pullback_too_deep"),
            (RejectionReason::PullbackTooLong, "pullback_too_long"),
            (RejectionReason::BreakoutFailed, "breakout_failed"),
            (
                RejectionReason::YesterdayLevelNotTested,
                "yesterday_level_not_tested",
            ),
            (RejectionReason::MomentumAgainst, "momentum_against"),
            (RejectionReason::NoBalanceArea, "no_balance_area"),
        ];

        for (reason, expected) in cases {
            let serialized = serde_json::to_string(&reason).unwrap();
            assert_eq!(serialized, format!("\"{expected}\""));
            let parsed: RejectionReason = serde_json::from_str(&serialized).unwrap();
            assert_eq!(parsed, reason);
        }
    }
}
