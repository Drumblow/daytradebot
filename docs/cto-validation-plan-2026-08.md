# Análise de CTO e Plano de Validação — 2026-08-03

**Autor:** análise automatizada (coding agent) a pedido do dono do projeto
**Objetivo:** transformar "o live está rodando" em "o bot está validado para operar de verdade".

---

## 1. Diagnóstico

O projeto está no fim da Fase 6 do roadmap: MVP completo, arquitetura ports & adapters respeitada, live rodando em paper na IBKR (smoke test validado em 2026-08-03). A base é sólida: `Decimal` em tudo, core puro, sinais com `market_snapshot` auditável, stop obrigatório e paper-only garantidos por código.

**Gap crítico:** o live que está rodando hoje **não gera os dados necessários para validar a estratégia**. `subscribe_order_events` é stub, então ordens e trades do modo live **não são persistidos** (`crates/trader-cli/src/commands/paper.rs`) — do live real só restam sinais, sem fills, sem P&L, sem trades. Sem amostra analisável, não há como verificar os critérios de aceitação definidos em `docs/strategies/pullback-trend-v1.md` (≥50 trades em backtest com PF ≥ 1.3; ≥20 trades em paper dentro de ±30% do backtest).

**Resposta à pergunta "a estratégia é boa o suficiente?":** hoje não é possível afirmar — não por ela parecer ruim, mas porque o checklist de validação está em aberto e a infraestrutura de evidência ainda não existe.

---

## 2. Lacunas encontradas (com evidência)

### 2.1 Ciclo de dados do live (bloqueio nº 1)
- `IbkrBrokerAdapter::subscribe_order_events` é stub (`crates/trader-adapters/src/ibkr/broker.rs`).
- Objeto `Order` é descartado no live; `order_repo` é código morto (`paper.rs`).
- `consecutive_losses` nunca é atualizado no live; P&L diário aproximado por equity.
- `RiskState` só existe em memória — restart zera perda diária, trades do dia e perdas consecutivas no meio do pregão.

### 2.2 Backtest com modelagem otimista
- Stops/alvos avaliados só no `close` do candle — perfuração intrabar do stop é ignorada.
- Entrada preenchida imediatamente, sem simular o buy stop que só executa se o próximo candle romper a máxima da barra de sinal.
- Sharpe anualizado com √252 sobre retornos de 15min — valor inflado.
- `RiskConfig` do backtest (100 trades/dia, janela 24h) diverge do live (3/dia, horário 14:30–21:00 UTC) — resultados não comparáveis.
- Fallback silencioso para dados sintéticos se o banco estiver vazio.
- Backtests não são persistidos nem exportados; `metrics.rs`/`report.rs` sem testes.

### 2.3 Validação estatística ausente
- Critérios de aceitação existem (`docs/strategies/pullback-trend-v1.md:191-211`) mas nenhum resultado de backtest está documentado no repo.
- Walk-forward / out-of-sample / Monte Carlo: totalmente ausentes.
- `ingest` nunca grava na tabela `ingestions` — qualidade/gaps dos dados não rastreada.
- Sem ferramenta de análise do live (win rate, PF, distribuição de rejeições) nem comparador backtest-vs-live.

### 2.4 Hardening operacional
- Guard de modo real é só a string `app.mode`; nada impede porta 7496 (real) com `mode=paper`.
- Migrações nunca rodam automaticamente; paper conecta com `.ok()` — banco fora ⇒ live roda **silenciosamente sem persistência**.
- Zero alertas e zero métricas; falhas de conexão só pulam o ciclo.
- `docs/runbooks/` vazio; reconciliação real posição-esperada vs real não existe.
- Gate do projeto: **3 meses de paper + aprovação documentada** antes de real (`docs/OPERATIONS.md:24`).

### 2.5 Bugs menores
- `min_candles_above_ema20`: parâmetro morto na config da estratégia (declarado, nunca lido).
- `max_spread_pct`: Default Rust = 0.0005 vs TOML = 0.05 (fator 100).
- Risco hardcoded em `paper.rs` (1%/2%/3 trades) em vez de vir da config.
- `setup.rs`/`entry.rs`/`config.rs` da estratégia sem testes unitários diretos (viola AGENTS.md §3.4).
- `get_order_status` assume `Filled` quando a ordem some das abertas (canceladas virariam "filled").
- `PositionAlreadyOpen` existe no domínio mas nunca é usado pelo `RiskManager`.
- `docs/phase2-progress.md` e rodapé do PRD desatualizados.

---

## 3. Plano de execução

