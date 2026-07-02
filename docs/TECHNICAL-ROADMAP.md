# Roadmap Técnico — HumanStyle Trader Bot

**Versão:** 1.0  
**Status:** Aprovado para implementação  
**Última atualização:** 2026-07-02  

---

## 1. Objetivo

Traduzir a visão do produto e a arquitetura em um plano de execução técnico sequencial, com entregáveis mensuráveis, critérios de sucesso e dependências explícitas.

---

## 2. Visão das fases

```text
Fase 0 ── Planejamento e fundação
    │
    ▼
Fase 1 ── Domínio e infraestrutura base
    │
    ▼
Fase 2 ── Conexão com dados e broker (paper)
    │
    ▼
Fase 3 ── Motor de contexto de mercado
    │
    ▼
Fase 4 ── Primeira estratégia: pullback em tendência
    │
    ▼
Fase 5 ── Execução em paper trading
    │
    ▼
Fase 6 ── Backtest e analytics
    │
    ▼
Fase 7 ── Dashboard e API
    │
    ▼
Fase 8 ── Hardening e preparação para real
```

---

## 3. Fase 0 — Planejamento e fundação

**Duração estimada:** 1 semana  
**Objetivo:** Deixar o projeto tecnicamente pronto para desenvolvimento.

### Entregáveis

- [ ] Estrutura de workspace Cargo criada com crates iniciais.
- [ ] Repositório Git inicializado com `.gitignore`, `README.md` e `AGENTS.md`.
- [ ] CI/CD inicial configurado (GitHub Actions ou similar): build, test, lint, fmt.
- [ ] Ambiente de desenvolvimento documentado (Rust, PostgreSQL, Docker Compose).
- [ ] Configuração base (`config/default.toml`) e carregamento via `config` crate.
- [ ] Decisões arquiteturais registradas em `docs/decisions/`.

### Critérios de sucesso

```text
cargo build passa sem erros.
cargo test passa (mesmo que só com testes de placeholder).
cargo clippy não gera warnings críticos.
docker-compose up sobe PostgreSQL local.
```

### Riscos

- Configuração de ambiente Rust no Windows pode gerar atrito.
- Migrações sqlx exigem DATABASE_URL configurada em tempo de compilação.

---

## 4. Fase 1 — Domínio e infraestrutura base

**Duração estimada:** 2 semanas  
**Objetivo:** Construir o vocabulário do sistema e a camada de persistência.

### Entregáveis

#### 4.1 `trader-domain`

- [x] Definição de todas as entidades iniciais:
  - `Candle`, `Quote`, `Tick`
  - `Signal`, `SignalStatus`, `Direction`, `RejectionReason`
  - `Order`, `OrderType`, `OrderStatus`, `TimeInForce`
  - `Fill`, `Trade`, `Position`, `AccountSummary`
  - `MarketContext`, `TrendState`, `VolatilityRegime`
  - `StrategyId`, `StrategyConfig`, `StrategyState`
- [x] Definição dos traits principais:
  - `MarketDataProvider`
  - `Broker`
  - `CandleRepository`
  - `Clock`
- [x] Erros tipados de domínio (`DataError`, `BrokerError`, `RepositoryError`, `ValidationError`).

#### 4.2 `trader-infra`

- [x] Conexão PostgreSQL via `sqlx` com pool async.
- [x] Sistema de migrations (`crates/trader-infra/src/db/migrations/`).
- [x] Implementação de `SqlxCandleRepository`.
- [x] Implementação de `SqlxSignalRepository`, `SqlxOrderRepository`, `SqlxTradeRepository`.
- [x] Carregamento de configuração TOML + env vars.
- [x] Inicialização de `tracing` com formato JSON opcional.
- [x] `SystemClock` e `MockClock`.

### Critérios de sucesso

```text
Migrations executam em banco limpo sem erros. ✅
É possível salvar e recuperar candles, sinais e ordens via repositories. ✅
Testes de integração com banco de teste passam. ✅
Configuração é carregada corretamente de arquivo e variáveis de ambiente. ✅
```

