# Modelo de Dados — HumanStyle Trader Bot

**Versão:** 1.0  
**Status:** Aprovado para implementação  
**Última atualização:** 2026-09-07  
**Banco:** PostgreSQL 15+  

---

## 1. Princípios do modelo

1. **Imutabilidade histórica:** candles, indicadores e contextos nunca são atualizados. Correções geram novos registros com novos IDs.
2. **Rastreabilidade:** toda decisão carrega `strategy_id`, `strategy_version`, `config_hash`, `correlation_id`.
3. **Deduplicação:** candles são únicos por `(symbol, timeframe, timestamp)`.
4. **Precisão financeira:** preços e quantidades usam `NUMERIC` (equivalente a `Decimal`), nunca `DOUBLE`/`FLOAT`.
5. **Timezones:** todos os timestamps são armazenados em UTC (`TIMESTAMPTZ`).

---

## 2. Diagrama entidade-relacionamento

```text
┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
│    assets       │     │    candles      │     │  indicators     │
└────────┬────────┘     └────────┬────────┘     └────────┬────────┘
         │                       │                       │
         │                       │                       │
         ▼                       ▼                       ▼
┌─────────────────────────────────────────────────────────────────┐
│                      market_contexts                              │
└────────────────────────────┬────────────────────────────────────┘
                             │
                             ▼
┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
│     signals     │     │     orders      │     │     fills       │
└────────┬────────┘     └────────┬────────┘     └────────┬────────┘
         │                       │                       │
         │                       ▼                       │
         │                 ┌─────────────────┐           │
         │                 │    positions    │           │
         │                 └────────┬────────┘           │
         │                          │                   │
         ▼                          ▼                   ▼
┌─────────────────────────────────────────────────────────────────┐
│                         trades                                    │
└─────────────────────────────────────────────────────────────────┘
```

---

## 3. Tabelas

### 3.1 `assets`

Registra os ativos monitorados ou negociados.

```sql
CREATE TABLE assets (
    id              SERIAL PRIMARY KEY,
    symbol          TEXT NOT NULL UNIQUE,
    name            TEXT,
    asset_type      TEXT NOT NULL CHECK (asset_type IN ('stock', 'etf', 'crypto', 'forex', 'future', 'option')),
    exchange        TEXT,
    currency        TEXT NOT NULL DEFAULT 'USD',
    tick_size       NUMERIC NOT NULL,       -- menor incremento de preço
    lot_size        NUMERIC NOT NULL DEFAULT 1,
    sector          TEXT,
    is_active       BOOLEAN NOT NULL DEFAULT true,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
```

**Justificativa:** `tick_size` é essencial para arredondar preços de ordem corretamente.

---

### 3.2 `candles`

Candles históricos e em tempo real, com deduplicação por symbol/timeframe/timestamp.

```sql
CREATE TABLE candles (
    id              BIGSERIAL PRIMARY KEY,
    asset_id        INTEGER NOT NULL REFERENCES assets(id),
    timeframe       TEXT NOT NULL,          -- '15m', '1h', '1d'
    timestamp       TIMESTAMPTZ NOT NULL,
    open            NUMERIC NOT NULL,
    high            NUMERIC NOT NULL,
    low             NUMERIC NOT NULL,
    close           NUMERIC NOT NULL,
    volume          NUMERIC NOT NULL DEFAULT 0,
    vwap            NUMERIC,                -- opcional
    source          TEXT NOT NULL,          -- 'ibkr', 'polygon', 'manual'
    is_complete     BOOLEAN NOT NULL DEFAULT true,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT candles_unique UNIQUE (asset_id, timeframe, timestamp),
    CONSTRAINT candles_high_low_check CHECK (high >= low),
    CONSTRAINT candles_ohlc_check CHECK (
        high >= open AND high >= close AND
        low <= open AND low <= close
    )
);

CREATE INDEX idx_candles_asset_timeframe_timestamp
    ON candles (asset_id, timeframe, timestamp DESC);
```

