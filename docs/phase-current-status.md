# Status Atual do Projeto — Pós-Sprint de Auditabilidade

> **Atualização 2026-09-07 (régua do live e harness de validação):** o ADR-018
> (flatten de fim de pregão), o hotfix v1.0.1 do veto de meio-dia da
> `range-extreme-fade-v1` e o harness do ADR-019 estão **implementados** na
> `main` — **sem push**. O **ADR-020** continua **proposto** e os critérios do
> **ADR-019 §7 não são gate**: o gate vigente continua sendo o ADR-010. O
> veredito de gate A que vale está em
> **`docs/reports/gate-a-com-flatten-2026-09-07.md`**. Ver a seção
> "Estado em 07/09/2026" logo abaixo; o resto do arquivo é o registro da sprint
> de 03/08 e não foi reescrito.

> **Atualização 2026-08-03 (ciclo de validação):** foi concluído um ciclo de 6
> sprints focado em validação para operação real — persistência completa do
> live (ordens/fills/trades), estado de risco durável, correções de backtest,
> walk-forward, comparador live-vs-backtest (`analyze`) e hardening
> operacional. Detalhes completos em **`docs/cto-validation-plan-2026-08.md`**.
> O texto abaixo descreve o estado ao fim da sprint anterior.

**Data:** 2026-08-03 (atualizado)  
**Sprint:** Correção do core, auditabilidade e MVP de paper trading simulado  
**Responsável:** CTO / Agente de IA  
**Status geral:** ✅ MVP de paper trading funcional e auditável — **incluindo modo live validado na conta paper da IBKR**

---

## Estado em 07/09/2026 — o que mudou depois desta sprint

Esta seção é o que vale hoje. Tudo a partir de "Resumo Executivo" é o registro
da sprint de 03/08 e ficou como estava — inclusive a tabela de pendências e o
bloco "Validação" (os 28 testes de lá são daquele dia; hoje o workspace tem
**268 testes**, eram 241, com `cargo clippy --all-targets -D warnings` limpo).

### Implementado — commits `16a0cea`, `cbc8be5`, `8c88fd7`, `13f83f2`, `e1f1266` na `main`, **sem push**

| Item | Status | Nota |
|------|--------|------|
| ADR-018 — flatten de fim de pregão | ✅ Implementado | `ExitReason::EndOfDay` (serde `end_of_day`), `ExitReason::as_str()`, `Trade::effective_exit_reason()`, migração 0004. Seção `[session]` em `config/default.toml` (`flatten_start` 15:55:00, `flatten_end` 16:10:00, `last_bar` 15:45:00 — **horário de Nova York**), tipo `SessionSettings` em `trader-infra::config`, lida pelo live **e** pelo motor. |
| Flatten no backtest | ✅ Implementado | `BacktestConfig.session_flatten_et: Option<(u32,u32)>`, default `Some((15,45))`. O gatilho é **mudança de data ET** (`trader_core::session::et_date()`), não relógio: fim da série não é sino. `--no-flatten` em `backtest` e `walkforward` reproduz o baseline antigo **ao centavo** (diff zero). |
| Hotfix v1.0.1 do veto de meio-dia da `range-extreme-fade-v1` | ✅ Implementado | UTC fixo → ET. Muda o `config_hash`: `49ee6f045b4c35a7` → `818b53394244ca62`. |
| ADR-019 — harness de validação | ✅ Implementado **em parte** — vários itens não entraram; lista e contagem canônicas na seção "Pendente" do ADR-019 | Métricas novas em `BacktestMetrics` e opções novas de CLI, abaixo. Runs OOS no banco dev: **725–732** e os rotulados `gate-a-adr019`. |
| ADR-020 — sizing/liquidez | ⛔ **Proposto** | **Não implementado.** Nenhum caminho de código depende dele; documentação que o trate como vigente está errada. |
| Critérios do ADR-019 §7 (PF_R ≥ 1,2; dois melhores meses ≤ 60%) | ⛔ **Proposta, não vigente** | São métricas medidas e publicadas, **não** gate. O gate de go-live continua sendo o **ADR-010**. |

O que o harness acrescentou, em concreto:

- `BacktestMetrics` ganhou `by_exit_reason`, `by_direction` e `by_entry_hour_et`
  (mapas de `GroupMetrics`, tipo público novo — as chaves de hora são de
  **Nova York**), mais `profit_factor_r`, `t_stat_avg_r`, `corr_risk_result`,
  `trading_days`, `top_day_share`, `top5_day_share`, `top2_month_share`,
  `months_total`, `months_positive` e `cost_total`.
- `trader-cli walkforward` ganhou `--output`, `--slippage-bps` (Decimal, aceita
  `2.5`), `--label`, `--holdout-from`, `--strategy-config` e `--set chave=valor`.