> Ver detalhes em `docs/phase1-progress.md`.

### Tarefas técnicas detalhadas

1. Criar `crates/trader-domain/Cargo.toml` com dependências mínimas.
2. Criar `crates/trader-infra/Cargo.toml` com `sqlx`, `tokio`, `tracing`, `config`.
3. Definir enums no banco como `TEXT` ou tipos customizados, com conversores sqlx.
4. Criar migration `V1__initial_schema.sql`.
5. Escrever testes de integração que rodam em banco isolado (`sqlx::test`).

---

## 5. Fase 2 — Conexão com dados e broker (paper)

**Duração estimada:** 2–3 semanas  
**Objetivo:** Estabelecer comunicação bidirecional com a Interactive Brokers em paper trading.

### Entregáveis

#### 5.1 Market data

- [ ] Escolha definitiva da API IBKR: **IBKR Client Portal API (REST)** ou **TWS API**.
  - Recomendação inicial: avaliar Client Portal API primeiro por simplicidade de HTTP.
  - Fallback: TWS API via `ibapi` crate ou socket raw se necessário.
- [ ] Implementação de `IbkrMarketDataProvider`.
- [ ] Busca de candles históricos.
- [ ] Subscrição de barras em tempo real (se disponível sem custo extra).
- [ ] Deduplicação de candles no banco.
- [ ] Detecção de gaps e registros de qualidade de dados.

#### 5.2 Broker

- [ ] Implementação de `IbkrBrokerAdapter`.
- [ ] Envio de ordem simples (market/limit).
- [ ] Cancelamento de ordem.
- [ ] Consulta de status e ordens abertas.
- [ ] Consulta de posições e saldo paper.
- [ ] Subscrição de eventos de fill.

#### 5.3 CLI

- [ ] Comando `trader-cli ingest`.
- [ ] Comando `trader-cli test-connection`.
- [ ] Comando `trader-cli account`.

### Critérios de sucesso

```text
Sistema conecta na IBKR paper e mantém sessão ativa por 8h.
Ingesta 1 mês de candles de SPY sem duplicatas.
Envia ordem de teste de 1 ação de SPY e recebe confirmação.
Recebe atualização de posição/saldo consistente.
```

### Decisões pendentes a serem tomadas nesta fase

1. **Qual API da IBKR usar?**
   - Client Portal API: REST, mais simples, requer autenticação web, pode expirar sessão.
   - TWS API: socket persistente, mais robusto para streaming, requer TWS/IB Gateway aberto.
2. **Como lidar com autenticação 2FA?**
   - Paper trading pode não exigir 2FA frequente.
   - Para automação, considerar IB Gateway com "Trust this device".

---

## 6. Fase 3 — Motor de contexto de mercado

**Duração estimada:** 2 semanas  
**Objetivo:** Classificar o mercado de forma objetiva e auditável.

### Entregáveis

- [ ] Implementação de indicadores no `trader-core`:
  - EMA/SMA.
  - ATR e ATR percentual.
  - Volume relativo.
  - Máximas/mínimas de swing.
- [ ] Implementação de `MarketContextAnalyzer`.
- [ ] Classificações:
  - `TrendState`: uptrend, downtrend, neutral.
  - `VolatilityRegime`: high, normal, low.
  - `MarketPhase`: pre_market, regular, after_hours.
- [ ] Persistência de contexto a cada candle fechado.
- [ ] Regras de rejeição de contexto testáveis unitariamente.

### Critérios de sucesso

```text
Para 100 candles históricos de SPY, o contexto é classificado consistentemente.
Regras geram os mesmos resultados em execuções repetidas.
Cada classificação armazena os valores brutos que a originaram.
```

---

## 7. Fase 4 — Primeira estratégia: pullback em tendência

**Duração estimada:** 3 semanas  
**Objetivo:** Implementar a estratégia `pullback-trend-v1` de forma auditável e testada.

### Entregáveis

