//! Engine de backtest determinístico.

use chrono::{DateTime, Timelike, Utc};
use rust_decimal::Decimal;
use tracing::{debug, info, warn};

use trader_adapters::simulated::{CommissionModel, SimulatedBroker, SimulatedBrokerConfig};
use trader_core::{
    context::MarketContextAnalyzer,
    execution::time_exit::{TimeExitConfig, TimeExitTracker},
    execution::{ExecutionEngine, ExecutionResult},
    risk::{RiskConfig, RiskManager, RiskState},
    session::{et_date, et_time},
};
use trader_domain::{Broker, Candle, ExitReason, SignalResult, Strategy, TradingMode};

/// Configuração de uma execução de backtest.
#[derive(Debug, Clone)]
pub struct BacktestConfig {
    /// Símbolo do ativo.
    pub symbol: String,
    /// Capital inicial.
    pub initial_capital: Decimal,
    /// Modelo de comissão. O padrão é a tabela real da IBKR (por ação);
    /// `CommissionModel::PerTrade` reproduz os runs anteriores a 08/09/2026,
    /// que cobravam US$ 0,35 fixos por perna.
    pub commission: CommissionModel,
    /// Slippage percentual aplicado no preço de execução a MERCADO.
    pub slippage_pct: Decimal,
    /// Desconto aplicado no fill do alvo (ordem limite), como fração do preço.
    ///
    /// Existe porque tocar o nível não é encher nele (§5.6 do plano); ver o
    /// campo homônimo em `SimulatedBrokerConfig`.
    pub limit_fill_haircut_pct: Decimal,
    /// Candles de validade da entrada stop aguardando o rompimento.
    pub entry_validity_candles: u32,
    /// Saída ativa por tempo (validação pós-entrada em R), quando a
    /// estratégia a habilita. Mesma lógica do live (paridade).
    pub time_exit: Option<TimeExitConfig>,
    /// Flatten de fim de pregão (ADR-018): `Some((hora, minuto))` em horário
    /// de NY habilita o encerramento na última barra do pregão; `None`
    /// (flag `--no-flatten`) reproduz os runs anteriores ao ADR.
    ///
    /// O par NÃO é o gatilho: a última barra é detectada por **mudança de
    /// data ET**, único critério que funciona nos pregões de meio expediente
    /// (4 dias da amostra terminam às 12h45 ET). O horário aqui é a barra de
    /// fechamento esperada e serve de checagem de sanidade — se o pregão
    /// terminar depois dela, os candles não são RTH-only e o log avisa.
    pub session_flatten_et: Option<(u32, u32)>,
}

impl Default for BacktestConfig {
    fn default() -> Self {
        Self {
            symbol: "SPY".to_string(),
            initial_capital: Decimal::from(100_000),
            commission: CommissionModel::ibkr_fixed_us(),
            // 2 bp (0,02%). Calibrado, nao chutado: os pares operados sao
            // ETFs cotados entre $120 e $435, onde 1 centavo de spread vale
            // 0,23 a 0,83 bp. 2 bp cobre cerca de um spread cheio nos nomes
            // mais caros de negociar (AVUV, SLYV) e e conservador nos demais.
            // O valor antigo, 0,1%, seria de 12 a 43 centavos por execucao —
            // irreal para estes ativos, e sozinho levava o portfolio de
            // +33k para -40k em 18 meses.
            slippage_pct: Decimal::from(2) / Decimal::from(10_000),
            limit_fill_haircut_pct: Decimal::from(2) / Decimal::from(10_000),
            entry_validity_candles: 1,
            time_exit: None,
            // 15h45 ET: a última barra de 15 min do RTH. O live encerra a
            // mercado às 15h55 (ADR-018); a diferença de 10 min e de tipo de
            // fill está registrada no ADR como assimetria conhecida.
            session_flatten_et: Some((15, 45)),
        }
    }
}

/// Resultado bruto de uma execução de backtest.
#[derive(Debug, Clone)]
pub struct BacktestRun {
    pub symbol: String,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub initial_capital: Decimal,
    pub final_equity: Decimal,
    pub total_trades: usize,
    pub closed_trades: Vec<trader_domain::Trade>,
    pub daily_pnl_series: Vec<(DateTime<Utc>, Decimal)>,
}

/// Engine de backtest.
#[derive(Debug, Clone)]
pub struct BacktestEngine {
    config: BacktestConfig,
    broker: SimulatedBroker,
    execution_engine: ExecutionEngine,
    risk_state: RiskState,
    /// Dia UTC corrente — o estado de risco é diário e reinicia a cada
    /// virada de dia (paridade com o live, que reconstrói os limites do
    /// banco a cada dia).
    current_day: Option<chrono::NaiveDate>,
    time_exit: Option<TimeExitTracker>,
    closed_trades: Vec<trader_domain::Trade>,
    daily_equity: Vec<(DateTime<Utc>, Decimal)>,
}