- **Travas que abortam** (existem para impedir run órfão ou run que se confunde
  com o baseline): `--set` sem `--label`; chave inexistente (os 9
  `StrategyParameters` ganharam `deny_unknown_fields`); `--set` que não muda o
  `config_hash`; `--set` junto com `--holdout-from`; TOML de `--strategy-config`
  com `id` diferente do `--strategy`; `--holdout-from` inválido é **erro**, não
  aviso. Módulo novo `crates/trader-cli/src/strategy_source.rs`; `toml` virou
  dependência do `trader-cli`.
- `trader-cli analyze` escolhe o baseline por (estratégia, par, `config_hash`)
  via `latest_for`, ignora runs `experimental` e filtra os trades do live por
  `strategy_id` + `config_hash`. Sem run compatível ele **avisa**, em vez de
  comparar contra outro. Migração 0005 cria `idx_backtest_runs_baseline`.
- `trade_repository` passou de `From<TradeRow>` para `TryFrom<TradeRow>` e a
  leitura de `exit_reason` **falha fechado** — antes um valor desconhecido virava
  `Target` em silêncio. Teste de integração novo em
  `crates/trader-infra/tests/trade_repository_test.rs`.

### Gate A com a régua do live — o veredito mudou

OOS do walk-forward, 6 janelas, com flatten e com o hotfix ET. Fonte:
`docs/reports/gate-a-com-flatten-2026-09-07.md`. Gate vigente = ADR-010
(≥ 50 trades · WR ≥ 40% · PF ≥ 1,3 · DD ≤ 10% · avg R > 0,15 · net > 0).

| Estratégia | Par | n | WR | PF $ | PF R | avg R | Veredito (ADR-010) |
|---|---|---|---|---|---|---|---|
| balance-area-breakout-v1 | IJS | 23 | 60,8% | 2,54 | 1,91 | 0,383 | passa, menos n |
| balance-area-breakout-v1 | VBR | 34 | 44,1% | 1,51 | **0,97** | **−0,013** | **reprova (avg R)** |
| balance-area-breakout-v1 | AVUV | 35 | **31,4%** | 1,43 | **0,74** | **−0,173** | **reprova (WR e avg R)** |
| range-extreme-fade-v1 | AVUV | 26 | 57,6%† | 1,47 | 1,37 | 0,159 | passa por 0,009, menos n |
| range-extreme-fade-v1 | SLYV | 20 | 70,0% | 2,64 | 2,34 | 0,424 | passa, menos n |
| range-extreme-fade-v1 | IWV | 18 | 55,5% | 1,31 | — | **0,043** | **reprova (avg R)** |
| opening-reversal-v1 | IWM | 32 | 59,3% | 1,72 | 2,03 | 0,450 | passa, menos n |
| opening-reversal-v1 | IWN | 26 | 53,8% | 1,56 | 1,56 | 0,283 | passa, menos n |

† A WR da fade em AVUV é a de §3 do relatório, medida **antes** do hotfix ET.
O PF e o avg R da mesma linha vêm de §3b, que é pós-hotfix e **não republica
win rate** — §7, de onde sai o PF em R, também não tem essa coluna. O hotfix
não muda a contagem de trades (26 nos dois lados), mas troca *quais* sinais
passam, então a WR pós-hotfix pode ser outra e ainda não foi medida. Enquanto
não for, esta célula é a única WR publicada para essa combinação; não use os
57,6% como número pós-hotfix.

O que isso contraria no que se dizia antes:

1. A `balance-area-breakout-v1` **reprova** o gate A com a régua do live; só IJS
   passa. Em unidades de risco ela perde em AVUV (PF_R 0,74) e VBR (0,97) — o PF
   em dólares acima de 1 vem da correlação entre tamanho e resultado (0,57 em
   AVUV), que é o cap de notional pondo posição maior justamente nos trades de
   stop largo.
2. A `opening-reversal-v1` **melhora** com o flatten (in-sample PF 1,23 → 1,74,
   +3.400 → +8.085) e passa em IWM e IWN, mas isso é aritmética: some 8 dos 67
   trades, todos overnight e todos perdedores. Não é reabilitação.
3. A `range-extreme-fade-v1` reprova em IWV (avg R 0,043), o que confirma a
   recomendação §5.10 do plano por outro caminho.
4. O hotfix ET tem efeito **negativo e grande** — in-sample a fade cai de
   PF 1,74 / +4.560 para **PF 1,57 · avg R 0,182 · +3.618** (−21% no net), tudo
   em AVUV. O bug estava ajudando; o plano previa "imensurável" e errou.
5. **Nenhum t-stat chega a 2** e só a fade em SLYV passa no critério de
   concentração (59% contra o teto **proposto** de 60%). Sob o gate proposto no
   ADR-019 §7, **nenhuma** das oito combinações passaria — motivo a mais para não
   tratar esses critérios como vigentes sem decisão do dono.

