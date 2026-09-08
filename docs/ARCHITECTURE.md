# Arquitetura de Software — HumanStyle Trader Bot

**Versão:** 1.0  
**Status:** Aprovado para implementação  
**Última atualização:** 2026-09-07  
**Autor:** Software Architect  

> **Estado do que está descrito aqui:** o ADR-018 (flatten de fim de pregão), o
> hotfix v1.0.1 do veto de meio-dia da `range-extreme-fade-v1` e o ADR-019
> (harness de validação) estão implementados na `main`, mas **ainda sem push** —
> ou seja, ainda não estão em produção. O ADR-020 e os critérios de gate do
> ADR-019 §7 são proposta e não estão implementados; onde este documento os
> cita, diz isso explicitamente.

---

## 1. Propósito deste documento

Este documento define a arquitetura técnica do sistema de trading automatizado *HumanStyle Trader Bot*. Ele estabelece:

- Os princípios arquiteturais que regem todas as decisões técnicas.
- A divisão de responsabilidades entre camadas e crates.
- Os contratos (traits/interfaces) que isolam o domínio de provedores externos.
- Os fluxos de dados em tempo real, backtest e paper trading.
- Os padrões de erro, logging, persistência e observabilidade.

Este documento deve ser consultado antes de qualquer alteração estrutural no código.

---

## 2. Visão geral

O sistema é um robô trader **contextual**, não um sistema de alta frequência. Ele opera como um trader humano disciplinado:

1. Coleta dados de mercado.
2. Classifica o contexto de mercado (tendência, range, volatilidade).
3. Detecta setups objetivos baseados em Price Action.
4. Valida risco/retorno e regras de segurança.
5. Executa ordens em ambiente de paper trading.
6. Registra tudo para auditoria, backtest e evolução.

A arquitetura prioriza:

- **Corretude:** regras testáveis, dados versionados, decisões auditáveis.
- **Segurança financeira:** stop obrigatório, limites diários, bloqueios automáticos.
- **Portabilidade:** estratégia desacoplada de corretora e provedor de dados.
- **Robustez:** reconexão, deduplicação, reconciliação, circuit breakers.
- **Previsibilidade:** Rust como linguagem principal para performance determinística.

---

## 3. Princípios arquiteturais

| Princípio | Descrição |
|-----------|-----------|
| **Domínio puro** | O core de estratégia não conhece Interactive Brokers, PostgreSQL, HTTP ou async. Ele recebe structs de domínio e retorna decisões. |
| **Ports & Adapters** | O domínio define traits (ports). Toda integração externa vive em adapters. |
| **Imutabilidade de candles** | Candles e indicadores calculados são imutáveis após persistidos. Correções geram novos registros, nunca updates destrutivos. |
| **Event sourcing para decisões** | Toda decisão (sinal, rejeição, ordem, fill) é um evento persistido com contexto completo. |
| **Fail-safe financeiro** | Em caso de ambiguidade, o sistema prefere não operar. Rejeição é segura. |
| **Configuração como código** | Parâmetros de estratégia e risco são versionados, rastreáveis e reproduzíveis. |
| **Testabilidade compulsiva** | Toda regra deve ter teste unitário com candles sintéticos antes de ir para backtest. |

---

## 4. Diagrama de alto nível

