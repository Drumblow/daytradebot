//! Comando `flatten` — encerramento MANUAL de uma posição no broker.
//!
//! Existe para o caso que a automação, de propósito, não resolve: posição que
//! nenhuma instância rastreia. O flatten de fim de sessão fecha apenas o que a
//! própria instância abriu — fechar posição de origem desconhecida a mercado
//! não é decisão de automação, é decisão de operação. Este comando é a mão
//! humana, com registro.
//!
//! O caso que motivou: 827 ações de IWM abertas em 07/08/2026, sem stop, que
//! sobraram do kill da sessão IBKR daquele dia. Elas travaram as duas
//! instâncias de IWM por 18 pregões — a reconciliação recusa novos setups
//! enquanto houver exposição no símbolo.
//!
//! Guardas, na ordem em que agem:
//!   1. sem `--confirm`, apenas mostra o que faria e sai com erro;
//!   2. porta de dinheiro real (7496/4001) é recusada — este comando é para a
//!      conta paper;
//!   3. sem posição no símbolo, não há o que fazer e a saída é limpa.

use anyhow::{bail, Result};
use tracing::{info, warn};

use trader_adapters::ibkr::IbkrBrokerAdapter;
use trader_domain::{Broker, Direction, Order, OrderId, OrderSide, OrderType, Position};

use crate::config::CliConfig;

/// Portas de conta com dinheiro real. Mesma lista que o live recusa.
const REAL_MONEY_PORTS: [u16; 2] = [7496, 4001];

pub async fn run(config: &CliConfig, symbol: &str, confirm: bool) -> Result<()> {
    if config.provider != "ibkr" {
        bail!(
            "flatten só faz sentido contra o broker real; provider atual: {}",
            config.provider
        );
    }

    let ibkr = config.ibkr_config()?;
    if REAL_MONEY_PORTS.contains(&ibkr.port) {
        bail!(
            "porta {} é de conta com dinheiro real; este comando é para a conta paper",
            ibkr.port
        );
    }

    let broker = IbkrBrokerAdapter::new(ibkr);

    let Some(position) = broker.get_position(symbol).await? else {
        println!("Nenhuma posição aberta em {symbol}. Nada a fazer.");
        return Ok(());
    };

    let exit_side = match position.direction {
        Direction::Long => OrderSide::Sell,
        Direction::Short => OrderSide::Buy,
    };

    println!("Posição encontrada em {symbol}:");
    println!(
        "  {:?} qty={} entrada={} stop={}",
        position.direction, position.quantity, position.avg_entry_price, position.stop_price
    );
    println!(
        "\nAção: enviar {:?} A MERCADO de {} {} para zerar a posição,",
        exit_side, position.quantity, symbol
    );
    println!("      e depois cancelar as ordens do lado da saída que sobrarem.");

    if !confirm {
        bail!("nada foi enviado. Repita com --confirm para executar de verdade.");
    }

    close_at_market(&broker, symbol, &position, exit_side).await?;
    cancel_exit_side_orders(&broker, symbol, exit_side).await;

    // Confere o resultado no próprio broker: um comando que fecha posição e
    // não mostra o depois obriga alguém a conferir por fora.
    match broker.get_position(symbol).await? {
        None => println!("\nOK: {symbol} não tem mais posição aberta."),
        Some(restante) => println!(
            "\nATENÇÃO: {symbol} ainda mostra {:?} qty={} — confira antes do próximo pregão.",
            restante.direction, restante.quantity
        ),
    }

    Ok(())
}

/// Fecha a mercado, com retentativa. Fecha ANTES de cancelar qualquer perna de
/// proteção: se o envio falhar, a posição continua protegida pelo que houver.
async fn close_at_market<B: Broker>(
    broker: &B,
    symbol: &str,
    position: &Position,
    exit_side: OrderSide,
) -> Result<()> {
    const TENTATIVAS: u32 = 3;
    let mut ultimo_erro = None;

    for tentativa in 1..=TENTATIVAS {
        let order = Order::new(
            symbol,
            exit_side,
            OrderType::Market,
            position.quantity,
            "ibkr",
        )
        .map_err(|e| anyhow::anyhow!("ordem de fechamento inválida: {e}"))?;
        match broker.place_order(order).await {
            Ok(id) => {
                info!(%symbol, %id, tentativa, "ordem de fechamento manual enviada");
                println!("\nOrdem de fechamento enviada: {id}");
                return Ok(());
            }
            Err(e) => {
                warn!(%symbol, tentativa, error = %e, "falha ao enviar fechamento; retentando");
                ultimo_erro = Some(e.to_string());
                if tentativa < TENTATIVAS {
                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                }
            }
        }
    }

    bail!(
        "falha ao fechar {symbol} após {TENTATIVAS} tentativas: {}",
        ultimo_erro.unwrap_or_default()
    )
}

/// Cancela as ordens do lado da saída (pernas de proteção órfãs). Filtrar pelo
/// lado evita derrubar a entrada pendente de outra estratégia no mesmo ativo —
/// IWM, IWV e AVUV rodam com duas instâncias cada.
async fn cancel_exit_side_orders<B: Broker>(broker: &B, symbol: &str, exit_side: OrderSide) {
    let orders = match broker.get_open_orders().await {
        Ok(orders) => orders,
        Err(e) => {
            warn!(%symbol, error = %e, "falha ao listar ordens abertas para cancelar");
            return;
        }
    };
    for order in orders
        .iter()
        .filter(|o| o.symbol == symbol && o.side == exit_side)
    {
        if let Some(broker_order_id) = &order.broker_order_id {
            let id = OrderId::from(broker_order_id.clone());
            match broker.cancel_order(&id).await {
                Ok(()) => println!("Ordem {id} cancelada."),
                Err(e) => warn!(order_id = %id, error = %e, "falha ao cancelar ordem"),
            }
        }
    }
}
