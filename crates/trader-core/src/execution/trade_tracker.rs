//! Rastreador de fills para reconstruir trades fechados no modo live.
//!
//! No modo live contra a IBKR, o bot não gerencia stops/alvos localmente —
//! eles ficam server-side no bracket. O que chega ao bot é uma sequência de
//! fills (via `subscribe_order_events`). Este tracker agrega fills de entrada
//! e saída e, quando a posição zera, produz um `ClosedTrade` com preços
//! médios ponderados e P&L — pronto para persistir e alimentar o `RiskState`.
//!
//! Premissa do MVP: no máximo uma posição aberta por símbolo (garantido pela
//! reconciliação do worker live), então um tracker por símbolo é suficiente.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use trader_domain::{Direction, OrderSide};

/// Fill normalizado para o tracker (independente de broker e de banco).
#[derive(Debug, Clone, Copy)]
pub struct TrackerFill {
    pub side: OrderSide,
    pub price: Decimal,
    pub quantity: Decimal,
    pub commission: Decimal,
    pub timestamp: DateTime<Utc>,
}

/// Trade fechado derivado de uma sequência de fills.
#[derive(Debug, Clone, PartialEq)]
pub struct ClosedTrade {
    pub direction: Direction,
    pub quantity: Decimal,
    /// Preço médio ponderado de entrada.
    pub entry_price: Decimal,
    /// Preço médio ponderado de saída.
    pub exit_price: Decimal,
    pub entry_time: DateTime<Utc>,
    pub exit_time: DateTime<Utc>,
    pub gross_pnl: Decimal,
    pub commissions: Decimal,
    pub net_pnl: Decimal,
}

/// Classifica o motivo da saída comparando o preço médio de saída com o stop
/// e o alvo planejados. Heurística: tolerância de ~2 ticks, pois o fill real
/// raramente é exatamente no preço do stop/alvo (slippage). Se não bater em
/// nenhum dos dois, considera `Manual` (saída fora do plano — ex.: ordem
/// cancelada/fechada manualmente na corretora).
pub fn classify_exit_reason(
    exit_price: Decimal,
    stop_price: Decimal,
    target_price: Option<Decimal>,
) -> trader_domain::ExitReason {
    const TOLERANCE_TICKS: i64 = 2;
    let tick = Decimal::new(1, 2); // 0.01 — tick padrão de ações US
    let tolerance = tick * Decimal::from(TOLERANCE_TICKS);

    if (exit_price - stop_price).abs() <= tolerance {
        return trader_domain::ExitReason::Stop;
    }
    if let Some(target) = target_price {
        if (exit_price - target).abs() <= tolerance {
            return trader_domain::ExitReason::Target;
        }
    }
    trader_domain::ExitReason::Manual
}

/// Agrega fills de uma posição até ela zerar.
///
/// O primeiro fill define a direção (compra → long, venda → short). Fills do
/// lado da direção acumulam como entrada; fills do lado oposto acumulam como
/// saída. Quando a quantidade de saída cobre a de entrada, o trade fecha e o
/// tracker volta ao estado inicial, pronto para o próximo ciclo.
#[derive(Debug, Default)]
pub struct FillTracker {
    direction: Option<Direction>,
    entry_qty: Decimal,
    entry_cost: Decimal,
    entry_time: Option<DateTime<Utc>>,
    exit_qty: Decimal,
    exit_value: Decimal,
    exit_time: Option<DateTime<Utc>>,
    commissions: Decimal,
}