**Justificativa:** A constraint `UNIQUE` garante deduplicação. Índice composto acelera buscas por range.

---

### 3.3 `indicators`

Indicadores calculados por candle. Permite auditoria e reprodução.

```sql
CREATE TABLE indicators (
    id              BIGSERIAL PRIMARY KEY,
    candle_id       BIGINT NOT NULL REFERENCES candles(id) ON DELETE CASCADE,
    indicator_name  TEXT NOT NULL,          -- 'ema_20', 'atr_14', 'volume_relative'
    value           NUMERIC NOT NULL,
    parameters      JSONB NOT NULL,         -- {period: 20, source: 'close'}
    calculated_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT indicators_unique UNIQUE (candle_id, indicator_name, parameters)
);

CREATE INDEX idx_indicators_candle ON indicators (candle_id);
```

**Justificativa:** `parameters` como JSONB permite flexibilidade sem alterar schema. Chave composta evita duplicatas.

---

### 3.4 `market_contexts`

Classificação de contexto de mercado a cada candle fechado.

```sql
CREATE TABLE market_contexts (
    id                  BIGSERIAL PRIMARY KEY,
    asset_id            INTEGER NOT NULL REFERENCES assets(id),
    timeframe           TEXT NOT NULL,
    timestamp           TIMESTAMPTZ NOT NULL,
    candle_id           BIGINT REFERENCES candles(id),

    trend_state         TEXT NOT NULL CHECK (trend_state IN ('uptrend', 'downtrend', 'neutral', 'unknown')),
    volatility_regime   TEXT NOT NULL CHECK (volatility_regime IN ('high', 'normal', 'low', 'unknown')),
    market_phase        TEXT NOT NULL CHECK (market_phase IN ('pre_market', 'regular', 'after_hours', 'unknown')),

    -- valores brutos que originaram a classificação
    ema_20              NUMERIC,
    ema_50              NUMERIC,
    sma_200             NUMERIC,
    atr_14              NUMERIC,
    atr_percent_14      NUMERIC,
    volume_relative     NUMERIC,
    hh_hl_count         INTEGER,            -- contagem de higher highs/higher lows
    lh_ll_count         INTEGER,            -- contagem de lower highs/lower lows
    range_percent       NUMERIC,            -- (high - low) / close do dia
    is_tradeable        BOOLEAN NOT NULL DEFAULT false,

    raw_values          JSONB NOT NULL DEFAULT '{}',
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT market_contexts_unique UNIQUE (asset_id, timeframe, timestamp)
);

CREATE INDEX idx_market_contexts_asset_timeframe_timestamp
    ON market_contexts (asset_id, timeframe, timestamp DESC);
```

**Justificativa:** Armazenar valores brutos permite explicar por que o mercado foi classificado de determinada forma.

---

### 3.5 `signals`

Sinais gerados pela estratégia (entradas aceitas ou rejeitadas).

```sql
CREATE TYPE signal_direction AS ENUM ('long', 'short');
CREATE TYPE signal_status AS ENUM ('accepted', 'rejected', 'pending', 'expired');

CREATE TABLE signals (
    id                  BIGSERIAL PRIMARY KEY,
    asset_id            INTEGER NOT NULL REFERENCES assets(id),
    strategy_id         TEXT NOT NULL,      -- ex: 'pullback-trend-v1'
    strategy_version    TEXT NOT NULL,      -- ex: '1.0.0'
    config_hash         TEXT NOT NULL,      -- sha256 da config usada

    timeframe           TEXT NOT NULL,
    timestamp           TIMESTAMPTZ NOT NULL,
    direction           signal_direction,
    status              signal_status NOT NULL,

    -- preços propostos
    entry_price         NUMERIC,
    stop_price          NUMERIC,
    target_price        NUMERIC,
    risk_reward_ratio   NUMERIC,

    -- risco estimado
    risk_amount         NUMERIC,            -- valor monetário do risco
    risk_percent        NUMERIC,            -- percentual do capital
    position_size       NUMERIC,            -- quantidade de ações

    -- motivos
    entry_reason        TEXT,
    rejection_reason    TEXT,
    rejection_details   JSONB,

    -- contexto no momento do sinal
    context_id          BIGINT REFERENCES market_contexts(id),
    market_snapshot     JSONB NOT NULL,     -- snapshot dos últimos N candles e indicadores

    correlation_id      UUID NOT NULL,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_signals_asset_strategy_timestamp
    ON signals (asset_id, strategy_id, timestamp DESC);
CREATE INDEX idx_signals_status ON signals (status);
```

