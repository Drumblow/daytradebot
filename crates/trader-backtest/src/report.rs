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
        // A base das métricas é a FATIA de capital da instância, não a conta
        // inteira (ADR-020). Com `capital_fraction = 1` são o mesmo número.
        let metrics = BacktestMetrics::from_trades_with_equity(
            &run.closed_trades,
            run.capital_base_metricas,
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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};
    use trader_domain::{Direction, ExitReason, Trade};

    fn trade(net_pnl: i64) -> Trade {
        let ts = Utc.with_ymd_and_hms(2026, 8, 3, 15, 0, 0).unwrap();
        Trade {
            id: None,
            symbol: "IJS".to_string(),
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
            result_in_r: Decimal::from(net_pnl) / Decimal::from(10),
            exit_reason: ExitReason::Target,
            strategy_id: "balance-area-breakout-v1".to_string(),
            strategy_version: "1.0.0".to_string(),
            config_hash: "hash".to_string(),
            journal: serde_json::Value::Object(Default::default()),
            correlation_id: "corr".to_string(),
        }
    }

    fn run(base: Decimal, trades: Vec<Trade>) -> BacktestRun {
        let ts = Utc.with_ymd_and_hms(2026, 8, 3, 15, 0, 0).unwrap();
        BacktestRun {
            symbol: "IJS".to_string(),
            start_time: ts,
            end_time: ts,
            initial_capital: Decimal::from(100_000),
            capital_base_metricas: base,
            final_equity: Decimal::from(100_000),
            total_trades: trades.len(),
            closed_trades: trades,
            daily_pnl_series: Vec::new(),
        }
    }

    /// O cuidado (a) do plano §5.5: sem fracionar a base, o `max_drawdown_pct`
    /// do gate A afrouxa **3×** só porque o sizing encolheu — o critério de
    /// 10% passaria a valer 30% sem ninguém tê-lo mudado.
    ///
    /// Com a base fracionada junto, o DD% de um run 3× menor é o MESMO. É por
    /// isso que `capital_fraction` é política de risco e não melhora número:
    /// o percentual não se move.
    #[test]
    fn a_base_do_dd_acompanha_a_fracao_de_capital() {
        // Conta inteira: perde 3.000 sobre 100.000 = 3%.
        let cheio = BacktestReport::from_run(run(
            Decimal::from(100_000),
            vec![trade(-1000), trade(-1000), trade(-1000)],
        ));
        assert_eq!(cheio.metrics.max_drawdown_pct.round_dp(2), Decimal::new(300, 2));

        // Um terço da conta com um terço do tamanho: mesmos 3%.
        let fracionado = BacktestReport::from_run(run(
            Decimal::new(3333333, 2),
            vec![trade(-333), trade(-333), trade(-334)],
        ));
        assert_eq!(
            fracionado.metrics.max_drawdown_pct.round_dp(2),
            Decimal::new(300, 2),
            "o DD% não pode mudar por causa do tamanho da posição"
        );

        // E se a base NÃO fosse fracionada, o mesmo prejuízo leria 1%: três
        // vezes mais leniente, em silêncio.
        let sem_fracionar_a_base = BacktestReport::from_run(run(
            Decimal::from(100_000),
            vec![trade(-333), trade(-333), trade(-334)],
        ));
        assert_eq!(
            sem_fracionar_a_base.metrics.max_drawdown_pct.round_dp(2),
            Decimal::new(100, 2)
        );
    }

    /// O relatório continua mostrando o capital da CONTA, não a fatia: são
    /// dois números com significados diferentes, e trocar um pelo outro faria
    /// o "capital inicial" do relatório mentir sobre o tamanho da conta.
    #[test]
    fn o_relatorio_mostra_o_capital_da_conta_e_nao_a_fatia() {
        let r = BacktestReport::from_run(run(Decimal::new(3333333, 2), vec![trade(100)]));
        assert_eq!(r.initial_capital, Decimal::from(100_000));
    }
}
