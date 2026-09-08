//! Broker simulado para testes, desenvolvimento e backtest.

use async_trait::async_trait;
use chrono::Utc;
use rust_decimal::Decimal;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc::Sender;
use tracing::{info, warn};

use trader_domain::market::{OrderEvent, SubscriptionHandle};
use trader_domain::{
    AccountSummary, Broker, BrokerError, Direction, EntryOrderType, ExitReason, Fill, Order,
    OrderId, OrderSide, OrderStatus, OrderType, Position, Trade,
};

/// Como a comissão de uma execução é calculada.
///
/// Até 08/09/2026 o simulador cobrava US$ 0,35 fixos por perna, valor que não
/// corresponde a nenhuma tabela real: a IBKR cobra **por ação**. Medido sobre
/// os 214 trades OOS das oito combinações vivas (quantidades de 234 a 1.311
/// ações, mediana 619), a comissão total vai de US$ 149,80 para US$ 1.489,36 —
/// **9,9×** —, ou de US$ 0,70 para US$ 2,34–13,11 por trade (mediana US$ 6,19).
/// Não é a "ordem de grandeza" que uma versão anterior deste comentário
/// afirmava a partir de um único trade de 1.262 ações; é um fator de dez, e o
/// custo passa a ser da mesma escala do risco orçado (§5.6 do plano).
#[derive(Debug, Clone, PartialEq)]
pub enum CommissionModel {
    /// Valor fixo por perna. Continua aqui para **reproduzir** os runs
    /// anteriores a 08/09/2026, não porque descreva alguma corretora.
    PerTrade(Decimal),
    /// Tabela Fixed da IBKR para ações e ETFs dos EUA.
    PerShare {
        /// US$ por ação.
        per_share: Decimal,
        /// Piso por ordem.
        min_per_order: Decimal,
        /// Teto como fração do valor negociado (1% na IBKR).
        max_pct_of_value: Decimal,
    },
}

impl CommissionModel {
    /// IBKR US Fixed: US$ 0,005/ação, mínimo US$ 1,00, máximo 1% do valor.
    ///
    /// Nenhum dos dois limites morde nos 214 trades OOS medidos — verificado,
    /// não suposto: zero acionamentos em 428 pernas. Mas a margem é menor do
    /// que parece: o teto de 1% só valeria para papel abaixo de US$ 0,50, e o
    /// piso de US$ 1,00 vale para ordem abaixo de 200 ações, contra uma ordem
    /// mínima real de **234** — 17% acima do piso, não uma ordem de grandeza.
    /// O cap de liquidez do ADR-020 vai reduzir tamanho e pode fazer o piso
    /// morder.
    pub fn ibkr_fixed_us() -> Self {
        Self::PerShare {
            per_share: Decimal::from(5) / Decimal::from(1000),
            min_per_order: Decimal::ONE,
            max_pct_of_value: Decimal::ONE / Decimal::from(100),
        }
    }

    /// Sem comissão — para testes que medem outra coisa.
    pub fn none() -> Self {
        Self::PerTrade(Decimal::ZERO)
    }

    /// Comissão de UMA execução (uma perna).
    pub fn for_execution(&self, quantity: Decimal, price: Decimal) -> Decimal {
        if quantity.is_zero() {
            return Decimal::ZERO;
        }
        match self {
            Self::PerTrade(valor) => *valor,
            Self::PerShare {
                per_share,
                min_per_order,
                max_pct_of_value,
            } => {
                let bruta = (*per_share * quantity.abs()).max(*min_per_order);
                let teto = price.abs() * quantity.abs() * *max_pct_of_value;
                if teto > Decimal::ZERO {
                    bruta.min(teto)
                } else {
                    bruta
                }
            }
        }
    }
}

/// Saídas pendentes associadas a uma posição aberta.
#[derive(Debug, Clone)]
struct PendingExit {
    stop_price: Decimal,
    target_price: Decimal,
}

/// Entrada stop pendente, aguardando o rompimento do gatilho.
///
/// Espelha o comportamento de um buy/sell stop no broker real: a ordem
/// trabalha até ser acionada (high/low do candle rompe o gatilho) ou expirar
/// após `entry_validity_candles` candles sem rompimento.
#[derive(Debug, Clone)]
struct PendingEntry {
    trigger_price: Decimal,
    direction: Direction,
    stop_price: Decimal,
    target_price: Decimal,
    quantity: Decimal,
    order_id: OrderId,
    signal_id: i64,
    candles_waiting: u32,
    /// Metadados da ordem de entrada (identidade da estrategia + snapshot do
    /// sinal). Viajam ate a `Position` e dai ate o `Trade.journal` (ADR-019
    /// §5); sem isso o trade que nasce de uma entrada stop e anonimo.
    metadata: serde_json::Value,
}

/// Estado interno do broker simulado.
#[derive(Debug, Clone)]
struct SimulatedState {
    next_order_id: i64,
    orders: HashMap<OrderId, Order>,
    positions: HashMap<String, Position>,
    pending_exits: HashMap<String, PendingExit>,
    pending_entries: HashMap<String, PendingEntry>,
    market_prices: HashMap<String, Decimal>,
    /// Relógio simulado: timestamp do último candle processado. Garante que
    /// trades do backtest carregam o tempo do mercado, não o relógio real —
    /// sem isso, análises por janela (walk-forward) não conseguem separar
    /// trades por período.
    now: chrono::DateTime<Utc>,
    cash: Decimal,
    equity: Decimal,
    buying_power: Decimal,
    daily_pnl: Decimal,
    closed_trades: Vec<Trade>,
}

impl Default for SimulatedState {
    fn default() -> Self {
        Self {
            next_order_id: 0,
            orders: HashMap::new(),
            positions: HashMap::new(),
            pending_exits: HashMap::new(),
            pending_entries: HashMap::new(),
            market_prices: HashMap::new(),
            now: Utc::now(),
            cash: Decimal::ZERO,
            equity: Decimal::ZERO,
            buying_power: Decimal::ZERO,
            daily_pnl: Decimal::ZERO,
            closed_trades: Vec::new(),
        }
    }
}

/// Configuração do broker simulado.
#[derive(Debug, Clone)]
pub struct SimulatedBrokerConfig {
    pub account_id: Option<String>,
    pub initial_cash: Decimal,
    pub commission: CommissionModel,
    /// Custo aplicado no fill de uma ordem LIMITE (o alvo), como fração do
    /// preço.
    ///
    /// Até 08/09/2026 o alvo não pagava **nada**: bastava o candle tocar o
    /// nível para encher no preço exato. Isso é otimista por um motivo que
    /// não é o spread — uma ordem limite parada no book é o lado passivo e
    /// não paga spread — e sim porque **tocar não é encher**: o high do
    /// candle no seu preço quase sempre significa que poucos lotes
    /// negociaram ali, e você está numa fila. Este campo é o desconto
    /// declarado que substitui esse otimismo, não uma cobrança de spread; o
    /// §5.6 do plano o chama de "spread cobrado no alvo".
    ///
    /// Calibração por ativo (§2.2) é passo separado: aqui é um valor único.
    pub limit_fill_haircut_pct: Decimal,
    /// Slippage como FRAÇÃO do preço: `0.001` = 0,1%.
    ///
    /// Já foi lido como "percentual" e dividido por 100 na aplicação, o que
    /// tornava o custo efetivo 0,001% — cem vezes menor do que a configuração
    /// dizia (A4 da auditoria de 30/08/2026). O teste
    /// `slippage_efetivo_e_a_fracao_configurada` trava a semântica.
    pub slippage_pct: Decimal,
    /// Candles de validade de uma entrada stop aguardando o rompimento.
    pub entry_validity_candles: u32,
    /// Tolerância de overshoot na entrada stop (ADR-015), como fração da
    /// distância do stop: se o candle ABRE além do gatilho mais do que isto,
    /// a entrada é cancelada (invalidada) em vez de preenchida — espelha a
    /// guarda pré-envio do live. Dentro da tolerância, o fill acontece no
    /// preço de abertura (gap realista), não no gatilho.
    pub entry_overshoot_tolerance: Decimal,
}

impl Default for SimulatedBrokerConfig {
    fn default() -> Self {
        Self {
            account_id: Some("DU_SIM".to_string()),
            initial_cash: Decimal::from(100_000),
            commission: CommissionModel::ibkr_fixed_us(),
            // Mesmo 2 bp do slippage de mercado. Não é medição: é o mesmo
            // valor, declarado, até a calibração por ativo do §5.6 existir.
            limit_fill_haircut_pct: Decimal::from(2) / Decimal::from(10_000),
            slippage_pct: Decimal::from(2) / Decimal::from(10_000), // 2 bp (0,02%)
            entry_validity_candles: 1,
            entry_overshoot_tolerance: Decimal::from(25) / Decimal::from(100), // 25%
        }
    }
}