```text
┌─────────────────────────────────────────────────────────────────────────────┐
│                              Provedores Externos                             │
│  ┌─────────────────┐    ┌─────────────────┐    ┌─────────────────────────┐  │
│  │ Interactive     │    │ PostgreSQL      │    │ Futuro: outros brokers  │  │
│  │ Brokers (IBKR)  │    │ (memória do bot)│    │ ou data providers       │  │
│  └────────┬────────┘    └────────┬────────┘    └─────────────────────────┘  │
└───────────┼──────────────────────┼──────────────────────────────────────────┘
            │                      │
┌───────────▼──────────────────────▼──────────────────────────────────────────┐
│                              Camada de Adapters                              │
│  ┌────────────────────────┐    ┌─────────────────────────────────────────┐  │
│  │ MarketDataProvider     │    │ BrokerAdapter                           │  │
│  │ (IbkrMarketDataProvider)│    │ (IbkrBrokerAdapter)                     │  │
│  └────────────────────────┘    └─────────────────────────────────────────┘  │
└───────────────────────────────┬─────────────────────────────────────────────┘
                                │
┌───────────────────────────────▼─────────────────────────────────────────────┐
│                           Camada de Aplicação / Core                         │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  ┌──────────────────┐ │
│  │ MarketContext│  │ SetupDetector│  │ RiskManager  │  │ ExecutionEngine  │ │
│  │ Analyzer     │  │ (Strategy)   │  │              │  │                  │ │
│  └──────────────┘  └──────────────┘  └──────────────┘  └──────────────────┘ │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  ┌──────────────────┐ │
│  │ Indicators   │  │ Portfolio    │  │ Journal      │  │ Scheduler        │ │
│  │ (SMA/EMA/ATR)│  │ Manager*     │  │ Generator*   │  │ (futuro)         │ │
│  └──────────────┘  └──────────────┘  └──────────────┘  └──────────────────┐ │
└───────────────────────────────────────────────────────────────────────────┘
                                │
┌───────────────────────────────▼─────────────────────────────────────────────┐
│                              Camada de Domínio                               │
│  Candle, Quote, Signal, Order, Fill, Trade, Position, AccountSummary, ...    │
└─────────────────────────────────────────────────────────────────────────────┘
                                │
┌───────────────────────────────▼─────────────────────────────────────────────┐
│                            Camada de Infraestrutura                          │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  ┌──────────────────┐ │
│  │ Repositories │  │ Event Store* │  │ Config       │  │ Logging/Tracing  │ │
│  │ (sqlx)       │  │              │  │ Loader       │  │                  │ │
│  └──────────────┘  └──────────────┘  └──────────────┘  └──────────────────┐ │
└───────────────────────────────────────────────────────────────────────────┘
                                │
┌───────────────────────────────▼─────────────────────────────────────────────┐
│                              Entrypoints                                     │
│  ┌─────────────────────────────────────────────────────────────────────────┐ │
│  │ trader-cli (entrypoint principal: comandos, worker de paper trading)     │ │
│  └─────────────────────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────────────────┘
```

> **Nota sobre o diagrama:** Componentes marcados com `*` (`Portfolio Manager`, `Journal Generator`, `Event Store` e `Scheduler`) ainda não existem como módulos independentes. No estado atual, suas responsabilidades estão distribuídas entre `RiskState`, `ExecutionEngine`, `SqlxTradeRepository` e o campo `journal` da tabela `trades`. Serão extraídos para módulos próprios conforme o sistema amadurecer.

---

## 5. Estrutura de crates Rust

O projeto utiliza um workspace Cargo com crates internas bem definidas:

```text
botdaytrade/
├── Cargo.toml                    # workspace root
├── crates/
│   ├── trader-domain/            # Entidades, enums, traits, erros de domínio
│   ├── trader-core/              # Lógica de estratégia, contexto, risco, execução
│   ├── trader-adapters/          # Implementações de broker e market data
│   ├── trader-infra/             # DB, config, logging, repositories
│   ├── trader-backtest/          # Engine de backtest
│   └── trader-cli/               # Binário CLI principal (entrypoint)
└── docs/
```

### 5.1 `trader-domain`

**Responsabilidade:** Definir o vocabulário comum do sistema. Não depende de nenhum crate externo além de `chrono`, `rust_decimal`, `serde`, `thiserror`.

**Conteúdo típico:**

- `Candle`, `Quote`, `Tick`
- `Signal`, `Direction`, `SignalStatus`
- `Order`, `OrderType`, `OrderStatus`, `Fill`
- `Trade`, `Position`, `AccountSummary`
- `MarketContext`, `VolatilityRegime`, `TrendState`
- `RejectionReason`, `RiskCheckResult`
- Traits: `MarketDataProvider`, `Broker`, `ExecutionListener`
- Erros de domínio: `DomainError`, `ValidationError`

### 5.2 `trader-core`

**Responsabilidade:** Implementar a inteligência do robô. Depende apenas de `trader-domain` e bibliotecas de cálculo (estatística, indicadores).

**Módulos principais:**

