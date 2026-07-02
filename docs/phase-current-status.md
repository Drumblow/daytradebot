# Status Atual do Projeto — Pós-Sprint de Auditabilidade

**Data:** 2026-07-02  
**Sprint:** Correção do core, auditabilidade e MVP de paper trading simulado  
**Responsável:** CTO / Agente de IA  
**Status geral:** ✅ MVP de paper trading simulado funcional e auditável

---

## Resumo Executivo

Esta sprint consolidou o projeto como um **MVP de paper trading simulado completo e auditável**. O código compila, todos os testes unitários e de integração passam, e o `cargo clippy --all-targets --all-features -- -D warnings` está limpo.

Principais conquistas:

- A estratégia `pullback-trend-v1` respeita totalmente a configuração e preenche `market_snapshot` com valores brutos.
- Repositórios de ativos, contextos, sinais, ordens e trades estão implementados e testados.
- O backtest aplica slippage, calcula Sharpe simplificado e pode carregar candles do PostgreSQL.
- O comando `paper` suporta loop contínuo simulado, modo `replay` com candles do banco e persistência completa.
- Comandos `status` e `journal` permitem acompanhar operações e decisões rejeitadas.
- A integração com IBKR via TWS API/IB Gateway está codificada, mas ainda não validada com conta liberada.

---

## O que foi entregue

### 1. Estratégia `pullback_trend_v1`

- `StrategyParameters` inclui `tick_size` e todos os parâmetros de contexto, setup e entrada.
- `MarketContextAnalyzer` é construído a partir dos parâmetros da estratégia (`ema_context_period`, `sma_context_period`, `max_atr_pct`).
- `check_context` usa os parâmetros de configuração em vez de valores hardcoded.
- `setup.rs` usa `params.tick_size` para arredondar preços de stop e entrada.
- `entry.rs` preenche `market_snapshot` com EMA, ATR, fase de mercado, índices do setup e valores brutos.
- Testes unitários cobrem setup perfeito, rejeição por contexto, risco-retorno, spread, horário e pullback que quebra estrutura.

### 2. Repositórios (`trader-infra`)

- `SqlxAssetRepository`: busca e salva ativos.
- `SqlxCandleRepository`: salva em lote com deduplicação (`ON CONFLICT DO UPDATE`) e busca por range.
- `SqlxMarketContextRepository`: salva, busca último e busca range de contextos.
- `SqlxSignalRepository`: `save`, `get_by_id`, `list_by_symbol`, `list_by_status`, `list_today`, `update_status`.
- `SqlxOrderRepository`: `save`, `get_by_id`, `list_open`, `list_by_signal`, `update_status`.
- `SqlxTradeRepository`: `save`, `get_by_id`, `list_by_symbol`, `list_today`.
- `RepositoryError::InvalidData` adicionado para validações de dados.
- `unwrap_or_default` removido de código de produção em favor de tratamento de erro explícito.

### 3. Backtest (`trader-backtest`)

- `SimulatedBrokerConfig` inclui `slippage_pct` e comissões.
- Slippage é aplicado no preço de execução contra o trader.
- `BacktestEngine` registra série de equity ao longo do tempo.
- `BacktestMetrics` calcula Sharpe ratio simplificado anualizado, win rate, profit factor, max drawdown, etc.
- `trader-cli backtest` pode carregar candles do banco via `--from`, `--to`, `--timeframe`.
- Fallback para série sintética quando não há dados no banco.

### 4. Paper Trading (`trader-cli paper`)

- Loop contínuo com shutdown gracioso via Ctrl+C.
- Modos `simulated` (candles sintéticos em memória) e `replay` (candles históricos do banco).
- Persiste sinais, ordens, trades e contextos no PostgreSQL durante o loop.
- Reconciliação simples: não busca novo sinal se já houver posição aberta no mesmo ativo.
- `SimulatedBroker` rejeita nova posição se já existir posição aberta.

### 5. CLI

- `test-connection --provider {ibkr,simulated}`
- `account --provider {ibkr,simulated}`
- `ingest --symbol <s> --timeframe <tf> --days <n> --provider <p>`
- `paper --symbol <s> --strategy <id> --mode {simulated,replay} --timeframe <tf>`
- `backtest --symbol <s> --strategy <id> --from <date> --to <date> --timeframe <tf>`
- `status`: modo, saldo simulado, posições abertas, sinais e trades recentes.
- `journal --date <date>`: trades e sinais rejeitados do dia.

### 6. Documentação

- `README.md` atualizado com status real e exemplos de comandos.
- `docs/TECHNICAL-ROADMAP.md` atualizado com itens concluídos e pendentes.
- `docs/ARCHITECTURE.md` ajustado para refletir crates reais existentes.
- `docs/phase-current-status.md` (este arquivo) revisado.

---

## O que ainda é stub ou pendente

| Item | Status | Nota |
|------|--------|------|
| Integração real com IBKR | Codificada, não testada | Requer conta liberada para TWS/Gateway. |
| `IbkrBrokerAdapter::get_open_orders` | Stub controlado | Retorna vazio; implementar após validação com conta. |
| `IbkrBrokerAdapter::get_positions` | Stub controlado | Retorna vazio; implementar após validação com conta. |
| `IbkrBrokerAdapter::get_account_summary` | Stub controlado | Retorna zeros/aviso; implementar após validação com conta. |
| `IbkrBrokerAdapter::subscribe_order_events` | Stub controlado | Não envia eventos; implementar após validação com conta. |
| Detecção de gaps e qualidade de dados | Parcial | `ingestions` registra `gaps_detected`, mas lógica automática ainda simples. |
| `PortfolioManager` dedicado | Não iniciado | P&L diário e exposição estão no `RiskState`. |
| Alertas de risco/falha | Não iniciado | Apenas logs por enquanto. |
| Exportação de relatório de backtest | Não iniciado | Relatório imprime no terminal; JSON/CSV futuro. |
| Dashboard frontend | Não iniciado | Fase futura (Fase 7). |
| `trader-journal` como crate separado | Não existe | Diário automático é gerado pelo `trader-cli` e persistido em `trades.journal`. |
| Dockerfile da aplicação | Não existe | Apenas `docker-compose.yml` para PostgreSQL. |

---

## Próximos passos recomendados

1. **Validar IBKR com conta liberada**
   - Testar `test-connection --provider ibkr`.
   - Implementar stubs do `IbkrBrokerAdapter` relacionados a conta/posições/eventos.
   - Rodar paper trading com dados reais da IBKR.

2. **Hardening operacional**
   - Reconexão automática do data provider/broker.
   - Circuit breaker para perda diária e falhas críticas.
   - Alertas (webhook/email).

3. **Analytics**
   - Exportar relatório de backtest em JSON/CSV.
   - Comparar performance entre estratégias.

4. **Portfólio e risco**
   - Criar `PortfolioManager` dedicado.
   - Melhorar rastreamento de P&L diário e exposição.

5. **Dashboard**
   - API HTTP leve.
   - Frontend React/Next.js.

---

## Validação

```bash
$ cargo build --all-targets           # ✅ OK
$ cargo test --workspace              # ✅ 28 testes passando
$ cargo clippy --all-targets --all-features -- -D warnings  # ✅ OK
$ cargo fmt --all -- --check          # ✅ OK
$ docker-compose up -d postgres       # ✅ PostgreSQL acessível em localhost:5433
```

---

## Referências

- `docs/PRD.md`
- `docs/ARCHITECTURE.md`
- `docs/TECHNICAL-ROADMAP.md`
- `docs/SECURITY.md`
- `AGENTS.md`
