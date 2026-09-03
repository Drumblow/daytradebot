//! Broker adapter para Interactive Brokers.

use std::collections::{HashMap, HashSet};
use std::str::FromStr;
use std::time::Duration;

use async_trait::async_trait;
use chrono::Utc;
use futures::StreamExt;
use ibapi::accounts::types::AccountGroup;
use ibapi::orders::{
    Action as IbAction, ExecutionData, ExecutionFilter, ExecutionSide, Executions,
    Order as IbOrder, OrderData, OrderStatus as IbOrderStatus, OrderStatusKind, PlaceOrder,
    TimeInForce as IbTimeInForce,
};
use ibapi::prelude::*;
use rust_decimal::Decimal;
use tokio::sync::mpsc::Sender;
use tracing::{debug, info, warn};

use trader_domain::market::{OrderEvent, SubscriptionHandle};
use trader_domain::{
    AccountSummary, Broker, BrokerError, Direction, EntryOrderType, Fill, Order, OrderId,
    OrderSide, OrderStatus, OrderType, Position, TimeInForce,
};

use super::config::IbkrConfig;

/// Adapter concreto de broker para Interactive Brokers.
#[derive(Debug, Clone)]
pub struct IbkrBrokerAdapter {
    config: IbkrConfig,
}

impl IbkrBrokerAdapter {
    pub fn new(config: IbkrConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Broker for IbkrBrokerAdapter {
    async fn place_order(&self, order: Order) -> Result<OrderId, BrokerError> {
        with_timeout("place_order", PLACE_ORDER_TIMEOUT, async move {
            info!(symbol = %order.symbol, side = ?order.side, qty = %order.quantity, "enviando ordem para IBKR");

            let client = connect(&self.config).await?;

            let contract = Contract::stock(&order.symbol).build();
            let quantity = f64_from_decimal(order.quantity)?;

            let result = match order.order_type {
                OrderType::Market | OrderType::Limit | OrderType::Stop => {
                    let ib_order = build_simple_order(&client, &contract, &order, quantity)?;
                    submit_and_confirm(&client, &contract, ib_order).await
                }
                OrderType::Bracket => {
                    let ib_orders = build_bracket_orders(&client, &contract, &order, quantity)?;
                    submit_bracket_and_confirm(&client, &contract, ib_orders).await
                }
                OrderType::StopLimit => Err(BrokerError::OrderRejected(
                    "stop-limit não suportado ainda".to_string(),
                )),
            };

            disconnect(&client).await;
            result.map(|broker_id| OrderId::from(broker_id.to_string()))
        })
        .await
    }

    async fn cancel_order(&self, id: &OrderId) -> Result<(), BrokerError> {
        with_timeout("cancel_order", BROKER_OP_TIMEOUT, async move {
            info!(%id, "cancelando ordem na IBKR");

            let client = connect(&self.config).await?;

            let numeric_id: i32 =
                id.0.parse()
                    .map_err(|e| BrokerError::Internal(format!("ID inválido: {e}")))?;

            let mut subscription = client
                .cancel_order(numeric_id, "")
                .await
                .map_err(|e| BrokerError::Internal(e.to_string()))?;

            // Consome a subscrição para confirmar cancelamento.
            while let Some(item) = subscription.next().await {
                match item {
                    Ok(SubscriptionItem::Data(_)) => {}
                    Ok(SubscriptionItem::Notice(n)) => {
                        warn!(notice = %n, "aviso no cancelamento");
                    }
                    // O código 202 ("Ordem cancelada") é a confirmação do
                    // cancelamento — mensagem informacional, não erro.
                    Err(ibapi::Error::Notice(n)) if n.is_cancellation() => {
                        info!(notice = %n, "cancelamento confirmado pelo gateway");
                        break;
                    }
                    Err(e) => {
                        disconnect(&client).await;
                        return Err(BrokerError::Internal(e.to_string()));
                    }
                }
            }

            disconnect(&client).await;
            Ok(())
        })
        .await
    }

    async fn get_order_status(&self, id: &OrderId) -> Result<OrderStatus, BrokerError> {
        debug!(%id, "consultando status da ordem na IBKR");

        let open_orders = self.get_open_orders().await?;
        let order = open_orders
            .iter()
            .find(|o| o.broker_order_id.as_deref() == Some(&id.0))
            .cloned();

        match order {
            Some(o) => Ok(o.status),
            None => Ok(OrderStatus::Filled),
        }
    }

    async fn get_open_orders(&self) -> Result<Vec<Order>, BrokerError> {
        with_timeout("get_open_orders", BROKER_OP_TIMEOUT, async move {
            debug!("consultando ordens abertas na IBKR");

            let client = connect(&self.config).await?;

            // `open_orders` retorna as ordens abertas deste client; o stream
            // termina sozinho ao receber `OpenOrderEnd` do gateway.
            let mut subscription = client
                .open_orders()
                .await
                .map_err(|e| BrokerError::Internal(e.to_string()))?;

            let result = collect_open_orders(&mut subscription).await;

            disconnect(&client).await;
            result
        })
        .await
    }

    async fn get_position(&self, symbol: &str) -> Result<Option<Position>, BrokerError> {
        let positions = self.get_positions().await?;
        Ok(positions.into_iter().find(|p| p.symbol == symbol))
    }

    async fn get_positions(&self) -> Result<Vec<Position>, BrokerError> {
        with_timeout("get_positions", BROKER_OP_TIMEOUT, async move {
            debug!("consultando posições abertas na IBKR");

            let client = connect(&self.config).await?;

            // `positions` é um stream contínuo: primeiro replay da lista completa,
            // encerrado por `PositionEnd`. Paramos aí e o drop cancela o stream.
            let mut subscription = client
                .positions()
                .await
                .map_err(|e| BrokerError::Internal(e.to_string()))?;

            let account_filter = self.config.account_id.as_deref().filter(|s| !s.is_empty());
            let result = collect_positions(&mut subscription, account_filter).await;

            disconnect(&client).await;
            result
        })
        .await
    }

    async fn get_account_summary(&self) -> Result<AccountSummary, BrokerError> {
        with_timeout("get_account_summary", BROKER_OP_TIMEOUT, async move {
            debug!("consultando resumo da conta na IBKR");

            let client = connect(&self.config).await?;

            let group = AccountGroup("All".to_string());
            let tags = [
                AccountSummaryTags::NET_LIQUIDATION,
                AccountSummaryTags::TOTAL_CASH_VALUE,
                AccountSummaryTags::BUYING_POWER,
            ];

            let mut subscription = client
                .account_summary(&group, &tags)
                .await
                .map_err(|e| BrokerError::Internal(e.to_string()))?;

            // String vazia na config significa "sem filtro de conta".
            let account_filter = self.config.account_id.as_deref().filter(|s| !s.is_empty());

            let values = collect_account_summary_values(&mut subscription, account_filter).await;

            disconnect(&client).await;
            let values = values?;

            let get = |tag: &str| values.get(tag).copied().unwrap_or(Decimal::ZERO);

            Ok(AccountSummary {
                broker: "ibkr".to_string(),
                account_id: account_filter.map(str::to_string),
                cash: get(AccountSummaryTags::TOTAL_CASH_VALUE),
                equity: get(AccountSummaryTags::NET_LIQUIDATION),
                buying_power: get(AccountSummaryTags::BUYING_POWER),
                // DailyPnL não é exposto via account summary (requer stream `pnl`
                // separado); fica zero até que essa fonte seja integrada.
                daily_pnl: Decimal::ZERO,
                timestamp: Utc::now(),
            })
        })
        .await
    }

    async fn subscribe_order_events(
        &self,
        tx: Sender<OrderEvent>,
    ) -> Result<SubscriptionHandle, BrokerError> {
        // Implementação via polling de `executions` (reqExecutions): a TWS API
        // retorna as execuções do dia da conta para qualquer cliente conectado,
        // o que dispensa manter um `Client` persistente no adapter. Um task em
        // background consulta a cada poucos segundos, deduplica por
        // `execution_id` e emite `OrderEvent::Fill`. O consumidor deve
        // deduplicar também de forma durável (banco), pois o replay do dia
        // recomeça a cada restart do processo.
        //
        // O poll abre uma conexão nova a cada tick e roda CONCORRENTE ao loop
        // principal (account summary, posições, ordens) — se compartilhasse o
        // mesmo client id do broker, duas conexões simultâneas seriam recusadas
        // pelo gateway com erro 326 (observado em 2026-08-20: 598 ocorrências
        // no dia, 1 CB em IWO). Por isso o poll usa um client id PRÓPRIO:
        // broker usa id da instância + 100 (101–111); o poll usa + 100 sobre
        // isso → instância + 200 (201–211), fora do range de market data
        // (1–11), do broker e do 99 de diagnóstico. Execuções do dia respondem
        // a qualquer client id conectado à conta (imutável por conexão).
        let mut config = self.config.clone();
        config.client_id = poll_client_id(config.client_id);
        info!(
            poll_client_id = config.client_id,
            "subscribe_order_events: polling de execuções IBKR com client id próprio"
        );
        tokio::spawn(async move {
            poll_executions_loop(config, tx).await;
        });
        info!("subscribe_order_events: polling de execuções IBKR iniciado");
        Ok(SubscriptionHandle {
            id: "ibkr-executions-poll".to_string(),
        })
    }
}

/// Timeout padrão de qualquer operação do broker na borda do adapter.
///
/// Um gateway travado (socket aberto, sem responder — cenário clássico de TWS
/// wedged) pendurava `get_positions`/`get_open_orders`/`get_account_summary`
/// PARA SEMPRE: os coletores drenam o stream até o marcador de fim, que nunca
/// chega. O circuit breaker conta erros, não travamentos, e nunca disparava
/// (A5 da auditoria de 30/08/2026). Com o timeout, o travamento vira erro e
/// alimenta o circuit breaker como qualquer outra falha do broker.
const BROKER_OP_TIMEOUT: Duration = Duration::from_secs(30);

/// Envio de ordem tem folga maior: `connect` faz até 4 tentativas com backoff
/// (~6 s só de sleeps) e `confirm_order` espera até 10 s pela confirmação.
const PLACE_ORDER_TIMEOUT: Duration = Duration::from_secs(60);

/// Envolve uma operação do broker em timeout, convertendo travamento em erro.
async fn with_timeout<T>(
    operation: &'static str,
    limit: Duration,
    fut: impl std::future::Future<Output = Result<T, BrokerError>>,
) -> Result<T, BrokerError> {
    match tokio::time::timeout(limit, fut).await {
        Ok(result) => result,
        Err(_) => {
            warn!(
                operation,
                timeout_s = limit.as_secs(),
                "operação do broker IBKR não respondeu dentro do timeout; gateway travado?"
            );
            Err(BrokerError::Internal(format!(
                "timeout de {}s em {operation}: gateway IBKR sem resposta",
                limit.as_secs()
            )))
        }
    }
}

/// Offset do client id do poll de execuções sobre o id do broker.
///
/// Broker = id da instância + 100; poll = broker + `EXECUTIONS_POLL_CLIENT_ID_OFFSET`
/// (= instância + 200). Range resultante no live: 201–211, único por instância.
const EXECUTIONS_POLL_CLIENT_ID_OFFSET: i32 = 100;

/// Deriva o client id do poll de execuções a partir do id do broker.
///
/// O poll roda em background, em paralelo ao loop principal que usa o id do
/// broker; dois client ids iguais em conexões simultâneas são recusados pelo
/// gateway (erro 326). O id do poll é deslocado para fora de todos os ranges:
/// market data 1–11, broker 101–111, diagnóstico 99.
fn poll_client_id(broker_client_id: i32) -> i32 {
    broker_client_id + EXECUTIONS_POLL_CLIENT_ID_OFFSET
}

fn f64_from_decimal(value: Decimal) -> Result<f64, BrokerError> {
    value
        .try_into()
        .map_err(|e| BrokerError::Internal(format!("falha ao converter Decimal: {e}")))
}

/// Intervalo do polling de execuções do modo live.
const EXECUTIONS_POLL_SECS: u64 = 15;

/// Timeout para drenar o stream de `executions` (ele encerra sozinho após o
/// `execDetailsEnd` do gateway; o timeout é só proteção contra stream preso).
const EXECUTIONS_DRAIN_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(20);

/// Loop de polling de execuções do dia: emite `OrderEvent::Fill` até o canal
/// fechar (consumidor encerrado).
async fn poll_executions_loop(config: IbkrConfig, tx: Sender<OrderEvent>) {
    let mut seen: HashSet<String> = HashSet::new();
    let mut tick = tokio::time::interval(std::time::Duration::from_secs(EXECUTIONS_POLL_SECS));

    loop {
        tick.tick().await;

        let events = match fetch_today_executions(&config, &mut seen).await {
            Ok(events) => events,
            Err(e) => {
                warn!(error = %e, "falha ao consultar execuções do dia; tentando no próximo ciclo");
                continue;
            }
        };

        for event in events {
            if tx.send(event).await.is_err() {
                info!("canal de eventos de ordem fechado; encerrando polling de execuções");
                return;
            }
        }
    }
}

/// Consulta as execuções do dia na conta e retorna eventos de fill ainda não
/// vistos neste processo (dedupe em memória por `execution_id`).
async fn fetch_today_executions(
    config: &IbkrConfig,
    seen: &mut HashSet<String>,
) -> Result<Vec<OrderEvent>, BrokerError> {
    let client = connect(config).await?;

    let filter = ExecutionFilter {
        account_code: config.account_id.clone().unwrap_or_default(),
        ..ExecutionFilter::default()
    };

    let mut subscription = client
        .executions(filter)
        .await
        .map_err(|e| BrokerError::Internal(e.to_string()))?;

    let mut events = Vec::new();
    // exec_id → posição em `events`, para casar o CommissionReport (que
    // chega logo após o ExecutionData da mesma execução) com o fill.
    let mut index_by_exec: HashMap<String, usize> = HashMap::new();

    let drain = async {
        while let Some(item) = subscription.next().await {
            match item {
                Ok(SubscriptionItem::Data(Executions::ExecutionData(data))) => {
                    let exec = &data.execution;
                    if !seen.insert(exec.execution_id.clone()) {
                        continue;
                    }
                    match map_execution_to_event(&data) {
                        Ok(event) => {
                            index_by_exec.insert(exec.execution_id.clone(), events.len());
                            events.push(event);
                        }
                        Err(e) => {
                            warn!(error = %e, exec_id = %exec.execution_id, "execução ignorada")
                        }
                    }
                }
                Ok(SubscriptionItem::Data(Executions::CommissionReport(report))) => {
                    // Comissão real da execução: aplica no fill correspondente
                    // deste lote (relatórios de execuções já vistas em polls
                    // anteriores não estão no índice e são ignorados).
                    if let Some(&idx) = index_by_exec.get(&report.execution_id) {
                        if let OrderEvent::Fill { fill, .. } = &mut events[idx] {
                            match decimal_from_f64(report.commission) {
                                Ok(commission) => fill.commission = commission,
                                Err(e) => warn!(
                                    error = %e,
                                    exec_id = %report.execution_id,
                                    "comissão inválida no CommissionReport"
                                ),
                            }
                        }
                    }
                }
                Ok(SubscriptionItem::Notice(n)) => {
                    debug!(notice = %n, "aviso ao consultar execuções");
                }
                Err(e) => {
                    return Err(BrokerError::Internal(e.to_string()));
                }
            }
        }
        Ok(())
    };

    let result = tokio::time::timeout(EXECUTIONS_DRAIN_TIMEOUT, drain)
        .await
        .unwrap_or_else(|_| {
            warn!("timeout ao drenar stream de execuções");
            Ok(())
        });

    disconnect(&client).await;
    result.map(|_| events)
}

/// Converte um `ExecutionData` do ibapi em `OrderEvent::Fill` de domínio.
///
/// `Fill.order_id` fica zero: o adapter não conhece o id interno do banco —
/// quem consome o evento resolve a ordem correspondente. O id da ordem na
/// IBKR vai em `OrderEvent::Fill.order_id`.
fn map_execution_to_event(data: &ExecutionData) -> Result<OrderEvent, BrokerError> {
    let exec = &data.execution;

    let side = match exec.side {
        ExecutionSide::Bought => OrderSide::Buy,
        ExecutionSide::Sold => OrderSide::Sell,
    };

    let mut fill = Fill::new(
        0,
        data.contract.symbol.as_str(),
        side,
        decimal_from_f64(exec.price)?,
        decimal_from_f64(exec.shares)?,
        parse_execution_time(&exec.time),
    )
    .map_err(|e| BrokerError::Internal(e.to_string()))?;
    fill.broker_fill_id = Some(exec.execution_id.clone());

    Ok(OrderEvent::Fill {
        order_id: OrderId::from(exec.order_id.to_string()),
        fill,
    })
}

/// Faz parse do timestamp de execução da IBKR.
///
/// Formato: `yyyymmdd  hh:mm:ss`, às vezes com sufixo de timezone
/// (ex.: `US/Eastern`). **A TWS reporta no fuso local do gateway/exchange,
/// não em UTC** — sem conversão, os fills ficam 4-5h no passado (bug observado
/// em live em 2026-08-04). Sem timezone explícita, assume America/New_York.
/// Para reconciliação fina com o statement da corretora, use `broker_fill_id`.
fn parse_execution_time(raw: &str) -> chrono::DateTime<Utc> {
    let mut parts = raw.split_whitespace();
    let date = parts.next().unwrap_or_default();
    let time = parts.next().unwrap_or_default();
    let tz_token = parts.next();

    let naive = chrono::NaiveDateTime::parse_from_str(&format!("{date} {time}"), "%Y%m%d %H:%M:%S");
    let naive = match naive {
        Ok(dt) => dt,
        Err(e) => {
            warn!(raw, error = %e, "timestamp de execução inválido; usando agora");
            return Utc::now();
        }
    };

    let tz = tz_token
        .and_then(|name| name.parse::<chrono_tz::Tz>().ok())
        .unwrap_or(chrono_tz::America::New_York);

    use chrono::TimeZone;
    match tz.from_local_datetime(&naive) {
        chrono::LocalResult::Single(dt) => dt.with_timezone(&Utc),
        // Horário ambíguo (virada do DST): usa a ocorrência mais cedo.
        chrono::LocalResult::Ambiguous(dt, _) => dt.with_timezone(&Utc),
        chrono::LocalResult::None => {
            warn!(
                raw,
                "timestamp de execução inexistente no fuso; usando agora"
            );
            Utc::now()
        }
    }
}

/// Constrói a ordem ibapi para tipos simples (market/limit/stop).
fn build_simple_order(
    client: &Client,
    contract: &Contract,
    order: &Order,
    quantity: f64,
) -> Result<IbOrder, BrokerError> {
    let builder = client.order(contract);

    let built = match order.order_type {
        OrderType::Market => match order.side {
            OrderSide::Buy => builder.buy(quantity).market().build_order(),
            OrderSide::Sell => builder.sell(quantity).market().build_order(),
        },
        OrderType::Limit => {
            let price: f64 = order
                .price
                .and_then(|p| p.try_into().ok())
                .ok_or_else(|| BrokerError::OrderRejected("preço limit ausente".to_string()))?;
            match order.side {
                OrderSide::Buy => builder.buy(quantity).limit(price).build_order(),
                OrderSide::Sell => builder.sell(quantity).limit(price).build_order(),
            }
        }
        OrderType::Stop => {
            let stop: f64 = order
                .stop_price
                .and_then(|p| p.try_into().ok())
                .ok_or_else(|| BrokerError::OrderRejected("stop ausente".to_string()))?;
            match order.side {
                OrderSide::Buy => builder.buy(quantity).stop(stop).build_order(),
                OrderSide::Sell => builder.sell(quantity).stop(stop).build_order(),
            }
        }
        other => {
            return Err(BrokerError::OrderRejected(format!(
                "tipo {other:?} não é ordem simples"
            )))
        }
    };

    built.map_err(|e| BrokerError::OrderRejected(e.to_string()))
}

/// Constrói as 3 ordens do bracket (parent + take profit + stop loss).
fn build_bracket_orders(
    client: &Client,
    contract: &Contract,
    order: &Order,
    quantity: f64,
) -> Result<Vec<IbOrder>, BrokerError> {
    let entry_price: f64 = order
        .price
        .and_then(|p| p.try_into().ok())
        .ok_or_else(|| BrokerError::OrderRejected("preço de entrada ausente".to_string()))?;
    let stop_price: f64 = order
        .stop_price
        .and_then(|p| p.try_into().ok())
        .ok_or_else(|| BrokerError::OrderRejected("stop ausente".to_string()))?;
    let take_profit: f64 = order
        .target_price
        .and_then(|p| p.try_into().ok())
        .ok_or_else(|| BrokerError::OrderRejected("alvo ausente".to_string()))?;

    // Entrada stop (buy stop no gatilho — regra do livro): o builder de
    // bracket do ibapi só suporta entrada limit/market, então montamos as 3
    // ordens manualmente. Ids, parent_id e transmit são atribuídos em
    // `submit_bracket_and_confirm`.
    if order.entry_order_type == EntryOrderType::Stop {
        return Ok(build_stop_entry_bracket(
            order,
            quantity,
            entry_price,
            stop_price,
            take_profit,
        ));
    }

    let builder = client.order(contract);
    let built = match order.side {
        OrderSide::Buy => builder
            .buy(quantity)
            .bracket()
            .entry_limit(entry_price)
            .stop_loss(stop_price)
            .take_profit(take_profit)
            .build(),
        OrderSide::Sell => builder
            .sell(quantity)
            .bracket()
            .entry_limit(entry_price)
            .stop_loss(stop_price)
            .take_profit(take_profit)
            .build(),
    };

    built.map_err(|e| BrokerError::OrderRejected(e.to_string()))
}

/// Monta um bracket com entrada STP (parent stop + TP limit + SL stop).
fn build_stop_entry_bracket(
    order: &Order,
    quantity: f64,
    entry_price: f64,
    stop_price: f64,
    take_profit: f64,
) -> Vec<IbOrder> {
    let (entry_action, exit_action) = match order.side {
        OrderSide::Buy => (IbAction::Buy, IbAction::Sell),
        OrderSide::Sell => (IbAction::Sell, IbAction::Buy),
    };

    let parent = IbOrder {
        action: entry_action,
        order_type: "STP".to_string(),
        total_quantity: quantity,
        aux_price: Some(entry_price),
        tif: IbTimeInForce::Day,
        transmit: false,
        ..Default::default()
    };
    let tp = IbOrder {
        action: exit_action,
        order_type: "LMT".to_string(),
        total_quantity: quantity,
        limit_price: Some(take_profit),
        tif: IbTimeInForce::Day,
        transmit: false,
        ..Default::default()
    };
    let sl = IbOrder {
        action: exit_action,
        order_type: "STP".to_string(),
        total_quantity: quantity,
        aux_price: Some(stop_price),
        tif: IbTimeInForce::Day,
        transmit: true,
        ..Default::default()
    };

    vec![parent, tp, sl]
}

/// Submete uma ordem simples e aguarda a confirmação do gateway.
///
/// Retorna o order id atribuído pela IBKR.
async fn submit_and_confirm(
    client: &Client,
    contract: &Contract,
    mut ib_order: IbOrder,
) -> Result<i32, BrokerError> {
    let order_id = client.next_order_id();
    ib_order.order_id = order_id;

    let mut subscription = client
        .place_order(order_id, contract, &ib_order)
        .await
        .map_err(|e| BrokerError::OrderRejected(e.to_string()))?;

    confirm_order(&mut subscription, order_id).await?;
    Ok(order_id)
}

/// Submete um bracket (parent + filhos) e aguarda a confirmação do parent.
///
/// Espelha o `submit_all` do ibapi: apenas a última ordem transmite, o que
/// libera a cadeia inteira. O parent vai por `place_order` para podermos
/// monitorar rejeições; os filhos vão fire-and-forget.
async fn submit_bracket_and_confirm(
    client: &Client,
    contract: &Contract,
    mut orders: Vec<IbOrder>,
) -> Result<i32, BrokerError> {
    if orders.len() != 3 {
        return Err(BrokerError::Internal(format!(
            "bracket deveria ter 3 ordens, veio {}",
            orders.len()
        )));
    }

    let parent_id = client.next_order_id();
    let tp_id = client.next_order_id();
    let sl_id = client.next_order_id();
    let ids = [parent_id, tp_id, sl_id];

    for (i, order) in orders.iter_mut().enumerate() {
        order.order_id = ids[i];
        if i > 0 {
            order.parent_id = parent_id;
        }
        order.transmit = i == 2;
    }

    let mut subscription = client
        .place_order(parent_id, contract, &orders[0])
        .await
        .map_err(|e| BrokerError::OrderRejected(e.to_string()))?;
    client
        .submit_order(tp_id, contract, &orders[1])
        .await
        .map_err(|e| BrokerError::OrderRejected(e.to_string()))?;
    client
        .submit_order(sl_id, contract, &orders[2])
        .await
        .map_err(|e| BrokerError::OrderRejected(e.to_string()))?;

    confirm_order(&mut subscription, parent_id).await?;
    Ok(parent_id)
}

/// Aguarda a primeira resposta do gateway para capturar rejeições síncronas
/// (ex.: buying power insuficiente — código 201, ou status `Inactive`).
///
/// A TWS API confirma ordens de forma assíncrona: o submit retorna o id antes
/// de qualquer status. Sem esta confirmação, uma ordem rejeitada parecia
/// aceita e o bot podia reenviar sinais (observado em sessão live real). Se o
/// gateway não responder dentro do timeout, assumimos a ordem como aceita —
/// ela foi transmitida — e registramos um aviso.
async fn confirm_order(
    subscription: &mut Subscription<PlaceOrder>,
    order_id: i32,
) -> Result<(), BrokerError> {
    const CONFIRM_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

    let result = tokio::time::timeout(CONFIRM_TIMEOUT, async {
        while let Some(item) = subscription.next().await {
            match item {
                Ok(SubscriptionItem::Data(PlaceOrder::OpenOrder(data)))
                    if data.order_id == order_id =>
                {
                    if data.order_state.status == OrderStatusKind::Inactive {
                        return Err(BrokerError::OrderRejected(
                            "ordem ficou inativa no gateway".to_string(),
                        ));
                    }
                    return Ok(());
                }
                Ok(SubscriptionItem::Data(PlaceOrder::OrderStatus(status)))
                    if status.order_id == order_id =>
                {
                    match status.status {
                        OrderStatusKind::Inactive
                        | OrderStatusKind::Cancelled
                        | OrderStatusKind::ApiCancelled => {
                            return Err(BrokerError::OrderRejected(format!(
                                "status {:?}",
                                status.status
                            )));
                        }
                        _ => return Ok(()),
                    }
                }
                Ok(SubscriptionItem::Data(_)) => {}
                Ok(SubscriptionItem::Notice(n)) => {
                    if let Some(e) = rejection_from_notice(&n) {
                        return Err(e);
                    }
                    debug!(notice = %n, "aviso ao confirmar ordem");
                }
                Err(ibapi::Error::Notice(n)) => {
                    if let Some(e) = rejection_from_notice(&n) {
                        return Err(e);
                    }
                    warn!(notice = %n, "aviso ao confirmar ordem");
                }
                Err(e) => return Err(BrokerError::Internal(e.to_string())),
            }
        }
        // Stream acabou sem status. A ordem JÁ FOI TRANSMITIDA — ausência de
        // confirmação não é rejeição, e tratá-la como erro é pior do que o
        // problema que se quer evitar: o chamador acha que falhou, não
        // rastreia a ordem, e ela fica órfã no broker (foi assim que 827 ações
        // de IWM ficaram penduradas desde 07/08/2026). Uma ordem que a IBKR
        // aceita mas segura para a abertura — aviso 399, "will not be placed
        // at the exchange until 09:30" — cai exatamente aqui.
        //
        // Mesma decisão do caminho de timeout logo abaixo: assume aceita e
        // avisa. Rejeição de verdade chega como Notice 2xx ou status
        // Inactive/Cancelled, e essas continuam virando erro.
        warn!(
            order_id,
            "stream de confirmação encerrou sem status; ordem foi transmitida, assumindo aceita"
        );
        Ok(())
    })
    .await;

    match result {
        Ok(inner) => inner,
        Err(_) => {
            warn!(
                order_id,
                "gateway não confirmou ordem em 10s; assumindo aceita"
            );
            Ok(())
        }
    }
}

/// Códigos 2xx (exceto 202, confirmação de cancelamento) indicam que a ordem
/// não vai trabalhar — 201 é "Order rejected" (ex.: buying power insuficiente).
fn rejection_from_notice(notice: &Notice) -> Option<BrokerError> {
    match notice.code {
        200..=299 if notice.code != 202 => Some(BrokerError::OrderRejected(notice.message.clone())),
        _ => None,
    }
}

/// Consome o stream de `account_summary` até o `End` e agrega os valores por tag.
///
/// Cada tag chega em múltiplas linhas (uma por moeda); preferimos a linha na
/// moeda BASE para evitar valores convertidos/duplicados.
async fn collect_account_summary_values(
    subscription: &mut Subscription<AccountSummaryResult>,
    account_filter: Option<&str>,
) -> Result<HashMap<String, Decimal>, BrokerError> {
    let mut values: HashMap<String, Decimal> = HashMap::new();

    while let Some(item) = subscription.next().await {
        match item {
            Ok(SubscriptionItem::Data(AccountSummaryResult::Summary(summary))) => {
                if let Some(account) = account_filter {
                    if summary.account != account {
                        continue;
                    }
                }
                let is_base = summary.currency == "BASE" || summary.currency.is_empty();
                if is_base || !values.contains_key(&summary.tag) {
                    match Decimal::from_str(&summary.value) {
                        Ok(value) => {
                            values.insert(summary.tag, value);
                        }
                        Err(e) => {
                            warn!(tag = %summary.tag, value = %summary.value, error = %e,
                                "valor de account summary não numérico, ignorado");
                        }
                    }
                }
            }
            Ok(SubscriptionItem::Data(AccountSummaryResult::End)) => break,
            Ok(SubscriptionItem::Notice(n)) => {
                warn!(notice = %n, "aviso no resumo da conta");
            }
            Err(e) => {
                return Err(BrokerError::Internal(e.to_string()));
            }
        }
    }

    Ok(values)
}

/// Consome o replay inicial do stream de `positions` (até `PositionEnd`) e
/// converte para o domínio.
async fn collect_positions(
    subscription: &mut Subscription<PositionUpdate>,
    account_filter: Option<&str>,
) -> Result<Vec<Position>, BrokerError> {
    let mut positions: Vec<Position> = Vec::new();

    while let Some(item) = subscription.next().await {
        match item {
            Ok(SubscriptionItem::Data(PositionUpdate::Position(p))) => {
                if let Some(account) = account_filter {
                    if p.account != account {
                        continue;
                    }
                }
                if p.position == 0.0 {
                    continue;
                }

                let quantity = decimal_from_f64(p.position.abs())?;
                let avg_entry_price = decimal_from_f64(p.average_cost)?;
                let direction = if p.position > 0.0 {
                    Direction::Long
                } else {
                    Direction::Short
                };

                // Snapshot do broker: não há signal/stop associados, então
                // stop_price fica zero e os metadados registram a origem.
                let mut position = Position::new(
                    p.contract.symbol.as_str(),
                    0,
                    direction,
                    quantity,
                    avg_entry_price,
                    Decimal::ZERO,
                    "ibkr",
                )
                .map_err(|e| BrokerError::Internal(e.to_string()))?;
                position.metadata = serde_json::json!({
                    "account": p.account,
                    "source": "ibkr_positions",
                });
                positions.push(position);
            }
            Ok(SubscriptionItem::Data(PositionUpdate::PositionEnd)) => break,
            Ok(SubscriptionItem::Notice(n)) => {
                warn!(notice = %n, "aviso ao consultar posições");
            }
            Err(e) => {
                return Err(BrokerError::Internal(e.to_string()));
            }
        }
    }

    Ok(positions)
}

/// Consome o stream de `open_orders` até o `OpenOrderEnd` e converte para o domínio.
async fn collect_open_orders(
    subscription: &mut Subscription<Orders>,
) -> Result<Vec<Order>, BrokerError> {
    let mut orders: Vec<Order> = Vec::new();
    let mut statuses: HashMap<i32, IbOrderStatus> = HashMap::new();

    while let Some(item) = subscription.next().await {
        match item {
            Ok(SubscriptionItem::Data(Orders::OrderData(data))) => {
                orders.push(map_order_data(&data)?);
            }
            Ok(SubscriptionItem::Data(Orders::OrderStatus(status))) => {
                statuses.insert(status.order_id, status);
            }
            Ok(SubscriptionItem::Notice(n)) => {
                warn!(notice = %n, "aviso ao consultar ordens abertas");
            }
            Err(e) => {
                return Err(BrokerError::Internal(e.to_string()));
            }
        }
    }

    // Consolida dados de execução (filled/avg price) recebidos via OrderStatus.
    for order in &mut orders {
        let ib_order_id = order
            .broker_order_id
            .as_deref()
            .and_then(|id| id.parse::<i32>().ok());
        if let Some(status) = ib_order_id.and_then(|id| statuses.get(&id)) {
            order.status = map_order_status(status.status);
            order.filled_quantity = decimal_from_f64(status.filled)?;
            order.avg_fill_price = optional_price(status.average_fill_price)?;
            if status.filled > 0.0 && status.remaining > 0.0 {
                order.status = OrderStatus::PartiallyFilled;
            }
            if order.status == OrderStatus::Filled {
                order.filled_at = Some(Utc::now());
            }
        }
    }

    Ok(orders)
}

/// Encerra a conexão com timeout.
///
/// `Client::disconnect` pode bloquear indefinidamente quando ainda há
/// subscrições abertas (ex.: a stream interna do `submit`); nesse caso o
/// `Drop` do `Client` já sinaliza o shutdown do message bus.
async fn disconnect(client: &Client) {
    if tokio::time::timeout(std::time::Duration::from_secs(5), client.disconnect())
        .await
        .is_err()
    {
        warn!("timeout ao encerrar conexão com a IBKR");
    }
}

/// Conecta ao gateway com retry.
///
/// O gateway ocasionalmente derruba o handshake (`early eof`) sob conexões em
/// rajada — falha transitória observada na prática. Algumas tentativas com
/// backoff crescente absorvem o problema sem mascarar falhas persistentes.
async fn connect(config: &IbkrConfig) -> Result<Client, BrokerError> {
    const ATTEMPTS: u32 = 4;

    let mut last_error = String::new();
    for attempt in 1..=ATTEMPTS {
        match Client::connect(&config.connection_string(), config.client_id).await {
            Ok(client) => return Ok(client),
            Err(e) => {
                last_error = e.to_string();
                if attempt < ATTEMPTS {
                    warn!(attempt, error = %last_error, "falha ao conectar na IBKR; tentando novamente");
                    tokio::time::sleep(std::time::Duration::from_secs(u64::from(attempt))).await;
                }
            }
        }
    }
    Err(BrokerError::ConnectionFailed(last_error))
}

/// Converte `f64` vindo do ibapi para `Decimal` na borda do adapter.
fn decimal_from_f64(value: f64) -> Result<Decimal, BrokerError> {
    Decimal::try_from(value)
        .map_err(|e| BrokerError::Internal(format!("falha ao converter f64 para Decimal: {e}")))
}

/// Converte preço opcional do ibapi, tratando zero/ausente como `None`.
fn optional_price(value: Option<f64>) -> Result<Option<Decimal>, BrokerError> {
    match value {
        Some(v) if v > 0.0 => Ok(Some(decimal_from_f64(v)?)),
        _ => Ok(None),
    }
}

/// Mapeia um `OrderData` do ibapi para a `Order` de domínio.
fn map_order_data(data: &OrderData) -> Result<Order, BrokerError> {
    let ib_order = &data.order;

    let side = match ib_order.action {
        IbAction::Buy => OrderSide::Buy,
        IbAction::Sell | IbAction::SellShort | IbAction::SellLong => OrderSide::Sell,
    };

    let order_type = match ib_order.order_type.as_str() {
        "MKT" => OrderType::Market,
        "LMT" => OrderType::Limit,
        "STP" => OrderType::Stop,
        "STP LMT" => OrderType::StopLimit,
        other => {
            warn!(
                ib_order_type = other,
                "tipo de ordem IBKR desconhecido, mapeando para Market"
            );
            OrderType::Market
        }
    };

    let quantity = decimal_from_f64(ib_order.total_quantity)?;
    let mut order = Order::new(
        data.contract.symbol.as_str(),
        side,
        order_type,
        quantity,
        "ibkr",
    )
    .map_err(|e| BrokerError::Internal(e.to_string()))?;

    order.broker_order_id = Some(data.order_id.to_string());
    order.entry_order_type = if ib_order.order_type == "STP" {
        trader_domain::EntryOrderType::Stop
    } else {
        trader_domain::EntryOrderType::Limit
    };
    order.status = map_order_status(data.order_state.status);
    order.time_in_force = map_time_in_force(&ib_order.tif);
    order.price = optional_price(ib_order.limit_price)?;
    order.stop_price = optional_price(ib_order.aux_price)?;
    order.parent_order_id = (ib_order.parent_id != 0).then_some(i64::from(ib_order.parent_id));
    order.submitted_at = Some(Utc::now());
    order.metadata = serde_json::json!({
        "perm_id": ib_order.perm_id,
        "ib_order_type": ib_order.order_type,
    });

    Ok(order)
}

/// Mapeia o status de ordem do ibapi para o status de domínio.
fn map_order_status(kind: OrderStatusKind) -> OrderStatus {
    match kind {
        OrderStatusKind::ApiPending | OrderStatusKind::PendingSubmit => OrderStatus::Pending,
        OrderStatusKind::PreSubmitted => OrderStatus::Accepted,
        OrderStatusKind::Submitted | OrderStatusKind::PendingCancel => OrderStatus::Submitted,
        OrderStatusKind::ApiCancelled | OrderStatusKind::Cancelled => OrderStatus::Cancelled,
        OrderStatusKind::Filled => OrderStatus::Filled,
        OrderStatusKind::Inactive => OrderStatus::Rejected,
    }
}

/// Mapeia o time-in-force do ibapi para o de domínio.
fn map_time_in_force(tif: &IbTimeInForce) -> TimeInForce {
    match tif {
        IbTimeInForce::GoodTilCanceled | IbTimeInForce::GoodTilDate => TimeInForce::Gtc,
        IbTimeInForce::ImmediateOrCancel => TimeInForce::Ioc,
        IbTimeInForce::FillOrKill => TimeInForce::Fok,
        _ => TimeInForce::Day,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn poll_client_id_fica_fora_de_todos_os_ranges_reservados() {
        // Broker usa id da instância + 100 (101–111); o poll usa + 100 sobre
        // isso (201–211). Nenhum range deve se sobrepor: market data 1–11,
        // broker 101–111, diagnóstico 99.
        for instancia in 1..=11 {
            let broker = instancia + 100;
            let poll = poll_client_id(broker);
            assert!(!(1..=11).contains(&poll), "poll colidiu com market data");
            assert!(!(101..=111).contains(&poll), "poll colidiu com broker");
            assert_ne!(poll, 99, "poll colidiu com id de diagnóstico");
        }
    }

    #[test]
    fn poll_client_id_preserva_unicidade_entre_instancias() {
        let ids: Vec<i32> = (1..=11).map(|i| poll_client_id(i + 100)).collect();
        let dedup: std::collections::HashSet<i32> = ids.iter().copied().collect();
        assert_eq!(ids.len(), dedup.len(), "ids do poll devem ser únicos");
        assert_eq!(ids[0], 201);
        assert_eq!(ids[10], 211);
    }
}
