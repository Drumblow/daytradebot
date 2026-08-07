//! Comando `analyze` — análise dos resultados do live/paper e comparação
//! com o backtest mais recente, contra os critérios de aceitação da estratégia.

use anyhow::Result;
use rust_decimal::Decimal;

use trader_backtest::BacktestMetrics;
use trader_domain::{Signal, SignalStatus};
use trader_infra::{
    db::create_pool,
    repositories::{SqlxBacktestRunRepository, SqlxSignalRepository, SqlxTradeRepository},
};

use crate::config::CliConfig;

/// Argumentos do comando analyze.
pub struct Args {
    pub symbol: String,
    pub strategy: String,
}

/// Tolerância do critério de paper: métricas do live devem estar dentro de
/// ±30% das métricas do backtest (docs/strategies/pullback-trend-v1.md).
const PAPER_TOLERANCE_PCT: i64 = 30;

/// Mínimo de trades em paper para considerar a amostra válida.
const PAPER_MIN_TRADES: usize = 20;

pub async fn run(config: &CliConfig, args: Args) -> Result<()> {
    println!("🔎 Análise de validação — {}", args.symbol);

    let database_url = config
        .app_config
        .database
        .url()
        .map_err(|e| anyhow::anyhow!("DATABASE_URL não configurada: {e}"))?;
    let pool = create_pool(&database_url)
        .await
        .map_err(|e| anyhow::anyhow!("falha ao conectar no banco: {e}"))?;

    let trade_repo = SqlxTradeRepository::new(pool.clone());
    let signal_repo = SqlxSignalRepository::new(pool.clone());
    let runs_repo = SqlxBacktestRunRepository::new(pool);

    let trades = trade_repo
        .list_by_symbol(&args.symbol, 10_000)
        .await
        .map_err(|e| anyhow::anyhow!("falha ao listar trades: {e}"))?;
    let signals = signal_repo
        .list_by_symbol(&args.symbol, 10_000)
        .await
        .map_err(|e| anyhow::anyhow!("falha ao listar sinais: {e}"))?;

    // Trades marcados como artefato operacional (ex.: bug de latência já
    // corrigido) não entram na amostra de validação.
    let (artifacts, sample): (Vec<_>, Vec<_>) =
        trades.into_iter().partition(|t| t.is_latency_artifact());

    // --- Live/paper ---
    let live_metrics = BacktestMetrics::from_trades(&sample, Decimal::from(100_000));
    println!("\n📈 Live/paper (trades persistidos):");
    print_metrics(&live_metrics);
    if !artifacts.is_empty() {
        println!(
            "   ({} trade(s) excluído(s) da amostra: artefato operacional — ver journal)",
            artifacts.len()
        );
    }

    // --- Sinais ---
    println!("\n📡 Sinais ({}):", signals.len());
    print_signal_breakdown(&signals);

    // --- Backtest mais recente ---
    let latest = runs_repo
        .latest_by_strategy(&args.strategy)
        .await
        .map_err(|e| anyhow::anyhow!("falha ao consultar backtest_runs: {e}"))?;

    let Some(run) = latest else {
        println!(
            "\n⚠️  Nenhum run de backtest encontrado para '{}'. \
             Rode 'trader-cli backtest' ou 'trader-cli walkforward' primeiro.",
            args.strategy
        );
        return Ok(());
    };

    let bt_metrics: BacktestMetrics = serde_json::from_value(run.metrics.clone())
        .map_err(|e| anyhow::anyhow!("métricas do run {} inválidas: {e}", run.id))?;

    println!(
        "\n📊 Backtest mais recente (id={}, label={}, {}) — {} → {}:",
        run.id,
        run.label.as_deref().unwrap_or("-"),
        run.created_at.date_naive(),
        run.period_start.date_naive(),
        run.period_end.date_naive()
    );
    print_metrics(&bt_metrics);

    // --- Veredito: critérios de aceitação em paper ---
    println!(
        "\n✅/❌ Critérios de aceitação em paper (±{}% do backtest):",
        PAPER_TOLERANCE_PCT
    );
    let verdict = evaluate_paper(&live_metrics, &bt_metrics);
    for (ok, label) in &verdict {
        println!("   [{}] {}", if *ok { "OK" } else { "--" }, label);
    }

    if verdict.iter().all(|(ok, _)| *ok) {
        println!("\n   Critérios de paper ATENDIDOS. Próximo gate: 3 meses de paper (docs/OPERATIONS.md).");
    } else {
        println!("\n   Critérios de paper AINDA NÃO atendidos — continue acumulando amostra.");
    }

    Ok(())
}