**Justificativa:** `market_snapshot` é essencial para auditoria. Mesmo que indicadores mudem no futuro, o sinal carrega o contexto exato do momento.

---

### 3.6 `orders`

Ordens enviadas ao broker.

```sql
CREATE TYPE order_type AS ENUM ('market', 'limit', 'stop', 'stop_limit', 'bracket');
CREATE TYPE order_side AS ENUM ('buy', 'sell');
CREATE TYPE order_status AS ENUM (
    'pending', 'submitted', 'accepted', 'partially_filled',
    'filled', 'cancelled', 'rejected', 'expired'
);
CREATE TYPE time_in_force AS ENUM ('day', 'gtc', 'ioc', 'fok');

CREATE TABLE orders (
    id                  BIGSERIAL PRIMARY KEY,
    signal_id           BIGINT REFERENCES signals(id),
    asset_id            INTEGER NOT NULL REFERENCES assets(id),

    broker_order_id     TEXT,               -- ID retornado pelo broker
    parent_order_id     BIGINT REFERENCES orders(id), -- para bracket/OCO

    side                order_side NOT NULL,
    order_type          order_type NOT NULL,
    status              order_status NOT NULL DEFAULT 'pending',
    time_in_force       time_in_force NOT NULL DEFAULT 'day',

    quantity            NUMERIC NOT NULL,
    filled_quantity     NUMERIC NOT NULL DEFAULT 0,
    remaining_quantity  NUMERIC GENERATED ALWAYS AS (quantity - filled_quantity) STORED,

    price               NUMERIC,            -- para limit
    stop_price          NUMERIC,            -- para stop
    avg_fill_price      NUMERIC,

    broker              TEXT NOT NULL,      -- 'ibkr', 'simulated'
    error_message       TEXT,
    metadata            JSONB NOT NULL DEFAULT '{}',

    correlation_id      UUID NOT NULL,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    submitted_at        TIMESTAMPTZ,
    filled_at           TIMESTAMPTZ,
    cancelled_at        TIMESTAMPTZ
);

CREATE INDEX idx_orders_signal ON orders (signal_id);
CREATE INDEX idx_orders_status ON orders (status);
CREATE INDEX idx_orders_broker_order_id ON orders (broker, broker_order_id);
```

**Justificativa:** `parent_order_id` modela bracket orders e OCOs. `broker_order_id` permite reconciliação com o broker.

---

### 3.7 `fills`

Execuções parciais ou totais de uma ordem.

```sql
CREATE TABLE fills (
    id              BIGSERIAL PRIMARY KEY,
    order_id        BIGINT NOT NULL REFERENCES orders(id),
    asset_id        INTEGER NOT NULL REFERENCES assets(id),

    side            TEXT NOT NULL DEFAULT 'buy',   -- migração 0002
    fill_price      NUMERIC NOT NULL,
    quantity        NUMERIC NOT NULL,
    commission      NUMERIC NOT NULL DEFAULT 0,
    fees            NUMERIC NOT NULL DEFAULT 0,

    broker_fill_id  TEXT,
    timestamp       TIMESTAMPTZ NOT NULL,

    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_fills_order ON fills (order_id);
-- migração 0002: dedupe/idempotência do replay de execuções do dia
CREATE UNIQUE INDEX idx_fills_broker_fill_id ON fills (broker_fill_id) WHERE broker_fill_id IS NOT NULL;
```

