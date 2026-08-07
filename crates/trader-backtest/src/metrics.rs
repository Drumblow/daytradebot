//! Métricas de performance de backtest.

use chrono::{DateTime, Utc};
use rust_decimal::{Decimal, MathematicalOps};
use serde::{Deserialize, Serialize};

use trader_domain::Trade;

/// Métricas calculadas a partir de uma série de trades.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestMetrics {
    pub total_trades: usize,
    pub winning_trades: usize,
    pub losing_trades: usize,
    pub win_rate: Decimal,
    pub gross_profit: Decimal,
    pub gross_loss: Decimal,
    pub net_pnl: Decimal,
    /// Razão lucro bruto / perda bruta. `None` quando não há perdas no
    /// período (PF matematicamente infinito) ou quando não há trades.
    pub profit_factor: Option<Decimal>,
    pub max_drawdown: Decimal,
    pub max_drawdown_pct: Decimal,
    pub avg_pnl_per_trade: Decimal,
    pub avg_r_per_trade: Decimal,
    pub max_consecutive_losses: usize,
    pub best_trade: Decimal,
    pub worst_trade: Decimal,
    pub sharpe_ratio: Decimal,
}

impl BacktestMetrics {
    /// Calcula métricas a partir de uma lista de trades e capital inicial.
    pub fn from_trades(trades: &[Trade], initial_capital: Decimal) -> Self {
        Self::compute(trades, initial_capital, None)
    }

    /// Calcula métricas a partir de um resultado de backtest completo,
    /// incluindo série de equity para Sharpe.
    pub fn from_trades_with_equity(
        trades: &[Trade],
        initial_capital: Decimal,
        daily_equity: &[(DateTime<Utc>, Decimal)],
    ) -> Self {
        Self::compute(trades, initial_capital, Some(daily_equity))
    }

    fn compute(
        trades: &[Trade],
        initial_capital: Decimal,
        daily_equity: Option<&[(DateTime<Utc>, Decimal)]>,
    ) -> Self {
        if trades.is_empty() {
            return Self::empty(initial_capital);
        }

        let total_trades = trades.len();
        let mut winning_trades = 0usize;
        let mut losing_trades = 0usize;
        let mut gross_profit = Decimal::ZERO;
        let mut gross_loss = Decimal::ZERO;
        let mut max_drawdown = Decimal::ZERO;
        let mut max_drawdown_pct = Decimal::ZERO;
        let mut peak = initial_capital;
        let mut current_equity = initial_capital;
        let mut max_consecutive_losses = 0usize;
        let mut current_consecutive_losses = 0usize;
        let mut best_trade = Decimal::MIN;
        let mut worst_trade = Decimal::MAX;
        let mut total_r = Decimal::ZERO;

        for trade in trades {
            let pnl = trade.net_pnl;
            current_equity += pnl;

            if pnl > Decimal::ZERO {
                winning_trades += 1;
                gross_profit += pnl;
                current_consecutive_losses = 0;
            } else {
                losing_trades += 1;
                gross_loss += pnl.abs();
                current_consecutive_losses += 1;
                max_consecutive_losses = max_consecutive_losses.max(current_consecutive_losses);
            }

            if current_equity > peak {
                peak = current_equity;
            }

            let drawdown = peak - current_equity;
            if drawdown > max_drawdown {
                max_drawdown = drawdown;
                max_drawdown_pct = if peak.is_zero() {
                    Decimal::ZERO
                } else {
                    drawdown / peak * Decimal::from(100)
                };
            }

            best_trade = best_trade.max(pnl);
            worst_trade = worst_trade.min(pnl);
            total_r += trade.result_in_r;
        }

        let net_pnl = gross_profit - gross_loss;
        let win_rate = Decimal::from(winning_trades as i64) / Decimal::from(total_trades as i64)
            * Decimal::from(100);
        let profit_factor = if gross_loss.is_zero() {
            None
        } else {
            Some(gross_profit / gross_loss)
        };
        let avg_pnl_per_trade = net_pnl / Decimal::from(total_trades as i64);
        let avg_r_per_trade = total_r / Decimal::from(total_trades as i64);
        let sharpe_ratio = daily_equity.map(calculate_sharpe).unwrap_or(Decimal::ZERO);

        Self {
            total_trades,
            winning_trades,
            losing_trades,
            win_rate,
            gross_profit,
            gross_loss,
            net_pnl,
            profit_factor,
            max_drawdown,
            max_drawdown_pct,
            avg_pnl_per_trade,
            avg_r_per_trade,
            max_consecutive_losses,
            best_trade,
            worst_trade,
            sharpe_ratio,
        }
    }

