//! Qualidade de dados: detecção de gaps em séries de candles.

use trader_domain::{Candle, TimeFrame};

/// Conta gaps em uma série de candles ordenada por timestamp.
///
/// Um gap é um intervalo entre candles consecutivos **do mesmo dia UTC** com
/// duração maior que 2× o timeframe. A restrição ao mesmo dia evita contar
/// como gap o fechamento noturno e fins de semana, que são esperados em
/// séries intraday.
pub fn count_gaps(candles: &[Candle], timeframe: TimeFrame) -> usize {
    let max_interval = timeframe.duration() * 2;

    candles
        .windows(2)
        .filter(|w| {
            let same_day = w[0].timestamp.date_naive() == w[1].timestamp.date_naive();
            same_day && (w[1].timestamp - w[0].timestamp) > max_interval
        })
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};
    use rust_decimal::Decimal;

    fn candle(day: u32, hour: u32, minute: u32) -> Candle {
        let ts = Utc
            .with_ymd_and_hms(2026, 8, day, hour, minute, 0)
            .single()
            .unwrap();
        Candle::new(
            "SPY",
            TimeFrame::M15,
            ts,
            Decimal::from(100),
            Decimal::from(101),
            Decimal::from(99),
            Decimal::from(100),
            Decimal::from(1000),
        )
        .unwrap()
    }

    #[test]
    fn contiguous_series_has_no_gaps() {
        let candles = vec![
            candle(3, 14, 30),
            candle(3, 14, 45),
            candle(3, 15, 0),
            candle(3, 15, 15),
        ];
        assert_eq!(count_gaps(&candles, TimeFrame::M15), 0);
    }

    #[test]
    fn missing_candles_same_day_count_as_gap() {
        let candles = vec![
            candle(3, 14, 30),
            candle(3, 14, 45),
            // faltam 15:00 e 15:15 → 45min de intervalo > 2x15m
            candle(3, 15, 30),
        ];
        assert_eq!(count_gaps(&candles, TimeFrame::M15), 1);
    }

    #[test]
    fn overnight_is_not_a_gap() {
        let candles = vec![candle(3, 20, 45), candle(4, 14, 30)];
        assert_eq!(count_gaps(&candles, TimeFrame::M15), 0);
    }
}
