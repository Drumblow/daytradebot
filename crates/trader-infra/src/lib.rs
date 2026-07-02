//! `trader-infra` — infraestrutura do HumanStyle Trader Bot.
//!
//! Contém conexão com PostgreSQL, migrations, repositories, configuração,
//! logging/tracing e abstrações de relógio.

pub mod clock;
pub mod config;
pub mod db;
pub mod logging;
pub mod ports;
pub mod repositories;
