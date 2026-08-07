-- 0003: histórico de execuções de backtest.
-- Permite comparar runs (walk-forward, mudanças de config) e alimenta o
-- comparador backtest-vs-live (critérios de aceitação da estratégia).

CREATE TABLE IF NOT EXISTS backtest_runs (
    id               BIGSERIAL PRIMARY KEY,
    asset_id         INTEGER NOT NULL REFERENCES assets(id),
    strategy_id      TEXT NOT NULL,
    strategy_version TEXT NOT NULL,
    config_hash      TEXT NOT NULL,
    timeframe        TEXT NOT NULL,
    period_start     TIMESTAMPTZ NOT NULL,
    period_end       TIMESTAMPTZ NOT NULL,
    initial_capital  NUMERIC NOT NULL,
    final_equity     NUMERIC NOT NULL,
    metrics          JSONB NOT NULL,
    label            TEXT,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_backtest_runs_strategy ON backtest_runs (strategy_id, created_at DESC);