- `context::MarketContextAnalyzer` — classificação de mercado.
- `risk::RiskManager` — validação de risco e sizing.
- `execution::ExecutionEngine` — orquestração de ordens, stops e alvos.
- `session` — janelas e datas de pregão em horário de Nova York (`et_time`,
  `et_date`, `parse_et_time`, `within_trading_window`). É a **única**
  implementação dessa regra: as janelas de negociação das estratégias, o veto de
  meio-dia da `range-extreme-fade-v1` e os dois lados do flatten do ADR-018
  chamam estas funções. Os **gatilhos** do flatten, porém, são diferentes de
  propósito: no live é o relógio — `in_flatten_window` compara `et_time(agora)`
  com a janela `[session] flatten_start`–`flatten_end`, e roda por tick, porque
  depois das 16h ET não chega mais candle fechado para disparar nada; no
  backtest é a mudança de `et_date` entre a barra atual e a seguinte
  (`is_last_bar_of_session`), porque ali não há relógio, há série. Comparação
  com UTC fixo desliza uma hora na virada do DST — foi exatamente o bug do
  hotfix v1.0.1.
- `strategies/` — implementações concretas de estratégias.
  - `pullback_trend_v1/`
    - `mod.rs`
    - `context.rs`
    - `setup.rs`
    - `entry.rs`
    - `config.rs`
- `indicators/` — EMA, ATR, volume relativo, etc.

> **Nota:** Um `PortfolioManager` dedicado ainda não foi criado. O rastreamento de P&L diário e exposição está atualmente no `RiskState`.

**Contrato mínimo de estratégia:**

```rust
pub trait Strategy {
    fn id(&self) -> StrategyId;
    fn name(&self) -> &'static str;
    fn source(&self) -> &'static str;
    fn version(&self) -> &'static str;
    fn analyze(&self, ctx: &MarketContext, state: &StrategyState) -> SignalResult;
}
```

### 5.3 `trader-adapters`

**Responsabilidade:** Implementar os ports definidos em `trader-domain` para provedores externos.

**Inicialmente:**

- `ibkr::IbkrMarketDataProvider`
- `ibkr::IbkrBrokerAdapter`
- `simulated::SimulatedBroker` e `simulated::SimulatedMarketDataProvider` — é
  aqui que mora o simulador de execução usado por testes e pelo backtest, não em
  `trader-backtest`. Expõe `pending_entry_order_id`, que o flatten do ADR-018 usa
  para cancelar a entrada ainda não preenchida pelo mesmo `Broker::cancel_order`
  do live, em vez de sumir com a ordem sem rastro.

**Futuramente:**

- `alpaca::AlpacaBrokerAdapter`
- `polygon::PolygonMarketDataProvider`

### 5.4 `trader-infra`

**Responsabilidade:** Tudo que conecta o sistema ao mundo operacional.

**Módulos:**

- `db` — conexão PostgreSQL, migrations sqlx.
- `repositories` — implementações sqlx de `CandleRepository`, `SignalRepository`, `OrderRepository`, `TradeRepository`, `FillRepository`, `MarketContextRepository`, `AssetRepository`, `IngestionRepository`, `SystemEventRepository` e `BacktestRunRepository`.
- `config` — carregamento de configuração (arquivos + env vars). Expõe `AppConfig` e, desde o ADR-018, `SessionSettings` (bloco `[session]`: `flatten_start` = 15:55:00, `flatten_end` = 16:10:00, `last_bar` = 15:45:00, todos em **horário de Nova York** e parseados por `trader_core::session::parse_et_time`). É a fonte única do fim de pregão: o worker de paper trading e o backtest leem do mesmo lugar. Antes disso o live tinha duas constantes em `paper.rs` e o backtest não tinha fim de sessão nenhum.
- `logging` — inicialização do `tracing`.
- `clock` — abstração de tempo para testes determinísticos.

> **`BacktestRunRepository` é peça de comparação, não só de gravação (ADR-019 §3).**
> Além de persistir runs, ele expõe `latest_for(strategy_id, symbol, config_hash)`,
> que é como o `analyze` escolhe o baseline — o `latest_by_strategy` antigo pegava
> o run mais recente em *qualquer* símbolo e *qualquer* config, então a primeira
> ablação rodada virava baseline do gate B em silêncio. Runs com override entram
> marcados como `experimental` no jsonb `metrics` e ficam de fora. A migração 0005
> cria `idx_backtest_runs_baseline` para essa busca.

