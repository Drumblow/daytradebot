//! Adapter para Interactive Brokers via TWS API / IB Gateway.
//!
//! Usa o crate [`ibapi`](https://docs.rs/ibapi) v3.x, que fala o protocolo
//! protobuf com TWS/IB Gateway (server version ≥ 213).
//!
//! Enquanto a conta não estiver liberada, os testes automatizados devem usar
//! `simulated::SimulatedBroker` e `simulated::SimulatedMarketDataProvider`.

pub mod broker;
pub mod config;
pub mod market_data;

pub use broker::IbkrBrokerAdapter;
pub use config::IbkrConfig;
pub use market_data::IbkrMarketDataProvider;
