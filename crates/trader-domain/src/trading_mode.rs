//! Modo de operação do robô.
//!
//! Garante que o sistema só opere em modo real após explícita confirmação
//! de configuração, prevenindo execução acidental em conta de produção.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// Modo de operação do robô.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TradingMode {
    /// Operação simulada / paper trading. Único modo permitido no MVP.
    #[default]
    Paper,
    /// Operação com dinheiro real. Requer confirmação explícita.
    Real,
}

impl TradingMode {
    /// Retorna `true` se o modo for paper.
    pub fn is_paper(&self) -> bool {
        matches!(self, TradingMode::Paper)
    }

    /// Retorna `true` se o modo for real.
    pub fn is_real(&self) -> bool {
        matches!(self, TradingMode::Real)
    }
}

impl fmt::Display for TradingMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TradingMode::Paper => write!(f, "paper"),
            TradingMode::Real => write!(f, "real"),
        }
    }
}

impl FromStr for TradingMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "paper" => Ok(TradingMode::Paper),
            "real" => Ok(TradingMode::Real),
            _ => Err(format!("modo de operação inválido: {s}")),
        }
    }
}