/// Broker em memória que imita respostas da Interactive Brokers.
///
/// - Ordens market geram fill imediato.
/// - Ordens bracket criam posição, stop loss e take profit pendentes.
/// - `set_market_price` executa stop/alvo quando o preço é alcançado.
/// - Rejeita nova posição no mesmo ativo se já existir posição aberta.
#[derive(Debug, Clone)]
pub struct SimulatedBroker {
    state: Arc<Mutex<SimulatedState>>,
    config: SimulatedBrokerConfig,
}

impl SimulatedBroker {
    pub fn new(config: SimulatedBrokerConfig) -> Self {
        let initial_cash = config.initial_cash;
        Self {
            state: Arc::new(Mutex::new(SimulatedState {
                cash: initial_cash,
                equity: initial_cash,
                buying_power: initial_cash,
                ..Default::default()
            })),
            config,
        }
    }

    /// Cria um broker simulado com configuração padrão.
    pub fn default_simulated() -> Self {
        Self::new(SimulatedBrokerConfig::default())
    }

    /// Define o preço de mercado atual para um ativo e executa stops/alvos.
    pub fn set_market_price(&self, symbol: &str, price: Decimal) {
        let mut state = match self.state.lock() {
            Ok(guard) => guard,
            Err(e) => {
                warn!(error = %e, "lock envenenado no broker simulado");
                return;
            }
        };

        state.market_prices.insert(symbol.to_string(), price);

        if let Some(position) = state.positions.get_mut(symbol) {
            let direction_multiplier = match position.direction {
                Direction::Long => Decimal::ONE,
                Direction::Short => -Decimal::ONE,
            };
            let quantity = position.quantity;
            position.unrealized_pnl =
                (price - position.avg_entry_price) * quantity * direction_multiplier;
            state.equity = state.cash + price * quantity * direction_multiplier;
        }

        if let Some(exit) = state.pending_exits.get(symbol).cloned() {
            let reason = if price <= exit.stop_price {
                Some(ExitReason::Stop)
            } else if price >= exit.target_price {
                Some(ExitReason::Target)
            } else {
                None
            };

            if let Some(reason) = reason {
                if let Some(position) = state.positions.remove(symbol) {
                    state.pending_exits.remove(symbol);
                    if let Some(trade) =
                        close_position_to_trade(&position, price, reason, &self.config, Utc::now())
                    {
                        let exit_commission = self
                            .config
                            .commission
                            .for_execution(position.quantity, price);
                        state.cash += exit_cash_flow(
                            position.direction,
                            price,
                            position.quantity,
                            exit_commission,
                        );
                        state.daily_pnl += trade.net_pnl;
                        state.equity = state.cash;
                        state.closed_trades.push(trade);
                    }
                }
            }
        }
    }

    /// Atualiza o mercado com um candle completo e executa stops/alvos usando
    /// os extremos intrabar (`high`/`low`), não apenas o fechamento.
    ///
    /// O fill de saída acontece no preço do stop/alvo (sem modelagem de gap).
    /// Se stop e alvo forem ambos atingidos no mesmo candle, assume o pior
    /// caso (stop primeiro) — aproximação conservadora documentada.
    pub fn set_market_candle(&self, symbol: &str, candle: &trader_domain::Candle) {
        let mut state = match self.state.lock() {
            Ok(guard) => guard,
            Err(e) => {
                warn!(error = %e, "lock envenenado no broker simulado");
                return;
            }
        };

        state.market_prices.insert(symbol.to_string(), candle.close);
        state.now = candle.timestamp;

        // Entradas stop pendentes: rompimento do gatilho enche a ordem;
        // validade excedida sem rompimento expira a ordem (regra do livro).
        let (entry_triggered, entry_expired) = match state.pending_entries.get_mut(symbol) {
            Some(entry) => {
                entry.candles_waiting += 1;
                let triggered = match entry.direction {
                    Direction::Long => candle.high >= entry.trigger_price,
                    Direction::Short => candle.low <= entry.trigger_price,
                };
                let expired = !triggered
                    && trader_domain::stop_entry_expired(
                        entry.candles_waiting,
                        self.config.entry_validity_candles,
                    );
                (triggered, expired)
            }
            None => (false, false),
        };

        if entry_triggered || entry_expired {
            let entry = state
                .pending_entries
                .remove(symbol)
                .expect("entrada pendente presente");

            // Overshoot na abertura (ADR-015): se o candle que aciona o
            // gatilho ABRE além dele, o fill real seria no preço de abertura,
            // não no gatilho. Além da tolerância (fração da distância do
            // stop), a entrada é invalidada — espelha a guarda pré-envio do
            // live. Dentro dela, o fill parte da abertura (gap realista; o
            // modelo antigo enchia sempre no gatilho e escondia esse custo).
            let stop_distance = (entry.trigger_price - entry.stop_price).abs();
            let (fill_base, overshoot) = match entry.direction {
                Direction::Long => (
                    candle.open.max(entry.trigger_price),
                    (candle.open - entry.trigger_price).max(Decimal::ZERO),
                ),
                Direction::Short => (
                    candle.open.min(entry.trigger_price),
                    (entry.trigger_price - candle.open).max(Decimal::ZERO),
                ),
            };
            let overshoot_invalidated = entry_triggered
                && stop_distance > Decimal::ZERO
                && overshoot > self.config.entry_overshoot_tolerance * stop_distance;

            if entry_expired {
                if let Some(order) = state.orders.get_mut(&entry.order_id) {
                    order.status = OrderStatus::Expired;
                }
                info!(order_id = %entry.order_id, "entrada stop expirada sem rompimento");
            } else if overshoot_invalidated {
                let now = state.now;
                if let Some(order) = state.orders.get_mut(&entry.order_id) {
                    order.status = OrderStatus::Cancelled;
                    order.cancelled_at = Some(now);
                }
                info!(
                    order_id = %entry.order_id,
                    open = %candle.open,
                    trigger = %entry.trigger_price,
                    %overshoot,
                    "entrada stop invalidada: abertura além do gatilho excede a tolerância de overshoot"
                );
            } else {
                let fill_price =
                    apply_slippage(fill_base, entry.direction, true, self.config.slippage_pct);
                let commission = self
                    .config
                    .commission
                    .for_execution(entry.quantity, fill_price);

                match Position::new(
                    symbol,
                    entry.signal_id,
                    entry.direction,
                    entry.quantity,
                    fill_price,
                    entry.stop_price,
                    "simulated",
                ) {
                    Ok(mut position) => {
                        let now = state.now;
                        position.entry_time = now;
                        position.target_price = Some(entry.target_price);
                        position.metadata = entry.metadata.clone();
                        state.positions.insert(symbol.to_string(), position);
                        state.pending_exits.insert(
                            symbol.to_string(),
                            PendingExit {
                                stop_price: entry.stop_price,
                                target_price: entry.target_price,
                            },
                        );
                        state.cash += entry_cash_flow(
                            entry.direction,
                            fill_price,
                            entry.quantity,
                            commission,
                        );
                        state.equity = mark_to_market_equity(
                            state.cash,
                            entry.direction,
                            fill_price,
                            entry.quantity,
                        );

                        if let Some(order) = state.orders.get_mut(&entry.order_id) {
                            order.status = OrderStatus::Filled;
                            order.filled_quantity = entry.quantity;
                            order.avg_fill_price = Some(fill_price);
                            order.filled_at = Some(now);
                        }
                        info!(order_id = %entry.order_id, "entrada stop acionada");
                    }
                    Err(e) => {
                        warn!(error = %e, "falha ao criar posição de entrada stop");
                    }
                }
            }
        }

        let direction = state.positions.get(symbol).map(|p| p.direction);

        if let Some(position) = state.positions.get_mut(symbol) {
            let direction_multiplier = match position.direction {
                Direction::Long => Decimal::ONE,
                Direction::Short => -Decimal::ONE,
            };
            let quantity = position.quantity;
            position.unrealized_pnl =
                (candle.close - position.avg_entry_price) * quantity * direction_multiplier;
            state.equity = state.cash + candle.close * quantity * direction_multiplier;
        }

        if let Some(exit) = state.pending_exits.get(symbol).cloned() {
            // Pior caso primeiro: stop antes do alvo quando ambos são
            // tocados no mesmo candle.
            // As entradas ganharam modelagem de gap no ADR-015; as saídas
            // enchiam no preço exato do nível, como se o mercado sempre
            // parasse ali (A4 da auditoria). Agora:
            //   - STOP vira ordem a mercado ao ser tocado: se o candle ABRE
            //     além dele, o fill é na abertura (pior), e ainda leva
            //     slippage;
            //   - ALVO é ordem limite: um gap além dele enche na abertura
            //     (melhor — foi o que aconteceu no trade 11, IWO 13/08/2026),
            //     e limite não sofre slippage.
            let hit = match direction {
                Some(Direction::Long) => {
                    if candle.low <= exit.stop_price {
                        let base = candle.open.min(exit.stop_price);
                        Some((
                            apply_slippage(base, Direction::Long, false, self.config.slippage_pct),
                            ExitReason::Stop,
                        ))
                    } else if candle.high >= exit.target_price {
                        Some((
                            apply_slippage(
                                candle.open.max(exit.target_price),
                                Direction::Long,
                                false,
                                self.config.limit_fill_haircut_pct,
                            ),
                            ExitReason::Target,
                        ))
                    } else {
                        None
                    }
                }
                Some(Direction::Short) => {
                    if candle.high >= exit.stop_price {
                        let base = candle.open.max(exit.stop_price);
                        Some((
                            apply_slippage(base, Direction::Short, false, self.config.slippage_pct),
                            ExitReason::Stop,
                        ))
                    } else if candle.low <= exit.target_price {
                        Some((
                            apply_slippage(
                                candle.open.min(exit.target_price),
                                Direction::Short,
                                false,
                                self.config.limit_fill_haircut_pct,
                            ),
                            ExitReason::Target,
                        ))
                    } else {
                        None
                    }
                }
                None => None,
            };

            if let Some((exit_price, reason)) = hit {
                if let Some(position) = state.positions.remove(symbol) {
                    state.pending_exits.remove(symbol);
                    if let Some(trade) = close_position_to_trade(
                        &position,
                        exit_price,
                        reason,
                        &self.config,
                        state.now,
                    ) {
                        // A comissão da SAÍDA, no preço da saída — não metade
                        // do total: com a tabela por ação as duas pernas só
                        // coincidem por acaso.
                        let exit_commission = self
                            .config
                            .commission
                            .for_execution(position.quantity, exit_price);
                        state.cash += exit_cash_flow(
                            position.direction,
                            exit_price,
                            position.quantity,
                            exit_commission,
                        );
                        state.daily_pnl += trade.net_pnl;
                        state.equity = state.cash;
                        state.closed_trades.push(trade);
                    }
                }
            }
        }
    }

