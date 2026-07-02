//! Configuração de conexão com IB Gateway/TWS.

use serde::Deserialize;

/// Configuração da Interactive Brokers (TWS API / IB Gateway).
#[derive(Debug, Clone, Deserialize)]
pub struct IbkrConfig {
    /// Host do IB Gateway. Padrão: `127.0.0.1`.
    pub host: String,

    /// Porta do IB Gateway.
    /// - Paper trading: `7497`
    /// - Conta real: `7496`
    pub port: u16,

    /// ID de cliente único por conexão. Padrão: `1`.
    pub client_id: i32,

    /// Identificador da conta (ex: `DU1234567`).
    pub account_id: Option<String>,

    /// Indica se é ambiente de paper trading.
    pub paper: bool,
}

impl Default for IbkrConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 7497,
            client_id: 1,
            account_id: None,
            paper: true,
        }
    }
}

impl IbkrConfig {
    /// Endereço completo de conexão no formato esperado pelo crate `ibapi`.
    pub fn connection_string(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}
