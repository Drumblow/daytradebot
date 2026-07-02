use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// Timeframe de análise (ex: 15m, 1h, 1d).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimeFrame {
    M1,
    M5,
    M15,
    M30,
    H1,
    H4,
    D1,
}

impl TimeFrame {
    pub fn as_str(&self) -> &'static str {
        match self {
            TimeFrame::M1 => "1m",
            TimeFrame::M5 => "5m",
            TimeFrame::M15 => "15m",
            TimeFrame::M30 => "30m",
            TimeFrame::H1 => "1h",
            TimeFrame::H4 => "4h",
            TimeFrame::D1 => "1d",
        }
    }
}

impl std::fmt::Display for TimeFrame {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for TimeFrame {
    type Err = crate::ValidationError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "1m" => Ok(TimeFrame::M1),
            "5m" => Ok(TimeFrame::M5),
            "15m" => Ok(TimeFrame::M15),
            "30m" => Ok(TimeFrame::M30),
            "1h" => Ok(TimeFrame::H1),
            "4h" => Ok(TimeFrame::H4),
            "1d" => Ok(TimeFrame::D1),
            _ => Err(crate::ValidationError::InvalidTimeFrame(s.to_string())),
        }
    }
}

/// Candle OHLCV.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Candle {
    pub symbol: String,
    pub timeframe: TimeFrame,
    pub timestamp: DateTime<Utc>,
    pub open: Decimal,
    pub high: Decimal,
    pub low: Decimal,
    pub close: Decimal,
    pub volume: Decimal,
    pub vwap: Option<Decimal>,
    pub source: DataSource,
    pub is_complete: bool,
}

impl Candle {
    /// Cria um candle completo com valores validados.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        symbol: impl Into<String>,
        timeframe: TimeFrame,
        timestamp: DateTime<Utc>,
        open: Decimal,
        high: Decimal,
        low: Decimal,
        close: Decimal,
        volume: Decimal,
    ) -> Result<Self, crate::ValidationError> {
        if high < low {
            return Err(crate::ValidationError::InvalidCandle(
                "high menor que low".to_string(),
            ));
        }
        if high < open || high < close || low > open || low > close {
            return Err(crate::ValidationError::InvalidCandle(
                "valores OHLC inconsistentes".to_string(),
            ));
        }
        Ok(Self {
            symbol: symbol.into(),
            timeframe,
            timestamp,
            open,
            high,
            low,
            close,
            volume,
            vwap: None,
            source: DataSource::Manual,
            is_complete: true,
        })
    }

    /// Range do candle (high - low).
    pub fn range(&self) -> Decimal {
        self.high - self.low
    }

    /// Corpo do candle (|close - open|).
    pub fn body(&self) -> Decimal {
        (self.close - self.open).abs()
    }

    /// Candle de alta (close >= open).
    pub fn is_bullish(&self) -> bool {
        self.close >= self.open
    }

    /// Candle de baixa (close < open).
    pub fn is_bearish(&self) -> bool {
        self.close < self.open
    }
}

/// Fonte dos dados de mercado.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DataSource {
    Ibkr,
    Polygon,
    Manual,
    Simulated,
}

/// Cotação atual (top-of-book).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Quote {
    pub symbol: String,
    pub bid: Decimal,
    pub ask: Decimal,
    pub bid_size: Decimal,
    pub ask_size: Decimal,
    pub timestamp: DateTime<Utc>,
}

impl Quote {
    /// Spread absoluto.
    pub fn spread(&self) -> Decimal {
        self.ask - self.bid
    }

    /// Spread relativo ao preço médio.
    pub fn spread_pct(&self) -> Decimal {
        let mid = (self.bid + self.ask) / Decimal::TWO;
        if mid.is_zero() {
            return Decimal::ZERO;
        }
        ((self.ask - self.bid) / mid) * Decimal::from(100)
    }
}

/// Tick de negócio (trade).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Tick {
    pub symbol: String,
    pub price: Decimal,
    pub size: Decimal,
    pub timestamp: DateTime<Utc>,
}

/// Ativo negociado no mercado.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Asset {
    pub id: Option<i32>,
    pub symbol: String,
    pub name: Option<String>,
    pub asset_type: String,
    pub exchange: Option<String>,
    pub currency: String,
    pub tick_size: Decimal,
    pub lot_size: Decimal,
    pub sector: Option<String>,
    pub is_active: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn candle_validation_rejects_inverted_high_low() {
        let result = Candle::new(
            "SPY",
            TimeFrame::H1,
            Utc::now(),
            Decimal::from(100),
            Decimal::from(99),
            Decimal::from(101),
            Decimal::from(100),
            Decimal::from(1000),
        );
        assert!(result.is_err());
    }

    #[test]
    fn quote_spread_pct_is_correct() {
        let quote = Quote {
            symbol: "SPY".to_string(),
            bid: Decimal::from(400),
            ask: Decimal::from(402),
            bid_size: Decimal::from(100),
            ask_size: Decimal::from(100),
            timestamp: Utc::now(),
        };
        let pct = quote.spread_pct();
        assert!(pct > Decimal::ZERO);
    }
}
