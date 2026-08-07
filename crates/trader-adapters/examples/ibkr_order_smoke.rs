//! Smoke test do ciclo completo de ordens contra a conta paper da IBKR.
//!
//! ATENÇÃO: envia ordens reais à conta paper (gateway 127.0.0.1:7497).
//!
//! Usa `client_id` próprio (padrão 2, sobrescrita via `IBKR_SMOKE_CLIENT_ID`)
//! porque a TWS API só permite uma conexão por client_id e a sessão
//! `paper --mode live` já ocupa o client_id 1.
//!
//! Etapas:
//! 1. Ordem LIMIT de compra bem abaixo do mercado: envia, confirma em
//!    `get_open_orders`, cancela, confirma que sumiu.
//! 2. Roundtrip MARKET: compra 1 ação, confirma posição em `get_position`,
//!    vende 1 ação, confirma que a posição zerou.
//! 3. Rejeição síncrona: ordem com quantidade absurda (buying power
//!    insuficiente) deve ser rejeitada já no `place_order`.
//!
//! Sai com exit code não-zero se alguma etapa falhar.

use std::time::{Duration, Instant};

use rust_decimal::Decimal;
use trader_adapters::ibkr::{IbkrBrokerAdapter, IbkrConfig, IbkrMarketDataProvider};
use trader_domain::{Broker, MarketDataProvider, Order, OrderSide, OrderType};

const SYMBOL: &str = "SPY";
const POLL_INTERVAL: Duration = Duration::from_secs(2);
const POLL_TIMEOUT: Duration = Duration::from_secs(30);
const CANCEL_TIMEOUT: Duration = Duration::from_secs(20);

