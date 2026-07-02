//! Traits mínimos de domínio.
//!
//! Este módulo contém apenas abstrações que não dependem de async, SQL, HTTP
//! ou corretora. Ports de infraestrutura (market data, broker, repositórios)
//! ficam em `trader-infra`.

use chrono::{DateTime, Utc};

/// Abstração de relógio para testes determinísticos.
pub trait Clock: Send + Sync {
    fn now(&self) -> DateTime<Utc>;
}