impl BacktestEngine {
    /// Cria uma nova engine de backtest.
    pub fn new(config: BacktestConfig, risk_config: RiskConfig) -> Self {
        let broker = SimulatedBroker::new(SimulatedBrokerConfig {
            account_id: Some("BACKTEST".to_string()),
            initial_cash: config.initial_capital,
            commission: config.commission.clone(),
            slippage_pct: config.slippage_pct,
            limit_fill_haircut_pct: config.limit_fill_haircut_pct,
            entry_validity_candles: config.entry_validity_candles,
            // Paridade com o live: a mesma tolerância de overshoot (ADR-015)
            // governa o cancelamento de entradas por gap além do gatilho.
            entry_overshoot_tolerance: risk_config.entry_overshoot_tolerance,
        });

        let risk_manager = RiskManager::new(risk_config);
        let execution_engine = ExecutionEngine::new(risk_manager);
        let time_exit = config.time_exit.map(TimeExitTracker::new);

        Self {
            config,
            broker,
            execution_engine,
            risk_state: RiskState::default(),
            current_day: None,
            time_exit,
            closed_trades: Vec::new(),
            daily_equity: Vec::new(),
        }
    }

    /// Executa o backtest sobre uma série de candles.
    ///
    /// A estratégia recebe apenas candles até o índice atual, evitando
    /// lookahead bias.
    pub async fn run<S: Strategy>(
        &mut self,
        strategy: &S,
        candles: &[Candle],
    ) -> anyhow::Result<BacktestRun> {
        if candles.is_empty() {
            anyhow::bail!("série de candles vazia");
        }

        let symbol = self.config.symbol.clone();
        let analyzer =
            MarketContextAnalyzer::new(trader_core::context::ContextAnalyzerConfig::default());

        let start_time = candles
            .first()
            .map(|c| c.timestamp)
            .unwrap_or_else(Utc::now);
        let end_time = candles.last().map(|c| c.timestamp).unwrap_or_else(Utc::now);

        for (idx, candle) in candles.iter().enumerate() {
            // Rollover diário: limites (perda diária, trades/dia, perdas
            // consecutivas) valem por dia, não para o backtest inteiro.
            let day = candle.timestamp.date_naive();
            if self.current_day != Some(day) {
                self.current_day = Some(day);
                self.risk_state = RiskState::default();
            }

            // Atualiza mercado com o candle completo: stops/alvos são
            // avaliados nos extremos intrabar (high/low), não só no close.
            self.broker.set_market_candle(&symbol, candle);

            // Saída ativa por tempo (quando a estratégia habilita): avaliada
            // no fechamento de cada candle com posição aberta. O trade
            // fechado aqui entra no sync logo abaixo, junto com stop/alvo.
            self.evaluate_time_exit(&symbol, candle).await;

            // Flatten de fim de pregão (ADR-018): o live cancela a entrada
            // pendente e fecha a posição a mercado no sino, porque as pernas
            // do bracket vão com TIF Day. Sem isto o backtest carrega posição
            // pela noite — edge que o live, por construção, nunca captura.
            if self.is_last_bar_of_session(candles, idx) {
                self.flatten_session(&symbol, candle).await;
            }

            // Registra equity no fechamento de cada candle para série diária.
            if let Ok(summary) = self.broker.get_account_summary().await {
                self.daily_equity.push((candle.timestamp, summary.equity));
            }

            // Sincroniza trades fechados antes de avaliar novo sinal.
            let newly_closed = self.sync_closed_trades();
            self.closed_trades.extend(newly_closed);

            // Se já houver posição aberta, não busca novo sinal.
            let has_position = self
                .broker
                .get_position(&symbol)
                .await
                .map(|p| p.is_some())
                .unwrap_or(false);

            if has_position {
                debug!(idx, "posição aberta; pulando análise de sinal");
                continue;
            }

            // Série histórica até o candle atual (inclusive).
            let history = &candles[..=idx];

            let ctx = match analyzer.analyze(&symbol, candle.timeframe, history) {
                Some(ctx) => ctx,
                None => continue,
            };

            match strategy.analyze(&ctx, &Default::default(), history) {
                SignalResult::Signal(signal) => {
                    let capital = self
                        .broker
                        .get_account_summary()
                        .await
                        .map(|s| s.equity)
                        .unwrap_or(self.config.initial_capital);

                    let result = self
                        .execution_engine
                        .process_signal(
                            &self.broker,
                            &signal,
                            &ctx,
                            None,
                            // Preço mais fresco que o backtest conhece no
                            // momento do envio: o close da barra do sinal.
                            Some(candle.close),
                            &self.risk_state,
                            capital,
                        )
                        .await;

                    match result {
                        ExecutionResult::Executed {
                            order_id,
                            position_size,
                            ..
                        } => {
                            // Contagem de trades/dia na entrada (paridade com
                            // o loop live em paper.rs).
                            self.risk_state.daily_trades += 1;
                            info!(
                                idx,
                                %order_id,
                                %position_size,
                                entry = ?signal.entry_price,
                                stop = ?signal.stop_price,
                                target = ?signal.target_price,
                                "entrada executada no backtest"
                            );
                        }
                        ExecutionResult::RejectedByRisk { reason, detail } => {
                            debug!(?reason, %detail, "sinal rejeitado pelo risk manager no backtest");
                        }
                        ExecutionResult::RejectedByBroker { error } => {
                            warn!(%error, "broker simulado rejeitou ordem no backtest");
                        }
                    }
                }
                SignalResult::Rejected { reason, details } => {
                    debug!(?reason, ?details, "setup rejeitado no backtest");
                }
                _ => {}
            }

            // A entrada que o sinal DESTA barra acabou de colocar não pode
            // atravessar a noite: no live ela vai com TIF Day e morre no sino
            // (o `flatten_session` das 15h55 a cancela). O sinal não é
            // bloqueado — ele é gerado, logado e contado; o que morre é a
            // ordem, como no live.
            if self.is_last_bar_of_session(candles, idx) {
                self.cancel_pending_entry(&symbol).await;
            }
        }

        // Sincroniza trades fechados no último candle.
        let newly_closed = self.sync_closed_trades();
        self.closed_trades.extend(newly_closed);

        let summary = self.broker.get_account_summary().await?;
        let mut closed_trades = self.closed_trades.clone();
        closed_trades.extend(self.broker.get_closed_trades());

        Ok(BacktestRun {
            symbol,
            start_time,
            end_time,
            initial_capital: self.config.initial_capital,
            final_equity: summary.equity,
            total_trades: closed_trades.len(),
            closed_trades,
            daily_pnl_series: self.daily_equity.clone(),
        })
    }