- [ ] Estrutura `trader-core/src/strategies/pullback_trend_v1/`.
- [ ] `config.rs` com todos os parâmetros parametrizáveis.
- [ ] `context.rs` com regras de contexto de mercado.
- [ ] `setup.rs` com detecção de high 2 e barra de sinal.
- [ ] `entry.rs` com regras de entrada, stop e alvo.
- [ ] `mod.rs` expondo a trait `Strategy`.
- [ ] Testes unitários com candles sintéticos cobrindo:
  - Setup perfeito → sinal de compra.
  - Setup sem contexto de tendência → rejeição.
  - Setup com risco-retorno ruim → rejeição.
  - Setup com spread alto → rejeição.
  - Setup fora do horário → rejeição.
  - Pullback que quebra estrutura → rejeição.

### Critérios de sucesso

```text
Todos os testes unitários passam.
Estratégia gera sinais apenas quando todas as regras são atendidas.
Cada rejeição produz um RejectionReason específico.
Configuração é totalmente parametrizável (nenhum valor hardcoded).
```

---

## 8. Fase 5 — Execução em paper trading

**Duração estimada:** 3 semanas  
**Objetivo:** Fazer o robô operar sozinho em paper trading, do sinal ao registro do trade.

### Entregáveis

- [ ] Implementação de `RiskManager` com todas as regras de segurança.
- [ ] Implementação de `ExecutionEngine`.
  - Estado da posição.
  - Envio de ordem de entrada.
  - Envio de stop loss e take profit (bracket order ou OCO).
  - Atualização de fills.
  - Verificação de saída por tempo.
- [ ] `PortfolioManager` para rastrear exposição e P&L diário.
- [ ] Integração completa no worker `trader-cli paper`.
- [ ] Diário automático por trade e por dia.
- [ ] Alertas de violação de risco ou falha de execução.

### Critérios de sucesso

```text
O bot realiza operações completas em paper trading sem intervenção manual.
Stop e alvo são respeitados em 100% das operações.
Nenhuma violação de regra de risco ocorre.
Diário é gerado automaticamente a cada trade.
```

### Regras de segurança financeira implementadas

```text
Nunca operar sem stop.
Nunca operar fora do horário configurado.
Nunca operar após perda máxima diária.
Nunca dobrar lote após perda.
Nunca abrir nova posição se já existir posição ativa no mesmo ativo.
Nunca operar dinheiro real.
```

---

## 9. Fase 6 — Backtest e analytics

**Duração estimada:** 2–3 semanas  
**Objetivo:** Permitir avaliar a estratégia em dados históricos com as mesmas regras do live.

### Entregáveis

- [ ] Implementação de `SimulatedBroker`.
- [ ] Engine de backtest com loop determinístico.
- [ ] Slippage configurável por ativo.
- [ ] Comissões configuráveis.
- [ ] Cálculo de métricas:
  - Número de trades.
  - Win rate.
  - Lucro/prejuízo total.
  - Drawdown máximo.
  - Profit factor.
  - Média de R por trade.
  - Sequência máxima de perdas.
  - Melhor/pior trade.
  - Tempo médio na operação.
  - Expectativa matemática.
  - Sharpe simplificado.
- [ ] Comando `trader-cli backtest`.
- [ ] Exportação de relatório em JSON/CSV.

### Critérios de sucesso

```text
Backtest de 6 meses de SPY executa em menos de 5 minutos.
Métricas calculadas corretamente e consistentemente.
Resultado do backtest é reproduzível (mesmo seed/config → mesmo resultado).
Não há lookahead bias nas decisões.
```

---

## 10. Fase 7 — Dashboard e API

**Duração estimada:** 3–4 semanas  
**Objetivo:** Criar interface visual para acompanhar o robô.

### Entregáveis

- [ ] API HTTP leve em Rust (`trader-api`) ou expandida no `trader-cli`.
  - Endpoints: status, trades, sinais rejeitados, equity curve, métricas.
- [ ] Frontend React + Next.js + Tailwind + Shadcn.
- [ ] Páginas:
  - Status do bot (conectado/desconectado, modo paper).
  - Trades recentes.
  - Sinais rejeitados com motivos.
  - Equity curve.
  - Métricas principais.
  - Logs em tempo real.