> **Leitura de trade falha fechado.** `TradeRow` converte para `Trade` por
> `TryFrom`, não `From`: `exit_reason` desconhecido no banco vira erro de
> repositório em vez do antigo `_ => Target` silencioso, que transformava dado
> corrompido em trade vencedor. Qualquer repositório novo deve seguir o mesmo
> contrato. Teste de integração em `crates/trader-infra/tests/trade_repository_test.rs`.

> **Nota:** Um `event_bus` interno ainda não foi implementado. Eventos importantes são persistidos diretamente nas tabelas (`signals`, `orders`, `fills`, `trades`, `system_events`).

### 5.5 `trader-backtest`

**Responsabilidade:** Executar estratégias sobre dados históricos de forma determinística.

**Módulos:**

- `engine` — loop de eventos por candle e flatten de fim de pregão (ADR-018).
  `BacktestConfig.session_flatten_et: Option<(u32, u32)>`, default
  `Some((15, 45))` em horário de NY, fecha a posição na última barra do pregão;
  `None` (flag `--no-flatten`) reproduz os runs anteriores ao ADR.
- `walkforward` — janelas out-of-sample e holdout travado; é o caminho que
  recebeu as flags novas do ADR-019.
- `metrics` — cálculo de métricas de performance (`BacktestMetrics`). Além dos
  agregados, expõe o tipo público `GroupMetrics` e as quebras `by_exit_reason`
  (chaveada por `ExitReason::as_str`, via `Trade::effective_exit_reason` para
  reconhecer o flatten gravado antes da variante existir), `by_direction` e
  `by_entry_hour_et` (hora de **Nova York**, não UTC), mais as métricas de
  dispersão e concentração do ADR-019: `profit_factor_r`, `t_stat_avg_r`,
  `corr_risk_result`, `trading_days`, `top_day_share`, `top5_day_share`,
  `top2_month_share`, `months_total`, `months_positive` e `cost_total`.
- `report` — geração de relatórios comparativos, incluindo a tabela "Saídas por
  motivo", que responde quanto do resultado veio de posição encerrada no sino em
  vez de stop ou alvo.

> **Nota:** o simulador de execução com slippage e comissão (`SimulatedBroker`)
> não vive neste crate — está em `trader-adapters::simulated`, para que backtest
> e testes usem o mesmo adapter que implementa o port `Broker`.

### 5.6 `trader-cli`

**Responsabilidade:** Entrypoint principal do sistema.

**Módulos:**

- `commands/` — um módulo por subcomando.
- `dispatch` — resolve `--strategy` para a struct de estratégia e faz o parse do TOML de parâmetros.
- `strategy_source` — de onde vem a config de um run: o TOML canônico (`config/strategies/<id>.toml`), um TOML alternativo (`--strategy-config`) ou o canônico com sobrescritas (`--set chave=valor`). ADR-019 §1–§2. É por causa dele que `toml` é dependência direta da CLI.
- `risk_config` — parâmetros de risco por instância.
- `alerts` — webhook de alertas críticos.
- `synthetic` — candles sintéticos para smoke test (`--allow-synthetic`).
- `config` — `CliConfig` (config da aplicação + provedor escolhido).

**Comandos:**

```text
trader-cli test-connection --provider ibkr
trader-cli account --provider ibkr
trader-cli ingest --symbol SPY --timeframe 15m --days 30
trader-cli paper --strategy pullback-trend-v1 --symbol SPY --mode simulated|replay|live --timeframe 15m
trader-cli backtest --strategy pullback-trend-v1 --symbol SPY --from 2025-01-01 --to 2025-12-31 --timeframe 15m [--output out/run.json] [--slippage-bps 10] [--allow-synthetic] [--no-flatten]
trader-cli walkforward --strategy pullback-trend-v1 --symbol SPY --windows 6 [--output out/wf.json] [--slippage-bps 2.5] [--label gate-a-adr019] [--holdout-from 2026-01-01] [--strategy-config caminho.toml] [--set chave=valor] [--no-flatten]
trader-cli analyze --strategy pullback-trend-v1 --symbol SPY
trader-cli flatten --symbol SPY --confirm
trader-cli cancel-orders --symbol SPY --confirm
trader-cli status
trader-cli journal --date 2026-07-01
trader-cli debug-candles --symbol IWV --timeframe 15m
```

