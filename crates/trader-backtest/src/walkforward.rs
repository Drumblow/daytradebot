//! Análise walk-forward (anchored) para validação out-of-sample.
//!
//! A estratégia não tem otimização de parâmetros (não há fitting), então o
//! walk-forward aqui responde: "a regra se sustenta fora do período em que
//! foi concebida?". Cada janela roda o backtest sobre o prefixo até o fim do
//! bloco de teste (garantindo warm-up dos indicadores) e separa os trades
//! em in-sample e out-of-sample pelo `entry_time`.

use std::ops::Range;

use chrono::{DateTime, Utc};

use trader_core::risk::RiskConfig;
use trader_domain::{Candle, Strategy, Trade};

use crate::engine::{BacktestConfig, BacktestEngine};
use crate::metrics::BacktestMetrics;

/// Resultado de uma janela de walk-forward.
#[derive(Debug, Clone)]
pub struct WindowResult {
    pub window: usize,
    pub train_start: DateTime<Utc>,
    pub train_end: DateTime<Utc>,
    pub test_start: DateTime<Utc>,
    pub test_end: DateTime<Utc>,
    pub in_sample: BacktestMetrics,
    pub out_of_sample: BacktestMetrics,
}

/// Resultado agregado do walk-forward.
#[derive(Debug, Clone)]
pub struct WalkForwardResult {
    pub windows: Vec<WindowResult>,
    /// Todos os trades out-of-sample concatenados (a amostra que conta).
    pub oos_trades: Vec<Trade>,
    pub oos_metrics: BacktestMetrics,
}

/// Divide a série em janelas anchored: com `windows` janelas, a série é
/// dividida em `windows + 1` blocos contíguos; a janela `i` treina nos
/// blocos `0..=i` e testa no bloco `i+1`.
///
/// Retorna `None` se não houver candles suficientes (mínimo de 2 candles por
/// bloco — na prática, muito mais, para warm-up dos indicadores).
pub fn split_windows(len: usize, windows: usize) -> Option<Vec<(Range<usize>, Range<usize>)>> {
    if windows == 0 || len < (windows + 1) * 2 {
        return None;
    }

    let block = len / (windows + 1);
    if block == 0 {
        return None;
    }

    Some(
        (0..windows)
            .map(|i| (0..(i + 1) * block, (i + 1) * block..(i + 2) * block))
            .collect(),
    )
}

/// Executa a análise walk-forward sobre uma série de candles.
pub async fn run_walk_forward<S: Strategy>(
    strategy: &S,
    candles: &[Candle],
    windows: usize,
    config: &BacktestConfig,
    risk_config: RiskConfig,
) -> anyhow::Result<WalkForwardResult> {
    let splits = split_windows(candles.len(), windows).ok_or_else(|| {
        anyhow::anyhow!(
            "série de {} candles insuficiente para {} janelas",
            candles.len(),
            windows
        )
    })?;

    let mut results = Vec::with_capacity(splits.len());
    let mut oos_trades: Vec<Trade> = Vec::new();

    for (i, (train_range, test_range)) in splits.iter().enumerate() {
        // Um único run sobre o prefixo até o fim do bloco de teste garante
        // warm-up dos indicadores; a separação IS/OOS é por entry_time.
        let run_candles = &candles[..test_range.end];
        let mut engine = BacktestEngine::new(config.clone(), risk_config);
        let run = engine.run(strategy, run_candles).await?;

        let test_start_ts = candles[test_range.start].timestamp;

        let is_trades: Vec<Trade> = run
            .closed_trades
            .iter()
            .filter(|t| t.entry_time < test_start_ts)
            .cloned()
            .collect();
        let window_oos: Vec<Trade> = run
            .closed_trades
            .into_iter()
            .filter(|t| t.entry_time >= test_start_ts)
            .collect();

        results.push(WindowResult {
            window: i + 1,
            train_start: candles[train_range.start].timestamp,
            train_end: candles[train_range.end - 1].timestamp,
            test_start: candles[test_range.start].timestamp,
            test_end: candles[test_range.end - 1].timestamp,
            in_sample: BacktestMetrics::from_trades(&is_trades, config.initial_capital),
            out_of_sample: BacktestMetrics::from_trades(&window_oos, config.initial_capital),
        });
        oos_trades.extend(window_oos);
    }

    let oos_metrics = BacktestMetrics::from_trades(&oos_trades, config.initial_capital);

    Ok(WalkForwardResult {
        windows: results,
        oos_trades,
        oos_metrics,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_covers_series_without_overlap() {
        // 100 candles, 4 janelas → blocos de 20.
        let splits = split_windows(100, 4).expect("split válido");
        assert_eq!(splits.len(), 4);
        assert_eq!(splits[0], (0..20, 20..40));
        assert_eq!(splits[3], (0..80, 80..100));

        // Blocos de teste não se sobrepõem e cobrem 20..100.
        let mut covered: Vec<usize> = Vec::new();
        for (_, test) in &splits {
            covered.extend(test.clone());
        }
        assert_eq!(covered, (20..100).collect::<Vec<_>>());
    }

    #[test]
    fn split_rejects_insufficient_data() {
        assert!(split_windows(5, 4).is_none());
        assert!(split_windows(100, 0).is_none());
    }
}
