//! Gestão de risco.
//!
//! O `RiskManager` valida sinais antes da execução e calcula o tamanho da
//! posição com base no capital e na distância até o stop.

use chrono::{DateTime, Timelike, Utc};
use rust_decimal::Decimal;
use tracing::{debug, warn};

use trader_domain::{MarketContext, Quote, RejectionReason, Signal, TradingMode, VolatilityRegime};

/// Configuração de risco.
#[derive(Debug, Clone, Copy)]
pub struct RiskConfig {
    pub trading_mode: TradingMode,
    pub risk_per_trade_pct: Decimal,
    pub max_daily_loss_pct: Decimal,
    pub max_trades_per_day: usize,
    pub max_consecutive_losses: usize,
    pub min_risk_reward: Decimal,
    pub max_spread_pct: Decimal,
    pub max_atr_pct: Decimal,
    pub trading_start_time_utc: (u32, u32, u32),
    pub trading_end_time_utc: (u32, u32, u32),
}

impl Default for RiskConfig {
    fn default() -> Self {
        Self {
            trading_mode: TradingMode::Paper,
            risk_per_trade_pct: Decimal::from(1), // 1%
            max_daily_loss_pct: Decimal::from(2), // 2%
            max_trades_per_day: 3,
            max_consecutive_losses: 3,
            min_risk_reward: Decimal::from(2),
            max_spread_pct: Decimal::from(5) / Decimal::from(100), // 0.05 (unidade percentual)
            max_atr_pct: Decimal::from(15) / Decimal::from(10),    // 1.5%
            trading_start_time_utc: (14, 30, 0),
            trading_end_time_utc: (21, 0, 0),
        }
    }
}

/// Estado de risco diário.
#[derive(Debug, Clone, Default)]
pub struct RiskState {
    pub daily_pnl: Decimal,
    pub daily_trades: usize,
    pub consecutive_losses: usize,
}

/// Resultado da validação de risco.
#[derive(Debug, Clone)]
pub enum RiskCheck {
    Approved {
        position_size: Decimal,
        risk_amount: Decimal,
    },
    Rejected(RejectionReason, String),
}

/// Gerenciador de risco.
#[derive(Debug, Clone)]
pub struct RiskManager {
    config: RiskConfig,
}

impl RiskManager {
    pub fn new(config: RiskConfig) -> Self {
        Self { config }
    }

