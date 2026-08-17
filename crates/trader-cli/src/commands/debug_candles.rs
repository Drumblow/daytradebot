//! Diagnóstico cru do feed de candles da IBKR.
//!
//! Criado em 2026-08-17 para investigar a degeneração de barras na VM Oracle:
//! a conta paper sem subscrição de dados em tempo real recebe barras recentes
//! de 1 print (OHLC iguais, volume 0), o que invalida price action.
//! Ver `docs/reports/validacao-live-vs-backtest-2026-08-07_a_08-14.md`.
//!
//! Uso:
//!   trader-cli debug-candles --symbol IWV                 # últimas barras como chegam
//!   trader-cli debug-candles --symbol IWV --realtime      # idem após pedir MarketDataType::Realtime
//!
//! O comando NÃO persiste nada — só imprime o que o gateway entrega.

use anyhow::Result;
use chrono::Utc;

use trader_adapters::ibkr::market_data::fetch_raw_bars;

use crate::config::CliConfig;

/// Argumentos do comando debug-candles.
pub struct Args {
    pub symbol: String,
    pub timeframe: trader_domain::TimeFrame,
    /// Quantos dias para trás buscar.
    pub days: i32,
    /// Quantas barras finais imprimir.
    pub bars: usize,
    /// Se true, pede MarketDataType::Realtime antes de buscar.
    pub realtime: bool,
}

pub async fn run(config: &CliConfig, args: Args) -> Result<()> {
    let ibkr = config.ibkr_config()?;
    println!("🔎 debug-candles {} {}", args.symbol, args.timeframe);
    println!(
        "   gateway: {} (client_id {})",
        ibkr.connection_string(),
        ibkr.client_id
    );
    if args.realtime {
        println!("   market data type: Realtime será solicitado antes da busca");
    }

    let bars = fetch_raw_bars(
        &ibkr,
        &args.symbol,
        args.timeframe,
        args.days,
        args.realtime,
    )
    .await
    .map_err(|e| anyhow::anyhow!("{e}"))?;

    let total = bars.len();
    let degenerate = bars.iter().filter(|b| b.is_degenerate()).count();
    println!(
        "   {} barras recebidas — {} degeneradas (high==low, {:.0}%)",
        total,
        degenerate,
        if total > 0 {
            degenerate as f64 / total as f64 * 100.0
        } else {
            0.0
        }
    );

    println!(
        "   últimas {} barras (cruas, como o gateway entrega):",
        args.bars
    );
    for bar in bars.iter().skip(total.saturating_sub(args.bars)) {
        let flag = if bar.is_degenerate() { "FLAT" } else { " ok " };
        println!(
            "   [{}] {}  O={:.2} H={:.2} L={:.2} C={:.2} V={:.0}",
            flag, bar.date, bar.open, bar.high, bar.low, bar.close, bar.volume
        );
    }
    println!("   horário local: {}", Utc::now());
    Ok(())
}
