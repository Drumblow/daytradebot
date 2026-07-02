//! `trader-core` — inteligência do HumanStyle Trader Bot.
//!
//! Este crate contém a lógica pura de trading:
//! - Indicadores técnicos (`indicators`).
//! - Análise de contexto de mercado (`context`).
//! - Gestão de risco (`risk`).
//! - Estratégias (`strategies`).
//!
//! Ele depende apenas de `trader-domain` e bibliotecas de cálculo, nunca de
//! brokers, banco de dados ou HTTP.

pub mod context;
pub mod execution;
pub mod indicators;
pub mod risk;
pub mod strategies;

pub use context::*;
pub use execution::*;
pub use indicators::*;
pub use risk::*;
pub use strategies::*;