    /// Valida um sinal contra todas as regras de risco.
    pub fn validate(
        &self,
        signal: &Signal,
        ctx: &MarketContext,
        quote: Option<&Quote>,
        state: &RiskState,
        capital: Decimal,
    ) -> RiskCheck {
        // Hard check de segurança: no MVP só é permitido operar em paper.
        if self.config.trading_mode.is_real() {
            warn!("rejeitado: modo de operação é real; MVP só permite paper");
            return RiskCheck::Rejected(
                RejectionReason::NotInPaperMode,
                "modo de operação real não é permitido no MVP; configure paper=true".to_string(),
            );
        }

        // Modo de operação e horário.
        if !is_within_trading_hours(
            ctx.timestamp,
            self.config.trading_start_time_utc,
            self.config.trading_end_time_utc,
        ) {
            return RiskCheck::Rejected(
                RejectionReason::OutsideTradingHours,
                "fora do horário de negociação configurado".to_string(),
            );
        }

        // Limite diário de perda.
        if state.daily_pnl <= -capital * self.config.max_daily_loss_pct / Decimal::from(100) {
            warn!(daily_pnl = %state.daily_pnl, "limite diário de perda atingido");
            return RiskCheck::Rejected(
                RejectionReason::DailyLossLimitReached,
                "limite diário de perda atingido".to_string(),
            );
        }

        // Máximo de trades por dia.
        if state.daily_trades >= self.config.max_trades_per_day {
            return RiskCheck::Rejected(
                RejectionReason::MaxTradesReached,
                "máximo de trades diários atingido".to_string(),
            );
        }

        // Perdas consecutivas.
        if state.consecutive_losses >= self.config.max_consecutive_losses {
            return RiskCheck::Rejected(
                RejectionReason::ConsecutiveLosses,
                "máximo de perdas consecutivas atingido".to_string(),
            );
        }

        // Stop obrigatório.
        let (entry, stop, target) =
            match (signal.entry_price, signal.stop_price, signal.target_price) {
                (Some(e), Some(s), Some(t)) => (e, s, t),
                _ => {
                    return RiskCheck::Rejected(
                        RejectionReason::StopMissing,
                        "preço de entrada, stop ou alvo ausente".to_string(),
                    );
                }
            };

        // Contexto de mercado.
        if !ctx.is_tradeable {
            return RiskCheck::Rejected(
                RejectionReason::NoContext,
                "contexto de mercado não é operável".to_string(),
            );
        }

        if matches!(ctx.volatility_regime, VolatilityRegime::High) {
            return RiskCheck::Rejected(
                RejectionReason::HighVolatility,
                "volatilidade acima do limite".to_string(),
            );
        }

        if let Some(atr_pct) = ctx.atr_14 {
            if atr_pct > self.config.max_atr_pct {
                return RiskCheck::Rejected(
                    RejectionReason::HighVolatility,
                    format!(
                        "ATR% {atr_pct}% acima do limite {}",
                        self.config.max_atr_pct
                    ),
                );
            }
        }

        // Spread.
        if let Some(q) = quote {
            let spread_pct = q.spread_pct();
            if spread_pct > self.config.max_spread_pct {
                return RiskCheck::Rejected(
                    RejectionReason::HighSpread,
                    format!(
                        "spread {spread_pct}% acima do limite {}",
                        self.config.max_spread_pct
                    ),
                );
            }
        }

        // Risco/retorno.
        let risk_distance = (entry - stop).abs();
        let reward_distance = (target - entry).abs();

        if risk_distance.is_zero() {
            return RiskCheck::Rejected(
                RejectionReason::PoorRiskReward,
                "distância de risco zero".to_string(),
            );
        }

        let risk_reward = reward_distance / risk_distance;
        if risk_reward < self.config.min_risk_reward {
            return RiskCheck::Rejected(
                RejectionReason::PoorRiskReward,
                format!(
                    "risco/retorno {risk_reward} abaixo do mínimo {}",
                    self.config.min_risk_reward
                ),
            );
        }

        // Tamanho da posição.
        let risk_amount = capital * self.config.risk_per_trade_pct / Decimal::from(100);
        // Arredonda para baixo para quantidade inteira de ações.
        let qty_by_risk = (risk_amount / risk_distance).trunc();

        if qty_by_risk <= Decimal::ZERO {
            return RiskCheck::Rejected(
                RejectionReason::InvalidQuantity,
                "tamanho da posição zero ou negativo".to_string(),
            );
        }

        // Cap de notional: qty × entry não pode exceder o capital disponível
        // (1x equity, sem margem — default seguro para o MVP).
        // Melhoria futura: tornar o multiplicador de cap configurável no RiskConfig.
        let qty_by_notional = (capital / entry).trunc();
        let position_size = qty_by_risk.min(qty_by_notional);

        if position_size < Decimal::ONE {
            return RiskCheck::Rejected(
                RejectionReason::InsufficientBuyingPower,
                format!(
                    "capital {capital} insuficiente para 1 unidade a {entry} (risco permitiria {qty_by_risk})"
                ),
            );
        }

        if position_size < qty_by_risk {
            debug!(
                qty_by_risk = %qty_by_risk,
                qty_by_notional = %qty_by_notional,
                "position size limitada pelo cap de notional (1x equity)"
            );
        }

        debug!(
            entry = %entry,
            stop = %stop,
            target = %target,
            risk_reward = %risk_reward,
            position_size = %position_size,
            "sinal aprovado pelo risk manager"
        );

        RiskCheck::Approved {
            position_size,
            risk_amount,
        }
    }