### Critérios de sucesso

```text
Usuário consegue acompanhar o bot sem abrir terminal.
Dashboard atualiza a cada 15 segundos.
Indicador visual claro de "MODO PAPER TRADING".
```

---

## 11. Fase 8 — Hardening e preparação para real

**Duração estimada:** 2–3 semanas  
**Objetivo:** Tornar o sistema confiável para operação contínua.

### Entregáveis

- [ ] Testes de longa duração em paper (mínimo 2 semanas).
- [ ] Reconciliação automática entre posição esperada e posição real.
- [ ] Circuit breaker para perda diária, volatilidade extrema, falhas de API.
- [ ] Documentação de runbooks (`docs/runbooks/`).
- [ ] Monitoramento com alertas (email, webhook, etc.).
- [ ] Checklist de migração para operação real.

### Critérios de sucesso

```text
Uptime de 99% em paper trading durante 2 semanas.
Nenhuma perda além do limite diário configurado.
Reconciliação detecta divergências em menos de 1 minuto.
Documentação permite que outra pessoa opere o sistema.
```

---

## 12. Cronograma resumido

| Fase | Duração | Início estimado | Término estimado |
|------|---------|-----------------|------------------|
| Fase 0 | 1 semana | Semana 1 | Semana 1 |
| Fase 1 | 2 semanas | Semana 1 | Semana 3 |
| Fase 2 | 2–3 semanas | Semana 3 | Semana 6 |
| Fase 3 | 2 semanas | Semana 5 | Semana 7 |
| Fase 4 | 3 semanas | Semana 7 | Semana 10 |
| Fase 5 | 3 semanas | Semana 10 | Semana 13 |
| Fase 6 | 2–3 semanas | Semana 12 | Semana 15 |
| Fase 7 | 3–4 semanas | Semana 15 | Semana 19 |
| Fase 8 | 2–3 semanas | Semana 18 | Semana 21 |

> Nota: algumas fases podem ser paralelizadas. Fases 3 e 4 podem começar antes do término total da Fase 2.

---

## 13. Dependências externas críticas

| Dependência | Impacto | Mitigação |
|-------------|---------|-----------|
| Conta IBKR Canadá ativa | Impede paper trading | Verificar status e permissões antes da Fase 2. |
| Assinatura de market data | Dados atrasados/limitados | Começar com dados disponíveis; avaliar upgrade antes do real. |
| PostgreSQL local/Docker | Desenvolvimento e testes | Manter Docker Compose funcional. |
| Conexão de internet estável | Execução live | Prever reconexão e fallback. |

---

## 14. Riscos técnicos e mitigações

| Risco | Probabilidade | Impacto | Mitigação |
|-------|---------------|---------|-----------|
| API IBKR instável ou difícil de automatizar | Média | Alto | Avaliar TWS API vs Client Portal; ter fallback manual. |
| Latência ou dados atrasados distorcem sinais | Média | Médio | Timeframes maiores (15min+); alertas de qualidade de dados. |
| Estratégia sem edge estatístico | Alta | Alto | Backtest rigoroso; paper trading longo; aceitar rejeição. |
| Overengineering antes de validar | Média | Médio | Foco no MVP; dashboard e multi-broker depois. |
| Bugs de execução causam perdas inesperadas | Baixa | Alto | Testes unitários; paper trading; stop sempre; limites diários. |

---

## 15. Checklist de transição entre fases

Antes de iniciar uma nova fase, o seguinte deve estar verdadeiro:

- [ ] Fase anterior concluída com critérios de sucesso atendidos.
- [ ] Código revisado e mergeado na branch principal.
- [ ] Documentação atualizada.
- [ ] Testes passando (unitários e de integração).
- [ ] Decisões arquiteturais impactantes registradas em ADR.

---

## 16. Referências

- `docs/ARCHITECTURE.md`
- `docs/PRD.md`
- `docs/DATA-MODEL.md`
- `docs/OPERATIONS.md`
- `docs/SECURITY.md`