- `--no-flatten` (ADR-018) desliga o encerramento de fim de pregão. Serve só para
  reproduzir runs antigos e medir o delta: o resultado carrega posição pela noite,
  coisa que o live nunca faz, e não vale como veredito de gate.
- `--strategy-config` e `--set` (ADR-019) permitem variar parâmetro **sem código
  novo** — não é preciso módulo novo em `trader-core` para uma ablação. Os dois
  marcam o run como `experimental`, e run experimental é ignorado pelo baseline do
  `analyze`.
- As travas que **abortam** o comando: `--set` sem `--label`; chave inexistente
  (as nove structs de `StrategyParameters` ganharam `deny_unknown_fields`); `--set`
  que não muda o `config_hash` (override que não alterou nada é engano do operador,
  não experimento); `--set` junto com `--holdout-from`; TOML de `--strategy-config`
  cujo `strategy.id` não bate com `--strategy`; e `--holdout-from` com data inválida.
- Além do veredito do gate vigente (ADR-010), o `walkforward` imprime os critérios
  do ADR-019 §7 rotulados como **proposta que ainda não é o gate vigente**.
- `analyze` compara o live contra o baseline de backtest por (estratégia, ativo,
  `config_hash`) e filtra os trades do live pelo mesmo par `strategy_id` +
  `config_hash`. Sem run compatível ele **avisa** em vez de pegar outro run.

---

## 6. Contratos principais (Ports)

### 6.1 MarketDataProvider

```rust
#[async_trait]
pub trait MarketDataProvider: Send + Sync {
    async fn get_historical_candles(
        &self,
        request: CandleRequest,
    ) -> Result<Vec<Candle>, DataError>;

    async fn subscribe_realtime_bars(
        &self,
        symbol: &str,
        timeframe: TimeFrame,
        tx: mpsc::Sender<Candle>,
    ) -> Result<SubscriptionHandle, DataError>;

    async fn get_quote(&self, symbol: &str) -> Result<Quote, DataError>;

    async fn health_check(&self) -> Result<ProviderHealth, DataError>;
}
```

### 6.2 Broker

```rust
#[async_trait]
pub trait Broker: Send + Sync {
    async fn place_order(&self, order: Order) -> Result<OrderId, BrokerError>;
    async fn cancel_order(&self, id: &OrderId) -> Result<(), BrokerError>;
    async fn get_order_status(&self, id: &OrderId) -> Result<OrderStatus, BrokerError>;
    async fn get_open_orders(&self) -> Result<Vec<Order>, BrokerError>;
    async fn get_position(&self, symbol: &str) -> Result<Option<Position>, BrokerError>;
    async fn get_positions(&self) -> Result<Vec<Position>, BrokerError>;
    async fn get_account_summary(&self) -> Result<AccountSummary, BrokerError>;
    async fn subscribe_order_events(
        &self,
        tx: mpsc::Sender<OrderEvent>,
    ) -> Result<SubscriptionHandle, BrokerError>;
}
```

### 6.3 Repository

```rust
#[async_trait]
pub trait CandleRepository: Send + Sync {
    async fn save(&self, candles: &[Candle]) -> Result<usize, RepositoryError>;
    async fn get_range(
        &self,
        symbol: &str,
        timeframe: TimeFrame,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<Vec<Candle>, RepositoryError>;
    async fn exists(
        &self,
        symbol: &str,
        timeframe: TimeFrame,
        timestamp: DateTime<Utc>,
    ) -> Result<bool, RepositoryError>;
}
```

---

## 7. Fluxos de execução

### 7.1 Live / Paper Trading

