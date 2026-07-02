//! Geradores de dados sintéticos para demonstrações e testes.

use chrono::{TimeZone, Utc};
use rust_decimal::Decimal;
use tracing::warn;

use trader_domain::{Candle, DataSource, TimeFrame};

/// Gera uma série sintética de tendência de alta com pullback.
pub fn generate_synthetic_uptrend(symbol: &str) -> Vec<Candle> {
    let base = match Utc.with_ymd_and_hms(2026, 7, 2, 14, 30, 0).single() {
        Some(ts) => ts,
        None => {
            warn!("timestamp base inválido; usando agora");
            Utc::now()
        }
    };
    let mut candles = Vec::new();

    for i in 0..60 {
        let close = Decimal::from(400 + i);
        if let Ok(c) = Candle::new(
            symbol,
            TimeFrame::M5,
            base + chrono::Duration::minutes(i as i64 * 5),
            close - Decimal::ONE,
            close + Decimal::ONE,
            close - Decimal::ONE,
            close,
            Decimal::from(1000),
        ) {
            candles.push(c);
        }
    }

    if let Some(last) = candles.last() {
        if let Ok(c) = Candle::new(
            symbol,
            TimeFrame::M5,
            last.timestamp + chrono::Duration::minutes(5),
            Decimal::from(459),
            Decimal::from(461),
            Decimal::from(458),
            Decimal::from(460),
            Decimal::from(1000),
        ) {
            candles.push(c);
        }
    }

    for (open, high, low, close) in [
        (
            Decimal::from(460),
            Decimal::from(460),
            Decimal::from(456),
            Decimal::from(456),
        ),
        (
            Decimal::from(456),
            Decimal::from(457),
            Decimal::from(455),
            Decimal::from(455),
        ),
    ] {
        if let Some(last) = candles.last() {
            if let Ok(c) = Candle::new(
                symbol,
                TimeFrame::M5,
                last.timestamp + chrono::Duration::minutes(5),
                open,
                high,
                low,
                close,
                Decimal::from(1000),
            ) {
                candles.push(c);
            }
        }
    }

    if let Some(last) = candles.last() {
        if let Ok(c) = Candle::new(
            symbol,
            TimeFrame::M5,
            last.timestamp + chrono::Duration::minutes(5),
            Decimal::from(457),
            Decimal::from(460),
            Decimal::from(456),
            Decimal::from(459),
            Decimal::from(1000),
        ) {
            candles.push(c);
        }
    }

    candles
}

/// Gera o próximo candle de continuação de tendência.
pub fn next_candle(symbol: &str, candles: &[Candle]) -> Option<Candle> {
    let last = candles.last()?;
    let close = last.close + Decimal::ONE;
    Candle::new(
        symbol,
        last.timeframe,
        last.timestamp + chrono::Duration::minutes(5),
        close - Decimal::ONE,
        close + Decimal::ONE,
        close - Decimal::ONE,
        close,
        Decimal::from(1000),
    )
    .ok()
    .map(|mut c| {
        c.source = DataSource::Simulated;
        c
    })
}