    fn empty(_initial_capital: Decimal) -> Self {
        Self {
            total_trades: 0,
            winning_trades: 0,
            losing_trades: 0,
            win_rate: Decimal::ZERO,
            gross_profit: Decimal::ZERO,
            gross_loss: Decimal::ZERO,
            net_pnl: Decimal::ZERO,
            profit_factor: None,
            max_drawdown: Decimal::ZERO,
            max_drawdown_pct: Decimal::ZERO,
            avg_pnl_per_trade: Decimal::ZERO,
            avg_r_per_trade: Decimal::ZERO,
            max_consecutive_losses: 0,
            best_trade: Decimal::ZERO,
            worst_trade: Decimal::ZERO,
            sharpe_ratio: Decimal::ZERO,
        }
    }

    /// PF para exibição: "∞" quando não houve perdas (mas houve trades),
    /// "N/A" sem trades, ou o valor com 2 casas.
    pub fn profit_factor_display(&self) -> String {
        match self.profit_factor {
            Some(pf) => format!("{pf:.2}"),
            None if self.total_trades > 0 => "∞".to_string(),
            None => "N/A".to_string(),
        }
    }
}

/// Calcula um Sharpe simplificado anualizado a partir da série de equity.
///
/// Usa retornos entre amostras consecutivas. Sem taxa livre de risco (rf = 0).
/// A anualização deriva do intervalo mediano entre amostras: para candles de
/// 15min, períodos/ano ≈ 35040; para 1 dia, ≈ 365.
fn calculate_sharpe(equity_series: &[(DateTime<Utc>, Decimal)]) -> Decimal {
    if equity_series.len() < 2 {
        return Decimal::ZERO;
    }

    let returns: Vec<Decimal> = equity_series
        .windows(2)
        .map(|w| {
            let prev = w[0].1;
            let curr = w[1].1;
            if prev.is_zero() {
                Decimal::ZERO
            } else {
                (curr - prev) / prev
            }
        })
        .collect();

    if returns.is_empty() {
        return Decimal::ZERO;
    }

    let mean = returns.iter().copied().sum::<Decimal>() / Decimal::from(returns.len() as i64);

    let variance = returns
        .iter()
        .map(|r| {
            let diff = *r - mean;
            diff * diff
        })
        .sum::<Decimal>()
        / Decimal::from(returns.len() as i64);

    let std_dev = variance.sqrt().unwrap_or(Decimal::ZERO);
    if std_dev.is_zero() {
        return Decimal::ZERO;
    }

    // Intervalo mediano entre amostras → número de períodos por ano.
    let mut intervals: Vec<i64> = equity_series
        .windows(2)
        .map(|w| (w[1].0 - w[0].0).num_seconds())
        .filter(|s| *s > 0)
        .collect();
    intervals.sort_unstable();
    let median_secs = intervals
        .get(intervals.len() / 2)
        .copied()
        .unwrap_or(86_400);

    let secs_per_year = 365.25 * 86_400.0;
    let periods_per_year = secs_per_year / median_secs as f64;
    let annualizer =
        Decimal::from_f64_retain(periods_per_year.sqrt()).unwrap_or_else(|| Decimal::from(15));

    mean / std_dev * annualizer
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use trader_domain::{Direction, ExitReason};

    fn trade(net_pnl: i64, result_in_r: &str) -> Trade {
        let ts = Utc.with_ymd_and_hms(2026, 8, 3, 15, 0, 0).unwrap();
        Trade {
            id: None,
            symbol: "SPY".to_string(),
            signal_id: 1,
            position_id: None,
            direction: Direction::Long,
            entry_price: Decimal::from(100),
            exit_price: Decimal::from(101),
            quantity: Decimal::from(10),
            entry_time: ts,
            exit_time: ts,
            stop_price: Decimal::from(99),
            target_price: Some(Decimal::from(102)),
            gross_pnl: Decimal::from(net_pnl),
            commissions: Decimal::ZERO,
            fees: Decimal::ZERO,
            net_pnl: Decimal::from(net_pnl),
            risk_amount: Decimal::from(10),
            result_in_r: result_in_r.parse().unwrap(),
            exit_reason: ExitReason::Target,
            strategy_id: "test".to_string(),
            strategy_version: "1.0.0".to_string(),
            config_hash: "hash".to_string(),
            journal: serde_json::Value::Object(Default::default()),
            correlation_id: "corr".to_string(),
        }
    }

    #[test]
    fn metrics_of_mixed_trades() {
        let trades = vec![
            trade(200, "2"),
            trade(-100, "-1"),
            trade(200, "2"),
            trade(-100, "-1"),
        ];
        let m = BacktestMetrics::from_trades(&trades, Decimal::from(100_000));

        assert_eq!(m.total_trades, 4);
        assert_eq!(m.winning_trades, 2);
        assert_eq!(m.win_rate, Decimal::from(50));
        assert_eq!(m.gross_profit, Decimal::from(400));
        assert_eq!(m.gross_loss, Decimal::from(200));
        assert_eq!(m.profit_factor, Some(Decimal::from(2)));
        assert_eq!(m.net_pnl, Decimal::from(200));
        assert_eq!(m.avg_pnl_per_trade, Decimal::from(50));
        assert_eq!(m.avg_r_per_trade, Decimal::new(5, 1)); // 0.5
        assert_eq!(m.max_consecutive_losses, 1);
        assert_eq!(m.best_trade, Decimal::from(200));
        assert_eq!(m.worst_trade, Decimal::from(-100));
    }

    #[test]
    fn drawdown_tracks_peak_to_valley() {
        // sobe 200, perde 100 → drawdown de 100 sobre pico de 100200.
        let trades = vec![trade(200, "2"), trade(-100, "-1")];
        let m = BacktestMetrics::from_trades(&trades, Decimal::from(100_000));

        assert_eq!(m.max_drawdown, Decimal::from(100));
        assert!(m.max_drawdown_pct > Decimal::ZERO);
    }

    #[test]
    fn empty_trades_yield_zeroed_metrics() {
        let m = BacktestMetrics::from_trades(&[], Decimal::from(100_000));
        assert_eq!(m.total_trades, 0);
        assert_eq!(m.profit_factor, None);
        assert_eq!(m.profit_factor_display(), "N/A");
    }

    #[test]
    fn all_winners_profit_factor_is_infinite() {
        let trades = vec![trade(200, "2"), trade(100, "1")];
        let m = BacktestMetrics::from_trades(&trades, Decimal::from(100_000));
        assert_eq!(m.profit_factor, None);
        assert_eq!(m.profit_factor_display(), "∞");
    }

    #[test]
    fn sharpe_annualizes_by_sample_interval() {
        // Série de equity com crescimento constante: variância zero → Sharpe 0.
        let base = Utc.with_ymd_and_hms(2026, 8, 3, 14, 30, 0).unwrap();
        let flat: Vec<(DateTime<Utc>, Decimal)> = (0..10)
            .map(|i| {
                (
                    base + chrono::Duration::minutes(i * 15),
                    Decimal::from(100_000),
                )
            })
            .collect();
        assert_eq!(calculate_sharpe(&flat), Decimal::ZERO);

        // Série com retornos variáveis a cada 15min: Sharpe deve ser bem maior
        // que a versão diária (√252 ≈ 15.9 vs √35040 ≈ 187).
        let mut equity = Decimal::from(100_000);
        let mut series = Vec::new();
        for i in 0..100 {
            let delta = if i % 2 == 0 { 100 } else { -50 };
            equity += Decimal::from(delta);
            series.push((base + chrono::Duration::minutes(i * 15), equity));
        }
        let sharpe_15m = calculate_sharpe(&series);
        assert!(sharpe_15m > Decimal::from(15));
    }
}
