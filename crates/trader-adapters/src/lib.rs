//! `trader-adapters` — integrações externas do HumanStyle Trader Bot.
//!
//! Este crate implementa as ports definidas em `trader-infra` para provedores
//! concretos. Inicialmente:
//!
//! - `ibkr`: Interactive Brokers via TWS API/IB Gateway (crate `ibapi`).
//! - `simulated`: implementações em memória para testes e backtest.

pub mod ibkr;
pub mod simulated;