```text
1. Inicializar configuração e conexões (DB, broker, data provider).
2. Carregar estratégia ativa e parâmetros.
3. Recuperar estado atual (posições, ordens abertas, P&L do dia).
4. Inscrever-se em barras de tempo real para o ativo.
5. Ao fechar um candle:
   5.1 Salvar candle no banco (deduplicar por symbol/timeframe/timestamp).
   5.2 Atualizar indicadores e contexto de mercado.
   5.3 Persistir contexto.
   5.4 Executar estratégia.
   5.5 Se sinal válido e sem posição aberta:
       - RiskManager valida risco/retorno e limites.
       - Se aprovado, calcular tamanho da posição.
       - Enviar ordem de entrada + stop + alvo (bracket/OCO).
       - Registrar sinal, ordem e motivo.
   5.6 Se rejeitado, registrar motivo.
6. Ao receber evento de fill:
   6.1 Atualizar posição e trade no banco.
   6.2 Gerar diário automático.
   6.3 Verificar limites diários.
7. Na janela de fim de pregão (`[session] flatten_start`–`flatten_end`, por padrão
   15h55–16h10 em horário de Nova York, lida da configuração — ADR-018):
   7.1 Cancelar a ordem de entrada pendente.
   7.2 Encerrar a mercado a posição que a própria instância abriu (posição órfã
       fica para o comando manual `trader-cli flatten`).
   7.3 Gravar o trade com `ExitReason::EndOfDay` — antes era `Manual` com
       `journal.forced_exit`.
8. Loop contínuo com health checks e reconexão.
```

> **Por que o passo 7 não é opcional:** as pernas do bracket vão com TIF Day e
> expiram no sino. Posição que atravessa a noite fica **sem stop**.

### 7.2 Backtest

```text
1. Carregar configuração da estratégia e parâmetros de risco.
2. Buscar candles históricos do banco.
3. Inicializar SimulatedBroker com slippage e comissão.
4. Para cada candle, em ordem cronológica:
   4.1 Alimentar estratégia apenas com dados até aquele ponto.
   4.2 Detectar sinais.
   4.3 Validar risco.
   4.4 Simular execução no fechamento do candle (modo conservador).
   4.5 Atualizar posições, stops e alvos.
   4.6 Registrar fills e resultados.
   4.7 Se o candle for o último do pregão, encerrar a posição no fechamento dele
       e gravar `ExitReason::EndOfDay` (ADR-018).
5. Ao final, calcular métricas e gerar relatório.
```

Sobre o passo 4.7:

- O gatilho é **mudança de data ET** entre o candle atual e o seguinte, não o
  relógio. Assim pregão de meio expediente também encerra, e o horário de
  `session_flatten_et` (15h45 ET, espelhando `[session] last_bar`) entra só como
  checagem de sanidade: se o pregão terminar depois dele, a série não é RTH-only e
  o log avisa.
- **O fim da série não é sino.** A última posição da amostra fica aberta — se o fim
  dos dados contasse como fim de pregão, cada fronteira de janela do walk-forward
  geraria um `EndOfDay` fantasma.
- `--no-flatten` desliga o passo e reproduz os runs anteriores ao ADR **ao
  centavo** (Σ|diff| = 0 no in-sample das três estratégias vivas). Serve para medir
  o delta, não como gate.

Esta era a divergência live/backtest que fazia o gate A comparar a operação real
com um backtest que dormia posicionado. Números do reparo em
`docs/reports/gate-a-com-flatten-2026-09-07.md`.

### 7.3 Ingestão histórica

```text
1. Receber comando com symbol, timeframe, intervalo.
2. Buscar candles no provedor de dados.
3. Para cada candle, inserir com UPSERT em (symbol, timeframe, timestamp).
4. Registrar log de ingestão (quantidade, gaps detectados, duplicatas).
```

---

## 8. Padrões de projeto

### 8.1 Tratamento de erros

- Domínio usa `thiserror` para erros tipados.
- Camada de aplicação usa `anyhow` para composição contextual.
- Erros de broker são classificados em:
  - `Retryable` — reconectar e tentar novamente (ex.: timeout, rate limit).
  - `Fatal` — parar o sistema e alertar (ex.: credencial inválida, conta bloqueada).
  - `Business` — registrar e continuar (ex.: ordem rejeitada por saldo insuficiente).

### 8.2 Logging e tracing

- `tracing` para logs estruturados.
- Cada decisão importante gera um span com:
  - `symbol`, `timeframe`, `timestamp`, `strategy_id`, `correlation_id`.