Delta in-sample do flatten, pares vivos, 2 bp, para referência: balance-area
PF 1,92 / avg R 0,214 / +14.648 → PF 1,55 / avg R −0,007 / +6.576 (20 de 96
trades eram overnight); range-fade PF 1,88 / +6.015 → PF 1,74 / +4.560 (9 de 69);
opening-reversal PF 1,23 / +3.400 → PF 1,74 / +8.085 (8 de 67).

**Ressalva:** o banco dev tem feed degradado do Gateway a partir de 07/08/2026.
O **nível** absoluto do último bloco está contaminado; o delta com/sem flatten
não está, porque os dois lados leem os mesmos candles.

### Aviso de deploy — nada disso foi enviado com push

`.github/workflows/images.yml` dispara em push para `main` com paths `crates/**`
e `config/**`: publica imagens e, com `APP_DEPLOY=enabled` e fora do pregão,
**recria as 8 instâncias de produção**. Dar push muda o `config_hash` da
`range-extreme-fade-v1` em produção no meio do gate B, o que reinicia as 4
semanas (§3.8 do plano). Os commits acima estão só na `main` local.

---

## Resumo Executivo

Esta sprint consolidou o projeto como um **MVP de paper trading simulado completo e auditável**. O código compila, todos os testes unitários e de integração passam, e o `cargo clippy --all-targets --all-features -- -D warnings` está limpo.

Principais conquistas:

- A estratégia `pullback-trend-v1` respeita totalmente a configuração e preenche `market_snapshot` com valores brutos.
- Repositórios de ativos, contextos, sinais, ordens e trades estão implementados e testados.
- O backtest aplica slippage, calcula Sharpe simplificado e pode carregar candles do PostgreSQL.
- O comando `paper` suporta loop contínuo simulado, modo `replay` com candles do banco e persistência completa.
- Comandos `status` e `journal` permitem acompanhar operações e decisões rejeitadas.
- A integração com IBKR via TWS API/IB Gateway foi **validada com conta paper** (2026-08-03): conexão, resumo de conta, posições, ordens abertas, ingest de candles reais e paper trading em modo `live`.

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
- Modos `simulated` (candles sintéticos em memória), `replay` (candles históricos do banco) e `live` (dados e ordens bracket na conta paper da IBKR).
- Persiste sinais, ordens, trades e contextos no PostgreSQL durante o loop.
- Reconciliação simples: não busca novo sinal se já houver posição aberta no mesmo ativo.
- `SimulatedBroker` rejeita nova posição se já existir posição aberta.

### 5. CLI

- `test-connection --provider {ibkr,simulated}`
- `account --provider {ibkr,simulated}`
- `ingest --symbol <s> --timeframe <tf> --days <n> --provider <p>`
- `paper --symbol <s> --strategy <id> --mode {simulated,replay,live} --timeframe <tf>`
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
| Integração real com IBKR | ✅ Validada em paper | Conexão, account summary, posições, ordens abertas e ingest testados contra gateway real (2026-08-03). |
| `IbkrBrokerAdapter::get_open_orders` | ✅ Implementado | Via `client.open_orders()`, consolida status no mesmo stream. |
| `IbkrBrokerAdapter::get_positions` | ✅ Implementado | Via `client.positions()`, ignora zeradas. |
| `IbkrBrokerAdapter::get_account_summary` | ✅ Implementado | Tags NetLiquidation/TotalCashValue/BuyingPower; `daily_pnl` segue zero (exigiria stream `pnl()`). |
| `IbkrBrokerAdapter::subscribe_order_events` | Stub controlado | Exige `Client` persistente no adapter (mudança arquitetural pendente). |
| Paper trading modo `live` | ✅ Implementado | Dados e ordens bracket na conta paper IBKR; smoke test OK. Ordens/trades do live ainda não persistem no banco (depende de eventos de fill). |
| `.env` não era carregado | ✅ Corrigido | `dotenvy` adicionado ao `trader-cli`. |
| Detecção de gaps e qualidade de dados | Parcial | `ingestions` registra `gaps_detected`, mas lógica automática ainda simples. |
| `PortfolioManager` dedicado | Não iniciado | P&L diário e exposição estão no `RiskState`. |
| Alertas de risco/falha | Não iniciado | Apenas logs por enquanto. |
| Exportação de relatório de backtest | Não iniciado | Relatório imprime no terminal; JSON/CSV futuro. |
| Dashboard frontend | Não iniciado | Fase futura (Fase 7). |
| `trader-journal` como crate separado | Não existe | Diário automático é gerado pelo `trader-cli` e persistido em `trades.journal`. |
| Dockerfile da aplicação | Não existe | Apenas `docker-compose.yml` para PostgreSQL. |

---

## Próximos passos recomendados

1. **Sessão longa de paper trading live**
   - Rodar `paper --mode live` por uma sessão completa de mercado para validar estabilidade, reconexão e o envio de ordens bracket em sinal real.
   - Implementar `subscribe_order_events` (requer `Client` persistente no adapter) para persistir ordens/trades do live e rastrear `consecutive_losses`.

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