    fn sync_closed_trades(&mut self) -> Vec<trader_domain::Trade> {
        let trades = self.broker.get_closed_trades();
        if trades.is_empty() {
            return Vec::new();
        }

        let pnls: Vec<Decimal> = trades.iter().map(|t| t.net_pnl).collect();
        self.execution_engine
            .sync_risk_state(&mut self.risk_state, &pnls);
        self.broker.clear_closed_trades();
        trades
    }

    /// `true` quando o candle de índice `idx` é a última barra do pregão —
    /// o instante em que o live faz o flatten (ADR-018).
    ///
    /// O critério é **mudança de data ET**, não o relógio. Nos pregões de
    /// meio expediente (03/07/2025, 28/11/2025, 24/12/2025, 07/08/2026 na
    /// amostra) o dia termina às 12h45 ET, e um critério por horário deixaria
    /// a posição atravessar a noite exatamente nos dias em que o live a
    /// fecha. O horário de `session_flatten_et` entra só como checagem de
    /// sanidade: se o pregão terminar depois dele, os candles não são
    /// RTH-only e o flatten está caindo na barra errada.
    ///
    /// O **fim da série NÃO conta** como fim de pregão. O motor não tem como
    /// saber se a série acabou porque o pregão acabou ou porque alguém a
    /// cortou: o walk-forward roda cada janela sobre um prefixo
    /// (`&candles[..test_range.end]`, `walkforward.rs`) que termina num índice
    /// arbitrário, quase sempre no meio de um pregão. Tratar isso como sino
    /// criaria um `EndOfDay` fantasma na fronteira de cada janela — um trade
    /// que não existe no run completo e que entraria nas métricas do gate.
    ///
    /// O preço dessa escolha é que uma posição ainda aberta na última barra
    /// da série nunca fecha e não entra em `closed_trades`. É exatamente o
    /// que a re-simulação dos críticos faz (só fecha posição cuja saída cai
    /// em outra data ET), então os números continuam comparáveis.
    fn is_last_bar_of_session(&self, candles: &[Candle], idx: usize) -> bool {
        let Some((hour, minute)) = self.config.session_flatten_et else {
            return false;
        };

        let current = candles[idx].timestamp;
        let last_of_day = match candles.get(idx + 1) {
            Some(next) => et_date(next.timestamp) != et_date(current),
            None => false,
        };

        if last_of_day {
            let et = et_time(current);
            if (et.hour(), et.minute()) > (hour, minute) {
                warn!(
                    bar = %current,
                    et = %et,
                    esperado = %format!("{hour:02}:{minute:02}"),
                    "última barra do pregão depois do fechamento esperado: \
                     a série não parece RTH-only e o flatten pode estar na barra errada"
                );
            }
        }

        last_of_day
    }

