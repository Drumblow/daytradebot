//! Indicadores técnicos puros.
//!
//! Todos os indicadores operam sobre `&[Candle]` e retornam `Decimal` ou `None`
//! quando não há dados suficientes. Nenhum indicador faz I/O ou mantém estado.

use rust_decimal::Decimal;
use trader_domain::Candle;

/// Média móvel simples (SMA) dos fechamentos.
pub fn sma(candles: &[Candle], period: usize) -> Option<Decimal> {
    if candles.len() < period || period == 0 {
        return None;
    }

    let sum: Decimal = candles.iter().rev().take(period).map(|c| c.close).sum();
    Some(sum / Decimal::from(period))
}

/// Média móvel exponencial (EMA) dos fechamentos.
///
/// O cálculo usa o EMA do período `period` sobre os últimos `period` candles.
/// A primeira amostra usa SMA como seed.
pub fn ema(candles: &[Candle], period: usize) -> Option<Decimal> {
    if candles.len() < period || period == 0 {
        return None;
    }

    let multiplier = Decimal::TWO / Decimal::from(period + 1);
    let seed = sma(candles, period)?;

    let relevant = &candles[candles.len() - period..];
    let mut ema = seed;
    for candle in relevant.iter().skip(1) {
        ema = (candle.close - ema) * multiplier + ema;
    }

    Some(ema)
}

/// Average True Range (ATR) simples.
pub fn atr(candles: &[Candle], period: usize) -> Option<Decimal> {
    if candles.len() < period + 1 || period == 0 {
        return None;
    }

    let mut tr_sum = Decimal::ZERO;
    for i in (candles.len() - period)..candles.len() {
        let current = &candles[i];
        let previous = &candles[i - 1];

        let tr1 = current.high - current.low;
        let tr2 = (current.high - previous.close).abs();
        let tr3 = (current.low - previous.close).abs();

        tr_sum += tr1.max(tr2).max(tr3);
    }

    Some(tr_sum / Decimal::from(period))
}

/// ATR percentual em relação ao fechamento do último candle.
pub fn atr_percent(candles: &[Candle], period: usize) -> Option<Decimal> {
    let atr = atr(candles, period)?;
    let last = candles.last()?;
    if last.close.is_zero() {
        return None;
    }
    Some((atr / last.close) * Decimal::from(100))
}

/// Volume relativo: volume do último candle / média de volume do período.
pub fn relative_volume(candles: &[Candle], period: usize) -> Option<Decimal> {
    if candles.len() < period || period == 0 {
        return None;
    }

    let avg_volume: Decimal = candles.iter().rev().take(period).map(|c| c.volume).sum();
    let avg_volume = avg_volume / Decimal::from(period);

    if avg_volume.is_zero() {
        return None;
    }

    Some(candles.last()?.volume / avg_volume)
}

/// Máxima do período.
pub fn highest_high(candles: &[Candle], period: usize) -> Option<Decimal> {
    if candles.len() < period || period == 0 {
        return None;
    }
    candles.iter().rev().take(period).map(|c| c.high).max()
}

/// Mínima do período.
pub fn lowest_low(candles: &[Candle], period: usize) -> Option<Decimal> {
    if candles.len() < period || period == 0 {
        return None;
    }
    candles.iter().rev().take(period).map(|c| c.low).min()
}

/// Range percentual do último candle em relação ao fechamento.
pub fn range_percent(candles: &[Candle]) -> Option<Decimal> {
    let last = candles.last()?;
    if last.close.is_zero() {
        return None;
    }
    Some(((last.high - last.low) / last.close) * Decimal::from(100))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use rust_decimal::Decimal;
    use trader_domain::{Candle, TimeFrame};

    fn candle(close: Decimal) -> Candle {
        Candle::new(
            "SPY",
            TimeFrame::M15,
            Utc::now(),
            close - Decimal::ONE,
            close + Decimal::ONE,
            close - Decimal::ONE,
            close,
            Decimal::from(1000),
        )
        .expect("candle válido")
    }

    #[test]
    fn sma_calculates_average() {
        let candles = vec![
            candle(Decimal::from(10)),
            candle(Decimal::from(20)),
            candle(Decimal::from(30)),
        ];
        assert_eq!(sma(&candles, 3), Some(Decimal::from(20)));
    }

    #[test]
    fn sma_returns_none_when_not_enough_data() {
        let candles = vec![candle(Decimal::from(10))];
        assert_eq!(sma(&candles, 3), None);
    }

    #[test]
    fn atr_requires_previous_candle() {
        let candles = vec![candle(Decimal::from(100))];
        assert_eq!(atr(&candles, 14), None);
    }

    #[test]
    fn relative_volume_equals_one_when_constant() {
        let candles: Vec<_> = (0..5).map(|_| candle(Decimal::from(100))).collect();
        assert_eq!(relative_volume(&candles, 5), Some(Decimal::ONE));
    }
}