    /// Atualiza o estado de risco com o resultado de um trade fechado.
    ///
    /// `daily_trades` NÃO é incrementado aqui: a contagem de trades do dia é
    /// feita na entrada (ordem enviada), não no fechamento — um trade que
    /// fica aberto o dia inteiro também conta para o limite diário.
    pub fn update_state(&self, state: &mut RiskState, pnl: Decimal) {
        state.daily_pnl += pnl;

        if pnl < Decimal::ZERO {
            state.consecutive_losses += 1;
        } else {
            state.consecutive_losses = 0;
        }
    }
}

fn is_within_trading_hours(
    timestamp: DateTime<Utc>,
    start: (u32, u32, u32),
    end: (u32, u32, u32),
) -> bool {
    let time = timestamp.time();
    let seconds = |h: u32, m: u32, s: u32| h * 3600 + m * 60 + s;

    let current = seconds(time.hour(), time.minute(), time.second());
    let start = seconds(start.0, start.1, start.2);
    let end = seconds(end.0, end.1, end.2);

    current >= start && current <= end
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use rust_decimal::Decimal;
    use trader_domain::{
        Direction, MarketPhase, Signal, SignalStatus, TimeFrame, TrendState, VolatilityRegime,
    };

    fn make_context(timestamp: DateTime<Utc>) -> MarketContext {
        MarketContext {
            symbol: "SPY".to_string(),
            timeframe: TimeFrame::M15,
            timestamp,
            candle_timestamp: Some(timestamp),
            trend_state: TrendState::Uptrend,
            volatility_regime: VolatilityRegime::Normal,
            market_phase: MarketPhase::Regular,
            ema_20: Some(Decimal::from(100)),
            ema_50: None,
            sma_200: None,
            atr_14: Some(Decimal::from(1)),
            atr_percent_14: Some(Decimal::from(1)),
            volume_relative: None,
            hh_hl_count: None,
            lh_ll_count: None,
            range_percent: None,
            is_tradeable: true,
            raw_values: serde_json::Value::Object(Default::default()),
        }
    }

    fn make_signal(entry: Decimal, stop: Decimal, target: Decimal) -> Signal {
        Signal {
            symbol: "SPY".to_string(),
            strategy_id: "pullback-trend-v1".to_string(),
            strategy_version: "1.0.0".to_string(),
            config_hash: "abc".to_string(),
            timeframe: TimeFrame::M15,
            timestamp: Utc::now(),
            direction: Direction::Long,
            status: SignalStatus::Accepted,
            entry_order_type: trader_domain::EntryOrderType::Stop,
            entry_price: Some(entry),
            stop_price: Some(stop),
            target_price: Some(target),
            risk_reward_ratio: Some(Decimal::from(2)),
            risk_amount: None,
            risk_percent: None,
            position_size: None,
            entry_reason: None,
            rejection_reason: None,
            rejection_details: None,
            market_snapshot: serde_json::Value::Object(Default::default()),
            correlation_id: "corr".to_string(),
        }
    }

    #[test]
    fn approves_valid_long_signal() {
        let config = RiskConfig::default();
        let manager = RiskManager::new(config);
        let ctx = make_context(within_trading_hours());
        let signal = make_signal(Decimal::from(500), Decimal::from(495), Decimal::from(510));
        let state = RiskState::default();

        match manager.validate(&signal, &ctx, None, &state, Decimal::from(100_000)) {
            RiskCheck::Approved { position_size, .. } => {
                assert!(position_size > Decimal::ZERO);
            }
            RiskCheck::Rejected(reason, _) => {
                panic!("esperado aprovado, rejeitado por {:?}", reason)
            }
        }
    }

    #[test]
    fn rejects_poor_risk_reward() {
        let config = RiskConfig::default();
        let manager = RiskManager::new(config);
        let ctx = make_context(within_trading_hours());
        let signal = make_signal(Decimal::from(500), Decimal::from(499), Decimal::from(501));
        let state = RiskState::default();

        match manager.validate(&signal, &ctx, None, &state, Decimal::from(100_000)) {
            RiskCheck::Rejected(RejectionReason::PoorRiskReward, _) => {}
            other => panic!("esperado rejeição por risco/retorno, obtido {:?}", other),
        }
    }

    #[test]
    fn rejects_outside_trading_hours() {
        let config = RiskConfig::default();
        let manager = RiskManager::new(config);
        // 03:00 UTC está fora do horário configurado (14:30–21:00 UTC).
        let timestamp = Utc::now()
            .date_naive()
            .and_hms_opt(3, 0, 0)
            .unwrap()
            .and_utc();
        let ctx = make_context(timestamp);
        let signal = make_signal(Decimal::from(500), Decimal::from(495), Decimal::from(510));
        let state = RiskState::default();

        match manager.validate(&signal, &ctx, None, &state, Decimal::from(100_000)) {
            RiskCheck::Rejected(RejectionReason::OutsideTradingHours, _) => {}
            other => panic!("esperado rejeição por horário, obtido {:?}", other),
        }
    }

    #[test]
    fn rejects_real_trading_mode() {
        let config = RiskConfig {
            trading_mode: TradingMode::Real,
            ..RiskConfig::default()
        };
        let manager = RiskManager::new(config);
        let ctx = make_context(within_trading_hours());
        let signal = make_signal(Decimal::from(500), Decimal::from(495), Decimal::from(510));
        let state = RiskState::default();

        match manager.validate(&signal, &ctx, None, &state, Decimal::from(100_000)) {
            RiskCheck::Rejected(RejectionReason::NotInPaperMode, _) => {}
            other => panic!("esperado rejeição por modo real, obtido {:?}", other),
        }
    }

    /// Timestamp determinístico dentro do horário de negociação (14:30–21:00 UTC).
    fn within_trading_hours() -> DateTime<Utc> {
        Utc::now()
            .date_naive()
            .and_hms_opt(15, 0, 0)
            .unwrap()
            .and_utc()
    }

    #[test]
    fn caps_position_size_by_notional() {
        let config = RiskConfig::default();
        let manager = RiskManager::new(config);
        let ctx = make_context(within_trading_hours());
        let signal = make_signal(Decimal::from(500), Decimal::from(499), Decimal::from(502));
        let state = RiskState::default();

        // Risco 1% de $10k = $100 / $1 de stop = 100 ações, mas o notional de
        // 100 ações ($50k) excede o capital: cap em floor(10k / 500) = 20.
        match manager.validate(&signal, &ctx, None, &state, Decimal::from(10_000)) {
            RiskCheck::Approved { position_size, .. } => {
                assert_eq!(position_size, Decimal::from(20));
            }
            other => panic!("esperado aprovado com cap de notional, obtido {:?}", other),
        }
    }

    #[test]
    fn rejects_when_capital_insufficient_for_one_share() {
        let config = RiskConfig::default();
        let manager = RiskManager::new(config);
        let ctx = make_context(within_trading_hours());
        let signal = make_signal(Decimal::from(500), Decimal::from(499), Decimal::from(502));
        let state = RiskState::default();

        // $400 não compra 1 ação de $500 (cap = 0), mesmo com o risco permitindo.
        match manager.validate(&signal, &ctx, None, &state, Decimal::from(400)) {
            RiskCheck::Rejected(RejectionReason::InsufficientBuyingPower, _) => {}
            other => panic!(
                "esperado rejeição por buying power insuficiente, obtido {:?}",
                other
            ),
        }
    }

    #[test]
    fn approves_risk_sized_position_when_notional_fits() {
        let config = RiskConfig::default();
        let manager = RiskManager::new(config);
        let ctx = make_context(within_trading_hours());
        let signal = make_signal(Decimal::from(100), Decimal::from(95), Decimal::from(110));
        let state = RiskState::default();

        // Risco 1% de $100k = $1000 / $5 de stop = 200 ações; notional de
        // $20k cabe no capital (cap = 1000) — sizing por risco prevalece.
        match manager.validate(&signal, &ctx, None, &state, Decimal::from(100_000)) {
            RiskCheck::Approved { position_size, .. } => {
                assert_eq!(position_size, Decimal::from(200));
            }
            other => panic!("esperado aprovado sem cap, obtido {:?}", other),
        }
    }
}