    /// Encerramento de fim de pregão: cancela a entrada pendente e fecha a
    /// posição a mercado no fechamento da barra (ADR-018).
    ///
    /// Espelha `flatten_session` do live (`paper.rs`), que existe porque as
    /// pernas do bracket vão com TIF Day e expiram no sino — posição que
    /// atravessa a noite fica sem stop.
    async fn flatten_session(&mut self, symbol: &str, candle: &Candle) {
        self.cancel_pending_entry(symbol).await;

        // `close_position_at_market` aplica o slippage de execução a mercado
        // contra o trader, modelando o MKT das 15h55.
        if self
            .broker
            .close_position_at_market(symbol, candle.close, ExitReason::EndOfDay)
        {
            info!(
                bar = %candle.timestamp,
                close = %candle.close,
                "flatten de fim de pregão: posição encerrada a mercado"
            );
            // A saída por tempo acompanha a posição; sem reset ela seguiria
            // rastreando um trade que não existe mais.
            if let Some(tracker) = self.time_exit.as_mut() {
                tracker.reset();
            }
        }
    }

    /// Cancela a entrada stop pendente do símbolo, se houver.
    ///
    /// Passa pelo `Broker::cancel_order` — o mesmo caminho do live — para a
    /// ordem terminar como `Cancelled` em vez de sumir do estado.
    async fn cancel_pending_entry(&mut self, symbol: &str) {
        let Some(order_id) = self.broker.pending_entry_order_id(symbol) else {
            return;
        };
        match self.broker.cancel_order(&order_id).await {
            Ok(()) => debug!(%order_id, "entrada pendente cancelada no fim do pregão"),
            Err(e) => warn!(%order_id, error = %e, "falha ao cancelar entrada pendente"),
        }
    }

    /// Avalia a saída por tempo no fechamento do candle atual.
    ///
    /// Acompanha a posição aberta (entrada/stop reais do fill) e encerra a
    /// mercado quando a janela de validação se esgota sem o lucro mínimo em
    /// R. Sem posição aberta, o tracker é reiniciado para o próximo trade.
    async fn evaluate_time_exit(&mut self, symbol: &str, candle: &Candle) {
        let Some(tracker) = self.time_exit.as_mut() else {
            return;
        };

        let position = match self.broker.get_position(symbol).await {
            Ok(position) => position,
            Err(e) => {
                warn!(error = %e, "falha ao consultar posição para saída por tempo");
                None
            }
        };

        match position {
            Some(position) => {
                tracker.ensure_tracking(
                    position.avg_entry_price,
                    position.stop_price,
                    position.direction,
                );
                if tracker.on_candle_close(candle.close)
                    && self
                        .broker
                        .close_position_at_market(symbol, candle.close, ExitReason::Time)
                {
                    info!(
                        idx_ts = %candle.timestamp,
                        close = %candle.close,
                        "saída por tempo: posição encerrada a mercado no fechamento"
                    );
                    tracker.reset();
                }
            }
            None => tracker.reset(),
        }
    }
}