- Logs de nível `INFO`: sinais, ordens, fills, rejeições importantes.
- Logs de nível `DEBUG`: cálculos de indicadores, verificações de contexto.
- Logs de nível `ERROR`: falhas de conexão, violações de invariantes.

### 8.3 Configuração

- Configuração base em arquivo TOML (`config/default.toml`); parâmetros de estratégia em `config/strategies/<id>.toml`.
- O bloco `[session]` (`flatten_start`, `flatten_end`, `last_bar`, em horário de Nova York) é a fonte única do fim de pregão: live e backtest leem dele, de modo que a paridade do ADR-018 é por configuração e não por coincidência entre constantes duplicadas.
- Sobreposição por variáveis de ambiente (`TRADER_BROKER__PAPER=true`).
- Segredos via variáveis de ambiente ou secret manager (nunca no repo).
- Cada execução registra o hash da configuração efetiva (`config_hash`). Ele é chave de comparação, não só metadado: o `analyze` escolhe o baseline de backtest por (estratégia, ativo, `config_hash`), ignorando runs experimentais, e filtra os trades do live pelo mesmo par (ADR-019 §3). Mudar um parâmetro invalida a comparação de propósito — foi o que o hotfix v1.0.1 fez com a `range-extreme-fade-v1` (`49ee6f045b4c35a7` → `818b53394244ca62`).

### 8.4 Tempo

- Todos os timestamps em UTC no banco e no domínio.
- Regra de negócio que depende do relógio do mercado converte para horário de Nova
  York e **nunca** compara UTC fixo: janelas de negociação, veto de meio-dia das
  estratégias, flatten de fim de pregão (ADR-018) e o bucket `by_entry_hour_et`
  das métricas.
- A conversão vive num único lugar, `trader_core::session`. Offset fixo desliza uma
  hora na virada do DST: foi assim que o veto de meio-dia da
  `range-extreme-fade-v1` rodou na hora errada **fora do horário de verão** —
  parte da amostra do gate A foi medida com a janela deslocada — até o hotfix
  v1.0.1, e o efeito da correção não foi neutro (−21% no net in-sample da fade;
  o bug estava ajudando).
- Apresentação converte de UTC para ET para exibir; nunca o contrário.
- `Clock` trait para testes determinísticos.

---

## 9. Decisões arquiteturais consolidadas

As decisões abaixo são detalhadas nos ADRs em `docs/decisions/`:

| # | Decisão | Resumo |
|---|---------|--------|
| ADR-001 | Backend em Rust | Performance previsível, segurança de memória, tipagem forte para finanças. |
| ADR-002 | PostgreSQL como datastore | Dados relacionais, auditabilidade, SQL puro, ecossistema maduro. |
| ADR-003 | Interactive Brokers como broker inicial | Conta canadense existente, API estável, paper trading disponível. |
| ADR-004 | Workspace com múltiplos crates | Separação de domínio, testabilidade, build incremental. |
| ADR-005 | Estratégias como plugins via trait | Permite backtest e live compartilharem a mesma lógica. |
| ADR-006 | Event sourcing para decisões | Auditoria completa e reprodução de cenários. |
| ADR-007 | TWS API/IB Gateway para IBKR | Conexão persistente para streaming e ordens. |
| ADR-008 | Paper trading com replay de candles do banco | Validação sem risco e auditoria completa antes do live. |
| ADR-009 | Tipo de entrada configurável por estratégia (stop vs limit) | A entrada deixa de ser fixa e passa a ser parâmetro da estratégia. |
| ADR-010 | Gate de go-live composto (estratégia + operação) | **É o gate em vigor.** Seis critérios: ≥ 50 trades, WR ≥ 40%, PF ≥ 1,3, DD ≤ 10%, avg R > 0,15, net > 0. |
| ADR-011 | Operação na VM Oracle | Tirar bot, Gateway e banco do PC. Desativada pelo ADR-012. |
| ADR-012 | Migração do live para o servidor da casa (umbrelOS, containers) | Substitui a VM Oracle. |
| ADR-013 | Empacotar o bot como app do umbrelOS | Implementado; cutover em 2026-08-28. |
| ADR-014 | Painel web de status (`trader-web`) | Implementado em 2026-08-28. |
| ADR-015 | Guarda de overshoot na entrada stop | Live ≡ backtest na entrada; precedente de que run anterior à correção não é comparável. |
| ADR-016 | Desligar a `pullback-trend-v1` | **Aplicado em produção desde 2026-09-04** (app v1.2.0): as instâncias da pullback saíram do compose e o `images.yml` recria 8 instâncias de 3 estratégias. |
| ADR-017 | Limite de risco da conta inteira, não só por instância | Soma a exposição das instâncias; fechou o bloqueador de go-live da auditoria de 30/08. |
| ADR-018 | Paridade de fim de sessão entre live e backtest | Flatten de fim de pregão como `ExitReason::EndOfDay`; janela no `[session]` da config para os dois modos; no backtest o gatilho é a mudança de data ET. Migração 0004 amplia o CHECK de `trades.exit_reason`. |
| ADR-019 | Harness de validação e gate A estatístico | `--strategy-config`/`--set` permitem ablação sem código novo; holdout travado por `--holdout-from`; baseline do `analyze` por (estratégia, ativo, `config_hash`); métricas de dispersão e concentração. Implementado exceto o item 8 (relatório Python em `trader-research/`) e o dedupe do item 3. **Os critérios do §7 são proposta ainda não vigente — o gate em vigor continua sendo o do ADR-010.** |
| ADR-020 | Dimensionamento por liquidez e fração de capital | **Proposto / especificado — NÃO implementado.** Nada do sizing descrito nele está no código. |