    /// Encerra a posição aberta a mercado no preço informado (saída ativa —
    /// ex.: saída por tempo por falta de validação em R).
    ///
    /// Espelha o caminho de stop/alvo: remove a posição e as saídas
    /// pendentes, registra o `Trade` fechado com o motivo informado.
    /// Retorna `false` quando não há posição aberta no símbolo.
    pub fn close_position_at_market(
        &self,
        symbol: &str,
        price: Decimal,
        reason: ExitReason,
    ) -> bool {
        let mut state = match self.state.lock() {
            Ok(guard) => guard,
            Err(e) => {
                warn!(error = %e, "lock envenenado no broker simulado");
                return false;
            }
        };

        state.market_prices.insert(symbol.to_string(), price);

        let Some(position) = state.positions.remove(symbol) else {
            return false;
        };
        state.pending_exits.remove(symbol);

        // Saída ativa é execução a mercado: escorrega contra o trader, como
        // a entrada e o stop (A4 da auditoria).
        let price = apply_slippage(price, position.direction, false, self.config.slippage_pct);

        if let Some(trade) =
            close_position_to_trade(&position, price, reason, &self.config, state.now)
        {
            // Recebe o valor da venda menos a comissão de saída, calculada
            // no preço da saída (não metade do total do trade).
            let exit_commission = self
                .config
                .commission
                .for_execution(position.quantity, price);
            state.cash += exit_cash_flow(
                position.direction,
                price,
                position.quantity,
                exit_commission,
            );
            state.daily_pnl += trade.net_pnl;
            state.equity = state.cash;
            state.closed_trades.push(trade);
        }
        info!(%symbol, %price, ?reason, "posição encerrada a mercado (saída ativa)");
        true
    }

    /// Id da ordem de entrada stop pendente no símbolo, se houver.
    ///
    /// Existe para o flatten de fim de pregão (ADR-018): o motor precisa
    /// cancelar a entrada que ainda não encheu, e o cancelamento tem de passar
    /// pelo `Broker::cancel_order` — o mesmo caminho do live — para a ordem
    /// terminar com `status = Cancelled` e `cancelled_at` preenchido, em vez
    /// de sumir do estado sem rastro.
    pub fn pending_entry_order_id(&self, symbol: &str) -> Option<OrderId> {
        match self.state.lock() {
            Ok(state) => state
                .pending_entries
                .get(symbol)
                .map(|e| e.order_id.clone()),
            Err(e) => {
                warn!(error = %e, "falha ao ler entrada pendente");
                None
            }
        }
    }

    /// Retorna as operações fechadas até o momento.
    pub fn get_closed_trades(&self) -> Vec<Trade> {
        match self.state.lock() {
            Ok(state) => state.closed_trades.clone(),
            Err(e) => {
                warn!(error = %e, "falha ao ler trades fechados");
                Vec::new()
            }
        }
    }

    /// Limpa o histórico de trades fechados.
    pub fn clear_closed_trades(&self) {
        if let Ok(mut state) = self.state.lock() {
            state.closed_trades.clear();
        }
    }

    fn next_id(state: &mut SimulatedState) -> OrderId {
        let id = OrderId::from(format!(
            "sim-{}-{}",
            Utc::now().timestamp_nanos_opt().unwrap_or(0),
            state.next_order_id
        ));
        state.next_order_id += 1;
        id
    }
}

#[async_trait]
impl Broker for SimulatedBroker {
    async fn place_order(&self, mut order: Order) -> Result<OrderId, BrokerError> {
        let mut state = self.state.lock().map_err(|e| {
            BrokerError::Internal(format!("erro ao adquirir lock do estado simulado: {e}"))
        })?;

        // Regra de segurança financeira: não abrir nova posição se já existir
        // posição aberta ou entrada stop pendente no mesmo ativo.
        if state.positions.contains_key(&order.symbol)
            || state.pending_entries.contains_key(&order.symbol)
        {
            return Err(BrokerError::OrderRejected(format!(
                "já existe posição aberta ou entrada pendente em {}; não é permitido sobrepor posições",
                order.symbol
            )));
        }

        let id = Self::next_id(&mut state);

        order.broker_order_id = Some(id.to_string());
        order.status = OrderStatus::Submitted;
        order.submitted_at = Some(state.now);

        let base_price = order
            .price
            .or_else(|| state.market_prices.get(&order.symbol).copied())
            .unwrap_or_else(|| Decimal::from(100));

        let direction = match order.side {
            OrderSide::Buy => Direction::Long,
            OrderSide::Sell => Direction::Short,
        };

        // Aplica slippage contra o trader: entrada pior para long, melhor para short.
        let slippage_factor = Decimal::ONE + self.config.slippage_pct;
        let fill_price = match direction {
            Direction::Long => base_price * slippage_factor,
            Direction::Short => base_price / slippage_factor,
        };

        let commission = self
            .config
            .commission
            .for_execution(order.quantity, fill_price);

        match order.order_type {
            OrderType::Market | OrderType::Limit => {
                Fill::new(
                    order.id.unwrap_or(0),
                    &order.symbol,
                    order.side,
                    fill_price,
                    order.quantity,
                    state.now,
                )
                .map_err(|e| BrokerError::Internal(e.to_string()))?;

                let mut position = Position::new(
                    &order.symbol,
                    order.signal_id.unwrap_or(0),
                    direction,
                    order.quantity,
                    fill_price,
                    order.stop_price.unwrap_or(Decimal::ZERO),
                    "simulated",
                )
                .map_err(|e| BrokerError::Internal(e.to_string()))?;
                position.entry_time = state.now;
                position.metadata = order.metadata.clone();

                state.positions.insert(order.symbol.clone(), position);

                state.cash += entry_cash_flow(direction, fill_price, order.quantity, commission);
                state.equity =
                    mark_to_market_equity(state.cash, direction, fill_price, order.quantity);

                order.status = OrderStatus::Filled;
                order.filled_quantity = order.quantity;
                order.avg_fill_price = Some(fill_price);
                order.filled_at = Some(state.now);
            }
            OrderType::Bracket => {
                let stop_price = order.stop_price.ok_or_else(|| {
                    BrokerError::OrderRejected("bracket order sem stop".to_string())
                })?;
                let target_price = order.target_price.ok_or_else(|| {
                    BrokerError::OrderRejected("bracket order sem alvo".to_string())
                })?;

                // Entrada stop: a ordem trabalha aguardando o rompimento do
                // gatilho (high/low do candle). Sem posição até o fill.
                if order.entry_order_type == EntryOrderType::Stop {
                    let trigger_price = order.price.ok_or_else(|| {
                        BrokerError::OrderRejected("entrada stop sem preço de gatilho".to_string())
                    })?;

                    state.pending_entries.insert(
                        order.symbol.clone(),
                        PendingEntry {
                            trigger_price,
                            direction,
                            stop_price,
                            target_price,
                            quantity: order.quantity,
                            order_id: id.clone(),
                            signal_id: order.signal_id.unwrap_or(0),
                            candles_waiting: 0,
                            metadata: order.metadata.clone(),
                        },
                    );

                    order.status = OrderStatus::Accepted;
                    state.orders.insert(id.clone(), order);
                    info!(%id, "entrada stop registrada; aguardando rompimento do gatilho");
                    return Ok(id);
                }

                let mut position = Position::new(
                    &order.symbol,
                    order.signal_id.unwrap_or(0),
                    direction,
                    order.quantity,
                    fill_price,
                    stop_price,
                    "simulated",
                )
                .map_err(|e| BrokerError::Internal(e.to_string()))?;
                position.entry_time = state.now;
                position.metadata = order.metadata.clone();

                state.positions.insert(order.symbol.clone(), position);

                let pending_exit = PendingExit {
                    stop_price,
                    target_price,
                };
                state
                    .pending_exits
                    .insert(order.symbol.clone(), pending_exit);

                state.cash += entry_cash_flow(direction, fill_price, order.quantity, commission);
                state.equity =
                    mark_to_market_equity(state.cash, direction, fill_price, order.quantity);

                order.status = OrderStatus::Filled;
                order.filled_quantity = order.quantity;
                order.avg_fill_price = Some(fill_price);
                order.filled_at = Some(state.now);
            }
            OrderType::Stop | OrderType::StopLimit => {
                return Err(BrokerError::OrderRejected(
                    "ordens stop/stop-limit isoladas não suportadas no simulador; use bracket"
                        .to_string(),
                ));
            }
        }

        state.orders.insert(id.clone(), order);
        info!(%id, "ordem simulada enviada");
        Ok(id)
    }

