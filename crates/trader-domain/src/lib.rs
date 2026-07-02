//! `trader-domain` — vocabulário central do HumanStyle Trader Bot.
//!
//! Este crate contém entidades, enums, traits e erros de domínio.
//! Ele não depende de async, SQL, HTTP ou de qualquer corretora específica.

pub mod context;
pub mod entities;
pub mod errors;
pub mod market;
pub mod orders;
pub mod ports;
pub mod signals;
pub mod strategy;
pub mod trades;
pub mod trading_mode;

pub use context::*;
pub use entities::*;
pub use errors::*;
pub use market::*;
pub use orders::*;
pub use ports::*;
pub use signals::*;
pub use strategy::*;
pub use trades::*;
pub use trading_mode::*;
