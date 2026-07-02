# Arquitetura de Software — HumanStyle Trader Bot

**Versão:** 1.0  
**Status:** Aprovado para implementação  
**Última atualização:** 2026-07-02  
**Autor:** Software Architect  

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
│  │ Portfolio    │  │ OrderRouter  │  │ Journal      │  │ Scheduler        │ │
│  │ Manager      │  │              │  │ Generator    │  │                  │ │
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
│  │ Repositories │  │ Event Store  │  │ Config       │  │ Logging/Tracing  │ │
│  │ (sqlx)       │  │              │  │ Loader       │  │                  │ │
│  └──────────────┘  └──────────────┘  └──────────────┘  └──────────────────┐ │
└───────────────────────────────────────────────────────────────────────────┘
                                │
┌───────────────────────────────▼─────────────────────────────────────────────┐
│                              Entrypoints                                     │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────────────────┐  │
│  │ trader-cli      │  │ trader-worker   │  │ trader-api (futuro)         │  │
│  └─────────────────┘  └─────────────────┘  └─────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────────────┘
```

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
│   ├── trader-infra/             # DB, config, fila de eventos, logging
│   ├── trader-journal/           # Diário automático e analytics
│   ├── trader-backtest/          # Engine de backtest
│   └── trader-cli/               # Binário CLI principal
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
- `portfolio::PortfolioManager` — rastreamento de posições abertas.
- `strategies/` — implementações concretas de estratégias.
  - `pullback_trend_v1/`
    - `mod.rs`
    - `context.rs`
    - `setup.rs`
    - `entry.rs`
    - `config.rs`
- `indicators/` — EMA, ATR, volume relativo, etc.

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

**Futuramente:**

- `alpaca::AlpacaBrokerAdapter`
- `polygon::PolygonMarketDataProvider`
- `simulated::SimulatedBroker` (para testes e backtest)

### 5.4 `trader-infra`

**Responsabilidade:** Tudo que conecta o sistema ao mundo operacional.

**Módulos:**

- `db` — conexão PostgreSQL, migrations sqlx, repositories.
- `config` — carregamento de configuração (arquivos + env vars).
- `event_bus` — canal de eventos internos (tokio broadcast/mpsc).
- `logging` — inicialização do `tracing`.
- `clock` — abstração de tempo para testes determinísticos.

### 5.5 `trader-journal`

**Responsabilidade:** Transformar eventos em diário de trades e relatórios de performance.

**Módulos:**

- `trade_journal` — registro por trade.
- `daily_journal` — resumo diário.
- `analytics` — cálculo de métricas (win rate, profit factor, drawdown, Sharpe simplificado).
- `reporters` — exportação CSV, JSON, PDF (futuro).

### 5.6 `trader-backtest`

**Responsabilidade:** Executar estratégias sobre dados históricos de forma determinística.

**Módulos:**

- `engine` — loop de eventos por candle.
- `broker_sim` — simulador de execução com slippage e comissão.
- `metrics` — cálculo de métricas de performance.
- `report` — geração de relatórios comparativos.

### 5.7 `trader-cli`

**Responsabilidade:** Entrypoint principal do sistema.

**Comandos iniciais:**

```text
trader-cli backtest --strategy pullback-trend-v1 --symbol SPY --from 2025-01-01 --to 2025-12-31
trader-cli paper --strategy pullback-trend-v1 --symbol SPY
trader-cli ingest --symbol SPY --timeframe 15m
trader-cli status
trader-cli journal --date 2026-07-01
```

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
7. Loop contínuo com health checks e reconexão.
```

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
5. Ao final, calcular métricas e gerar relatório.
```

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

- Configuração base em arquivo TOML (`config/default.toml`).
- Sobreposição por variáveis de ambiente (`TRADER_BROKER__PAPER=true`).
- Segredos via variáveis de ambiente ou secret manager (nunca no repo).
- Cada execução registra o hash da configuração efetiva.

### 8.4 Tempo

- Todos os timestamps em UTC no banco e no domínio.
- Conversão para timezone do mercado apenas na camada de apresentação.
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

---

## 10. Restrições e premissas

- O MVP opera apenas em **paper trading**.
- O primeiro ativo é **SPY** (futuro: QQQ, AAPL, MSFT).
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