    async fn cancel_order(&self, id: &OrderId) -> Result<(), BrokerError> {
        let mut state = self.state.lock().map_err(|e| {
            BrokerError::Internal(format!("erro ao adquirir lock do estado simulado: {e}"))
        })?;

        let order = state
            .orders
            .get_mut(id)
            .ok_or_else(|| BrokerError::OrderNotFound(id.to_string()))?;

        if order.is_filled() {
            return Err(BrokerError::OrderRejected(format!(
                "ordem {id} já preenchida"
            )));
        }

        let symbol = order.symbol.clone();
        order.status = OrderStatus::Cancelled;
        order.cancelled_at = Some(Utc::now());
        state.pending_entries.remove(&symbol);
        info!(%id, "ordem simulada cancelada");
        Ok(())
    }

    async fn get_order_status(&self, id: &OrderId) -> Result<OrderStatus, BrokerError> {
        let state = self.state.lock().map_err(|e| {
            BrokerError::Internal(format!("erro ao adquirir lock do estado simulado: {e}"))
        })?;

        state
            .orders
            .get(id)
            .map(|o| o.status)
            .ok_or_else(|| BrokerError::OrderNotFound(id.to_string()))
    }

    async fn get_open_orders(&self) -> Result<Vec<Order>, BrokerError> {
        let state = self.state.lock().map_err(|e| {
            BrokerError::Internal(format!("erro ao adquirir lock do estado simulado: {e}"))
        })?;

        Ok(state
            .orders
            .values()
            .filter(|o| {
                o.status == OrderStatus::Submitted
                    || o.status == OrderStatus::Accepted
                    || o.status == OrderStatus::PartiallyFilled
            })
            .cloned()
            .collect())
    }

    async fn get_position(&self, symbol: &str) -> Result<Option<Position>, BrokerError> {
        let state = self.state.lock().map_err(|e| {
            BrokerError::Internal(format!("erro ao adquirir lock do estado simulado: {e}"))
        })?;

        Ok(state.positions.get(symbol).cloned())
    }

    async fn get_positions(&self) -> Result<Vec<Position>, BrokerError> {
        let state = self.state.lock().map_err(|e| {
            BrokerError::Internal(format!("erro ao adquirir lock do estado simulado: {e}"))
        })?;

        Ok(state.positions.values().cloned().collect())
    }

    async fn get_account_summary(&self) -> Result<AccountSummary, BrokerError> {
        let state = self.state.lock().map_err(|e| {
            BrokerError::Internal(format!("erro ao adquirir lock do estado simulado: {e}"))
        })?;

        Ok(AccountSummary {
            broker: "simulated".to_string(),
            account_id: self.config.account_id.clone(),
            cash: state.cash,
            equity: state.equity,
            buying_power: state.buying_power,
            daily_pnl: state.daily_pnl,
            timestamp: Utc::now(),
            // O simulador não tem moeda: inventar "USD" aqui faria o snapshot
            // do paper simulado afirmar algo que ninguém mediu.
            currencies: Vec::new(),
        })
    }

    async fn subscribe_order_events(
        &self,
        _tx: Sender<OrderEvent>,
    ) -> Result<SubscriptionHandle, BrokerError> {
        warn!("subscribe_order_events simulado: não envia eventos");
        Ok(SubscriptionHandle {
            id: "simulated-orders".to_string(),
        })
    }
}

/// Aplica slippage SEMPRE contra o trader.
///
/// `slippage` é fração do preço (`0.001` = 0,1%). Numa ENTRADA, comprar sai
/// mais caro e vender a descoberto sai mais barato; numa SAÍDA é o espelho:
/// vender (fechar long) sai mais barato, cobrir short sai mais caro.
///
/// Vale só para execução a mercado — stop acionado e fechamento ativo. Ordem
/// limite (o alvo) não escorrega: ou enche no preço, ou não enche.
fn apply_slippage(
    price: Decimal,
    direction: Direction,
    is_entry: bool,
    slippage: Decimal,
) -> Decimal {
    let factor = Decimal::ONE + slippage;
    let compra = matches!(
        (direction, is_entry),
        (Direction::Long, true) | (Direction::Short, false)
    );
    if compra {
        price * factor
    } else {
        price / factor
    }
}

/// Fluxo de caixa da ENTRADA, com direção.
///
/// Comprar debita o caixa; vender a descoberto CREDITA (o short recebe o
/// valor da venda e passa a dever as ações). Tratar short como compra
/// invertia a equity: um short vencedor derrubava a curva e um perdedor a
/// subia, corrompendo Sharpe, drawdown e o capital que dimensiona os
/// trades seguintes.
fn entry_cash_flow(
    direction: Direction,
    fill_price: Decimal,
    quantity: Decimal,
    commission: Decimal,
) -> Decimal {
    let notional = fill_price * quantity;
    match direction {
        Direction::Long => -notional - commission,
        Direction::Short => notional - commission,
    }
}

/// Fluxo de caixa da SAÍDA, espelho de [`entry_cash_flow`]: vender credita,
/// cobrir o short debita.
fn exit_cash_flow(
    direction: Direction,
    exit_price: Decimal,
    quantity: Decimal,
    commission: Decimal,
) -> Decimal {
    let notional = exit_price * quantity;
    match direction {
        Direction::Long => notional - commission,
        Direction::Short => -notional - commission,
    }
}

/// Equity marcada a mercado: caixa mais o valor da posição — que é negativo
/// no short, onde a posição é um passivo de recompra.
fn mark_to_market_equity(
    cash: Decimal,
    direction: Direction,
    price: Decimal,
    quantity: Decimal,
) -> Decimal {
    match direction {
        Direction::Long => cash + price * quantity,
        Direction::Short => cash - price * quantity,
    }
}