#[tokio::main]
async fn main() {
    let _ = dotenvy::dotenv();

    let config = IbkrConfig {
        host: std::env::var("TRADER__IBKR__HOST").unwrap_or_else(|_| "127.0.0.1".to_string()),
        port: std::env::var("TRADER__IBKR__PORT")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(7497),
        client_id: std::env::var("IBKR_SMOKE_CLIENT_ID")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(2),
        account_id: std::env::var("TRADER__IBKR__ACCOUNT_ID")
            .ok()
            .filter(|s| !s.is_empty()),
        paper: true,
    };

    println!("🚀 Smoke test de ordens IBKR (conta paper)");
    println!(
        "   gateway: {} | client_id: {}\n",
        config.connection_string(),
        config.client_id
    );

    let broker = IbkrBrokerAdapter::new(config.clone());
    let market_data = IbkrMarketDataProvider::new(config);

    let mut failures = 0u32;

    // ── Etapa 0: cotação de referência ────────────────────────────────
    let reference = match market_data.get_quote(SYMBOL).await {
        Ok(quote) => {
            println!("✅ Cotação {SYMBOL}: ~{}", quote.bid);
            quote.bid
        }
        Err(e) => {
            println!("⚠️  falha ao buscar cotação ({e}); usando referência fixa 600.00");
            Decimal::from(600)
        }
    };

    // ── Etapa 1: ordem LIMIT pendente + cancelamento ──────────────────
    println!("\n── Etapa 1: ordem LIMIT pendente (não deve executar) ──");
    let limit_price = (reference * Decimal::from(90) / Decimal::from(100)).round_dp(2);

    let mut limit_order = match Order::new(
        SYMBOL,
        OrderSide::Buy,
        OrderType::Limit,
        Decimal::ONE,
        "ibkr",
    ) {
        Ok(order) => order,
        Err(e) => {
            println!("❌ falha ao montar ordem limit: {e}");
            std::process::exit(1);
        }
    };
    limit_order.price = Some(limit_price);

    match broker.place_order(limit_order).await {
        Ok(order_id) => {
            println!(
                "✅ Ordem LIMIT enviada: id={} preço={}",
                order_id, limit_price
            );

            let appeared = wait_until("ordem aparecer em get_open_orders", || async {
                match broker.get_open_orders().await {
                    Ok(orders) => orders
                        .iter()
                        .any(|o| o.broker_order_id.as_deref() == Some(order_id.0.as_str())),
                    Err(_) => false,
                }
            })
            .await;

            if appeared {
                println!("✅ Ordem {} visível em get_open_orders", order_id);
            } else {
                println!("❌ Ordem {} NÃO apareceu em get_open_orders", order_id);
                failures += 1;
            }

            match tokio::time::timeout(CANCEL_TIMEOUT, broker.cancel_order(&order_id)).await {
                Ok(Ok(())) => println!("✅ Cancelamento da ordem {} confirmado", order_id),
                Ok(Err(e)) => {
                    println!("❌ falha ao cancelar ordem {}: {}", order_id, e);
                    failures += 1;
                }
                Err(_) => {
                    println!("❌ timeout ao cancelar ordem {}", order_id);
                    failures += 1;
                }
            }

            let gone = wait_until("ordem sumir de get_open_orders", || async {
                match broker.get_open_orders().await {
                    Ok(orders) => !orders
                        .iter()
                        .any(|o| o.broker_order_id.as_deref() == Some(order_id.0.as_str())),
                    Err(_) => false,
                }
            })
            .await;

            if gone {
                println!("✅ Ordem {} não consta mais em get_open_orders", order_id);
            } else {
                println!("❌ Ordem {} ainda consta em get_open_orders", order_id);
                failures += 1;
            }
        }
        Err(e) => {
            println!("❌ falha ao enviar ordem LIMIT: {e}");
            failures += 1;
        }
    }

    // ── Etapa 2: roundtrip MARKET (compra e venda de 1 ação) ──────────
    println!("\n── Etapa 2: roundtrip MARKET (compra + venda de 1 ação) ──");

    let buy_order = match Order::new(
        SYMBOL,
        OrderSide::Buy,
        OrderType::Market,
        Decimal::ONE,
        "ibkr",
    ) {
        Ok(order) => order,
        Err(e) => {
            println!("❌ falha ao montar ordem de compra: {e}");
            std::process::exit(1);
        }
    };

    match broker.place_order(buy_order).await {
        Ok(order_id) => {
            println!("✅ Ordem MARKET de compra enviada: id={}", order_id);

            let positioned = wait_until("posição Long de 1 ação aparecer", || async {
                match broker.get_position(SYMBOL).await {
                    Ok(Some(p)) => {
                        p.direction == trader_domain::Direction::Long && p.quantity == Decimal::ONE
                    }
                    _ => false,
                }
            })
            .await;

            if positioned {
                println!("✅ Posição confirmada: Long 1 {}", SYMBOL);
            } else {
                println!("❌ Posição NÃO confirmada após compra");
                failures += 1;
            }

            let sell_order = match Order::new(
                SYMBOL,
                OrderSide::Sell,
                OrderType::Market,
                Decimal::ONE,
                "ibkr",
            ) {
                Ok(order) => order,
                Err(e) => {
                    println!("❌ falha ao montar ordem de venda: {e}");
                    std::process::exit(1);
                }
            };

            match broker.place_order(sell_order).await {
                Ok(sell_id) => {
                    println!("✅ Ordem MARKET de venda enviada: id={}", sell_id);

                    let flat = wait_until("posição zerar", || async {
                        matches!(broker.get_position(SYMBOL).await, Ok(None))
                    })
                    .await;

                    if flat {
                        println!("✅ Posição zerada após venda");
                    } else {
                        println!("❌ Posição AINDA aberta após venda");
                        failures += 1;
                    }
                }
                Err(e) => {
                    println!("❌ falha ao enviar ordem de venda: {e}");
                    failures += 1;
                }
            }
        }
        Err(e) => {
            println!("❌ falha ao enviar ordem MARKET de compra: {e}");
            failures += 1;
        }
    }

    // ── Etapa 3: rejeição síncrona (buying power insuficiente) ────────
    println!("\n── Etapa 3: rejeição síncrona (ordem grande demais) ──");
    let mut oversized = match Order::new(
        SYMBOL,
        OrderSide::Buy,
        OrderType::Limit,
        Decimal::from(1_000_000),
        "ibkr",
    ) {
        Ok(order) => order,
        Err(e) => {
            println!("❌ falha ao montar ordem oversized: {e}");
            std::process::exit(1);
        }
    };
    oversized.price = Some(limit_price);

    match broker.place_order(oversized).await {
        Ok(id) => {
            println!(
                "❌ ordem oversized foi ACEITA (id={}) — rejeição síncrona não funcionou; cancelando",
                id
            );
            let _ = broker.cancel_order(&id).await;
            failures += 1;
        }
        Err(trader_domain::BrokerError::OrderRejected(reason)) => {
            println!("✅ Ordem oversized rejeitada sincronamente: {}", reason);
        }
        Err(e) => {
            println!("⚠️  ordem oversized falhou com erro inesperado (não-OrderRejected): {e}");
        }
    }

    // ── Estado final ──────────────────────────────────────────────────
    println!("\n── Estado final da conta ──");
    match broker.get_positions().await {
        Ok(positions) if positions.is_empty() => println!("   Posições abertas: nenhuma ✅"),
        Ok(positions) => {
            println!("   Posições abertas: {}", positions.len());
            for p in &positions {
                println!("   - {:?} {} x{}", p.direction, p.symbol, p.quantity);
            }
        }
        Err(e) => println!("   ⚠️  falha ao consultar posições finais: {e}"),
    }

    if failures == 0 {
        println!("\n🏁 Smoke test concluído: TODAS as etapas passaram");
        std::process::exit(0);
    } else {
        println!("\n💥 Smoke test concluído com {failures} falha(s)");
        std::process::exit(1);
    }
}

/// Faz polling da condição até ela ser satisfeita ou estourar o timeout.
async fn wait_until<F, Fut>(what: &str, mut check: F) -> bool
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = bool>,
{
    let start = Instant::now();
    while start.elapsed() < POLL_TIMEOUT {
        if check().await {
            return true;
        }
        tokio::time::sleep(POLL_INTERVAL).await;
    }
    println!("⏱️  timeout ({:?}) aguardando: {}", POLL_TIMEOUT, what);
    false
}
