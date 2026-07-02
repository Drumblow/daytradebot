//! Adapters simulados para testes e desenvolvimento sem conexão com broker.

pub mod broker;
pub mod market_data;

pub use broker::{SimulatedBroker, SimulatedBrokerConfig};
pub use market_data::SimulatedMarketDataProvider;
