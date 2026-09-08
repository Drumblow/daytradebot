//! Gestão de risco.
//!
//! O `RiskManager` valida sinais antes da execução e calcula o tamanho da
//! posição com base no capital e na distância até o stop.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use tracing::{debug, warn};

use trader_domain::{
    Candle, Direction, MarketContext, Quote, RejectionReason, Signal, TradingMode,
    VolatilityRegime,
};

pub mod liquidity;

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
    /// Início da janela de negociação em horário de NOVA YORK.
    pub trading_start_time_et: (u32, u32, u32),
    /// Fim da janela de negociação em horário de NOVA YORK.
    pub trading_end_time_et: (u32, u32, u32),
    /// Tolerância de overshoot numa entrada stop, como fração da distância do
    /// stop. Se o preço de referência já passou do gatilho além disso, a
    /// entrada é invalidada em vez de perseguida — o risco real do trade já
    /// não seria o desenhado (trade 12 do live: overshoot de 0.38 num stop de
    /// 0.35 dobrou o risco). Ver ADR-015.
    pub entry_overshoot_tolerance: Decimal,

    // --- ADR-020: de quanto é uma posição ---
    //
    // Os quatro defaults abaixo reproduzem EXATAMENTE o sizing anterior
    // (fração 1, multiplicador 1, sem teto absoluto, sem cap de liquidez).
    // Nenhum run existente muda de resultado por esta ADR entrar.
    /// Multiplicador do teto de notional (ADR-020 §1). 1 = o cap de 1× a
    /// equity que estava hardcoded. Acima de 1 é alavancagem intraday, e o
    /// teto de 200% da conta (ADR-017) continua valendo por cima.
    pub max_notional_multiple: Decimal,
    /// Teto absoluto de notional por posição, em dólares da conta (§2).
    /// Aplicado DEPOIS do multiplicador. `None` = sem teto absoluto.
    pub max_notional_usd: Option<Decimal>,
    /// Fatia do capital que esta instância pode ocupar (§4). Com 3 instâncias
    /// e 1/3, três posições cheias cabem em 100% de notional — a regra de
    /// dinheiro real do ADR-017 sem recusar o cluster.
    ///
    /// **Não promete retorno**: corta o P&L em $ na mesma proporção, com PF e
    /// avg R invariantes. É política de risco.
    pub capital_fraction: Decimal,
    /// Fração (em %) da barra mediana de 15m que uma posição pode ocupar
    /// (§3). `None` = cap desligado. Ligado, uma janela insuficiente é
    /// RECUSA, não "segue sem cap".
    pub max_pct_of_median_bar_notional: Option<Decimal>,
    /// Quantas barras entram na mediana de liquidez.
    ///
    /// 600 é a janela do live (`LIVE_MAX_CANDLES`, ≈ 23 pregões). A ADR-020
    /// §3 pedia 60 pregões, mas exige — em letras maiúsculas — que a fonte e
    /// o N sejam **iguais** no live e no backtest; 60 pregões custariam
    /// triplicar a busca de candles na IBKR a cada poll, no feed que o §5.8
    /// já mostra frágil. Entre os dois requisitos, o que não se pode
    /// negociar é a paridade.
    pub liquidity_lookback_bars: usize,
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
            trading_start_time_et: (9, 30, 0),
            trading_end_time_et: (16, 0, 0),
            entry_overshoot_tolerance: Decimal::from(25) / Decimal::from(100), // 25% da distância do stop
            max_notional_multiple: Decimal::ONE,
            max_notional_usd: None,
            capital_fraction: Decimal::ONE,
            max_pct_of_median_bar_notional: None,
            liquidity_lookback_bars: 600,
        }
    }
}

/// Qual regra fixou o teto de notional de uma posição (ADR-020).
///
/// Existe para o log e para a tabela de modos do harness: "o cap prendeu" é
/// um fato diferente de "a liquidez prendeu", e o §5.5 do plano pede a fração
/// de trades presos em cada um.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TetoDeNotional {
    /// `capital × capital_fraction × max_notional_multiple` — o teto de
    /// sempre, que até o ADR-020 era 1× a equity e hardcoded.
    Capital,
    /// `max_notional_usd`, o teto absoluto da instância.
    Absoluto,
    /// Fração da barra mediana de 15m (ADR-020 §3).
    Liquidez,
}