**Justificativa:** Separa fills de ordens para suportar execuções parciais. `side` torna o fill autodescritivo (fills de saída do bracket pertencem a ordens filhas que não existem no banco). O índice único parcial em `broker_fill_id` garante idempotência: o replay diário de execuções da IBKR nunca grava o mesmo fill duas vezes.

---

### 3.7.1 `backtest_runs` (migrações 0003 e 0005)

Histórico de execuções de backtest / walk-forward, para comparação entre runs e com o live (`trader-cli analyze`).

```sql
CREATE TABLE backtest_runs (
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
    -- default "walkforward-oos-<N>w"; com o ADR-019 deixou de ser decorativo:
    -- `--label` é OBRIGATÓRIO quando o run é experimental (`--set` /
    -- `--strategy-config`), senão a ablação entra no histórico indistinguível
    -- do run de produção e vira baseline do gate B por ordem de chegada.
    label            TEXT,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- migração 0005 (ADR-019): índice da busca de baseline do gate B.
CREATE INDEX idx_backtest_runs_baseline
    ON backtest_runs (strategy_id, asset_id, config_hash, created_at DESC);
```

**O que vive no `metrics` (jsonb).** Além das métricas do `BacktestMetrics`,
os runs de `walkforward` a partir do ADR-019 gravam o que DEFINE o run:

| Chave | Para quê |
|---|---|
| `slippage_bps` | custo do run (texto, aceita fração: `"2.5"`) |
| `session_flatten` | hora **ET** (America/New_York) esperada da última barra do RTH (`[session].last_bar`, `"15:45"` no default) — ou `null` (`--no-flatten`); registra se a régua de fim de pregão do ADR-018 estava ligada |
| `experimental` | `true` quando houve `--set`/`--strategy-config` |
| `overrides` | mapa chave→valor dos `--set` aplicados |
| `windows`, `holdout_from`, `holdout_metrics` | desenho da validação |

`session_flatten` grava o **horário, não o gatilho**. No backtest a última
barra do pregão é detectada por **mudança de data ET**
(`is_last_bar_of_session`, `crates/trader-backtest/src/engine.rs`), porque nos
pregões de meio expediente o dia acaba às 12h45; o horário da chave é só a
checagem de sanidade, e quando a barra fecha depois dele o motor emite um
`warn!` de "série não parece RTH-only". No live o gatilho é outro de
propósito: a janela `[session] flatten_start`..`flatten_end`, checada por tick.
E a barra em questão não é "a última barra de entrada" — o sinal dela é
gerado, logado e contado normalmente; o que morre é a ordem
(`cancel_pending_entry`), como no live, onde a perna do bracket vai com TIF
Day e expira no sino.

Sem essas chaves, dois runs com custo ou régua diferentes são
indistinguíveis. Não as têm os runs anteriores ao ADR-019 nem — hoje, depois
dele — os gravados pelo comando `backtest`, que persiste o `metrics` puro e
`label: None` (`crates/trader-cli/src/commands/backtest.rs`). Em qualquer dos
dois casos o `analyze` avisa que o run não é comparável com os novos.

**As chaves do próprio `BacktestMetrics`** (`crates/trader-backtest/src/metrics.rs`).
O ADR-018/019 acrescentou três sub-objetos — `by_exit_reason`, `by_direction`
e `by_entry_hour_et`, todos mapas de `GroupMetrics` (tipo público novo) — e as
métricas em R e de concentração: `profit_factor_r`, `t_stat_avg_r`,
`corr_risk_result`, `trading_days`, `top_day_share`, `top5_day_share`,
`top2_month_share`, `months_total`, `months_positive` e `cost_total`. Duas
convenções que não se leem do nome:

- as chaves de `by_exit_reason` são o texto canônico de `ExitReason::as_str()`
  e incluem `end_of_day` por `Trade::effective_exit_reason()`, de modo que o
  flatten gravado antes da migração 0004 (como `manual` + journal) cai na
  linha certa;