---

## 10. Restrições e premissas

- O MVP opera apenas em **paper trading**.
- **SPY** é só o valor padrão dos comandos da CLI, não o ativo operado. A produção roda 8 instâncias, e os pares medidos no gate A de 07/09/2026 são IJS, VBR e AVUV (`balance-area-breakout-v1`), AVUV, SLYV e IWV (`range-extreme-fade-v1`) e IWM e IWN (`opening-reversal-v1`).
- Timeframes operacionais: **15min** (operação), **1h** (contexto), **diário** (macro).
- O sistema não fará HFT, scalping de alta frequência ou arbitragem.
- A latência aceitável é de segundos, não milissegundos.
- Dados de mercado podem ser limitados/atrasados sem assinatura IBKR adequada.

---

## 11. Métricas e observabilidade

### 11.1 Métricas técnicas

- Uptime do worker.
- Latência entre fechamento de candle e decisão.
- Taxa de reconexão do data provider.
- Taxa de ordens rejeitadas pelo broker.
- Candles perdidos ou duplicados.

### 11.2 Métricas de negócio

- Win rate, profit factor, drawdown máximo.
- Média de R por trade.
- Número de sinais rejeitados por motivo.
- Violações de risco (deve ser sempre zero).

### 11.3 Alertas

- Perda máxima diária atingida.
- Falha de conexão com broker por mais de N segundos.
- Posição real divergente da posição esperada.
- Ordem sem atualização de status por mais de N minutos.

---

## 12. Evolução planejada

| Fase | Foco | Mudança arquitetural |
|------|------|----------------------|
| Fase 1 | Conexão e ingestão | Crates `domain`, `adapters`, `infra` estabilizados. |
| Fase 2 | Contexto | `MarketContextAnalyzer` e tabelas de contexto. |
| Fase 3 | Setup | Primeira estratégia `pullback-trend-v1`. |
| Fase 4 | Paper trading | `ExecutionEngine` e `IbkrBrokerAdapter`. |
| Fase 5 | Backtest | Crate `trader-backtest` e `SimulatedBroker`. |
| Fase 6 | Dashboard | API HTTP e frontend React (fora do workspace Rust). |
| Fase 7 | Multi-broker | Novos adapters validando portabilidade. |

---

## 13. Referências

- `docs/PRD.md`
- `docs/strategy-analysis-framework.md`
- `docs/strategies/pullback-trend-v1.md`
- `docs/TECHNICAL-ROADMAP.md`
- `docs/DATA-MODEL.md`
- `docs/OPERATIONS.md`
- `docs/SECURITY.md`
- `docs/decisions/ADR-*.md`