impl TetoDeNotional {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Capital => "capital",
            Self::Absoluto => "max_notional_usd",
            Self::Liquidez => "liquidez",
        }
    }
}

impl RiskConfig {
    /// Capital que ESTA instância pode ocupar (ADR-020 §4).
    ///
    /// Com `capital_fraction = 1` (default) é a equity inteira, como sempre
    /// foi. É o número de onde saem AS DUAS pernas do sizing — orçamento de
    /// risco e teto de notional —, e por isso a fração corta o tamanho uma
    /// vez só, não duas.
    pub fn capital_efetivo(&self, capital: Decimal) -> Decimal {
        // Arredondado em CENTAVOS. A fração natural desta ADR é 1/3, que em
        // decimal não termina: 240.000 × 0,333…3 dá 79.999,99…, e o `trunc`
        // do sizing transformaria isso em 799 ações em vez de 800. Perder uma
        // ação para a base 10 seria um artefato, não uma decisão de risco —
        // e dinheiro tem centavos de qualquer forma.
        (capital * self.capital_fraction).round_dp(2)
    }

    /// Teto de notional de UMA posição e a regra que o fixou.
    ///
    /// `liquidez` é a mediana do notional por barra; `None` significa "cap de
    /// liquidez desligado". Quando o cap está LIGADO e a mediana não pôde ser
    /// medida, quem chama recusa o sinal antes de chegar aqui — este método
    /// não tem como sinalizar recusa, e um teto que evapora por falta de dado
    /// é pior do que não ter teto.
    pub fn teto_de_notional(
        &self,
        capital: Decimal,
        liquidez: Option<Decimal>,
    ) -> (Decimal, TetoDeNotional) {
        let mut teto = self.capital_efetivo(capital) * self.max_notional_multiple;
        let mut qual = TetoDeNotional::Capital;

        if let Some(usd) = self.max_notional_usd {
            if usd < teto {
                teto = usd;
                qual = TetoDeNotional::Absoluto;
            }
        }

        if let (Some(pct), Some(mediana)) = (self.max_pct_of_median_bar_notional, liquidez) {
            let cap = mediana * pct / Decimal::from(100);
            if cap < teto {
                teto = cap;
                qual = TetoDeNotional::Liquidez;
            }
        }

        (teto, qual)
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

    /// Configuração ativa (leitura) — usada pela `ExecutionEngine` para regras
    /// que dependem de dados que o `validate` não recebe (ex.: overshoot).
    pub fn config(&self) -> &RiskConfig {
        &self.config
    }

    /// Valida um sinal contra todas as regras de risco.
    /// `candles` é o MESMO buffer que a estratégia analisou — é dele que sai
    /// a mediana de liquidez do ADR-020 §3. Passar o buffer (em vez da
    /// mediana já calculada) é o que garante que live e backtest medem a
    /// liquidez na mesma fonte e com o mesmo N: a conta acontece aqui dentro,
    /// uma vez só.
    pub fn validate(
        &self,
        signal: &Signal,
        ctx: &MarketContext,
        quote: Option<&Quote>,
        state: &RiskState,
        capital: Decimal,
        candles: &[Candle],
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
            self.config.trading_start_time_et,
            self.config.trading_end_time_et,
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

        // Lado do stop e do alvo. O cálculo abaixo usa distâncias ABSOLUTAS,
        // então um sinal long com stop ACIMA da entrada passava por todos os
        // filtros e virava um bracket que estopa no instante seguinte. Barato
        // de checar, caro de descobrir em produção.
        let lados_ok = match signal.direction {
            Direction::Long => stop < entry && entry < target,
            Direction::Short => target < entry && entry < stop,
        };
        if !lados_ok {
            return RiskCheck::Rejected(
                RejectionReason::StopMissing,
                format!(
                    "stop/alvo do lado errado para {:?}: entrada {entry}, stop {stop}, alvo {target}",
                    signal.direction
                ),
            );
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

        // Tamanho da posição (ADR-020).
        //
        // `capital_fraction` entra ANTES de tudo: é a fatia da conta que esta
        // instância pode ocupar, e ela corta as duas pernas do sizing de uma
        // vez — orçamento de risco e teto de notional.
        let capital_ef = self.config.capital_efetivo(capital);
        let risk_budget = capital_ef * self.config.risk_per_trade_pct / Decimal::from(100);
        // Arredonda para baixo para quantidade inteira de ações.
        let qty_by_risk = (risk_budget / risk_distance).trunc();

        if qty_by_risk <= Decimal::ZERO {
            return RiskCheck::Rejected(
                RejectionReason::InvalidQuantity,
                "tamanho da posição zero ou negativo".to_string(),
            );
        }

        // Cap de liquidez (ADR-020 §3): uma posição não pode ser uma fração
        // grande da barra que o ativo negocia. FALHA FECHADO — com o cap
        // ligado e sem janela para medir a mediana, o sinal é recusado. A
        // alternativa ("segue sem cap") faria o teto sumir exatamente no dia
        // em que o feed falha, que é quando ele mais importaria.
        let liquidez = match self.config.max_pct_of_median_bar_notional {
            None => None,
            Some(_) => {
                match liquidity::median_bar_notional(candles, self.config.liquidity_lookback_bars) {
                    Some(mediana) => Some(mediana),
                    None => {
                        return RiskCheck::Rejected(
                            RejectionReason::NotionalAboveLiquidityCap,
                            format!(
                                "cap de liquidez ligado, mas a janela de {} barras não cabe no \
                                 buffer ({} barras): sem a barra mediana o teto não existe",
                                self.config.liquidity_lookback_bars,
                                candles.len()
                            ),
                        );
                    }
                }
            }
        };

        let (teto, qual_teto) = self.config.teto_de_notional(capital, liquidez);
        let qty_by_notional = (teto / entry).trunc();
        let position_size = qty_by_risk.min(qty_by_notional);

        if position_size < Decimal::ONE {
            // "Não cabe uma ação" tem duas causas diferentes, e chamar as duas
            // de falta de poder de compra apagaria a que interessa: o ativo é
            // ilíquido demais para o tamanho pedido.
            return match qual_teto {
                TetoDeNotional::Liquidez => RiskCheck::Rejected(
                    RejectionReason::NotionalAboveLiquidityCap,
                    format!(
                        "cap de liquidez {teto} não cabe 1 unidade a {entry} \
                         (risco permitiria {qty_by_risk})"
                    ),
                ),
                _ => RiskCheck::Rejected(
                    RejectionReason::InsufficientBuyingPower,
                    format!(
                        "capital {capital} insuficiente para 1 unidade a {entry} (risco permitiria {qty_by_risk})"
                    ),
                ),
            };
        }

        if position_size < qty_by_risk {
            debug!(
                qty_by_risk = %qty_by_risk,
                qty_by_notional = %qty_by_notional,
                teto = %teto,
                qual_teto = qual_teto.as_str(),
                "position size limitada pelo teto de notional"
            );
        }

        // Risco REAL assumido: distância do stop × quantidade final. Difere do
        // orçamento sempre que o cap de notional (ou o trunc) corta a posição.
        // Gravar o orçamento aqui comprimia o result_in_r do live em direção a
        // zero (um stop cheio lia −0,21R em vez de −1R) e quebrava a paridade
        // com o backtest, cujo broker simulado sempre calculou o R sobre
        // |entrada − stop| × quantidade — e avgR é métrica do gate (ADR-010).
        let risk_amount = risk_distance * position_size;

        debug!(
            entry = %entry,
            stop = %stop,
            target = %target,
            risk_reward = %risk_reward,
            position_size = %position_size,
            risk_amount = %risk_amount,
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

/// Janela de negociação em horário de NOVA YORK (A2 da auditoria).
///
/// A regra mora em [`crate::session`] — este wrapper existe só para manter o
/// nome usado dentro do `validate`.
fn is_within_trading_hours(
    timestamp: DateTime<Utc>,
    start: (u32, u32, u32),
    end: (u32, u32, u32),
) -> bool {
    crate::session::within_trading_window(timestamp, start, end)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use rust_decimal::Decimal;
    use trader_domain::{
        Direction, MarketPhase, Signal, SignalStatus, TimeFrame, TrendState, VolatilityRegime,
    };
    // `Candle` vem do escopo do módulo (usado pelo `validate`).
    use trader_domain::Candle;

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

    /// Barras de 15m com notional `close × volume` conhecido, para os testes
    /// do cap de liquidez.
    fn barras(n: usize, close: i64, volume: i64) -> Vec<Candle> {
        (0..n)
            .map(|i| {
                Candle::new(
                    "SPY",
                    TimeFrame::M15,
                    Utc::now() - chrono::Duration::minutes(15 * (n - i) as i64),
                    Decimal::from(close),
                    Decimal::from(close),
                    Decimal::from(close),
                    Decimal::from(close),
                    Decimal::from(volume),
                )
                .expect("candle válido")
            })
            .collect()
    }

    /// O caso central do ADR-020 §4, com os números da própria ADR.
    ///
    /// Conta de 240.000 e fração 1/3: a instância enxerga 80.000. O risco de
    /// 1% incide sobre a FATIA (800, não 2.400) e o teto de notional também
    /// (80.000, não 240.000) — a fração corta o tamanho uma vez, não duas.
    /// O `risk_amount` continua sendo o risco REAL da posição, então o avg R
    /// do gate não muda de significado por causa da fração.
    #[test]
    fn fracao_de_capital_corta_o_tamanho_uma_vez_so() {
        let config = RiskConfig {
            capital_fraction: Decimal::ONE / Decimal::from(3),
            ..RiskConfig::default()
        };
        let manager = RiskManager::new(config);
        let ctx = make_context(within_trading_hours());
        // Stop a 22 bp — o stop mediano medido nas três estratégias.
        let signal = make_signal(
            Decimal::from(100),
            Decimal::new(9978, 2),
            Decimal::from(101),
        );

        match manager.validate(
            &signal,
            &ctx,
            None,
            &RiskState::default(),
            Decimal::from(240_000),
            &[],
        ) {
            RiskCheck::Approved {
                position_size,
                risk_amount,
            } => {
                assert_eq!(position_size, Decimal::from(800), "800 ações = 80.000 de notional");
                // 0,22 × 800. O orçamento de risco era 800: o cap prendeu, e
                // o risco real ficou em 176 — 0,073% da conta, não 1%.
                assert_eq!(risk_amount, Decimal::new(17600, 2));
            }
            outro => panic!("esperado aprovado, obtido {outro:?}"),
        }
    }

    /// Sem fração, o mesmo sinal ocupa a conta inteira: é o mundo anterior à
    /// ADR-020, e ele tem de continuar reproduzível — todo run gravado até
    /// 08/09/2026 foi medido assim.
    #[test]
    fn sem_fracao_o_sizing_e_exatamente_o_de_antes() {
        let manager = RiskManager::new(RiskConfig::default());
        let ctx = make_context(within_trading_hours());
        let signal = make_signal(
            Decimal::from(100),
            Decimal::new(9978, 2),
            Decimal::from(101),
        );

        match manager.validate(
            &signal,
            &ctx,
            None,
            &RiskState::default(),
            Decimal::from(240_000),
            &[],
        ) {
            RiskCheck::Approved { position_size, .. } => {
                assert_eq!(position_size, Decimal::from(2400));
            }
            outro => panic!("esperado aprovado, obtido {outro:?}"),
        }
    }

    /// Teto absoluto por instância (ADR-020 §2): 50.000 a US$ 108 = 462 ações.
    /// É o mecanismo que vai limitar SLYV e IJS por `env_file`, sem tocar em
    /// código nem no `config_hash` da estratégia.
    #[test]
    fn teto_absoluto_em_dolares_prende_antes_do_capital() {
        let config = RiskConfig {
            capital_fraction: Decimal::ONE / Decimal::from(3),
            max_notional_usd: Some(Decimal::from(50_000)),
            ..RiskConfig::default()
        };
        let manager = RiskManager::new(config);
        let ctx = make_context(within_trading_hours());
        let signal = make_signal(
            Decimal::from(108),
            Decimal::new(10778, 2),
            Decimal::from(109),
        );

        match manager.validate(
            &signal,
            &ctx,
            None,
            &RiskState::default(),
            Decimal::from(240_000),
            &[],
        ) {
            RiskCheck::Approved { position_size, .. } => {
                assert_eq!(position_size, Decimal::from(462));
            }
            outro => panic!("esperado aprovado, obtido {outro:?}"),
        }
    }

    /// Cap de liquidez (ADR-020 §3) com o número medido de SLYV: barra
    /// mediana de 15m ≈ US$ 290.000. A 1/3 dela o teto é ≈ 96.667, e a
    /// posição cai para 895 ações — MENOS do que a fração de capital daria.
    /// É o resultado desconfortável que a ADR manda mostrar antes de qualquer
    /// escada de risco: em SLYV e IJS o cap REDUZ o tamanho de hoje.
    #[test]
    fn cap_de_liquidez_reduz_o_tamanho_nos_ativos_finos() {
        let config = RiskConfig {
            // 1/3 da BARRA mediana. A fração de capital fica em 1 de
            // propósito: com 1/3 dela o teto de 80.000 prenderia ANTES dos
            // 96.667 da liquidez, e o teste não estaria medindo o cap.
            max_pct_of_median_bar_notional: Some(Decimal::from(100) / Decimal::from(3)),
            liquidity_lookback_bars: 5,
            ..RiskConfig::default()
        };
        let manager = RiskManager::new(config);
        let ctx = make_context(within_trading_hours());
        let signal = make_signal(
            Decimal::from(108),
            Decimal::new(10778, 2),
            Decimal::from(109),
        );
        // 5 barras de 100 × 2.900 = 290.000 de notional cada.
        let candles = barras(5, 100, 2_900);

        match manager.validate(
            &signal,
            &ctx,
            None,
            &RiskState::default(),
            Decimal::from(240_000),
            &candles,
        ) {
            RiskCheck::Approved { position_size, .. } => {
                assert_eq!(position_size, Decimal::from(895));
            }
            outro => panic!("esperado aprovado, obtido {outro:?}"),
        }

        // Sem o cap, a mesma conta compraria 2.222 ações — 2,5× mais, e
        // 240.000 de notional numa barra mediana de 290.000.
        let sem_cap = RiskManager::new(RiskConfig::default());
        match sem_cap.validate(
            &signal,
            &ctx,
            None,
            &RiskState::default(),
            Decimal::from(240_000),
            &candles,
        ) {
            RiskCheck::Approved { position_size, .. } => {
                assert_eq!(position_size, Decimal::from(2222));
            }
            outro => panic!("esperado aprovado, obtido {outro:?}"),
        }
    }

    /// FALHA FECHADO: com o cap ligado e sem janela para medir a mediana, o
    /// sinal é recusado — e com motivo PRÓPRIO, não "sem poder de compra".
    ///
    /// A alternativa (seguir sem cap) faria o teto sumir exatamente no dia em
    /// que o feed entrega menos barras, que é quando ele mais importa. E o
    /// motivo separado é o que permite contar, no banco, quantas entradas o
    /// cap barrou: `InsufficientBuyingPower` já significa outra coisa.
    #[test]
    fn cap_ligado_sem_janela_recusa_em_vez_de_ignorar() {
        let config = RiskConfig {
            max_pct_of_median_bar_notional: Some(Decimal::from(100) / Decimal::from(3)),
            liquidity_lookback_bars: 600,
            ..RiskConfig::default()
        };
        let manager = RiskManager::new(config);
        let ctx = make_context(within_trading_hours());
        let signal = make_signal(
            Decimal::from(108),
            Decimal::new(10778, 2),
            Decimal::from(109),
        );

        // 599 barras: uma a menos que a janela pedida.
        let candles = barras(599, 100, 2_900);
        match manager.validate(
            &signal,
            &ctx,
            None,
            &RiskState::default(),
            Decimal::from(240_000),
            &candles,
        ) {
            RiskCheck::Rejected(RejectionReason::NotionalAboveLiquidityCap, detalhe) => {
                assert!(detalhe.contains("599"), "detalhe: {detalhe}");
            }
            outro => panic!("esperado recusa por liquidez, obtido {outro:?}"),
        }

        // Com o cap DESLIGADO (o default), as mesmas 599 barras não impedem
        // nada: o buffer só é exigido por quem depende dele.
        let sem_cap = RiskManager::new(RiskConfig::default());
        assert!(matches!(
            sem_cap.validate(
                &signal,
                &ctx,
                None,
                &RiskState::default(),
                Decimal::from(240_000),
                &candles,
            ),
            RiskCheck::Approved { .. }
        ));
    }

    /// Ativo fino demais para uma ação sequer: recusa por LIQUIDEZ, não por
    /// falta de capital. São fatos diferentes e o banco precisa distingui-los.
    #[test]
    fn cap_de_liquidez_abaixo_de_uma_acao_recusa_com_motivo_proprio() {
        let config = RiskConfig {
            max_pct_of_median_bar_notional: Some(Decimal::ONE),
            liquidity_lookback_bars: 3,
            ..RiskConfig::default()
        };
        let manager = RiskManager::new(config);
        let ctx = make_context(within_trading_hours());
        let signal = make_signal(
            Decimal::from(500),
            Decimal::new(49978, 2),
            Decimal::from(501),
        );
        // Barra mediana de 1.000: 1% dela é 10 — não paga uma ação de 500.
        let candles = barras(3, 10, 100);

        match manager.validate(
            &signal,
            &ctx,
            None,
            &RiskState::default(),
            Decimal::from(240_000),
            &candles,
        ) {
            RiskCheck::Rejected(RejectionReason::NotionalAboveLiquidityCap, _) => {}
            outro => panic!("esperado recusa por liquidez, obtido {outro:?}"),
        }
    }

    /// O multiplicador de notional (ADR-020 §1) é o que era hardcoded como 1.
    /// Acima de 1 é alavancagem intraday — aqui só se prova que o parâmetro
    /// existe e faz o que diz; o teto de 200% da conta é outra trava, no live.
    #[test]
    fn multiplicador_de_notional_libera_alavancagem_intraday() {
        let config = RiskConfig {
            max_notional_multiple: Decimal::from(2),
            ..RiskConfig::default()
        };
        let manager = RiskManager::new(config);
        let ctx = make_context(within_trading_hours());
        let signal = make_signal(
            Decimal::from(100),
            Decimal::new(9978, 2),
            Decimal::from(101),
        );

        match manager.validate(
            &signal,
            &ctx,
            None,
            &RiskState::default(),
            Decimal::from(240_000),
            &[],
        ) {
            RiskCheck::Approved { position_size, .. } => {
                // Teto de 480.000 a US$ 100 = 4.800 ações, o dobro do 1×.
                // O orçamento de risco (2.400 / 0,22 = 10.909) continua sem
                // prender: com stop de 22 bp, quem decide o tamanho é o cap —
                // é o fato que o modo B' do harness existe para testar.
                assert_eq!(position_size, Decimal::from(4800));
            }
            outro => panic!("esperado aprovado, obtido {outro:?}"),
        }
    }

    #[test]
    fn approves_valid_long_signal() {
        let config = RiskConfig::default();
        let manager = RiskManager::new(config);
        let ctx = make_context(within_trading_hours());
        let signal = make_signal(Decimal::from(500), Decimal::from(495), Decimal::from(510));
        let state = RiskState::default();

        match manager.validate(&signal, &ctx, None, &state, Decimal::from(100_000), &[]) {
            RiskCheck::Approved { position_size, .. } => {
                assert!(position_size > Decimal::ZERO);
            }
            RiskCheck::Rejected(reason, _) => {
                panic!("esperado aprovado, rejeitado por {:?}", reason)
            }
        }
    }

    /// `risk_amount` é o risco REAL (distância do stop × quantidade final),
    /// não o orçamento de 1% — quando o cap de notional corta a posição, o
    /// orçamento inflaria o denominador do result_in_r e comprimiria o avgR
    /// do gate (ADR-010), além de quebrar a paridade com o backtest. Cenário
    /// do trade 9 do live: stop apertado → qty por risco enorme → cap trava
    /// no notional e o risco efetivo fica bem abaixo do orçamento.
    #[test]
    fn risk_amount_reflects_actual_position_risk_under_notional_cap() {
        let config = RiskConfig::default(); // 1% de risco por trade
        let manager = RiskManager::new(config);
        let ctx = make_context(within_trading_hours());
        // Distância do stop 0.50: orçamento (1% de 100k = 1000) pediria 2000
        // ações, mas o notional (100k / 500) só permite 200.
        let signal = make_signal(
            Decimal::from(500),
            Decimal::new(4995, 1), // 499.5
            Decimal::from(510),
        );
        let state = RiskState::default();

        match manager.validate(&signal, &ctx, None, &state, Decimal::from(100_000), &[]) {
            RiskCheck::Approved {
                position_size,
                risk_amount,
            } => {
                assert_eq!(position_size, Decimal::from(200)); // cap de notional
                                                               // Risco real: 0.50 × 200 = 100 — e não o orçamento de 1000.
                assert_eq!(risk_amount, Decimal::from(100));
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

        match manager.validate(&signal, &ctx, None, &state, Decimal::from(100_000), &[]) {
            RiskCheck::Rejected(RejectionReason::PoorRiskReward, _) => {}
            other => panic!("esperado rejeição por risco/retorno, obtido {:?}", other),
        }
    }

    /// O cálculo de risco usa distâncias absolutas: sem esta checagem, um long
    /// com stop ACIMA da entrada passa por todos os filtros e vira um bracket
    /// que estopa no instante seguinte.
    #[test]
    fn rejeita_stop_e_alvo_do_lado_errado() {
        let manager = RiskManager::new(RiskConfig::default());
        let ctx = make_context(within_trading_hours());
        let state = RiskState::default();

        // Long invertido: stop acima da entrada, alvo abaixo.
        let mut sinal = make_signal(Decimal::from(100), Decimal::from(105), Decimal::from(90));
        sinal.direction = Direction::Long;
        assert!(
            matches!(
                manager.validate(&sinal, &ctx, None, &state, Decimal::from(100_000), &[]),
                RiskCheck::Rejected(RejectionReason::StopMissing, _)
            ),
            "long invertido deveria ser recusado"
        );

        // Short invertido: stop abaixo da entrada.
        let mut curto = make_signal(Decimal::from(100), Decimal::from(95), Decimal::from(110));
        curto.direction = Direction::Short;
        assert!(
            matches!(
                manager.validate(&curto, &ctx, None, &state, Decimal::from(100_000), &[]),
                RiskCheck::Rejected(RejectionReason::StopMissing, _)
            ),
            "short invertido deveria ser recusado"
        );

        // Short correto (stop acima, alvo abaixo) NÃO pode ser recusado por lado.
        let mut ok = make_signal(Decimal::from(100), Decimal::from(105), Decimal::from(90));
        ok.direction = Direction::Short;
        assert!(
            !matches!(
                manager.validate(&ok, &ctx, None, &state, Decimal::from(100_000), &[]),
                RiskCheck::Rejected(RejectionReason::StopMissing, _)
            ),
            "short correto não deveria falhar na checagem de lado"
        );
    }

    #[test]
    fn rejects_outside_trading_hours() {
        let config = RiskConfig::default();
        let manager = RiskManager::new(config);
        // 03:00 UTC = 22h/23h ET do dia anterior: fora de 09h30–16h00 ET.
        let timestamp = Utc::now()
            .date_naive()
            .and_hms_opt(3, 0, 0)
            .unwrap()
            .and_utc();
        let ctx = make_context(timestamp);
        let signal = make_signal(Decimal::from(500), Decimal::from(495), Decimal::from(510));
        let state = RiskState::default();

        match manager.validate(&signal, &ctx, None, &state, Decimal::from(100_000), &[]) {
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

        match manager.validate(&signal, &ctx, None, &state, Decimal::from(100_000), &[]) {
            RiskCheck::Rejected(RejectionReason::NotInPaperMode, _) => {}
            other => panic!("esperado rejeição por modo real, obtido {:?}", other),
        }
    }

    /// Timestamp determinístico dentro do horário de negociação.
    /// 15h UTC é 10h ou 11h ET conforme o DST — dentro de 09h30–16h00 nos dois.
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
        match manager.validate(&signal, &ctx, None, &state, Decimal::from(10_000), &[]) {
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
        match manager.validate(&signal, &ctx, None, &state, Decimal::from(400), &[]) {
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
        match manager.validate(&signal, &ctx, None, &state, Decimal::from(100_000), &[]) {
            RiskCheck::Approved { position_size, .. } => {
                assert_eq!(position_size, Decimal::from(200));
            }
            other => panic!("esperado aprovado sem cap, obtido {:?}", other),
        }
    }
}
