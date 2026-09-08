//! Relatório de backtest.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::engine::BacktestRun;
use crate::metrics::BacktestMetrics;

/// Relatório completo de uma execução de backtest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestReport {
    pub symbol: String,
    pub start_time: chrono::DateTime<chrono::Utc>,
    pub end_time: chrono::DateTime<chrono::Utc>,
    pub initial_capital: Decimal,
    pub final_equity: Decimal,
    pub metrics: BacktestMetrics,
    pub trades: Vec<trader_domain::Trade>,
}

impl BacktestReport {
    /// Cria um relatório a partir do resultado bruto de um backtest.
    pub fn from_run(run: BacktestRun) -> Self {
        let metrics = BacktestMetrics::from_trades_with_equity(
            &run.closed_trades,
            run.initial_capital,
            &run.daily_pnl_series,
        );

        Self {
            symbol: run.symbol,
            start_time: run.start_time,
            end_time: run.end_time,
            initial_capital: run.initial_capital,
            final_equity: run.final_equity,
            metrics,
            trades: run.closed_trades,
        }
    }

    /// Retorna o relatório formatado como JSON.
    pub fn to_json(&self) -> anyhow::Result<String> {
        Ok(serde_json::to_string_pretty(self)?)
    }
}

impl std::fmt::Display for BacktestReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "📊 Backtest Report: {}", self.symbol)?;
        writeln!(f, "   Período: {} → {}", self.start_time, self.end_time)?;
        writeln!(f, "   Capital inicial: {}", self.initial_capital)?;
        writeln!(f, "   Equity final:    {}", self.final_equity)?;
        writeln!(f, "   Net P&L:         {}", self.metrics.net_pnl)?;
        writeln!(f, "   Total trades:    {}", self.metrics.total_trades)?;
        writeln!(f, "   Win rate:        {}%", self.metrics.win_rate)?;
        writeln!(
            f,
            "   Profit factor:   {}",
            self.metrics.profit_factor_display()
        )?;
        writeln!(
            f,
            "   Max drawdown:    {} ({}%)",
            self.metrics.max_drawdown, self.metrics.max_drawdown_pct
        )?;
        writeln!(f, "   Avg R/trade:     {}", self.metrics.avg_r_per_trade)?;
        writeln!(f, "   Best trade:      {}", self.metrics.best_trade)?;
        writeln!(f, "   Worst trade:     {}", self.metrics.worst_trade)?;

        // Quebra por motivo de saída (ADR-018). A linha `end_of_day` responde
        // à pergunta que a régua antiga escondia: quanto do resultado vinha de
        // posição encerrada no sino em vez de stop/alvo.
        if !self.metrics.by_exit_reason.is_empty() {
            writeln!(f, "\n   Saídas por motivo:")?;
            writeln!(
                f,
                "   {:<14} {:>7} {:>7} {:>8} {:>12}",
                "motivo", "trades", "win%", "PF", "net P&L"
            )?;
            for (reason, g) in &self.metrics.by_exit_reason {
                let win_pct = if g.trades > 0 {
                    Decimal::from(g.wins as i64) / Decimal::from(g.trades as i64)
                        * Decimal::from(100)
                } else {
                    Decimal::ZERO
                };
                let pf = match g.profit_factor {
                    Some(pf) => format!("{pf:.2}"),
                    None if g.trades > 0 => "∞".to_string(),
                    None => "N/A".to_string(),
                };
                writeln!(
                    f,
                    "   {:<14} {:>7} {:>6.1}% {:>8} {:>12.2}",
                    reason, g.trades, win_pct, pf, g.net_pnl
                )?;
            }
        }
        Ok(())
    }
}