| Sprint | Entrega | Critério de aceite |
|--------|---------|--------------------|
| **S1** | `subscribe_order_events` com `Client` persistente; persistir ordens no envio; montar `Trade` a partir de fills; rastrear `consecutive_losses` no live | Live paper persiste order→fill→trade no Postgres; testes passando |
| **S2** | `RiskState` reconstruído do banco no boot; risco via `config/default.toml`; corrigir bugs de §2.5; testes unitários de setup/entry/config | Restart não zera limites; limites vêm da config; clippy/testes OK |
| **S3** | Backtest: stops/alvos intrabar (high/low), entrada no candle seguinte via buy stop, Sharpe correto por timeframe, `RiskConfig` alinhado ao live, falhar sem dados reais (flag `--allow-synthetic`), persistir/exportar runs | Testes de métricas e engine; run exportado em JSON |
| **S4** | Walk-forward / out-of-sample no `trader-backtest`; ingestão registrando em `ingestions` com gaps detectados | Comando de walk-forward funcional com relatório por janela |
| **S5** | Comando de análise do live (agregações sobre trades/sinais) + comparador backtest-vs-live contra critérios de `pullback-trend-v1.md` | Relatório único dizendo se critérios de aceitação foram atingidos |
| **S6** | Hardening: guard de porta/modo real no boot, migrações no startup com falha fechada, circuit breaker de falhas consecutivas, alertas (webhook), health, runbooks em `docs/runbooks/` | Checklist Fase 8; live falha fechado sem banco |

Após S1–S6, o gate final permanece: **3 meses de paper com métricas dentro dos critérios** antes de qualquer dinheiro real.

---

## 4. Status de execução

- [x] Análise documentada (este arquivo)
- [x] S1 — eventos de ordem + persistência do live — **entregue 2026-08-03**
  - `subscribe_order_events` via polling de `executions` (reqExecutions) a cada 15s, com dedupe por `execution_id`
  - `FillTracker` no core (fills → trade fechado, com testes); ordens persistidas no envio; fills idempotentes (`broker_fill_id` único); `Trade` montado no fechamento da posição; `consecutive_losses` rastreado; recuperação de ordem aberta após restart
  - Migração 0002: `fills.side` + índice único parcial
- [x] S2 — RiskState durável + config de risco + bugs menores — **entregue 2026-08-03**
  - `rebuild_risk_state` do banco no boot e no rollover UTC; limites em `[risk]` do TOML; `PositionAlreadyOpen` como invariante da `ExecutionEngine`; `min_candles_above_ema20` implementado; defaults de `max_spread_pct` corrigidos (0.05); `daily_trades` contado na entrada; testes de setup/entry/config/context
- [x] S3 — correções do backtest — **entregue 2026-08-03**
  - Stops/alvos intrabar (high/low, pior caso primeiro) no `SimulatedBroker.set_market_candle`; Sharpe anualizado pelo intervalo mediano; `RiskConfig` compartilhado live/backtest (`risk_config.rs`); backtest falha sem dados reais (`--allow-synthetic`); export JSON (`--output`); runs persistidos (`backtest_runs`, migração 0003); relógio simulado usa timestamp do candle; testes de métricas
  - Divergência conhecida documentada: entrada é limit imediata (não buy stop no candle seguinte) em live e backtest — ver `docs/strategies/pullback-trend-v1.md`
- [x] S4 — walk-forward/OOS + ingestions — **entregue 2026-08-03**
  - `trader-cli walkforward --windows N` (anchored, IS vs OOS por janela, agregado OOS com veredito vs critérios); ingest registra em `ingestions` com gaps intraday (`data_quality::count_gaps`); `TimeFrame::duration()`
- [x] S5 — análise do live + comparador — **entregue 2026-08-03**
  - `trader-cli analyze`: métricas do live, distribuição de rejeições, backtest mais recente, veredito dos critérios de paper (±30%); correção do round-trip de `rejection_reason` (snake_case + fallback legado)
- [x] S6 — hardening + runbooks — **entregue 2026-08-03**
  - Guards: `ibkr.paper` obrigatório + recusa de portas reais (7496/4001); migrações no startup; live falha fechado sem banco; circuit breaker (10 falhas consecutivas → alerta + encerramento); alertas via webhook (`[alerts]`); `system_events` para start/stop/circuit breaker; runbooks em `docs/runbooks/`

## 5. Pendências deliberadas (fora deste ciclo)

- Fase 7 (dashboard HTTP + frontend) — não iniciada.
- Streaming de candles (live usa polling de 30s sobre histórico).
- Comissões reais nos fills (CommissionReport da IBKR não é parseado; fills gravam comissão 0).
- Alertas de rejeição de risco em tempo real (hoje: logs; trades fechados e circuit breaker têm webhook).
- Gate final: gate composto da ADR-010 (ver `docs/runbooks/go-live-checklist.md`).

## 6. Ciclo 2026-08-04 — entrada stop + gate de go-live

- [x] **ADR-009**: tipo de entrada configurável por estratégia (`entry_order_type`), com a `pullback-trend-v1` usando buy stop (regra do livro) e expiração por `entry_validity_candles`. Simulador (fill no rompimento intrabar + expiração), IBKR (bracket parent STP), live (cancela entrada expirada). Backtest re-rodado: 13 trades, PF 6.96 (SPY jun–jul/2026).
- [x] **ADR-010**: gate de go-live composto — validação estatística com histórico (agora) + 4 semanas de paper live + ≥20 trades ±30% do backtest + aprovação documentada, substituindo o "3 meses" de calendário.
- [x] Testes de tempo real corrigidos (pré-existentes): horário fixo dentro do pregão em vez de `Utc::now()`.