/// Constrói uma configuração de risco padrão para backtest.
pub fn default_backtest_risk_config() -> RiskConfig {
    RiskConfig {
        trading_mode: TradingMode::Paper,
        risk_per_trade_pct: Decimal::from(1),
        max_daily_loss_pct: Decimal::from(2),
        max_trades_per_day: 100, // ilimitado para backtest
        max_consecutive_losses: 100,
        min_risk_reward: Decimal::from(2),
        max_spread_pct: Decimal::from(5) / Decimal::from(10000),
        max_atr_pct: Decimal::from(15) / Decimal::from(10),
        trading_start_time_et: (0, 0, 0),
        trading_end_time_et: (23, 59, 59),
        entry_overshoot_tolerance: Decimal::from(25) / Decimal::from(100),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};
    use rust_decimal::Decimal;
    use trader_core::strategies::pullback_trend_v1::PullbackTrendV1;
    use trader_domain::{Candle, TimeFrame};

    fn candle(
        symbol: &str,
        timestamp: chrono::DateTime<Utc>,
        open: Decimal,
        high: Decimal,
        low: Decimal,
        close: Decimal,
    ) -> Candle {
        Candle::new(
            symbol,
            TimeFrame::M5,
            timestamp,
            open,
            high,
            low,
            close,
            Decimal::from(1000),
        )
        .expect("candle válido")
    }

    fn generate_test_series(symbol: &str) -> Vec<Candle> {
        let base = Utc
            .with_ymd_and_hms(2026, 7, 2, 14, 30, 0)
            .single()
            .unwrap();
        let mut candles = Vec::new();

        // Tendência de alta.
        for i in 0..60 {
            let close = Decimal::from(400 + i);
            candles.push(candle(
                symbol,
                base + chrono::Duration::minutes(i as i64 * 5),
                close - Decimal::ONE,
                close + Decimal::ONE,
                close - Decimal::ONE,
                close,
            ));
        }

        // Nova máxima + pullback + barra de sinal.
        let last = candles.last().unwrap().timestamp;
        candles.push(candle(
            symbol,
            last + chrono::Duration::minutes(5),
            Decimal::from(459),
            Decimal::from(461),
            Decimal::from(458),
            Decimal::from(460),
        ));
        candles.push(candle(
            symbol,
            last + chrono::Duration::minutes(10),
            Decimal::from(460),
            Decimal::from(460),
            Decimal::from(456),
            Decimal::from(456),
        ));
        candles.push(candle(
            symbol,
            last + chrono::Duration::minutes(15),
            Decimal::from(456),
            Decimal::from(457),
            Decimal::from(455),
            Decimal::from(455),
        ));
        candles.push(candle(
            symbol,
            last + chrono::Duration::minutes(20),
            Decimal::from(457),
            Decimal::from(460),
            Decimal::from(456),
            Decimal::from(459),
        ));

        // Continuação até atingir alvo.
        let mut last_close = Decimal::from(459);
        for i in 1..=20 {
            last_close += Decimal::ONE;
            candles.push(candle(
                symbol,
                last + chrono::Duration::minutes((20 + i * 5) as i64),
                last_close - Decimal::ONE,
                last_close + Decimal::ONE,
                last_close - Decimal::ONE,
                last_close,
            ));
        }

        candles
    }

    #[tokio::test]
    async fn backtest_closes_trade_on_take_profit() {
        let candles = generate_test_series("SPY");
        let strategy = PullbackTrendV1::new(
            trader_core::strategies::pullback_trend_v1::config::PullbackTrendV1Config::default(),
        );
        let mut engine =
            BacktestEngine::new(BacktestConfig::default(), default_backtest_risk_config());

        let run = engine.run(&strategy, &candles).await.unwrap();

        assert!(
            !run.closed_trades.is_empty(),
            "esperado pelo menos um trade fechado"
        );
        assert_eq!(
            run.closed_trades[0].exit_reason,
            trader_domain::ExitReason::Target
        );
    }

    /// Estratégia stub: emite um sinal long a mercado em todo candle, com
    /// stop/alvo simétricos em torno do fechamento atual.
    struct AlwaysSignal;

    impl trader_domain::Strategy for AlwaysSignal {
        fn id(&self) -> trader_domain::StrategyId {
            trader_domain::StrategyId {
                id: "always-signal".to_string(),
                version: "0.0.1".to_string(),
            }
        }
        fn name(&self) -> &'static str {
            "Always Signal"
        }
        fn source(&self) -> &'static str {
            "teste"
        }
        fn version(&self) -> &'static str {
            "0.0.1"
        }
        fn analyze(
            &self,
            _ctx: &trader_domain::MarketContext,
            _state: &trader_domain::StrategyState,
            candles: &[Candle],
        ) -> trader_domain::SignalResult {
            let last = candles.last().expect("histórico não vazio");
            let entry = last.close + Decimal::new(5, 1); // stop 0.5 acima
            trader_domain::SignalResult::Signal(trader_domain::Signal {
                symbol: last.symbol.clone(),
                strategy_id: "always-signal".to_string(),
                strategy_version: "0.0.1".to_string(),
                config_hash: "test".to_string(),
                timeframe: last.timeframe,
                timestamp: last.timestamp,
                direction: trader_domain::Direction::Long,
                status: trader_domain::SignalStatus::Accepted,
                entry_order_type: trader_domain::EntryOrderType::Stop,
                entry_price: Some(entry),
                stop_price: Some(entry - Decimal::from(2)),
                target_price: Some(entry + Decimal::from(4)),
                risk_reward_ratio: Some(Decimal::from(2)),
                risk_amount: None,
                risk_percent: None,
                position_size: None,
                entry_reason: None,
                rejection_reason: None,
                rejection_details: None,
                market_snapshot: serde_json::Value::Object(Default::default()),
                correlation_id: "test".to_string(),
            })
        }
    }

    /// Dois dias de candles (30/dia) em leve tendência de alta (fecha acima
    /// da EMA20 → contexto operável). Cada candle rompe o gatilho na máxima
    /// e estopa na mínima — trade abre e fecha no mesmo candle, liberando
    /// novo sinal no candle seguinte.
    fn two_day_range_series(symbol: &str) -> Vec<Candle> {
        let mut candles = Vec::new();
        for day in 0..2i64 {
            let base = Utc
                .with_ymd_and_hms(2026, 7, 2, 14, 30, 0)
                .single()
                .unwrap()
                + chrono::Duration::days(day);
            for i in 0..30 {
                let n = day * 30 + i as i64;
                let price = Decimal::from(500) + Decimal::new(5 * n, 2);
                candles.push(candle(
                    symbol,
                    base + chrono::Duration::minutes(i as i64 * 15),
                    price,
                    price + Decimal::ONE,
                    price - Decimal::from(2),
                    price,
                ));
            }
        }
        candles
    }

    /// Regressão de paridade live/backtest: os limites de risco (trades/dia,
    /// perdas consecutivas, perda diária) valem POR DIA. Antes da correção,
    /// o estado acumulava para o backtest inteiro e 3 perdas consecutivas
    /// bloqueavam a estratégia para sempre.
    #[tokio::test]
    async fn risk_limits_reset_daily() {
        let candles = two_day_range_series("SPY");
        let risk = RiskConfig {
            max_trades_per_day: 1,
            ..default_backtest_risk_config()
        };
        let mut engine = BacktestEngine::new(BacktestConfig::default(), risk);

        let run = engine.run(&AlwaysSignal, &candles).await.unwrap();

        let day1 = Utc.with_ymd_and_hms(2026, 7, 2, 0, 0, 0).unwrap();
        let trades_d1 = run
            .closed_trades
            .iter()
            .filter(|t| t.entry_time.date_naive() == day1.date_naive())
            .count();
        let trades_d2 = run.closed_trades.len() - trades_d1;

        assert_eq!(trades_d1, 1, "limite de 1 trade/dia no dia 1");
        assert_eq!(
            trades_d2, 1,
            "dia 2 deve operar de novo: limites resetam na virada do dia"
        );
    }

    // ---------------------------------------------------------------------
    // ADR-018 — flatten de fim de pregão
    // ---------------------------------------------------------------------

    /// Série RTH realista: `days` pregões de 26 barras de 15 min, 09h30 →
    /// 15h45 ET (13h30 → 19h45 UTC no horário de verão).
    ///
    /// Os preços sobem devagar (contexto de alta, para o sinal passar) e a
    /// mínima nunca alcança o stop nem a máxima o alvo: a posição que abrir
    /// **fica aberta** até alguém fechá-la. Sem flatten ela atravessa a noite;
    /// é exatamente o trade que inflava o backtest antes do ADR-018.
    fn rth_hold_series(symbol: &str, days: i64) -> Vec<Candle> {
        let mut candles = Vec::new();
        for day in 0..days {
            let base = Utc
                .with_ymd_and_hms(2026, 7, 6, 13, 30, 0)
                .single()
                .unwrap()
                + chrono::Duration::days(day);
            for i in 0..26i64 {
                let n = day * 26 + i;
                let price = Decimal::from(500) + Decimal::new(5 * n, 2);
                candles.push(candle(
                    symbol,
                    base + chrono::Duration::minutes(i * 15),
                    price,
                    price + Decimal::ONE, // máxima cobre o gatilho (close + 0,5)
                    price - Decimal::new(1, 1), // mínima longe do stop (entrada − 2)
                    price,
                ));
            }
        }
        candles
    }

    /// O trade que ficaria aberto na virada do dia é encerrado na última barra
    /// do pregão, com motivo próprio. É o achado 1 de §2.3 do plano: sem isto
    /// o backtest ganha dinheiro dormindo posicionado e o gate A mede outra
    /// coisa que não o live.
    #[tokio::test]
    async fn posicao_aberta_no_fim_do_pregao_fecha_como_end_of_day() {
        let candles = rth_hold_series("SPY", 2);
        let mut engine =
            BacktestEngine::new(BacktestConfig::default(), default_backtest_risk_config());

        let run = engine.run(&AlwaysSignal, &candles).await.unwrap();

        let eod: Vec<_> = run
            .closed_trades
            .iter()
            .filter(|t| t.exit_reason == ExitReason::EndOfDay)
            .collect();
        assert!(
            !eod.is_empty(),
            "esperado ao menos um encerramento de fim de pregão; saídas: {:?}",
            run.closed_trades
                .iter()
                .map(|t| t.exit_reason)
                .collect::<Vec<_>>()
        );

        // Nenhum trade atravessa a noite: entrada e saída no mesmo pregão.
        for trade in &run.closed_trades {
            assert_eq!(
                et_date(trade.entry_time),
                et_date(trade.exit_time),
                "trade de {} a {} atravessou a noite com o flatten ligado",
                trade.entry_time,
                trade.exit_time
            );
        }
    }

    /// `--no-flatten` reproduz a régua antiga — é o que permite medir o delta
    /// por par contra os runs 413–421 (ADR-018, "Como aplicar").
    #[tokio::test]
    async fn sem_flatten_a_posicao_atravessa_a_noite() {
        let candles = rth_hold_series("SPY", 2);
        let config = BacktestConfig {
            session_flatten_et: None,
            ..BacktestConfig::default()
        };
        let mut engine = BacktestEngine::new(config, default_backtest_risk_config());

        let run = engine.run(&AlwaysSignal, &candles).await.unwrap();

        assert!(
            run.closed_trades
                .iter()
                .all(|t| t.exit_reason != ExitReason::EndOfDay),
            "sem flatten não pode existir saída end_of_day"
        );
        // A posição do dia 1 segue aberta na virada e nunca fecha: a régua
        // antiga simplesmente não registra o trade.
        assert!(
            run.closed_trades.is_empty(),
            "esperado nenhum trade fechado sem flatten, veio {}",
            run.closed_trades.len()
        );
    }

    /// Meio expediente: o pregão acaba às 13h00 ET (última barra 12h45). Um
    /// critério por horário (\">= 15h45\") deixaria a posição atravessar a
    /// noite justamente nos 4 dias em que o live a fecha — por isso o gatilho
    /// é a mudança de data ET.
    #[tokio::test]
    async fn meio_expediente_tambem_faz_flatten() {
        let mut candles = rth_hold_series("SPY", 1);
        // Segundo dia com apenas 14 barras: 09h30 → 12h45 ET.
        let base = Utc
            .with_ymd_and_hms(2026, 7, 7, 13, 30, 0)
            .single()
            .unwrap();
        for i in 0..14i64 {
            let price = Decimal::from(500) + Decimal::new(5 * (26 + i), 2);
            candles.push(candle(
                "SPY",
                base + chrono::Duration::minutes(i * 15),
                price,
                price + Decimal::ONE,
                price - Decimal::new(1, 1),
                price,
            ));
        }
        // Um terceiro pregão normal: o flatten do dia curto é disparado pela
        // MUDANÇA DE DATA ET, e o fim da série não conta como sino.
        let dia3 = Utc
            .with_ymd_and_hms(2026, 7, 8, 13, 30, 0)
            .single()
            .unwrap();
        for i in 0..26i64 {
            let price = Decimal::from(500) + Decimal::new(5 * (40 + i), 2);
            candles.push(candle(
                "SPY",
                dia3 + chrono::Duration::minutes(i * 15),
                price,
                price + Decimal::ONE,
                price - Decimal::new(1, 1),
                price,
            ));
        }

        let mut engine =
            BacktestEngine::new(BacktestConfig::default(), default_backtest_risk_config());
        let run = engine.run(&AlwaysSignal, &candles).await.unwrap();

        let dia_curto = chrono::NaiveDate::from_ymd_opt(2026, 7, 7).unwrap();
        assert!(
            run.closed_trades.iter().any(|t| {
                t.exit_reason == ExitReason::EndOfDay && et_date(t.exit_time) == dia_curto
            }),
            "o pregão de meio expediente também tem de flattenar; saídas: {:?}",
            run.closed_trades
                .iter()
                .map(|t| (et_date(t.exit_time), t.exit_reason))
                .collect::<Vec<_>>()
        );
    }

    /// Regressão do artefato de walk-forward: cada janela roda sobre um
    /// PREFIXO da série, que termina num índice arbitrário. Se o fim da série
    /// contasse como sino, a fronteira de cada janela geraria um `EndOfDay`
    /// fantasma — um trade a mais nas métricas do gate, ausente no run
    /// completo.
    #[tokio::test]
    async fn fim_da_serie_no_meio_do_pregao_nao_e_sino() {
        let candles = rth_hold_series("SPY", 2);
        // Corta no meio do 2º pregão, como faz `run_walk_forward`.
        let prefixo = &candles[..26 + 13];
        assert_ne!(
            et_time(prefixo.last().unwrap().timestamp),
            et_time(candles[25].timestamp),
            "o corte tem de cair no meio do pregão para o teste valer"
        );

        let mut engine =
            BacktestEngine::new(BacktestConfig::default(), default_backtest_risk_config());
        let run = engine.run(&AlwaysSignal, prefixo).await.unwrap();

        // O único flatten legítimo é o do dia 1 (mudança de data ET).
        let eod = run
            .closed_trades
            .iter()
            .filter(|t| t.exit_reason == ExitReason::EndOfDay)
            .count();
        assert_eq!(
            eod,
            1,
            "esperado só o flatten do dia 1; saídas: {:?}",
            run.closed_trades
                .iter()
                .map(|t| (et_date(t.exit_time), t.exit_reason))
                .collect::<Vec<_>>()
        );
    }

    /// ADR-019 §5: o trade do backtest tem de nascer com a mesma identidade
    /// que o live grava. Antes disto ele saía com `strategy_id = "unknown"` e
    /// `journal = {}`, e qualquer corte por bucket (PF por distância de stop,
    /// por tipo de dia) exigia sair do motor para um script com outra régua.
    #[tokio::test]
    async fn trade_do_backtest_carrega_identidade_e_snapshot() {
        let candles = rth_hold_series("SPY", 2);
        let mut engine =
            BacktestEngine::new(BacktestConfig::default(), default_backtest_risk_config());
        let run = engine.run(&AlwaysSignal, &candles).await.unwrap();

        let trade = run
            .closed_trades
            .first()
            .expect("a série gera ao menos um trade");
        assert_eq!(trade.strategy_id, "always-signal");
        assert_eq!(trade.strategy_version, "0.0.1");
        assert_eq!(trade.config_hash, "test");
        assert_eq!(
            trade.journal.get("source").and_then(|v| v.as_str()),
            Some("simulated_broker")
        );
        assert!(
            trade.journal.get("market_snapshot").is_some(),
            "o snapshot do sinal tem de chegar ao journal; veio {}",
            trade.journal
        );
    }

    /// A entrada stop que não encheu até o sino é cancelada — no live ela vai
    /// com TIF Day e morre lá. Se sobrevivesse, encheria na abertura do dia
    /// seguinte, num preço que o live nunca veria.
    #[tokio::test]
    async fn entrada_pendente_e_cancelada_no_fim_do_pregao() {
        // Série em QUEDA lenta: dá contexto operável (tendência definida, o
        // que `is_tradeable` exige) e garante que a máxima nunca alcança o
        // gatilho de um stop de compra colocado acima — a entrada fica
        // pendente o dia inteiro, que é o caso que o flatten tem de tratar.
        let mut candles = Vec::new();
        let base = Utc
            .with_ymd_and_hms(2026, 7, 6, 13, 30, 0)
            .single()
            .unwrap();
        for day in 0..2i64 {
            for i in 0..26i64 {
                let n = day * 26 + i;
                let price = Decimal::from(500) - Decimal::new(5 * n, 2);
                candles.push(candle(
                    "SPY",
                    base + chrono::Duration::days(day) + chrono::Duration::minutes(i * 15),
                    price,
                    price + Decimal::new(2, 1), // < gatilho (close + 0,5)
                    price - Decimal::new(3, 1),
                    price,
                ));
            }
        }

        let config = BacktestConfig {
            // Validade longa: sem isto a entrada expira por contagem de
            // candles e o teste mediria a expiração, não o flatten.
            entry_validity_candles: 100,
            ..BacktestConfig::default()
        };
        let mut engine = BacktestEngine::new(config, default_backtest_risk_config());
        // Prefixo até a 1ª barra do dia 2: a fronteira de data ET no meio.
        engine.run(&AlwaysSignal, &candles[..27]).await.unwrap();

        // O simulado rejeita nova entrada enquanto houver uma pendente no
        // mesmo símbolo, e numera as ordens em sequência a partir de zero
        // (`sim-<nanos>-<n>`). A entrada do dia 1 é a PRIMEIRA ordem do run,
        // `-0`. Se ela tivesse sobrevivido ao sino, o sinal do dia 2 teria
        // sido recusado e a pendente ainda seria a `-0`.
        let pendente = engine
            .broker
            .pending_entry_order_id("SPY")
            .expect("o dia 2 tem de conseguir colocar a própria entrada")
            .to_string();
        assert!(
            !pendente.ends_with("-0"),
            "a entrada pendente ainda é a primeira ordem do run ({pendente}): \
             a entrada do dia 1 atravessou a noite em vez de morrer no sino"
        );
    }
}