- as de `by_entry_hour_et` são horas de **Nova York** com dois dígitos
  (`"09"`, `"10"`, `"15"`), não UTC — é a exceção ao princípio 5 desta página,
  e é deliberada: um bucket em UTC junta duas horas de pregão diferentes na
  virada do DST.

Todas carregam `#[serde(default)]` — os três mapas e os dez campos em R e de
concentração: runs anteriores ao ADR-019 simplesmente não as trazem, e
desserializam com zero/mapa vazio em vez de falhar. Não é detalhe de estilo.
O jsonb `metrics` guarda esta struct serializada, e há centenas de runs
gravados antes do ADR sem nenhum desses campos: sem o atributo, o `analyze`
abortaria com "métricas do run N inválidas" ao cair num deles e o histórico do
gate B ficaria ilegível. O teste `metrics_de_run_antigo_ainda_desserializa`
(`crates/trader-backtest/src/metrics.rs`) trava a regressão com o jsonb de um
run real.

**Não há índice ÚNICO nem dedupe**, apesar de o plano os pedir. A auditoria de
07/09/2026 mediu que, no maior grupo "duplicado" por
`(strategy_id, asset_id, config_hash, período, label)` — 24 linhas —, existem
**sete valores distintos de `final_equity`**: não são cópias. O `config_hash`
cobre só o TOML da estratégia, não a versão do motor, o slippage nem a régua
de fim de sessão. Deduplicar por essa chave apagaria resultados diferentes
entre si. Fica para quando a identidade do run for recuperável (as chaves
acima são o primeiro passo) e o dono autorizar apagar linhas.

---

### 3.8 `positions`

Posições abertas no momento.

```sql
CREATE TYPE position_direction AS ENUM ('long', 'short');

CREATE TABLE positions (
    id                  BIGSERIAL PRIMARY KEY,
    asset_id            INTEGER NOT NULL REFERENCES assets(id),
    signal_id           BIGINT NOT NULL REFERENCES signals(id),

    direction           position_direction NOT NULL,
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

CREATE INDEX idx_positions_asset_status ON positions (asset_id, status);
```

**Justificativa:** Posições são derivadas de fills, mas mantê-las separadamente acelera consultas de estado atual.

---

### 3.9 `trades`

Trades fechados, resultado final de uma operação.

```sql
CREATE TABLE trades (
    id                  BIGSERIAL PRIMARY KEY,
    asset_id            INTEGER NOT NULL REFERENCES assets(id),
    signal_id           BIGINT NOT NULL REFERENCES signals(id),
    position_id         BIGINT REFERENCES positions(id),

    direction           position_direction NOT NULL,
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

    -- métricas em R
    risk_amount         NUMERIC NOT NULL,
    result_in_r         NUMERIC NOT NULL,   -- ex: +2.0, -1.0

    -- 'end_of_day' entrou na migração 0004 (ADR-018): o encerramento de fim de
    -- pregão passou a ter motivo próprio. Antes ia como 'manual' com
    -- journal->>'forced_exit' = 'session_flatten', e o gate B contava flatten
    -- como saída discricionária. Os trades gravados ANTES do ADR continuam
    -- 'manual' no banco — a reclassificação é opcional e depende de decisão do
    -- dono (`sql/maintenance/0004-reclassificar-flatten.sql`); o código os
    -- reconhece pelo journal via `Trade::effective_exit_reason()`.
    exit_reason         TEXT NOT NULL CHECK (exit_reason IN ('target', 'stop', 'time', 'manual', 'risk_manager', 'end_of_day')),
    strategy_id         TEXT NOT NULL,
    strategy_version    TEXT NOT NULL,
    config_hash         TEXT NOT NULL,

    journal             JSONB NOT NULL DEFAULT '{}',
    correlation_id      UUID NOT NULL,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_trades_asset_strategy ON trades (asset_id, strategy_id, exit_time DESC);
CREATE INDEX idx_trades_exit_time ON trades (exit_time DESC);
```

**Justificativa:** `trades` é a tabela analítica principal. `journal` armazena o diário automático em formato estruturado.

---

### 3.10 `strategy_configs`

