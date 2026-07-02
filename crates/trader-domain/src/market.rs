//! Tipos de mercado puros usados pelas ports de infraestrutura.
//!
//! Esses tipos não dependem de async, SQL, HTTP ou corretora específica.

use chrono::{DateTime, Utc};

use crate::{Fill, OrderId, OrderStatus, TimeFrame};

/// Requisição de candles históricos.
#[derive(Debug, Clone)]
pub struct CandleRequest {
    pub symbol: String,
    pub timeframe: TimeFrame,
    pub from: DateTime<Utc>,
    pub to: DateTime<Utc>,
}

/// Handle de uma subscrição de dados em tempo real.
#[derive(Debug, Clone)]
pub struct SubscriptionHandle {
    pub id: String,
}

/// Saúde de um provedor de dados.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderHealth {
    Healthy,
    Degraded,
    Unhealthy,
}

/// Evento de ordem vindo do broker.
#[derive(Debug, Clone)]
pub enum OrderEvent {
    Fill {
        order_id: OrderId,
        fill: Fill,
    },
    StatusUpdate {
        order_id: OrderId,
        status: OrderStatus,
    },
}