/// Fecha uma posição e gera o `Trade` correspondente.
fn close_position_to_trade(
    position: &Position,
    exit_price: Decimal,
    exit_reason: ExitReason,
    config: &SimulatedBrokerConfig,
    exit_time: chrono::DateTime<Utc>,
) -> Option<Trade> {
    let direction_multiplier = match position.direction {
        Direction::Long => Decimal::ONE,
        Direction::Short => -Decimal::ONE,
    };

    let gross_pnl =
        (exit_price - position.avg_entry_price) * position.quantity * direction_multiplier;
    // Comissão das DUAS pernas, cada uma no seu preço. Com a tabela por ação
    // a quantidade é a mesma nos dois lados, mas o teto de 1% do valor depende
    // do preço — e derivar a saída da entrada esconderia isso.
    let commissions = config
        .commission
        .for_execution(position.quantity, position.avg_entry_price)
        + config
            .commission
            .for_execution(position.quantity, exit_price);
    let net_pnl = gross_pnl - commissions;

    let risk_amount = (position.avg_entry_price - position.stop_price).abs() * position.quantity;
    let result_in_r = if risk_amount.is_zero() {
        Decimal::ZERO
    } else {
        net_pnl / risk_amount
    };

    let strategy_id = position
        .metadata
        .get("strategy_id")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();
    let strategy_version = position
        .metadata
        .get("strategy_version")
        .and_then(|v| v.as_str())
        .unwrap_or("0.0.0")
        .to_string();
    let config_hash = position
        .metadata
        .get("config_hash")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();

    Some(Trade {
        id: None,
        symbol: position.symbol.clone(),
        signal_id: position.signal_id,
        position_id: position.id,
        direction: position.direction,
        entry_price: position.avg_entry_price,
        exit_price,
        quantity: position.quantity,
        entry_time: position.entry_time,
        exit_time,
        stop_price: position.stop_price,
        target_price: position.target_price,
        gross_pnl,
        commissions,
        fees: Decimal::ZERO,
        net_pnl,
        risk_amount,
        result_in_r,
        exit_reason,
        strategy_id,
        strategy_version,
        config_hash,
        // O snapshot do sinal e a origem ficam no journal, como no live. E o
        // que permite cortar os trades por bucket (distancia de stop, tipo de
        // dia) sem sair do motor.
        journal: serde_json::json!({
            "source": "simulated_broker",
            "market_snapshot": position
                .metadata
                .get("market_snapshot")
                .cloned()
                .unwrap_or(serde_json::Value::Null),
        }),
        correlation_id: position.correlation_id.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use trader_domain::{Candle, TimeFrame};

    fn market_order(symbol: &str, quantity: Decimal) -> Order {
        Order::new(
            symbol,
            OrderSide::Buy,
            OrderType::Market,
            quantity,
            "simulated",
        )
        .unwrap()
    }

    fn bracket_order(symbol: &str, quantity: Decimal, stop: Decimal, target: Decimal) -> Order {
        let mut order = Order::new(
            symbol,
            OrderSide::Buy,
            OrderType::Bracket,
            quantity,
            "simulated",
        )
        .unwrap();
        order.price = Some(Decimal::from(100));
        order.stop_price = Some(stop);
        order.target_price = Some(target);
        // Limit preserva o comportamento de fill imediato testado abaixo;
        // a entrada stop (default) tem testes próprios.
        order.entry_order_type = EntryOrderType::Limit;
        order
    }

    fn zero_cost_config(initial_cash: i64, commission: Decimal) -> SimulatedBrokerConfig {
        SimulatedBrokerConfig {
            account_id: Some("DU_SIM".to_string()),
            initial_cash: Decimal::from(initial_cash),
            commission: CommissionModel::PerTrade(commission),
            limit_fill_haircut_pct: Decimal::ZERO,
            slippage_pct: Decimal::ZERO,
            entry_validity_candles: 1,
            entry_overshoot_tolerance: Decimal::from(25) / Decimal::from(100),
        }
    }

    fn directional_bracket(
        symbol: &str,
        side: OrderSide,
        quantity: Decimal,
        entry: Decimal,
        stop: Decimal,
        target: Decimal,
    ) -> Order {
        let mut order =
            Order::new(symbol, side, OrderType::Bracket, quantity, "simulated").unwrap();
        order.price = Some(entry);
        order.stop_price = Some(stop);
        order.target_price = Some(target);
        // Limit enche na hora; a entrada stop tem testes próprios.
        order.entry_order_type = EntryOrderType::Limit;
        order
    }

    fn candle(symbol: &str, open: i64, high: i64, low: i64, close: i64) -> Candle {
        Candle::new(
            symbol,
            TimeFrame::M15,
            Utc::now(),
            Decimal::from(open),
            Decimal::from(high),
            Decimal::from(low),
            Decimal::from(close),
            Decimal::from(1_000),
        )
        .unwrap()
    }

    /// Invariante de caixa (C5 da auditoria): fechado o round-trip, a equity
    /// final tem de ser exatamente o capital inicial mais a soma dos
    /// `net_pnl`. Antes do fix a entrada short debitava caixa como se fosse
    /// compra: um short vencedor DERRUBAVA a equity, corrompendo curva de
    /// equity, Sharpe e o capital que dimensiona os trades seguintes.
    #[tokio::test]
    async fn short_round_trip_fecha_equity_com_o_net_pnl() {
        let commission = Decimal::from(35) / Decimal::from(100);
        let broker = SimulatedBroker::new(zero_cost_config(100_000, commission));

        broker
            .place_order(directional_bracket(
                "SPY",
                OrderSide::Sell,
                Decimal::from(10),
                Decimal::from(100),
                Decimal::from(105),
                Decimal::from(90),
            ))
            .await
            .unwrap();

        // Candle que atinge o alvo do short (low <= 90) sem tocar o stop.
        broker.set_market_candle("SPY", &candle("SPY", 95, 96, 89, 90));

        let trades = broker.get_closed_trades();
        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].exit_reason, ExitReason::Target);
        // Short vencedor: 10 × (100 - 90) menos 2 comissões.
        assert_eq!(
            trades[0].net_pnl,
            Decimal::from(100) - commission * Decimal::TWO
        );
        assert!(trades[0].net_pnl > Decimal::ZERO);

        let summary = broker.get_account_summary().await.unwrap();
        let soma: Decimal = trades.iter().map(|t| t.net_pnl).sum();
        assert_eq!(summary.equity, Decimal::from(100_000) + soma);
        assert_eq!(summary.cash, summary.equity);
    }

    #[tokio::test]
    async fn short_perdedor_derruba_equity_na_medida_do_net_pnl() {
        let broker = SimulatedBroker::new(zero_cost_config(100_000, Decimal::ZERO));

        broker
            .place_order(directional_bracket(
                "SPY",
                OrderSide::Sell,
                Decimal::from(10),
                Decimal::from(100),
                Decimal::from(105),
                Decimal::from(90),
            ))
            .await
            .unwrap();

        broker.set_market_candle("SPY", &candle("SPY", 102, 106, 101, 105));

        let trades = broker.get_closed_trades();
        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].exit_reason, ExitReason::Stop);
        assert_eq!(trades[0].net_pnl, Decimal::from(-50));

        let summary = broker.get_account_summary().await.unwrap();
        assert_eq!(summary.equity, Decimal::from(100_000) - Decimal::from(50));
    }

    #[tokio::test]
    async fn long_round_trip_fecha_equity_com_o_net_pnl() {
        let commission = Decimal::from(35) / Decimal::from(100);
        let broker = SimulatedBroker::new(zero_cost_config(100_000, commission));

        broker
            .place_order(directional_bracket(
                "SPY",
                OrderSide::Buy,
                Decimal::from(10),
                Decimal::from(100),
                Decimal::from(95),
                Decimal::from(110),
            ))
            .await
            .unwrap();

        broker.set_market_candle("SPY", &candle("SPY", 105, 111, 104, 110));

        let trades = broker.get_closed_trades();
        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].exit_reason, ExitReason::Target);

        let summary = broker.get_account_summary().await.unwrap();
        let soma: Decimal = trades.iter().map(|t| t.net_pnl).sum();
        assert_eq!(summary.equity, Decimal::from(100_000) + soma);
    }

    /// A equity marcada a mercado de um short sobe quando o preço cai.
    #[tokio::test]
    async fn equity_marcada_a_mercado_do_short_sobe_com_a_queda() {
        let broker = SimulatedBroker::new(zero_cost_config(100_000, Decimal::ZERO));

        broker
            .place_order(directional_bracket(
                "SPY",
                OrderSide::Sell,
                Decimal::from(10),
                Decimal::from(100),
                Decimal::from(105),
                Decimal::from(80),
            ))
            .await
            .unwrap();

        // Sem custos, abrir o short não muda a equity.
        assert_eq!(
            broker.get_account_summary().await.unwrap().equity,
            Decimal::from(100_000)
        );

        // Preço cai 5 sem tocar stop nem alvo: +50 não realizados.
        broker.set_market_candle("SPY", &candle("SPY", 99, 100, 94, 95));

        assert!(broker.get_position("SPY").await.unwrap().is_some());
        assert_eq!(
            broker.get_account_summary().await.unwrap().equity,
            Decimal::from(100_050)
        );
    }

    /// Trava a semântica do slippage: `0.001` significa 0,1% do preço, não
    /// 0,001%. A configuração dizia "0,1%" e a aplicação dividia por 100 de
    /// novo — o custo efetivo era cem vezes menor (A4 da auditoria).
    #[test]
    fn slippage_efetivo_e_a_fracao_configurada() {
        let um_decimo_de_por_cento = Decimal::from(1) / Decimal::from(1000);
        // Compra de entrada long a 100 paga 100,10 — não 100,001.
        assert_eq!(
            apply_slippage(
                Decimal::from(100),
                Direction::Long,
                true,
                um_decimo_de_por_cento
            ),
            Decimal::from(1001) / Decimal::from(10)
        );
    }

    /// O slippage nunca ajuda: sai contra o trader nos quatro casos.
    #[test]
    fn slippage_sempre_contra_o_trader() {
        let sl = Decimal::from(1) / Decimal::from(100); // 1%
        let p = Decimal::from(100);
        // Entradas
        assert!(
            apply_slippage(p, Direction::Long, true, sl) > p,
            "long entra mais caro"
        );
        assert!(
            apply_slippage(p, Direction::Short, true, sl) < p,
            "short entra mais barato"
        );
        // Saídas
        assert!(
            apply_slippage(p, Direction::Long, false, sl) < p,
            "long sai mais barato"
        );
        assert!(
            apply_slippage(p, Direction::Short, false, sl) > p,
            "short cobre mais caro"
        );
    }

    /// Candle que ABRE além do stop enche na abertura, não no stop: as
    /// entradas ganharam modelagem de gap no ADR-015 e as saídas não tinham.
    #[tokio::test]
    async fn stop_com_gap_enche_na_abertura_e_nao_no_stop() {
        let broker = SimulatedBroker::new(zero_cost_config(100_000, Decimal::ZERO));
        broker
            .place_order(directional_bracket(
                "SPY",
                OrderSide::Buy,
                Decimal::from(10),
                Decimal::from(100),
                Decimal::from(95),
                Decimal::from(110),
            ))
            .await
            .unwrap();
        // Abre em 90, cinco abaixo do stop de 95.
        broker.set_market_candle("SPY", &candle("SPY", 90, 91, 89, 90));

        let trades = broker.get_closed_trades();
        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].exit_reason, ExitReason::Stop);
        assert_eq!(
            trades[0].exit_price,
            Decimal::from(90),
            "deveria encher na abertura (90), não no stop (95)"
        );
    }

    /// Espelho do gap no alvo: ordem limite além do preço enche melhor —
    /// foi o que aconteceu de verdade no trade 11 (IWO, 13/08/2026).
    #[tokio::test]
    async fn alvo_com_gap_enche_na_abertura_melhor() {
        let broker = SimulatedBroker::new(zero_cost_config(100_000, Decimal::ZERO));
        broker
            .place_order(directional_bracket(
                "SPY",
                OrderSide::Buy,
                Decimal::from(10),
                Decimal::from(100),
                Decimal::from(95),
                Decimal::from(110),
            ))
            .await
            .unwrap();
        broker.set_market_candle("SPY", &candle("SPY", 115, 116, 114, 115));

        let trades = broker.get_closed_trades();
        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].exit_reason, ExitReason::Target);
        assert_eq!(trades[0].exit_price, Decimal::from(115));
    }

    #[tokio::test]
    async fn place_market_order_fills_immediately() {
        let broker = SimulatedBroker::default_simulated();
        let order = market_order("SPY", Decimal::from(10));

        let id = broker.place_order(order).await.unwrap();
        let status = broker.get_order_status(&id).await.unwrap();

        assert_eq!(status, OrderStatus::Filled);
    }

    #[tokio::test]
    async fn account_summary_reflects_initial_cash() {
        let broker = SimulatedBroker::new(SimulatedBrokerConfig {
            account_id: Some("DU_SIM".to_string()),
            initial_cash: Decimal::from(50_000),
            commission: CommissionModel::none(),
            limit_fill_haircut_pct: Decimal::ZERO,
            slippage_pct: Decimal::ZERO,
            entry_validity_candles: 1,
            entry_overshoot_tolerance: Decimal::from(25) / Decimal::from(100),
        });
        let summary = broker.get_account_summary().await.unwrap();

        assert_eq!(summary.cash, Decimal::from(50_000));
        assert_eq!(summary.broker, "simulated");
    }

    #[tokio::test]
    async fn rejects_second_position_in_same_symbol() {
        let broker = SimulatedBroker::default_simulated();
        let first = market_order("SPY", Decimal::from(10));
        broker.place_order(first).await.unwrap();
        // slippage default não afeta o teste.

        let second = market_order("SPY", Decimal::from(5));
        let result = broker.place_order(second).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn bracket_order_hits_stop_loss() {
        let broker = SimulatedBroker::new(SimulatedBrokerConfig {
            account_id: Some("DU_SIM".to_string()),
            initial_cash: Decimal::from(100_000),
            commission: CommissionModel::none(),
            limit_fill_haircut_pct: Decimal::ZERO,
            slippage_pct: Decimal::ZERO,
            entry_validity_candles: 1,
            entry_overshoot_tolerance: Decimal::from(25) / Decimal::from(100),
        });

        let order = bracket_order(
            "SPY",
            Decimal::from(10),
            Decimal::from(95),
            Decimal::from(110),
        );
        broker.place_order(order).await.unwrap();

        assert!(broker.get_position("SPY").await.unwrap().is_some());

        broker.set_market_price("SPY", Decimal::from(94));

        assert!(broker.get_position("SPY").await.unwrap().is_none());
        let trades = broker.get_closed_trades();
        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].exit_reason, ExitReason::Stop);
    }

    #[tokio::test]
    async fn bracket_order_hits_take_profit() {
        let broker = SimulatedBroker::new(SimulatedBrokerConfig {
            account_id: Some("DU_SIM".to_string()),
            initial_cash: Decimal::from(100_000),
            commission: CommissionModel::none(),
            limit_fill_haircut_pct: Decimal::ZERO,
            slippage_pct: Decimal::ZERO,
            entry_validity_candles: 1,
            entry_overshoot_tolerance: Decimal::from(25) / Decimal::from(100),
        });

        let order = bracket_order(
            "SPY",
            Decimal::from(10),
            Decimal::from(95),
            Decimal::from(110),
        );
        broker.place_order(order).await.unwrap();

        broker.set_market_price("SPY", Decimal::from(111));

        let trades = broker.get_closed_trades();
        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].exit_reason, ExitReason::Target);
        assert!(trades[0].net_pnl > Decimal::ZERO);
    }

    #[tokio::test]
    async fn intrabar_low_hits_stop_even_when_close_is_above() {
        let broker = SimulatedBroker::new(SimulatedBrokerConfig {
            account_id: Some("DU_SIM".to_string()),
            initial_cash: Decimal::from(100_000),
            commission: CommissionModel::none(),
            limit_fill_haircut_pct: Decimal::ZERO,
            slippage_pct: Decimal::ZERO,
            entry_validity_candles: 1,
            entry_overshoot_tolerance: Decimal::from(25) / Decimal::from(100),
        });

        let order = bracket_order(
            "SPY",
            Decimal::from(10),
            Decimal::from(95),
            Decimal::from(110),
        );
        broker.place_order(order).await.unwrap();

        // Candle perfura o stop (low=94) mas fecha acima dele (close=101):
        // com avaliação só no close, a saída não aconteceria.
        let candle = Candle::new(
            "SPY",
            trader_domain::TimeFrame::M15,
            Utc::now(),
            Decimal::from(100),
            Decimal::from(102),
            Decimal::from(94),
            Decimal::from(101),
            Decimal::from(1000),
        )
        .unwrap();
        broker.set_market_candle("SPY", &candle);

        assert!(broker.get_position("SPY").await.unwrap().is_none());
        let trades = broker.get_closed_trades();
        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].exit_reason, ExitReason::Stop);
        // Fill no preço do stop, não no close.
        assert_eq!(trades[0].exit_price, Decimal::from(95));
    }

    #[tokio::test]
    async fn intrabar_stop_wins_when_stop_and_target_in_same_candle() {
        let broker = SimulatedBroker::new(SimulatedBrokerConfig {
            account_id: Some("DU_SIM".to_string()),
            initial_cash: Decimal::from(100_000),
            commission: CommissionModel::none(),
            limit_fill_haircut_pct: Decimal::ZERO,
            slippage_pct: Decimal::ZERO,
            entry_validity_candles: 1,
            entry_overshoot_tolerance: Decimal::from(25) / Decimal::from(100),
        });

        let order = bracket_order(
            "SPY",
            Decimal::from(10),
            Decimal::from(95),
            Decimal::from(110),
        );
        broker.place_order(order).await.unwrap();

        // Candle toca stop (low=94) e alvo (high=111): pior caso prevalece.
        let candle = Candle::new(
            "SPY",
            trader_domain::TimeFrame::M15,
            Utc::now(),
            Decimal::from(100),
            Decimal::from(111),
            Decimal::from(94),
            Decimal::from(105),
            Decimal::from(1000),
        )
        .unwrap();
        broker.set_market_candle("SPY", &candle);

        let trades = broker.get_closed_trades();
        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].exit_reason, ExitReason::Stop);
    }

    fn stop_entry_bracket(symbol: &str, trigger: Decimal, stop: Decimal, target: Decimal) -> Order {
        let mut order = bracket_order(symbol, Decimal::from(10), stop, target);
        order.price = Some(trigger);
        order.entry_order_type = EntryOrderType::Stop;
        order
    }

    fn candle_at(high: i64, low: i64, close: i64) -> Candle {
        Candle::new(
            "SPY",
            trader_domain::TimeFrame::M15,
            Utc::now(),
            Decimal::from(close),
            Decimal::from(high),
            Decimal::from(low),
            Decimal::from(close),
            Decimal::from(1000),
        )
        .unwrap()
    }

    #[tokio::test]
    async fn stop_entry_fills_only_on_breakout() {
        let broker = SimulatedBroker::new(SimulatedBrokerConfig {
            account_id: Some("DU_SIM".to_string()),
            initial_cash: Decimal::from(100_000),
            commission: CommissionModel::none(),
            limit_fill_haircut_pct: Decimal::ZERO,
            slippage_pct: Decimal::ZERO,
            entry_validity_candles: 2,
            entry_overshoot_tolerance: Decimal::from(25) / Decimal::from(100),
        });

        let order = stop_entry_bracket(
            "SPY",
            Decimal::from(105),
            Decimal::from(95),
            Decimal::from(115),
        );
        broker.place_order(order).await.unwrap();

        // Sem posição até o rompimento; ordem aparece como aberta.
        assert!(broker.get_position("SPY").await.unwrap().is_none());
        assert_eq!(broker.get_open_orders().await.unwrap().len(), 1);

        // Candle que não rompe o gatilho: nada acontece.
        broker.set_market_candle("SPY", &candle_at(104, 98, 102));
        assert!(broker.get_position("SPY").await.unwrap().is_none());

        // Candle que rompe: fill no gatilho e posição aberta com stop/alvo.
        broker.set_market_candle("SPY", &candle_at(106, 101, 105));
        let position = broker.get_position("SPY").await.unwrap();
        assert!(position.is_some());
        assert_eq!(position.unwrap().avg_entry_price, Decimal::from(105));
    }

    #[tokio::test]
    async fn stop_entry_expires_without_breakout() {
        let broker = SimulatedBroker::new(SimulatedBrokerConfig {
            account_id: Some("DU_SIM".to_string()),
            initial_cash: Decimal::from(100_000),
            commission: CommissionModel::none(),
            limit_fill_haircut_pct: Decimal::ZERO,
            slippage_pct: Decimal::ZERO,
            entry_validity_candles: 1,
            entry_overshoot_tolerance: Decimal::from(25) / Decimal::from(100),
        });

        let order = stop_entry_bracket(
            "SPY",
            Decimal::from(105),
            Decimal::from(95),
            Decimal::from(115),
        );
        let id = broker.place_order(order).await.unwrap();

        // Validade 1 = só o próximo candle: um candle sem rompimento expira.
        broker.set_market_candle("SPY", &candle_at(104, 98, 102));

        assert_eq!(
            broker.get_order_status(&id).await.unwrap(),
            OrderStatus::Expired
        );
        assert!(broker.get_open_orders().await.unwrap().is_empty());

        // Rompimento tardio não enche mais.
        broker.set_market_candle("SPY", &candle_at(110, 101, 108));
        assert!(broker.get_position("SPY").await.unwrap().is_none());
    }

    /// Candle com OHLC arbitrário para os testes de gap na entrada stop.
    fn candle_ohlc(open: i64, high: i64, low: i64, close: i64) -> Candle {
        Candle::new(
            "SPY",
            trader_domain::TimeFrame::M15,
            Utc::now(),
            Decimal::from(open),
            Decimal::from(high),
            Decimal::from(low),
            Decimal::from(close),
            Decimal::from(1000),
        )
        .unwrap()
    }

    /// ADR-015: gap de abertura DENTRO da tolerância enche na abertura (custo
    /// real do gap), não no gatilho como o modelo antigo.
    #[tokio::test]
    async fn stop_entry_gap_within_tolerance_fills_at_open() {
        let broker = SimulatedBroker::new(SimulatedBrokerConfig {
            account_id: Some("DU_SIM".to_string()),
            initial_cash: Decimal::from(100_000),
            commission: CommissionModel::none(),
            limit_fill_haircut_pct: Decimal::ZERO,
            slippage_pct: Decimal::ZERO,
            entry_validity_candles: 1,
            entry_overshoot_tolerance: Decimal::from(25) / Decimal::from(100),
        });

        // Gatilho 105, stop 95: distância 10 → overshoot tolerado até 2.5.
        let order = stop_entry_bracket(
            "SPY",
            Decimal::from(105),
            Decimal::from(95),
            Decimal::from(115),
        );
        broker.place_order(order).await.unwrap();

        // Abre em 107 (overshoot 2 ≤ 2.5): enche em 107, não em 105.
        broker.set_market_candle("SPY", &candle_ohlc(107, 108, 106, 107));
        let position = broker.get_position("SPY").await.unwrap();
        assert_eq!(position.unwrap().avg_entry_price, Decimal::from(107));
    }

    /// ADR-015: gap de abertura ALÉM da tolerância invalida a entrada —
    /// cenário do trade 12 do live (overshoot maior que a distância do stop).
    #[tokio::test]
    async fn stop_entry_gap_beyond_tolerance_cancels() {
        let broker = SimulatedBroker::new(SimulatedBrokerConfig {
            account_id: Some("DU_SIM".to_string()),
            initial_cash: Decimal::from(100_000),
            commission: CommissionModel::none(),
            limit_fill_haircut_pct: Decimal::ZERO,
            slippage_pct: Decimal::ZERO,
            entry_validity_candles: 2,
            entry_overshoot_tolerance: Decimal::from(25) / Decimal::from(100),
        });

        // Gatilho 105, stop 95: overshoot tolerado até 2.5; abre em 109 (4).
        let order = stop_entry_bracket(
            "SPY",
            Decimal::from(105),
            Decimal::from(95),
            Decimal::from(115),
        );
        let id = broker.place_order(order).await.unwrap();

        broker.set_market_candle("SPY", &candle_ohlc(109, 110, 108, 109));

        assert!(broker.get_position("SPY").await.unwrap().is_none());
        assert_eq!(
            broker.get_order_status(&id).await.unwrap(),
            OrderStatus::Cancelled
        );

        // A entrada foi removida: candle seguinte não enche mais.
        broker.set_market_candle("SPY", &candle_ohlc(105, 106, 104, 105));
        assert!(broker.get_position("SPY").await.unwrap().is_none());
    }

    /// Direção short: overshoot é a abertura ABAIXO do gatilho de venda.
    #[tokio::test]
    async fn short_stop_entry_gap_beyond_tolerance_cancels() {
        let broker = SimulatedBroker::new(SimulatedBrokerConfig {
            account_id: Some("DU_SIM".to_string()),
            initial_cash: Decimal::from(100_000),
            commission: CommissionModel::none(),
            limit_fill_haircut_pct: Decimal::ZERO,
            slippage_pct: Decimal::ZERO,
            entry_validity_candles: 1,
            entry_overshoot_tolerance: Decimal::from(25) / Decimal::from(100),
        });

        // Sell stop 100, stop de proteção 105: distância 5 → tolerado 1.25.
        let mut order = Order::new(
            "SPY",
            OrderSide::Sell,
            OrderType::Bracket,
            Decimal::from(10),
            "simulated",
        )
        .unwrap();
        order.price = Some(Decimal::from(100));
        order.stop_price = Some(Decimal::from(105));
        order.target_price = Some(Decimal::from(90));
        order.entry_order_type = EntryOrderType::Stop;
        let id = broker.place_order(order).await.unwrap();

        // Abre em 97 (overshoot 3 > 1.25): invalidada.
        broker.set_market_candle("SPY", &candle_ohlc(97, 98, 96, 97));

        assert!(broker.get_position("SPY").await.unwrap().is_none());
        assert_eq!(
            broker.get_order_status(&id).await.unwrap(),
            OrderStatus::Cancelled
        );
    }

    #[tokio::test]
    async fn stop_entry_cancelable_while_pending() {
        let broker = SimulatedBroker::default_simulated();
        let order = stop_entry_bracket(
            "SPY",
            Decimal::from(105),
            Decimal::from(95),
            Decimal::from(115),
        );
        let id = broker.place_order(order).await.unwrap();

        broker.cancel_order(&id).await.unwrap();
        assert_eq!(
            broker.get_order_status(&id).await.unwrap(),
            OrderStatus::Cancelled
        );

        broker.set_market_candle("SPY", &candle_at(110, 101, 108));
        assert!(broker.get_position("SPY").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn rejects_new_entry_while_stop_entry_pending() {
        let broker = SimulatedBroker::default_simulated();
        let order = stop_entry_bracket(
            "SPY",
            Decimal::from(105),
            Decimal::from(95),
            Decimal::from(115),
        );
        broker.place_order(order).await.unwrap();

        let second = market_order("SPY", Decimal::from(5));
        assert!(broker.place_order(second).await.is_err());
    }

    #[tokio::test]
    async fn close_position_at_market_closes_with_given_reason() {
        let broker = SimulatedBroker::new(SimulatedBrokerConfig {
            account_id: Some("DU_SIM".to_string()),
            initial_cash: Decimal::from(100_000),
            commission: CommissionModel::none(),
            limit_fill_haircut_pct: Decimal::ZERO,
            slippage_pct: Decimal::ZERO,
            entry_validity_candles: 1,
            entry_overshoot_tolerance: Decimal::from(25) / Decimal::from(100),
        });

        let order = bracket_order(
            "SPY",
            Decimal::from(10),
            Decimal::from(95),
            Decimal::from(110),
        );
        broker.place_order(order).await.unwrap();
        assert!(broker.get_position("SPY").await.unwrap().is_some());

        // Saída ativa (ex.: saída por tempo): encerra no preço informado,
        // sem tocar stop nem alvo.
        assert!(broker.close_position_at_market("SPY", Decimal::from(101), ExitReason::Time));
        assert!(broker.get_position("SPY").await.unwrap().is_none());

        let trades = broker.get_closed_trades();
        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].exit_reason, ExitReason::Time);
        assert_eq!(trades[0].exit_price, Decimal::from(101));

        // Sem posição, não há o que encerrar.
        assert!(!broker.close_position_at_market("SPY", Decimal::from(101), ExitReason::Time));
    }

    // ---------------------------------------------------------------------
    // Custo real de execucao (§5.6 do plano).
    //
    // A licao da rodada adversarial de 08/09: formula certa sem teste de
    // VALOR nao e formula guardada. Cada numero abaixo foi calculado fora do
    // codigo, a partir da tabela publicada da IBKR.
    // ---------------------------------------------------------------------

    #[test]
    fn comissao_ibkr_e_por_acao() {
        let m = CommissionModel::ibkr_fixed_us();
        // 1.262 acoes x US$ 0,005 = US$ 6,31. O piso de US$ 1,00 nao morde e
        // o teto de 1% (US$ 1.000 num papel de US$ 79) tampouco.
        assert_eq!(
            m.for_execution(Decimal::from(1262), Decimal::from(79)),
            Decimal::new(631, 2)
        );
    }

    #[test]
    fn comissao_ibkr_respeita_o_piso_por_ordem() {
        let m = CommissionModel::ibkr_fixed_us();
        // 100 acoes dariam US$ 0,50; o piso e US$ 1,00.
        assert_eq!(
            m.for_execution(Decimal::from(100), Decimal::from(50)),
            Decimal::ONE
        );
    }

    #[test]
    fn comissao_ibkr_respeita_o_teto_de_1_por_cento() {
        let m = CommissionModel::ibkr_fixed_us();
        // 1.000 acoes de um papel de US$ 0,10: por acao dariam US$ 5,00, mas
        // 1% do valor negociado (US$ 100) e US$ 1,00.
        assert_eq!(
            m.for_execution(Decimal::from(1000), Decimal::new(10, 2)),
            Decimal::ONE
        );
    }

    #[test]
    fn execucao_de_quantidade_zero_nao_paga_piso() {
        let m = CommissionModel::ibkr_fixed_us();
        assert_eq!(
            m.for_execution(Decimal::ZERO, Decimal::from(100)),
            Decimal::ZERO
        );
    }

    #[test]
    fn modelo_fixo_ignora_quantidade_e_preco() {
        let m = CommissionModel::PerTrade(Decimal::new(35, 2));
        assert_eq!(
            m.for_execution(Decimal::from(1), Decimal::from(10)),
            Decimal::new(35, 2)
        );
        assert_eq!(
            m.for_execution(Decimal::from(5000), Decimal::from(400)),
            Decimal::new(35, 2)
        );
    }

    #[test]
    fn a_diferenca_entre_os_dois_modelos_na_amostra_medida() {
        // Este teste guarda o numero que os documentos publicam, e ele foi
        // medido, nao escolhido: nos 214 trades OOS das oito combinacoes
        // vivas, as quantidades vao de 234 a 1.311 acoes (mediana 619).
        //
        // A versao anterior deste teste usava 1.262 acoes fixas e afirmava
        // "> 17x" — passava sem tocar em dado nenhum, e o 18x que ela
        // sustentava vazou para o titulo do relatorio, para o plano e para o
        // HANDOFF. A razao REAL, sobre a amostra inteira, e 9,9x.
        let antigo = CommissionModel::PerTrade(Decimal::new(35, 2));
        let real = CommissionModel::ibkr_fixed_us();
        let preco = Decimal::from(79);
        let perna_dupla = |m: &CommissionModel, q: i64| {
            m.for_execution(Decimal::from(q), preco) * Decimal::from(2)
        };

        // O modelo antigo nao depende do tamanho: US$ 0,70 sempre.
        assert_eq!(perna_dupla(&antigo, 234), Decimal::new(70, 2));
        assert_eq!(perna_dupla(&antigo, 1311), Decimal::new(70, 2));

        // O real depende, e cobre a faixa medida.
        assert_eq!(perna_dupla(&real, 234), Decimal::new(234, 2)); // US$ 2,34
        assert_eq!(perna_dupla(&real, 619), Decimal::new(619, 2)); // US$ 6,19 (mediana)
        assert_eq!(perna_dupla(&real, 1311), Decimal::new(1311, 2)); // US$ 13,11

        // A razao vai de 3,3x na menor ordem a 18,7x na maior. Na MEDIANA da
        // amostra e 8,8x; no agregado dos 214 trades, 9,9x. Publicar o extremo
        // como se fosse o tipico foi o erro.
        assert!(perna_dupla(&real, 234) < perna_dupla(&antigo, 234) * Decimal::from(4));
        assert!(perna_dupla(&real, 1311) > perna_dupla(&antigo, 1311) * Decimal::from(18));
    }

    #[test]
    fn a_menor_ordem_real_fica_apenas_17_por_cento_acima_do_piso() {
        // 234 acoes x US$ 0,005 = US$ 1,17 contra o piso de US$ 1,00. A
        // afirmacao "o piso so valeria para ordem abaixo de 200 acoes — as
        // posicoes tem de 1.000 a 2.500" descrevia uma folga que nao existe.
        let m = CommissionModel::ibkr_fixed_us();
        assert_eq!(
            m.for_execution(Decimal::from(234), Decimal::from(79)),
            Decimal::new(117, 2)
        );
        // Uma ordem de 199 acoes ja cai no piso.
        assert_eq!(
            m.for_execution(Decimal::from(199), Decimal::from(79)),
            Decimal::ONE
        );
    }

    #[test]
    fn a_comissao_da_saida_usa_o_preco_da_saida() {
        // Com o teto de 1% do valor, as duas pernas so coincidem quando os
        // precos coincidem. Derivar a saida da entrada (ou dividir o total por
        // dois) esconde isso.
        let m = CommissionModel::ibkr_fixed_us();
        let qtd = Decimal::from(1000);
        // Entrada a US$ 0,40: teto = 1% x 400 = US$ 4,00 (< US$ 5,00 por acao).
        assert_eq!(m.for_execution(qtd, Decimal::new(40, 2)), Decimal::from(4));
        // Saida a US$ 0,80: teto = US$ 8,00, entao vale o por acao, US$ 5,00.
        assert_eq!(m.for_execution(qtd, Decimal::new(80, 2)), Decimal::from(5));
    }

    /// O trade fechado tem de cobrar as DUAS pernas pelo modelo, nao o dobro
    /// de um valor fixo.
    #[tokio::test]
    async fn trade_cobra_comissao_das_duas_pernas_pela_tabela() {
        let config = SimulatedBrokerConfig {
            account_id: Some("DU_SIM".to_string()),
            initial_cash: Decimal::from(100_000),
            commission: CommissionModel::ibkr_fixed_us(),
            limit_fill_haircut_pct: Decimal::ZERO,
            slippage_pct: Decimal::ZERO,
            entry_validity_candles: 1,
            entry_overshoot_tolerance: Decimal::from(25) / Decimal::from(100),
        };
        let broker = SimulatedBroker::new(config);
        let ordem = directional_bracket(
            "SPY",
            OrderSide::Buy,
            Decimal::from(1000),
            Decimal::from(100),
            Decimal::from(99),
            Decimal::from(102),
        );
        broker.place_order(ordem).await.unwrap();
        broker.set_market_candle("SPY", &candle("SPY", 100, 103, 100, 103));

        let trades = broker.get_closed_trades();
        assert_eq!(trades.len(), 1);
        // 1.000 acoes x US$ 0,005 = US$ 5,00 por perna, US$ 10,00 no trade.
        assert_eq!(trades[0].commissions, Decimal::from(10));
    }

    /// O alvo enchia de graca: bastava o candle tocar o nivel.
    #[tokio::test]
    async fn o_alvo_paga_o_desconto_de_fill_limite() {
        let mut config = SimulatedBrokerConfig {
            account_id: Some("DU_SIM".to_string()),
            initial_cash: Decimal::from(100_000),
            commission: CommissionModel::none(),
            limit_fill_haircut_pct: Decimal::ZERO,
            slippage_pct: Decimal::ZERO,
            entry_validity_candles: 1,
            entry_overshoot_tolerance: Decimal::from(25) / Decimal::from(100),
        };

        let sem_desconto = {
            let broker = SimulatedBroker::new(config.clone());
            let ordem = directional_bracket(
                "SPY",
                OrderSide::Buy,
                Decimal::from(100),
                Decimal::from(100),
                Decimal::from(99),
                Decimal::from(102),
            );
            broker.place_order(ordem).await.unwrap();
            broker.set_market_candle("SPY", &candle("SPY", 100, 103, 100, 103));
            broker.get_closed_trades().remove(0).exit_price
        };
        // Sem desconto, o fill e o proprio alvo.
        assert_eq!(sem_desconto, Decimal::from(102));

        config.limit_fill_haircut_pct = Decimal::from(2) / Decimal::from(10_000);
        let com_desconto = {
            let broker = SimulatedBroker::new(config);
            let ordem = directional_bracket(
                "SPY",
                OrderSide::Buy,
                Decimal::from(100),
                Decimal::from(100),
                Decimal::from(99),
                Decimal::from(102),
            );
            broker.place_order(ordem).await.unwrap();
            broker.set_market_candle("SPY", &candle("SPY", 100, 103, 100, 103));
            broker.get_closed_trades().remove(0).exit_price
        };
        // Vender long com desconto: preco MENOR que o alvo, sempre contra o
        // trader.
        assert!(com_desconto < sem_desconto);
        assert_eq!(
            com_desconto,
            Decimal::from(102) / (Decimal::ONE + config_haircut())
        );
    }

    fn config_haircut() -> Decimal {
        Decimal::from(2) / Decimal::from(10_000)
    }
}
