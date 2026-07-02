-- HumanStyle Trader Bot — Schema Inicial (Fase 1)
-- Baseado em docs/DATA-MODEL.md

CREATE TABLE IF NOT EXISTS assets (
    id              SERIAL PRIMARY KEY,
    symbol          TEXT NOT NULL UNIQUE,
    name            TEXT,
    asset_type      TEXT NOT NULL CHECK (asset_type IN ('stock', 'etf', 'crypto', 'forex', 'future', 'option')),
    exchange        TEXT,
    currency        TEXT NOT NULL DEFAULT 'USD',
    tick_size       NUMERIC NOT NULL,
    lot_size        NUMERIC NOT NULL DEFAULT 1,
    sector          TEXT,
    is_active       BOOLEAN NOT NULL DEFAULT true,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS candles (
    id              BIGSERIAL PRIMARY KEY,
    asset_id        INTEGER NOT NULL REFERENCES assets(id),
    timeframe       TEXT NOT NULL,
    timestamp       TIMESTAMPTZ NOT NULL,
    open            NUMERIC NOT NULL,
    high            NUMERIC NOT NULL,
    low             NUMERIC NOT NULL,
    close           NUMERIC NOT NULL,
    volume          NUMERIC NOT NULL DEFAULT 0,
    vwap            NUMERIC,
    source          TEXT NOT NULL,
    is_complete     BOOLEAN NOT NULL DEFAULT true,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT candles_unique UNIQUE (asset_id, timeframe, timestamp),
    CONSTRAINT candles_high_low_check CHECK (high >= low),
    CONSTRAINT candles_ohlc_check CHECK (
        high >= open AND high >= close AND
        low <= open AND low <= close
    )
);

CREATE INDEX IF NOT EXISTS idx_candles_asset_timeframe_timestamp
    ON candles (asset_id, timeframe, timestamp DESC);

CREATE TABLE IF NOT EXISTS indicators (
    id              BIGSERIAL PRIMARY KEY,
    candle_id       BIGINT NOT NULL REFERENCES candles(id) ON DELETE CASCADE,
    indicator_name  TEXT NOT NULL,
    value           NUMERIC NOT NULL,
    parameters      JSONB NOT NULL,
    calculated_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT indicators_unique UNIQUE (candle_id, indicator_name, parameters)
);

CREATE INDEX IF NOT EXISTS idx_indicators_candle ON indicators (candle_id);

CREATE TABLE IF NOT EXISTS market_contexts (
    id                  BIGSERIAL PRIMARY KEY,
    asset_id            INTEGER NOT NULL REFERENCES assets(id),
    timeframe           TEXT NOT NULL,
    timestamp           TIMESTAMPTZ NOT NULL,
    candle_id           BIGINT REFERENCES candles(id),

    trend_state         TEXT NOT NULL CHECK (trend_state IN ('uptrend', 'downtrend', 'neutral', 'unknown')),
    volatility_regime   TEXT NOT NULL CHECK (volatility_regime IN ('high', 'normal', 'low', 'unknown')),
    market_phase        TEXT NOT NULL CHECK (market_phase IN ('pre_market', 'regular', 'after_hours', 'unknown')),

    ema_20              NUMERIC,
    ema_50              NUMERIC,
    sma_200             NUMERIC,
    atr_14              NUMERIC,
    atr_percent_14      NUMERIC,
    volume_relative     NUMERIC,
    hh_hl_count         INTEGER,
    lh_ll_count         INTEGER,
    range_percent       NUMERIC,
    is_tradeable        BOOLEAN NOT NULL DEFAULT false,

    raw_values          JSONB NOT NULL DEFAULT '{}',
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT market_contexts_unique UNIQUE (asset_id, timeframe, timestamp)
);

CREATE INDEX IF NOT EXISTS idx_market_contexts_asset_timeframe_timestamp
    ON market_contexts (asset_id, timeframe, timestamp DESC);

CREATE TABLE IF NOT EXISTS signals (
    id                  BIGSERIAL PRIMARY KEY,
    asset_id            INTEGER NOT NULL REFERENCES assets(id),
    strategy_id         TEXT NOT NULL,
    strategy_version    TEXT NOT NULL,
    config_hash         TEXT NOT NULL,

    timeframe           TEXT NOT NULL,
    timestamp           TIMESTAMPTZ NOT NULL,
    direction           TEXT CHECK (direction IN ('long', 'short')),
    status              TEXT NOT NULL CHECK (status IN ('accepted', 'rejected', 'pending', 'expired')),

    entry_price         NUMERIC,
    stop_price          NUMERIC,
    target_price        NUMERIC,
    risk_reward_ratio   NUMERIC,

    risk_amount         NUMERIC,
    risk_percent        NUMERIC,
    position_size       NUMERIC,

    entry_reason        TEXT,
    rejection_reason    TEXT,
    rejection_details   JSONB,

    context_id          BIGINT REFERENCES market_contexts(id),
    market_snapshot     JSONB NOT NULL,

    correlation_id      UUID NOT NULL,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_signals_asset_strategy_timestamp
    ON signals (asset_id, strategy_id, timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_signals_status ON signals (status);

CREATE TABLE IF NOT EXISTS orders (
    id                  BIGSERIAL PRIMARY KEY,
    signal_id           BIGINT REFERENCES signals(id),
    asset_id            INTEGER NOT NULL REFERENCES assets(id),

    broker_order_id     TEXT,
    parent_order_id     BIGINT REFERENCES orders(id),

    side                TEXT NOT NULL CHECK (side IN ('buy', 'sell')),
    order_type          TEXT NOT NULL CHECK (order_type IN ('market', 'limit', 'stop', 'stop_limit', 'bracket')),
    status              TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'submitted', 'accepted', 'partially_filled', 'filled', 'cancelled', 'rejected', 'expired')),
    time_in_force       TEXT NOT NULL DEFAULT 'day' CHECK (time_in_force IN ('day', 'gtc', 'ioc', 'fok')),

    quantity            NUMERIC NOT NULL,
    filled_quantity     NUMERIC NOT NULL DEFAULT 0,
    remaining_quantity  NUMERIC GENERATED ALWAYS AS (quantity - filled_quantity) STORED,

    price               NUMERIC,
    stop_price          NUMERIC,
    avg_fill_price      NUMERIC,

    broker              TEXT NOT NULL,
    error_message       TEXT,
    metadata            JSONB NOT NULL DEFAULT '{}',

    correlation_id      UUID NOT NULL,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    submitted_at        TIMESTAMPTZ,
    filled_at           TIMESTAMPTZ,
    cancelled_at        TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_orders_signal ON orders (signal_id);
CREATE INDEX IF NOT EXISTS idx_orders_status ON orders (status);
CREATE INDEX IF NOT EXISTS idx_orders_broker_order_id ON orders (broker, broker_order_id);

CREATE TABLE IF NOT EXISTS fills (
    id              BIGSERIAL PRIMARY KEY,
    order_id        BIGINT NOT NULL REFERENCES orders(id),
    asset_id        INTEGER NOT NULL REFERENCES assets(id),

    fill_price      NUMERIC NOT NULL,
    quantity        NUMERIC NOT NULL,
    commission      NUMERIC NOT NULL DEFAULT 0,
    fees            NUMERIC NOT NULL DEFAULT 0,

    broker_fill_id  TEXT,
    timestamp       TIMESTAMPTZ NOT NULL,

    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_fills_order ON fills (order_id);

CREATE TABLE IF NOT EXISTS positions (
    id                  BIGSERIAL PRIMARY KEY,
    asset_id            INTEGER NOT NULL REFERENCES assets(id),
    signal_id           BIGINT NOT NULL REFERENCES signals(id),

    direction           TEXT NOT NULL CHECK (direction IN ('long', 'short')),
    quantity            NUMERIC NOT NULL,
    avg_entry_price     NUMERIC NOT NULL,
    entry_time          TIMESTAMPTZ NOT NULL,

    stop_price          NUMERIC NOT NULL,
    target_price        NUMERIC,

    unrealized_pnl      NUMERIC NOT NULL DEFAULT 0,
    realized_pnl        NUMERIC NOT NULL DEFAULT 0,

    status              TEXT NOT NULL DEFAULT 'open' CHECK (status IN ('open', 'closed')),
    closed_at           TIMESTAMPTZ,

    broker              TEXT NOT NULL,
    metadata            JSONB NOT NULL DEFAULT '{}',

    correlation_id      UUID NOT NULL,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_positions_asset_status ON positions (asset_id, status);

CREATE TABLE IF NOT EXISTS trades (
    id                  BIGSERIAL PRIMARY KEY,
    asset_id            INTEGER NOT NULL REFERENCES assets(id),
    signal_id           BIGINT NOT NULL REFERENCES signals(id),
    position_id         BIGINT REFERENCES positions(id),

    direction           TEXT NOT NULL CHECK (direction IN ('long', 'short')),
    entry_price         NUMERIC NOT NULL,
    exit_price          NUMERIC NOT NULL,
    quantity            NUMERIC NOT NULL,

    entry_time          TIMESTAMPTZ NOT NULL,
    exit_time           TIMESTAMPTZ NOT NULL,

    stop_price          NUMERIC NOT NULL,
    target_price        NUMERIC,

    gross_pnl           NUMERIC NOT NULL,
    commissions         NUMERIC NOT NULL DEFAULT 0,
    fees                NUMERIC NOT NULL DEFAULT 0,
    net_pnl             NUMERIC NOT NULL,

    risk_amount         NUMERIC NOT NULL,
    result_in_r         NUMERIC NOT NULL,

    exit_reason         TEXT NOT NULL CHECK (exit_reason IN ('target', 'stop', 'time', 'manual', 'risk_manager')),
    strategy_id         TEXT NOT NULL,
    strategy_version    TEXT NOT NULL,
    config_hash         TEXT NOT NULL,

    journal             JSONB NOT NULL DEFAULT '{}',
    correlation_id      UUID NOT NULL,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_trades_asset_strategy ON trades (asset_id, strategy_id, exit_time DESC);
CREATE INDEX IF NOT EXISTS idx_trades_exit_time ON trades (exit_time DESC);

CREATE TABLE IF NOT EXISTS strategy_configs (
    id              SERIAL PRIMARY KEY,
    strategy_id     TEXT NOT NULL,
    version         TEXT NOT NULL,
    config_hash     TEXT NOT NULL UNIQUE,
    config          JSONB NOT NULL,
    source          TEXT,
    description     TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT strategy_configs_unique UNIQUE (strategy_id, version)
);

CREATE TABLE IF NOT EXISTS risk_limits (
    id                      SERIAL PRIMARY KEY,
    name                    TEXT NOT NULL UNIQUE,
    risk_per_trade_pct      NUMERIC NOT NULL,
    max_daily_loss_pct      NUMERIC NOT NULL,
    max_trades_per_day      INTEGER NOT NULL,
    max_consecutive_losses  INTEGER NOT NULL,
    max_spread_pct          NUMERIC NOT NULL,
    max_atr_pct             NUMERIC NOT NULL,
    min_risk_reward         NUMERIC NOT NULL,
    trading_start_time      TIME NOT NULL,
    trading_end_time        TIME NOT NULL,
    is_active               BOOLEAN NOT NULL DEFAULT true,
    created_at              TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at              TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS account_snapshots (
    id                  BIGSERIAL PRIMARY KEY,
    broker              TEXT NOT NULL,
    account_id          TEXT,
    timestamp           TIMESTAMPTZ NOT NULL,
    cash                NUMERIC NOT NULL,
    equity              NUMERIC NOT NULL,
    buying_power        NUMERIC NOT NULL,
    daily_pnl           NUMERIC NOT NULL DEFAULT 0,
    daily_return_pct    NUMERIC,
    metadata            JSONB NOT NULL DEFAULT '{}',

    CONSTRAINT account_snapshots_unique UNIQUE (broker, account_id, timestamp)
);

CREATE INDEX IF NOT EXISTS idx_account_snapshots_time ON account_snapshots (timestamp DESC);

CREATE TABLE IF NOT EXISTS system_events (
    id              BIGSERIAL PRIMARY KEY,
    timestamp       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    level           TEXT NOT NULL CHECK (level IN ('debug', 'info', 'warning', 'error', 'critical')),
    component       TEXT NOT NULL,
    event_type      TEXT NOT NULL,
    message         TEXT NOT NULL,
    payload         JSONB,
    correlation_id  UUID
);

CREATE INDEX IF NOT EXISTS idx_system_events_timestamp ON system_events (timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_system_events_level ON system_events (level);
CREATE INDEX IF NOT EXISTS idx_system_events_correlation ON system_events (correlation_id);

CREATE TABLE IF NOT EXISTS ingestions (
    id                  BIGSERIAL PRIMARY KEY,
    asset_id            INTEGER NOT NULL REFERENCES assets(id),
    timeframe           TEXT NOT NULL,
    source              TEXT NOT NULL,
    start_time          TIMESTAMPTZ NOT NULL,
    end_time            TIMESTAMPTZ NOT NULL,
    candles_inserted    INTEGER NOT NULL DEFAULT 0,
    candles_updated     INTEGER NOT NULL DEFAULT 0,
    gaps_detected       INTEGER NOT NULL DEFAULT 0,
    status              TEXT NOT NULL CHECK (status IN ('running', 'completed', 'failed')),
    error_message       TEXT,
    started_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    finished_at         TIMESTAMPTZ
);
