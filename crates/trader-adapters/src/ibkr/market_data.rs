//! Provedor de dados de mercado da Interactive Brokers.

use async_trait::async_trait;
use chrono::Utc;
use futures::StreamExt;
use ibapi::prelude::*;
use rust_decimal::Decimal;
use tokio::sync::mpsc::Sender;
use tracing::{debug, error, info, warn};

use trader_domain::market::{CandleRequest, ProviderHealth, SubscriptionHandle};
use trader_domain::{Candle, DataError, DataSource, Quote, TimeFrame};
use trader_infra::ports::MarketDataProvider;

use super::config::IbkrConfig;

/// Provedor de dados concreto para Interactive Brokers.
#[derive(Debug, Clone)]
pub struct IbkrMarketDataProvider {
    config: IbkrConfig,
}

impl IbkrMarketDataProvider {
    pub fn new(config: IbkrConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl MarketDataProvider for IbkrMarketDataProvider {
    async fn get_historical_candles(
        &self,
        request: CandleRequest,
    ) -> Result<Vec<Candle>, DataError> {
        info!(
            symbol = %request.symbol,
            timeframe = %request.timeframe,
            "buscando candles históricos na IBKR"
        );

        let connection_string = self.config.connection_string();
        let client = ibapi::Client::connect(&connection_string, self.config.client_id)
            .await
            .map_err(|e| DataError::ProviderUnavailable(e.to_string()))?;

        let contract = Contract::stock(&request.symbol).build();
        let bar_size = timeframe_to_historical_bar_size(request.timeframe)?;

        let historical_data = client
            .historical_data(&contract, bar_size)
            .duration(1.days())
            .what_to_show(HistoricalWhatToShow::Trades)
            .fetch()
            .await
            .map_err(|e| DataError::Provider(e.to_string()))?;

        let mut candles = Vec::new();
        for bar in historical_data.bars {
            if let Some(candle) = historical_bar_to_candle(&request.symbol, request.timeframe, &bar)
            {
                candles.push(candle);
            }
        }

        if candles.is_empty() {
            return Err(DataError::NoData {
                symbol: request.symbol,
            });
        }

        Ok(candles)
    }

    async fn subscribe_realtime_bars(
        &self,
        symbol: &str,
        _timeframe: TimeFrame,
        tx: Sender<Candle>,
    ) -> Result<SubscriptionHandle, DataError> {
        info!(%symbol, "subscrevendo barras em tempo real na IBKR (5s)");

        let connection_string = self.config.connection_string();
        let client = ibapi::Client::connect(&connection_string, self.config.client_id)
            .await
            .map_err(|e| DataError::ProviderUnavailable(e.to_string()))?;

        let contract = Contract::stock(symbol).build();

        let mut subscription = client
            .realtime_bars(&contract)
            .what_to_show(RealtimeWhatToShow::Trades)
            .subscribe()
            .await
            .map_err(|e| DataError::Provider(e.to_string()))?;

        let handle = SubscriptionHandle {
            id: format!("ibkr-{}-{}", symbol, Utc::now().timestamp()),
        };

        let symbol = symbol.to_string();
        tokio::spawn(async move {
            while let Some(item) = subscription.next().await {
                match item {
                    Ok(ibapi::subscriptions::SubscriptionItem::Data(bar)) => {
                        if let Some(candle) = realtime_bar_to_candle(&symbol, &bar) {
                            if tx.send(candle).await.is_err() {
                                warn!("canal de candles fechado; cancelando subscrição");
                                break;
                            }
                        }
                    }
                    Ok(ibapi::subscriptions::SubscriptionItem::Notice(n)) => {
                        warn!(notice = %n, "aviso do IBKR em realtime bars");
                    }
                    Err(e) => {
                        error!(error = %e, "erro em barras em tempo real");
                        break;
                    }
                }
            }
        });

        Ok(handle)
    }

    async fn get_quote(&self, symbol: &str) -> Result<Quote, DataError> {
        debug!(%symbol, "buscando cotação na IBKR");

        let connection_string = self.config.connection_string();
        let client = ibapi::Client::connect(&connection_string, self.config.client_id)
            .await
            .map_err(|e| DataError::ProviderUnavailable(e.to_string()))?;

        let contract = Contract::stock(symbol).build();

        let mut subscription = client
            .market_data(&contract)
            .subscribe()
            .await
            .map_err(|e| DataError::Provider(e.to_string()))?;

        let timeout = tokio::time::Duration::from_secs(5);

        match tokio::time::timeout(timeout, subscription.next()).await {
            Ok(Some(Ok(ibapi::subscriptions::SubscriptionItem::Data(tick)))) => {
                tick_to_quote(symbol, &tick)
            }
            Ok(Some(Ok(ibapi::subscriptions::SubscriptionItem::Notice(n)))) => {
                Err(DataError::Provider(n.to_string()))
            }
            Ok(Some(Err(e))) => Err(DataError::Provider(e.to_string())),
            Ok(None) => Err(DataError::NoData {
                symbol: symbol.to_string(),
            }),
            Err(_) => Err(DataError::Timeout(format!(
                "timeout ao buscar cotação de {symbol}"
            ))),
        }
    }

    async fn health_check(&self) -> Result<ProviderHealth, DataError> {
        match ibapi::Client::connect(&self.config.connection_string(), self.config.client_id).await
        {
            Ok(client) => {
                let _ = client.server_version();
                Ok(ProviderHealth::Healthy)
            }
            Err(e) => {
                warn!(error = %e, "health check IBKR falhou");
                Ok(ProviderHealth::Unhealthy)
            }
        }
    }
}

/// Converte `ibapi::market_data::historical::Bar` para `trader_domain::Candle`.
fn historical_bar_to_candle(
    symbol: &str,
    timeframe: TimeFrame,
    bar: &ibapi::market_data::historical::Bar,
) -> Option<Candle> {
    let timestamp = Utc::now();

    Candle::new(
        symbol,
        timeframe,
        timestamp,
        Decimal::from_f64_retain(bar.open)?,
        Decimal::from_f64_retain(bar.high)?,
        Decimal::from_f64_retain(bar.low)?,
        Decimal::from_f64_retain(bar.close)?,
        Decimal::from_f64_retain(bar.volume)?,
    )
    .map(|mut c| {
        c.source = DataSource::Ibkr;
        c
    })
    .ok()
}

/// Converte `ibapi::market_data::realtime::Bar` para `trader_domain::Candle`.
fn realtime_bar_to_candle(symbol: &str, bar: &ibapi::market_data::realtime::Bar) -> Option<Candle> {
    let timestamp = Utc::now();

    Candle::new(
        symbol,
        TimeFrame::M5,
        timestamp,
        Decimal::from_f64_retain(bar.open)?,
        Decimal::from_f64_retain(bar.high)?,
        Decimal::from_f64_retain(bar.low)?,
        Decimal::from_f64_retain(bar.close)?,
        Decimal::from_f64_retain(bar.volume)?,
    )
    .map(|mut c| {
        c.source = DataSource::Ibkr;
        c
    })
    .ok()
}

/// Converte `ibapi::market_data::TickTypes` para `trader_domain::Quote`.
///
/// Por enquanto usa o primeiro tick de preço disponível como proxy de cotação.
fn tick_to_quote(symbol: &str, tick: &TickTypes) -> Result<Quote, DataError> {
    match tick {
        TickTypes::Price(price_tick) => {
            let price = Decimal::from_f64_retain(price_tick.price)
                .ok_or_else(|| DataError::Provider("preço inválido".to_string()))?;
            Ok(Quote {
                symbol: symbol.to_string(),
                bid: price,
                ask: price,
                bid_size: Decimal::ZERO,
                ask_size: Decimal::ZERO,
                timestamp: Utc::now(),
            })
        }
        _ => Err(DataError::Provider(
            "tipo de tick não é cotação".to_string(),
        )),
    }
}

/// Mapeia `TimeFrame` do domínio para `ibapi::market_data::historical::BarSize`.
fn timeframe_to_historical_bar_size(
    timeframe: TimeFrame,
) -> Result<ibapi::market_data::historical::BarSize, DataError> {
    use ibapi::market_data::historical::BarSize;
    match timeframe {
        TimeFrame::M1 => Ok(BarSize::Min),
        TimeFrame::M5 => Ok(BarSize::Min5),
        TimeFrame::M15 => Ok(BarSize::Min15),
        TimeFrame::M30 => Ok(BarSize::Min30),
        TimeFrame::H1 => Ok(BarSize::Hour),
        TimeFrame::H4 => Ok(BarSize::Hour4),
        TimeFrame::D1 => Ok(BarSize::Day),
    }
}
