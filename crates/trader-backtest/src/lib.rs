//! `trader-backtest` — engine de backtest para o HumanStyle Trader Bot.
//!
//! Executa estratégias sobre candles históricos de forma determinística,
//! aplicando as mesmas regras de risco e execução do modo live/paper.

pub mod engine;
pub mod metrics;
pub mod report;

pub use engine::*;
pub use metrics::*;
pub use report::*;