Versionamento de configurações de estratégia.

```sql
CREATE TABLE strategy_configs (
    id              SERIAL PRIMARY KEY,
    strategy_id     TEXT NOT NULL,
    version         TEXT NOT NULL,
    config_hash     TEXT NOT NULL UNIQUE,
    config          JSONB NOT NULL,
    source          TEXT,                   -- livro/capítulo
    description     TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT strategy_configs_unique UNIQUE (strategy_id, version)
);
```

**Justificativa:** Permite rastrear exatamente qual configuração produziu cada sinal/trade.

---

### 3.11 `risk_limits`

Limites de risco por conta/estratégia.

```sql
CREATE TABLE risk_limits (
    id                      SERIAL PRIMARY KEY,
    name                    TEXT NOT NULL UNIQUE,
    risk_per_trade_pct      NUMERIC NOT NULL,
    max_daily_loss_pct      NUMERIC NOT NULL,
    max_trades_per_day      INTEGER NOT NULL,
    max_consecutive_losses  INTEGER NOT NULL,
    max_spread_pct          NUMERIC NOT NULL,
    max_atr_pct             NUMERIC NOT NULL,
    min_risk_reward         NUMERIC NOT NULL,
    trading_start_time      TIME NOT NULL,    -- horário em UTC ou timezone configurado
    trading_end_time        TIME NOT NULL,
    is_active               BOOLEAN NOT NULL DEFAULT true,
    created_at              TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at              TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
```

**Justificativa:** Centraliza regras de segurança financeira. Pode haver múltiplos perfis no futuro.

---

### 3.12 `account_snapshots`

Snapshots de saldo e equity ao longo do tempo.

```sql
CREATE TABLE account_snapshots (
    id              BIGSERIAL PRIMARY KEY,
    broker          TEXT NOT NULL,
    account_id      TEXT,
    timestamp       TIMESTAMPTZ NOT NULL,
    cash            NUMERIC NOT NULL,
    equity          NUMERIC NOT NULL,
    buying_power    NUMERIC NOT NULL,
    daily_pnl       NUMERIC NOT NULL DEFAULT 0,
    daily_return_pct NUMERIC,
    metadata        JSONB NOT NULL DEFAULT '{}',

    CONSTRAINT account_snapshots_unique UNIQUE (broker, account_id, timestamp)
);

CREATE INDEX idx_account_snapshots_time ON account_snapshots (timestamp DESC);
```

**Justificativa:** Permite reconstruir equity curve e verificar limites diários.

---

### 3.13 `system_events`

Log de eventos operacionais do robô.

```sql
CREATE TYPE event_level AS ENUM ('debug', 'info', 'warning', 'error', 'critical');

CREATE TABLE system_events (
    id              BIGSERIAL PRIMARY KEY,
    timestamp       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    level           event_level NOT NULL,
    component       TEXT NOT NULL,          -- 'broker', 'data_provider', 'risk_manager'
    event_type      TEXT NOT NULL,          -- 'reconnect', 'order_rejected', 'daily_limit_hit'
    message         TEXT NOT NULL,
    payload         JSONB,
    correlation_id  UUID
);

CREATE INDEX idx_system_events_timestamp ON system_events (timestamp DESC);
CREATE INDEX idx_system_events_level ON system_events (level);
CREATE INDEX idx_system_events_correlation ON system_events (correlation_id);
```

**Justificativa:** Eventos estruturados complementam os logs de texto e facilitam alertas.

---

### 3.14 `ingestions`

Registro de ingestões de dados históricos.

```sql
CREATE TABLE ingestions (
    id              BIGSERIAL PRIMARY KEY,
    asset_id        INTEGER NOT NULL REFERENCES assets(id),
    timeframe       TEXT NOT NULL,
    source          TEXT NOT NULL,
    start_time      TIMESTAMPTZ NOT NULL,
    end_time        TIMESTAMPTZ NOT NULL,
    candles_inserted INTEGER NOT NULL DEFAULT 0,
    candles_updated  INTEGER NOT NULL DEFAULT 0,
    gaps_detected    INTEGER NOT NULL DEFAULT 0,
    status          TEXT NOT NULL CHECK (status IN ('running', 'completed', 'failed')),
    error_message   TEXT,
    started_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    finished_at     TIMESTAMPTZ
);
```

