//! Smoke test do bracket com entrada STOP (ADR-009) contra a conta paper.
//!
//! ATENÇÃO: envia ordens reais à conta paper (gateway 127.0.0.1:7497).
//!
//! Usa `client_id` próprio (padrão 3, sobrescrita via `IBKR_SMOKE_CLIENT_ID`)
//! porque a TWS API só permite uma conexão por client_id: o client_id 1 é da
//! sessão `paper --mode live` e o 2 do `ibkr_order_smoke`.
//!
//! Etapas:
//! 1. Bracket STP (buy stop) com gatilho ~5% ACIMA do mercado: não pode
//!    executar durante o teste. Confirma que o parent aparece em
//!    `get_open_orders` como tipo Stop.
//! 2. Cancela o parent e confirma que a cadeia inteira some (filhos OCA).
//!
//! Sai com exit code não-zero se alguma etapa falhar.

use std::time::{Duration, Instant};

use rust_decimal::Decimal;
use trader_adapters::ibkr::{IbkrBrokerAdapter, IbkrConfig, IbkrMarketDataProvider};
use trader_domain::{Broker, EntryOrderType, MarketDataProvider, Order, OrderSide, OrderType};

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
            .unwrap_or(3),
        account_id: std::env::var("TRADER__IBKR__ACCOUNT_ID")
            .ok()
            .filter(|s| !s.is_empty()),
        paper: true,
    };

    println!("🚀 Smoke test de bracket STP (entrada stop) — conta paper");
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

    // ── Etapa 1: bracket STP pendente (não deve executar) ─────────────
    println!("\n── Etapa 1: bracket STP com gatilho acima do mercado ──");
    let trigger = (reference * Decimal::from(105) / Decimal::from(100)).round_dp(2);
    let target = (reference * Decimal::from(107) / Decimal::from(100)).round_dp(2);
    let stop = (reference * Decimal::from(103) / Decimal::from(100)).round_dp(2);

    let mut order = match Order::new(
        SYMBOL,
        OrderSide::Buy,
        OrderType::Bracket,
        Decimal::ONE,
        "ibkr",
    ) {
        Ok(order) => order,
        Err(e) => {
            println!("❌ falha ao montar bracket: {e}");
            std::process::exit(1);
        }
    };
    order.entry_order_type = EntryOrderType::Stop;
    order.price = Some(trigger);
    order.target_price = Some(target);
    order.stop_price = Some(stop);

    println!(
        "   gatilho={} alvo={} stop={} (referência ~{})",
        trigger, target, stop, reference
    );

    match broker.place_order(order).await {
        Ok(order_id) => {
            println!("✅ Bracket STP enviado: parent id={}", order_id);

            let appeared = wait_until("parent STP aparecer em get_open_orders", || async {
                match broker.get_open_orders().await {
                    Ok(orders) => orders.iter().any(|o| {
                        o.broker_order_id.as_deref() == Some(order_id.0.as_str())
                            && o.order_type == OrderType::Stop
                    }),
                    Err(_) => false,
                }
            })
            .await;

            if appeared {
                println!("✅ Parent visível como ordem STOP em get_open_orders");
            } else {
                println!("❌ Parent NÃO apareceu como STOP em get_open_orders");
                failures += 1;
            }

            // A cadeia deve ter 3 ordens abertas no símbolo (parent + TP + SL).
            let chain = wait_until("cadeia completa (3 ordens) visível", || async {
                match broker.get_open_orders().await {
                    Ok(orders) => orders.iter().filter(|o| o.symbol == SYMBOL).count() >= 3,
                    Err(_) => false,
                }
            })
            .await;

            if chain {
                println!("✅ Cadeia bracket completa (parent + alvo + stop) visível");
            } else {
                println!("⚠️  cadeia com menos de 3 ordens visíveis (verificar na TWS)");
            }

            match tokio::time::timeout(CANCEL_TIMEOUT, broker.cancel_order(&order_id)).await {
                Ok(Ok(())) => println!("✅ Cancelamento do parent {} confirmado", order_id),
                Ok(Err(e)) => {
                    println!("❌ falha ao cancelar parent {}: {}", order_id, e);
                    failures += 1;
                }
                Err(_) => {
                    println!("❌ timeout ao cancelar parent {}", order_id);
                    failures += 1;
                }
            }

            let gone = wait_until("cadeia sumir de get_open_orders", || async {
                match broker.get_open_orders().await {
                    Ok(orders) => orders.iter().all(|o| o.symbol != SYMBOL),
                    Err(_) => false,
                }
            })
            .await;

            if gone {
                println!("✅ Cadeia inteira cancelada (parent + filhos OCA)");
            } else {
                println!("❌ Ainda há ordens de {SYMBOL} abertas após cancelamento");
                failures += 1;
            }
        }
        Err(e) => {
            println!("❌ falha ao enviar bracket STP: {e}");
            failures += 1;
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
        println!("\n🏁 Smoke test STP concluído: TODAS as etapas passaram");
        std::process::exit(0);
    } else {
        println!("\n💥 Smoke test STP concluído com {failures} falha(s)");
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
