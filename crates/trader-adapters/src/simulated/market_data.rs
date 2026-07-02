//! Provedor de dados de mercado simulado.

use async_trait::async_trait;
use tokio::sync::mpsc::Sender;
use tracing::{debug, warn};

use trader_domain::market::{CandleRequest, ProviderHealth, SubscriptionHandle};
use trader_domain::{Candle, DataError, Quote};
use trader_infra::ports::MarketDataProvider;

/// Provedor de dados em memória para testes.
///
/// Útil para validar ingestão, backtest e componentes que dependem de
/// `MarketDataProvider` sem exigir conexão com a Interactive Brokers.
#[derive(Debug, Clone)]
pub struct SimulatedMarketDataProvider {
    symbol: String,
    candles: Vec<Candle>,
    quotes: Vec<Quote>,
}

impl SimulatedMarketDataProvider {
    pub fn new(symbol: impl Into<String>) -> Self {
        Self {
            symbol: symbol.into(),
            candles: Vec::new(),
            quotes: Vec::new(),
        }
    }

    /// Predefine candles que serão retornados por `get_historical_candles`.
    pub fn with_candles(mut self, candles: Vec<Candle>) -> Self {
        self.candles = candles;
        self
    }

    /// Predefine cotações que serão retornadas por `get_quote`.
    pub fn with_quotes(mut self, quotes: Vec<Quote>) -> Self {
        self.quotes = quotes;
        self
    }
}

#[async_trait]
impl MarketDataProvider for SimulatedMarketDataProvider {
    async fn get_historical_candles(
        &self,
        request: CandleRequest,
    ) -> Result<Vec<Candle>, DataError> {
        debug!(symbol = %request.symbol, "simulando busca de candles históricos");

        if request.symbol != self.symbol {
            return Err(DataError::InvalidSymbol(format!(
                "provedor simulado configurado apenas para {}; requisitado {}",
                self.symbol, request.symbol
            )));
        }

        let filtered: Vec<Candle> = self
            .candles
            .iter()
            .filter(|c| {
                c.timeframe == request.timeframe
                    && c.timestamp >= request.from
                    && c.timestamp <= request.to
            })
            .cloned()
            .collect();

        if filtered.is_empty() {
            return Err(DataError::NoData {
                symbol: request.symbol,
            });
        }

        Ok(filtered)
    }

    async fn subscribe_realtime_bars(
        &self,
        symbol: &str,
        _timeframe: trader_domain::TimeFrame,
        _tx: Sender<Candle>,
    ) -> Result<SubscriptionHandle, DataError> {
        warn!(%symbol, "subscribe_realtime_bars simulado: não envia dados");
        Ok(SubscriptionHandle {
            id: format!("simulated-{symbol}"),
        })
    }

    async fn get_quote(&self, symbol: &str) -> Result<Quote, DataError> {
        debug!(%symbol, "simulando busca de cotação");

        self.quotes
            .iter()
            .find(|q| q.symbol == symbol)
            .cloned()
            .ok_or_else(|| DataError::NoData {
                symbol: symbol.to_string(),
            })
    }

    async fn health_check(&self) -> Result<ProviderHealth, DataError> {
        Ok(ProviderHealth::Healthy)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{DateTime, Duration, Utc};
    use rust_decimal::Decimal;
    use trader_domain::{DataSource, TimeFrame};

    fn sample_candle(symbol: &str, timestamp: DateTime<Utc>, close: Decimal) -> Candle {
        Candle {
            symbol: symbol.to_string(),
            timeframe: TimeFrame::M15,
            timestamp,
            open: close - Decimal::ONE,
            high: close + Decimal::ONE,
            low: close - Decimal::ONE,
            close,
            volume: Decimal::from(1000),
            vwap: None,
            source: DataSource::Simulated,
            is_complete: true,
        }
    }

    #[tokio::test]
    async fn returns_candles_in_range() {
        let now = Utc::now();
        let candles = vec![
            sample_candle("SPY", now - Duration::minutes(30), Decimal::from(400)),
            sample_candle("SPY", now - Duration::minutes(15), Decimal::from(401)),
            sample_candle("SPY", now, Decimal::from(402)),
        ];

        let provider = SimulatedMarketDataProvider::new("SPY").with_candles(candles);

        let request = CandleRequest {
            symbol: "SPY".to_string(),
            timeframe: TimeFrame::M15,
            from: now - Duration::minutes(20),
            to: now + Duration::minutes(1),
        };

        let result = provider.get_historical_candles(request).await.unwrap();
        assert_eq!(result.len(), 2);
    }

    #[tokio::test]
    async fn health_check_is_healthy() {
        let provider = SimulatedMarketDataProvider::new("SPY");
        assert_eq!(
            provider.health_check().await.unwrap(),
            ProviderHealth::Healthy
        );
    }
}