**Justificativa:** Permite auditar a qualidade e completude dos dados históricos.

---

## 4. Relacionamentos resumidos

| Tabela | Dependência | Tipo |
|--------|-------------|------|
| `candles` | `assets` | N:1 |
| `indicators` | `candles` | N:1 |
| `market_contexts` | `assets`, `candles` | N:1, N:1 |
| `signals` | `assets`, `market_contexts`, `strategy_configs` | N:1, N:1, implícito |
| `orders` | `signals`, `assets`, `orders` (parent) | N:1, N:1, N:1 |
| `fills` | `orders`, `assets` | N:1, N:1 |
| `backtest_runs` | `assets` | N:1 |
| `positions` | `assets`, `signals` | N:1, N:1 |
| `trades` | `assets`, `signals`, `positions` | N:1, N:1, N:1 |
| `account_snapshots` | — | — |
| `system_events` | — | — |

---

## 5. Convenções de nomenclatura

- Tabelas no plural, snake_case: `market_contexts`, `account_snapshots`.
- Colunas em snake_case.
- Enums PostgreSQL refletem enums Rust (via conversores sqlx).
- JSONB usado para snapshots e metadados extensíveis.
- Campos monetários sempre `NUMERIC` com precisão suficiente (ex.: `NUMERIC(19, 8)` opcional).

---

## 6. Estratégia de migrações

Aplicadas até 07/09/2026:

| # | O quê | ADR |
|---|---|---|
| 0001 | schema inicial | — |
| 0002 | `fills.side` + dedupe/idempotência do replay de execuções | — |
| 0003 | `backtest_runs` | — |
| 0004 | `end_of_day` no CHECK de `trades.exit_reason` | ADR-018 |
| 0005 | índice `idx_backtest_runs_baseline` | ADR-019 |

As migrações vivem em `crates/trader-infra/src/db/migrations/` e são
**embutidas no binário** por `sqlx::migrate!` (`db/mod.rs`), então um arquivo
`.sql` novo exige recompilar `trader-infra`. Elas rodam no boot do `paper`;
`backtest`, `walkforward`, `analyze` e `status` **não** as executam.

O `DROP CONSTRAINT` da 0004 é por **busca** (`pg_constraint` + `ILIKE`), não
por nome: o CHECK da 0001 é anônimo, e derrubar pelo nome implícito falharia
em silêncio num ambiente que divergisse — o `ADD` criaria o novo, o antigo
continuaria valendo e o primeiro INSERT de `end_of_day` quebraria só em
produção (SQLSTATE 23514).

- Ferramenta: `sqlx migrate`, embutido via `sqlx::migrate!` (`refinery` foi
  descartado e não existe no repositório).
- Migrations versionadas em `crates/trader-infra/src/db/migrations/`, como
  acima — **não** há diretório `migrations/` na raiz. Criar a 0006 lá escreve
  um arquivo que nunca é aplicado, e como as migrações são embutidas no
  binário o erro só apareceria no boot do `paper`.
- Ambiente de desenvolvimento com `docker-compose.yml` subindo PostgreSQL.
- Testes de integração usam banco de teste isolado (`sqlx::test`).
- Nunca alterar migrations já aplicadas em produção. Correções via nova migration.

---

## 7. Particionamento e retenção (futuro)

- `candles` e `indicators` podem ser particionados por `asset_id` e/ou `timestamp` quando o volume crescer.
- `system_events` pode ter retenção agressiva (ex.: 90 dias) com arquivamento.
- `trades` e `signals` devem ser retidos indefinidamente por requisitos de auditoria.

---

## 8. Referências

- `docs/ARCHITECTURE.md`
- `docs/TECHNICAL-ROADMAP.md`
- `docs/PRD.md`