fn print_metrics(m: &BacktestMetrics) {
    println!("   Trades:        {}", m.total_trades);
    println!("   Win rate:      {:.1}%", m.win_rate);
    println!("   Profit factor: {}", m.profit_factor_display());
    println!("   Avg R/trade:   {:.3}", m.avg_r_per_trade);
    println!(
        "   Max drawdown:  {:.2} ({:.2}%)",
        m.max_drawdown, m.max_drawdown_pct
    );
    println!("   Net P&L:       {:.2}", m.net_pnl);
}

fn print_signal_breakdown(signals: &[Signal]) {
    let executed = signals
        .iter()
        .filter(|s| s.status != SignalStatus::Rejected)
        .count();
    println!(
        "   Executados: {} | Rejeitados: {}",
        executed,
        signals.len() - executed
    );

    let mut by_reason: std::collections::HashMap<String, usize> = Default::default();
    for s in signals
        .iter()
        .filter(|s| s.status == SignalStatus::Rejected)
    {
        let reason = s
            .rejection_reason
            .map(|r| format!("{r:?}"))
            .unwrap_or_else(|| "unknown".to_string());
        *by_reason.entry(reason).or_default() += 1;
    }

    let mut reasons: Vec<_> = by_reason.into_iter().collect();
    reasons.sort_by_key(|b| std::cmp::Reverse(b.1));
    for (reason, count) in reasons.into_iter().take(8) {
        println!("   - {:<28} {}", reason, count);
    }
}

/// Avalia os critérios de aceitação em paper trading:
/// ≥ 20 trades e métricas dentro de ±30% do backtest.
fn evaluate_paper(live: &BacktestMetrics, backtest: &BacktestMetrics) -> Vec<(bool, String)> {
    let tol = Decimal::from(PAPER_TOLERANCE_PCT);

    vec![
        (
            live.total_trades >= PAPER_MIN_TRADES,
            format!(
                "≥ {} trades em paper (atual: {})",
                PAPER_MIN_TRADES, live.total_trades
            ),
        ),
        (
            within_tolerance(live.win_rate, backtest.win_rate, tol),
            format!(
                "win rate dentro de ±{}% do backtest ({:.1}% vs {:.1}%)",
                PAPER_TOLERANCE_PCT, live.win_rate, backtest.win_rate
            ),
        ),
        (
            // PF é None quando não há perdas: só é comparável se ambos forem Some.
            match (live.profit_factor, backtest.profit_factor) {
                (Some(l), Some(b)) => within_tolerance(l, b, tol),
                (None, None) => true,
                _ => false,
            },
            format!(
                "profit factor dentro de ±{}% ({} vs {})",
                PAPER_TOLERANCE_PCT,
                live.profit_factor_display(),
                backtest.profit_factor_display()
            ),
        ),
        (
            within_tolerance(live.avg_r_per_trade, backtest.avg_r_per_trade, tol),
            format!(
                "avg R dentro de ±{}% ({:.3} vs {:.3})",
                PAPER_TOLERANCE_PCT, live.avg_r_per_trade, backtest.avg_r_per_trade
            ),
        ),
        (
            live.net_pnl > Decimal::ZERO,
            format!(
                "expectativa positiva no paper (net P&L: {:.2})",
                live.net_pnl
            ),
        ),
    ]
}

/// `live` está dentro de ±`tolerance_pct`% de `reference`?
fn within_tolerance(live: Decimal, reference: Decimal, tolerance_pct: Decimal) -> bool {
    if reference.is_zero() {
        return live.is_zero();
    }
    let diff_pct = (live - reference).abs() / reference.abs() * Decimal::from(100);
    diff_pct <= tolerance_pct
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tolerance_check() {
        let tol = Decimal::from(30);
        assert!(within_tolerance(
            Decimal::from(130),
            Decimal::from(100),
            tol
        ));
        assert!(within_tolerance(Decimal::from(70), Decimal::from(100), tol));
        assert!(!within_tolerance(
            Decimal::from(131),
            Decimal::from(100),
            tol
        ));
        assert!(!within_tolerance(
            Decimal::from(69),
            Decimal::from(100),
            tol
        ));
        assert!(within_tolerance(Decimal::ZERO, Decimal::ZERO, tol));
        assert!(!within_tolerance(Decimal::ONE, Decimal::ZERO, tol));
    }
}