impl FillTracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// `true` enquanto há uma posição em aberto sendo rastreada.
    pub fn is_open(&self) -> bool {
        self.direction.is_some()
    }

    /// Registra um fill. Retorna `Some(ClosedTrade)` quando a posição zera.
    pub fn on_fill(&mut self, fill: TrackerFill) -> Option<ClosedTrade> {
        if fill.quantity <= Decimal::ZERO {
            return None;
        }

        let direction = *self.direction.get_or_insert(match fill.side {
            OrderSide::Buy => Direction::Long,
            OrderSide::Sell => Direction::Short,
        });

        self.commissions += fill.commission;

        let is_entry_side = matches!(
            (direction, fill.side),
            (Direction::Long, OrderSide::Buy) | (Direction::Short, OrderSide::Sell)
        );

        if is_entry_side {
            self.entry_qty += fill.quantity;
            self.entry_cost += fill.price * fill.quantity;
            self.entry_time.get_or_insert(fill.timestamp);
        } else {
            self.exit_qty += fill.quantity;
            self.exit_value += fill.price * fill.quantity;
            self.exit_time = Some(fill.timestamp);
        }

        if self.direction.is_some() && self.exit_qty >= self.entry_qty {
            return Some(self.close());
        }
        None
    }

    fn close(&mut self) -> ClosedTrade {
        let direction = self.direction.unwrap_or(Direction::Long);
        let quantity = self.entry_qty;
        let entry_price = self.entry_cost / quantity;
        let exit_price = self.exit_value / self.exit_qty;
        let gross_pnl = match direction {
            Direction::Long => (exit_price - entry_price) * quantity,
            Direction::Short => (entry_price - exit_price) * quantity,
        };

        let trade = ClosedTrade {
            direction,
            quantity,
            entry_price,
            exit_price,
            entry_time: self.entry_time.unwrap_or_else(Utc::now),
            exit_time: self.exit_time.unwrap_or_else(Utc::now),
            gross_pnl,
            commissions: self.commissions,
            net_pnl: gross_pnl - self.commissions,
        };

        *self = Self::default();
        trade
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn ts(minute: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 8, 3, 14, minute, 0).unwrap()
    }

    fn fill(side: OrderSide, price: i64, qty: i64, minute: u32) -> TrackerFill {
        TrackerFill {
            side,
            price: Decimal::from(price),
            quantity: Decimal::from(qty),
            commission: Decimal::ZERO,
            timestamp: ts(minute),
        }
    }

    #[test]
    fn long_round_trip_closes_trade() {
        let mut tracker = FillTracker::new();
        assert!(tracker.on_fill(fill(OrderSide::Buy, 100, 10, 30)).is_none());
        assert!(tracker.is_open());

        let trade = tracker
            .on_fill(fill(OrderSide::Sell, 105, 10, 45))
            .expect("posição zerada deveria fechar o trade");

        assert_eq!(trade.direction, Direction::Long);
        assert_eq!(trade.quantity, Decimal::from(10));
        assert_eq!(trade.entry_price, Decimal::from(100));
        assert_eq!(trade.exit_price, Decimal::from(105));
        assert_eq!(trade.gross_pnl, Decimal::from(50));
        assert_eq!(trade.net_pnl, Decimal::from(50));
        assert_eq!(trade.entry_time, ts(30));
        assert_eq!(trade.exit_time, ts(45));
        assert!(!tracker.is_open());
    }

    #[test]
    fn partial_entries_and_exits_use_weighted_average() {
        let mut tracker = FillTracker::new();
        tracker.on_fill(fill(OrderSide::Buy, 100, 5, 30));
        tracker.on_fill(fill(OrderSide::Buy, 110, 5, 31));
        assert!(tracker.on_fill(fill(OrderSide::Sell, 120, 4, 40)).is_none());

        let trade = tracker
            .on_fill(fill(OrderSide::Sell, 130, 6, 41))
            .expect("segunda saída zera a posição");

        // entrada média: (100*5 + 110*5) / 10 = 105
        assert_eq!(trade.entry_price, Decimal::from(105));
        // saída média: (120*4 + 130*6) / 10 = 126
        assert_eq!(trade.exit_price, Decimal::from(126));
        assert_eq!(trade.gross_pnl, Decimal::from(210));
    }

    #[test]
    fn short_round_trip() {
        let mut tracker = FillTracker::new();
        tracker.on_fill(fill(OrderSide::Sell, 100, 10, 30));
        let trade = tracker
            .on_fill(fill(OrderSide::Buy, 90, 10, 45))
            .expect("cobertura fecha o short");

        assert_eq!(trade.direction, Direction::Short);
        assert_eq!(trade.gross_pnl, Decimal::from(100));
    }

    #[test]
    fn commissions_reduce_net_pnl() {
        let mut tracker = FillTracker::new();
        let mut entry = fill(OrderSide::Buy, 100, 10, 30);
        entry.commission = Decimal::from(1);
        let mut exit = fill(OrderSide::Sell, 100, 10, 45);
        exit.commission = Decimal::from(1);

        tracker.on_fill(entry);
        let trade = tracker.on_fill(exit).expect("trade fechado");

        assert_eq!(trade.gross_pnl, Decimal::ZERO);
        assert_eq!(trade.commissions, Decimal::from(2));
        assert_eq!(trade.net_pnl, Decimal::from(-2));
    }

    #[test]
    fn tracker_is_reusable_after_close() {
        let mut tracker = FillTracker::new();
        tracker.on_fill(fill(OrderSide::Buy, 100, 10, 30));
        assert!(tracker
            .on_fill(fill(OrderSide::Sell, 100, 10, 35))
            .is_some());

        tracker.on_fill(fill(OrderSide::Buy, 200, 5, 50));
        let trade = tracker
            .on_fill(fill(OrderSide::Sell, 210, 5, 55))
            .expect("novo ciclo fecha novo trade");
        assert_eq!(trade.entry_price, Decimal::from(200));
    }

    #[test]
    fn zero_quantity_fill_is_ignored() {
        let mut tracker = FillTracker::new();
        assert!(tracker.on_fill(fill(OrderSide::Buy, 100, 0, 30)).is_none());
        assert!(!tracker.is_open());
    }

    #[test]
    fn exit_reason_classification() {
        use trader_domain::ExitReason;
        let stop = Decimal::from(95);
        let target = Some(Decimal::from(110));

        assert_eq!(
            classify_exit_reason(Decimal::new(9499, 2), stop, target),
            ExitReason::Stop
        );
        assert_eq!(
            classify_exit_reason(Decimal::new(11001, 2), stop, target),
            ExitReason::Target
        );
        assert_eq!(
            classify_exit_reason(Decimal::from(102), stop, target),
            ExitReason::Manual
        );
    }
}
