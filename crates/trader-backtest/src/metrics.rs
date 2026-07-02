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
    pub profit_factor: Decimal,
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
            Decimal::ZERO
        } else {
            gross_profit / gross_loss
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
            profit_factor: Decimal::ZERO,
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
}

/// Calcula um Sharpe simplificado anualizado a partir da série de equity.
///
/// Usa retornos candle-a-candle. Sem taxa livre de risco (rf = 0).
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

    // Anualização simplificada: assume ~252 dias úteis com um retorno por dia.
    // Para candles mais frequentes, ajustar depois.
    let sqrt_252 = Decimal::from_f64_retain(252.0_f64.sqrt()).unwrap_or(Decimal::from(15));
    mean / std_dev * sqrt_252
}
