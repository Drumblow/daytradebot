//! Implementações do trait `Clock` definido em `trader-domain`.

use chrono::{DateTime, Utc};
use trader_domain::Clock;

/// Relógio de sistema.
pub struct SystemClock;

impl SystemClock {
    pub fn new() -> Self {
        Self
    }
}

impl Default for SystemClock {
    fn default() -> Self {
        Self::new()
    }
}

impl Clock for SystemClock {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }
}

/// Relógio mock para testes determinísticos.
pub struct MockClock {
    now: DateTime<Utc>,
}

impl MockClock {
    pub fn new(now: DateTime<Utc>) -> Self {
        Self { now }
    }

    pub fn advance(&mut self, duration: chrono::Duration) {
        self.now += duration;
    }
}

impl Clock for MockClock {
    fn now(&self) -> DateTime<Utc> {
        self.now
    }
}
