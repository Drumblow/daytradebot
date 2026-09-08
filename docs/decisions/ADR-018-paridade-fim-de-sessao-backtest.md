# ADR-018 — Paridade de fim de sessão no backtest (`ExitReason::EndOfDay`)

**Status:** **IMPLEMENTADO** em 07/09/2026. Veredito e números do motor em
`docs/reports/gate-a-com-flatten-2026-09-07.md` (runs OOS 725–732). **Quatro**
decisões de implementação divergem do texto original — ver
"Ajustes feitos na implementação" ao final.
**Data:** 2026-09-07
**Fecha:** achado 1 de §2.3 e item §5.1 de `docs/cto-plano-lucratividade-2026-09.md`
**Contexto anterior:** ADR-010 (gate de go-live), ADR-015 (guarda de overshoot —
precedente de "backtests anteriores não são comparáveis"), ADR-017 (limite da
conta; a mesma auditoria, item C1, trouxe o flatten ao live)

---

## Contexto

O live encerra tudo no fim do pregão; o backtest não. Os dois mundos divergem
exatamente onde o AGENTS.md exige paridade (`live == backtest`):

- **Live:** a cada tick entre 15h55 e 16h10 ET (`paper.rs:2189-2198`,
  `in_flatten_window`; chamada em `paper.rs:925-930`), `flatten_session`
  (`paper.rs:2208-2270`) cancela a entrada stop pendente e fecha a posição a
  mercado. Motivo (C1 da auditoria de 30/08): as pernas do bracket vão com TIF
  Day e expiram no sino — uma posição que atravessa a noite fica sem stop.
  Está em produção desde o commit `33bf831`.
- **Backtest:** o laço de `BacktestEngine::run`
  (`crates/trader-backtest/src/engine.rs:136-240`) só chama
  `set_market_candle`, `evaluate_time_exit` e o sync de trades fechados. Não
  há fim de sessão; a posição fica aberta até o stop ou o alvo encherem, dias
  depois se preciso. A saída por tempo está desligada nas três estratégias
  ativas (`crates/trader-cli/src/dispatch.rs:129-141`), então nada encerra.
- **Domínio:** `ExitReason` (`crates/trader-domain/src/trades.rs:106-112`) tem
  cinco variantes (`Target`, `Stop`, `Time`, `Manual`, `RiskManager`). Por
  falta de variante própria, o live grava o flatten como `Manual` com
  `journal.forced_exit = "session_flatten"` (`paper.rs:1783-1787`, `1827-1831`).
- **Banco:** o CHECK de `trades.exit_reason`
  (`crates/trader-infra/src/db/migrations/0001_initial_schema.sql:235`) só
  aceita os cinco textos atuais; `trade_repository.rs:25-31` tem `match` exaustivo apenas na
  **escrita** (quebra a compilação); a leitura (`:325-331`) termina em
  `_ => Target` e falha em SILÊNCIO — o braço `"end_of_day"` é obrigatório e
  precisa de teste de round-trip Trade→DB→Trade, porque nada mais o pega. As migrations rodam no boot
  (`crates/trader-infra/src/db/mod.rs:52`, `sqlx::migrate!`).
- **Dados:** os candles do banco são RTH-only — 26 barras de 15 min por dia,
  09h30 a 15h45 ET, sem pré/pós-mercado (verificado por SQL pelos críticos em
  06/09/2026), **exceto nos pregões de meio expediente**: 4 dias da amostra
  (03/07/2025, 28/11/2025, 24/12/2025, 07/08/2026) têm 14 barras terminando às
  12:45 ET. Por isso o critério de última barra é APENAS "a próxima barra é de
  outra data ET"; `last_bar` fica só como checagem de sanidade. Nesses dias o
  flatten do LIVE (janela fixa 15:55–16:10 ET, `paper.rs:2189-2197`) roda com o
  mercado fechado e a posição atravessa a noite sem stop — buraco que este ADR
  não fecha.

Consequência: o gate A de 04/09 (`docs/reports/gate-a-revalidacao-2026-09-04.md`)
e o gate B em curso comparam o live com um backtest que ganha dinheiro
dormindo posicionado — um edge que o live, por construção, nunca captura.

## Evidência

Re-simulação dos críticos (06/09/2026): três críticos independentes
reproduziram os JSONs de backtest do binário de 06/09 ao centavo (Σ|diff| = 0
no baseline da balance-area) e fecharam cada posição aberta no close da barra
15h45 ET do dia da entrada, com 2 bp de slippage. In-sample, pares vivos:

| Estratégia (2 bp) | Sem flatten | Com flatten | Overnight |
|---|---|---|---|
| `balance-area-breakout-v1` IJS+VBR+AVUV (96 t) | PF 1,92 · avgR 0,214 · +14.648 | **PF 1,55 · WR 42,7% · avgR −0,007 · +6.730** | 20 t = +9.577 (65% do net) |
| — por par (PF / avgR com flatten) | 1,95 / 1,96 / 1,85 | IJS 1,86 / 0,269 · VBR 1,47 / −0,035 · AVUV 1,43 / −0,174 | |
| `range-extreme-fade-v1` AVUV+SLYV+IWV (69 t) | PF 1,88 · avgR 0,297 · +6.015 | PF 1,74 · avgR 0,218 · +4.578 | 9 t = +1.933 (32%); IWV 0 |
| `opening-reversal-v1` IWM+IWN (67 t) | PF 1,23 · +3.400 | **PF 1,74 · +8.072** | 8 t = −2.722 (6 stopados no dia seguinte) |

Detalhes que mudam a leitura:

- Os 20 overnight da balance-area são **simétricos**: 10 long / 10 short, 14
  alvos / 6 stops (7/3 em cada lado; long +4.589, short +4.987). Não é beta do
  bull; sugere edge multi-dia (ver "O que este ADR NÃO resolve"). Fechados às
  15h45 eles rendem +1.658 em vez de +9.577.
- "Descartar" os 20 trades (o PF 1,43 / avgR −0,048 do mapa) não é o que o
  live faz; fechá-los dá PF 1,55 / avgR −0,007. É o número deste ADR.
- Mesmo com flatten, a balance viva depende de um dia: 10/10/2025 = +4.430 de
  +6.730. Em 7 blocos, o bloco 7 (jun–set/2026) dá −3.203 (viva) e −7.145 no
  pool de 8 símbolos (pool: PF 1,75 → 1,49, avgR −0,014 com flatten).
- Range-fade: os 9 overnight são 5 longs (todos no alvo) e 4 shorts (3 stops);
  com flatten passa o gate A, mas com t ≈ 1,7 in-sample.

## Decisão

Replicar o flatten no motor, com nome próprio para a saída, horário vindo de
config compartilhada e nada de novo bloqueando sinal. Nenhum arquivo de
estratégia v1 muda; é mudança de motor (por isso ADR).

1. **Domínio.** `ExitReason::EndOfDay` (serde `end_of_day`) em
   `trades.rs:106-112` + teste de serde. Braços novos nos dois `match` de
   `trade_repository.rs` e nos demais usos de `ExitReason::` (simulated
   broker, engine, metrics, paper, trade_tracker).
2. **Migração `0004_exit_reason_end_of_day.sql`.** Amplia o CHECK de
   `trades.exit_reason` com `'end_of_day'`. Reclassificação dos trades já
   gravados em produção (`exit_reason = 'manual'` com
   `journal->>'forced_exit' = 'session_flatten'`) — no `analyze`, lendo o
   journal (re-simulação dos críticos, 06/09/2026); reescrever linhas de
   produção na própria migração é alternativa que depende de decisão do dono.
   Sem uma das duas o gate B mistura categorias.
3. **Engine.** `BacktestConfig.session_flatten_et: Option<(u32, u32)>`
   (`engine.rs:18-32`), default `Some((15, 45))` = última barra RTH. A barra é
   a última do dia quando **a próxima barra é de outra data ET** ou a hora ET
   de abertura é ≥ 15:45 — via `trader_core::session::et_time`
   (`crates/trader-core/src/session.rs:18`; `trader-backtest` não depende de
   `chrono-tz`, só de `chrono`). Nessa barra, depois de `set_market_candle`
   (stop e alvo avaliados no range inteiro da barra, como hoje) e de
   `evaluate_time_exit`: (a) cancelar a entrada pendente — pelo
   `Broker::cancel_order` com o id da ordem pendente, que no simulado já limpa
   `pending_entries` (`crates/trader-adapters/src/simulated/broker.rs:655-677`);
   não existe cancelamento por símbolo hoje, criar um auxiliar é decisão de
   implementação; (b) se há posição,
   `broker.close_position_at_market(&symbol, candle.close, ExitReason::EndOfDay)`
   (`crates/trader-adapters/src/simulated/broker.rs:425-467`, que já aplica o
   slippage de execução a mercado contra o trader — modela o MKT das 15h55).
   O padrão de chamada é o de `evaluate_time_exit` (`engine.rs:280-316`).

   ```rust
   // esboço — não implementado
   if self.is_last_bar_of_et_day(candles, idx) {
       // cancelamento via Broker::cancel_order(id da entrada pendente)
       self.cancel_pending_entry(&symbol).await;
       self.broker.close_position_at_market(&symbol, candle.close, ExitReason::EndOfDay);
   }
   ```

   **Não bloquear sinal na última barra.** O live coloca a entrada às 15h45
   (sinal da barra 15h30) e só a cancela às 15h55; a entrada pode encher nesse
   intervalo. O backtest deixa a entrada encher na barra 15h45 e fecha no
   close — paridade. (Nas três ativas, `trading_end_time ≤ 15:30` já impede
   sinal na própria barra 15h45.)
4. **Config compartilhada.** Seção `[session]` em `config/default.toml` (hoje
   inexistente; seções: app, database, broker, ibkr, risk, alerts, logging)
   com `flatten_start = "15:55:00"`, `flatten_end = "16:10:00"` e
   `last_bar = "15:45:00"`, em horário de NY, lidas com `parse_et_time`.
   `paper.rs` troca as constantes `FLATTEN_START_MINUTES`/`FLATTEN_END_MINUTES`
   pela config; o engine lê `last_bar`. Paridade por config, não por
   coincidência. Flag `--no-flatten` em `backtest` e `walkforward`
   (`session_flatten_et = None`) para reproduzir os runs 413–421 e medir o
   delta por par.
5. **Live.** `paper.rs:1783-1787` e `1827-1831`: `EndOfDay` em vez de
   `Manual`; o journal `forced_exit: session_flatten` permanece.
   `trade_tracker.rs` continua classificando saída fora do plano como `Manual`.
6. **Métricas.** `BacktestMetrics` (`metrics.rs:11-30`) ganha contagem, net e
   PF por `exit_reason`; o report imprime a linha `end_of_day`.
7. **Testes.** Posição aberta na barra 15h45 fecha como `EndOfDay`; trade que
   abre e fecha no mesmo dia não muda em nada; entrada pendente é cancelada na
   última barra; `--no-flatten` reproduz o baseline. Os testes atuais com
   candles sintéticos em horários arbitrários (`engine.rs:511-537`, 30 barras
   a partir de 14h30 UTC, que cruzam 15h45 ET) passam a flattenar e precisam
   de revisão.

## Consequências

- **A `balance-area-breakout-v1` reprova o gate A** (ADR-010: ≥ 50 trades,
  WR ≥ 40%, PF ≥ 1,3, DD ≤ 10%, avg R > 0,15) **pelo avg R**, não pelo PF;
  só IJS passa sozinha. A range-fade continua passando; a opening-reversal
  sobe para PF ~1,7 em IWM/IWN. Recomendação ao dono (plano §10.1, decisão
  pendente): o gate B da balance continua em paper, mas lido contra o
  backtest com flatten, sem contar overnight como edge.
- **O gate A de 04/09 fica formalmente substituído** pela re-rodada com
  flatten (mesmo precedente do ADR-015 §4; plano §10.2, decisão pendente do
  dono). Nenhum run anterior a este ADR é comparável com os novos.
- **O gate B passa a ser lido contra o backtest com flatten.** `analyze`
  (`crates/trader-cli/src/commands/analyze.rs:46-50`) compara WR/PF/avgR sem
  filtrar por `exit_reason` e usa `latest_by_strategy` (`analyze.rs:75-78`):
  a re-rodada precisa acontecer antes de qualquer outro run, senão o baseline
  vira o run errado (o ADR-019 fecha isso por `config_hash` e label).
- **Assimetrias registradas, não corrigidas:** o backtest sai no close da
  barra 15h45 (o print das 16h00, o momento mais líquido do dia); o live sai a
  mercado às 15h55, dentro dessa barra — 10 min e tipo de fill diferentes. O
  slippage real do flatten em produção é **a medir**: por trade `end_of_day`,
  comparar o preço do fill MKT com o close da barra 15h45 em `candles`.
  Sensibilidade: rodar também com `--slippage-bps 4` em IJS/SLYV, onde o
  notional (~US$ 238k) é 64–82% da barra mediana de meio-dia (plano §2.4); o
  replay de portfólio (plano §6.4) deve precificar o flatten no open da barra
  15h45 ou no close da 15h30, não no print das 16h00.
- O `config_hash` das estratégias não muda (a config é global); até o
  ADR-019 gravar `session_flatten` no `metrics` jsonb, runs com e sem flatten
  só se distinguem pela label e pela data — rotular todos.

## Alternativas rejeitadas

- **MOC no live em grupo OCA com as pernas stop/alvo** (proposta
  `execucao-custo-3`, metade "live"). Os 7 ETFs operados são NYSE Arca; a
  bolsa recebe MOC até 15h50 e **não permite cancelar após 15h45** (Nasdaq:
  15h55/15h50). Uma MOC enviada às 15h49 é irrevogável: se o stop ou o alvo
  encher entre 15h49 e 16h00, a MOC executa no leilão e **abre posição
  invertida** — a família do incidente de 03/09
  (`docs/reports/incidente-2026-09-03-ordens-duplicadas.md`: três vendas
  enfileiradas, −1.654 ações). O adapter não tem `oca_group` (grep vazio em
  `ibkr/broker.rs`; o bracket é ligado só por `parent_id`,
  `submit_bracket_and_confirm`, `ibkr/broker.rs:666-672`) e o trait `Broker`
  (`crates/trader-domain/src/ports.rs:44-56`) não tem modify. A variante
  "cancelar as pernas às 15h49 e mandar MOC" deixa a posição 10–15 min sem
  stop — viola "sempre stop server-side". Ganho estimado: 0,5–1 bp em ~20% dos
  trades de uma estratégia (≈ US$ 30–60/ano por instância). Manter MKT às
  15h55 e modelar os 2 bp no backtest.
- **Descartar os trades overnight** em vez de fechá-los: subestima (PF 1,43)
  e não é o que o live faz.
- **Bloquear sinal/entrada na última barra:** quebraria a paridade descrita
  na decisão 3.
- **Usar `TimeExitConfig` como fim de sessão:** a saída por tempo é validação
  em R pós-entrada, desligada nas v1 por doc (§6 de cada estratégia); ligá-la
  mudaria regra de v1.

## O que este ADR NÃO resolve

- **O edge multi-dia da balance-area.** Os 20 overnight simétricos (14 alvos /
  6 stops) são hipótese para uma `balance-area-breakout` swing v2 na Onda C
  (plano §7: GTC, hold ≤ 1 noite, ≥ 50 overnight em ≥ 5 pares, pré-mercado
  cego, ADR-018-bis) — nunca ajuste da v1.
- **Conta compartilhada** (3 posições, 200% de notional — ADR-017): o backtest
  segue single-symbol; o replay de portfólio é o §6.4 do plano.
- **Liquidez do fill** em IJS/SLYV (ADR-020) e **feed esparso de produção**
  (plano §5.8): o flatten a 2 bp continua um piso nesses pares.

## Riscos

- Re-rodar antes da reclassificação dos trades `Manual/session_flatten`
  deixa o `analyze` misturando categorias no gate B.
- Entradas com `entry_validity_candles` que sobrevivem à última barra são
  canceladas — mesmo comportamento do `flatten_session`, mas muda a contagem
  de entradas vivas nos runs antigos; medir com `--no-flatten`.
- O rollover diário do engine usa a data UTC (`engine.rs:139-144`); com
  candles RTH-only (13h30–19h45 UTC no horário de verão, 14h30–20h45 fora
  dele) as datas UTC e ET coincidem, então não altera resultado —
  interpretação nossa, a confirmar no teste de paridade.

## Como aplicar

```bash
# re-rodada do gate A das 3 estratégias, 8 pares, com flatten (default)
trader-cli walkforward --symbol IJS  --strategy balance-area-breakout-v1 --from 2025-02-24 --to 2026-09-03 -w 6
trader-cli walkforward --symbol VBR  --strategy balance-area-breakout-v1 --from 2025-02-24 --to 2026-09-03 -w 6
trader-cli walkforward --symbol AVUV --strategy balance-area-breakout-v1 --from 2025-02-24 --to 2026-09-03 -w 6
trader-cli walkforward --symbol AVUV --strategy range-extreme-fade-v1    --from 2025-02-24 --to 2026-09-03 -w 6
trader-cli walkforward --symbol SLYV --strategy range-extreme-fade-v1    --from 2025-02-24 --to 2026-09-03 -w 6
trader-cli walkforward --symbol IWV  --strategy range-extreme-fade-v1    --from 2025-02-24 --to 2026-09-03 -w 6
trader-cli walkforward --symbol IWM  --strategy opening-reversal-v1      --from 2025-02-24 --to 2026-09-03 -w 6
trader-cli walkforward --symbol IWN  --strategy opening-reversal-v1      --from 2025-02-24 --to 2026-09-03 -w 6
# delta por par: mesmos comandos com --no-flatten devem reproduzir os runs 413-421
# contagem de saídas end_of_day por par (esperado ~20% na balance-area):
trader-cli backtest --symbol IJS --strategy balance-area-breakout-v1 --from 2025-02-24 --to 2026-09-03 --slippage-bps 2 -o bab_IJS_eod.json
```

Comparar com os runs 413–421 (label `walkforward-oos-6w`, 05/09/2026; 414 é
duplicata de 413, mesmo `config_hash`). O walkforward grava label fixa
(`walkforward.rs:156`), então a comparação é por `created_at` até o ADR-019
trazer `--label`. Esperado: balance agregada PF ~1,5 e avgR ~0 (reprova; IJS
passa sozinha), range-fade PF ~1,7 e avgR ~0,2 (passa), opening-reversal PF
~1,7 em IWM/IWN. Registrar o veredito em novo relatório de gate A antes de
qualquer v2 e atualizar `docs/HANDOFF.md`.

---

## Ajustes feitos na implementação (07/09/2026)

O ADR foi seguido à risca com **quatro** exceções. Três mudam comportamento
observável; a terceira (cancelar a entrada colocada na própria última barra) é
**inerte com as estratégias de hoje** — todas têm `trading_end_time` ≤ 15h30 e
nunca geram sinal na barra 15h45 — e está registrada porque deixa de ser inerte
no instante em que alguma estratégia estender a janela.

### 1. O gatilho do flatten é SÓ a mudança de data ET

A seção "Decisão", item 3, dizia "a próxima barra é de outra data ET **ou** a
hora ET de abertura é ≥ 15:45". A seção "Contexto" do mesmo ADR já corrigia
isso ("o critério de última barra é APENAS 'a próxima barra é de outra data
ET'; `last_bar` fica só como checagem de sanidade") — e é essa a versão
implementada, por dois motivos medidos:

- O `ou hora ≥ 15:45` é redundante em dado RTH-only (a barra 15h45 já é a
  última) e **inerte** nos 4 pregões de meio expediente, onde o dia acaba às
  12h45.
- Numa série que contenha barras pós-RTH, ele dispara em **toda** barra a
  partir das 15h45, não só na última — encerrando qualquer posição aberta na
  hora e destruindo os testes sintéticos do motor.

`session_flatten_et` continua existindo com default `Some((15, 45))`: liga o
flatten e serve de checagem de sanidade. Se o pregão terminar depois desse
horário, o motor emite `warn!` — sinal de que a série não é RTH-only e o
flatten pode estar caindo na barra errada. `None` (`--no-flatten`) desliga.

### 2. O fim da SÉRIE não é fim de pregão

O esboço tratava "não há próxima barra" como sino. Isso quebra o walk-forward:
`run_walk_forward` roda cada janela sobre um **prefixo**
(`&candles[..test_range.end]`, `walkforward.rs`) que termina num índice
arbitrário, quase sempre no meio de um pregão. Cada fronteira de janela geraria
um `EndOfDay` fantasma — um trade a mais nas métricas do gate, inexistente no
run completo.

Implementado como `None => false`, com teste de regressão
(`fim_da_serie_no_meio_do_pregao_nao_e_sino`). O preço é que uma posição ainda
aberta na última barra da série nunca fecha e não entra em `closed_trades` —
que é exatamente o que a re-simulação dos críticos faz, e por isso os números
continuam comparáveis (verificado: contagem de trades idêntica, 96/69/67).

### 3. A entrada colocada NA última barra também é cancelada

O ADR insiste em "não bloquear sinal na última barra", e isso foi respeitado: o
sinal é gerado, logado e contado. Mas a **ordem** que ele cria é cancelada no
fim da iteração, porque no live ela vai com TIF Day e morre no sino. Sem isso
ela atravessaria a noite e encheria na abertura do dia seguinte, num preço que
o live nunca veria. São dois pontos de chamada no laço
(`flatten_session` após `evaluate_time_exit`; `cancel_pending_entry` no fim da
iteração), ambos comentados no código.

**Nota de precisão (corrigida em 08/09/2026 por uma revisão adversarial).** Uma
primeira versão desta seção afirmava que a entrada colocada na última barra
"nunca pode encher" no motor, e que isso seria menos permissivo que o live. As
duas metades estavam erradas:

- A ordem de que o §5.1 do plano fala é a gerada pela barra que **fecha** às
  15h45 (a barra 15h30). Ela é colocada no fim da iteração da 15h30 e ganha a
  barra 15h45 inteira: `set_market_candle` roda **antes** do `flatten_session`
  no laço, e é ele que avalia a entrada stop pendente contra o high/low. Ou
  seja, ela **pode** encher — e tem 15 min de janela contra os ~10 min do live.
  O motor é ligeiramente **mais** permissivo, não menos.
- No live não existe ordem "colocada na barra 15h45": o loop só trata a barra
  como fechada em `timestamp + timeframe`, então a barra 15h45 só seria
  analisada às 16h00, depois do flatten. E as três estratégias ativas têm
  `trading_end_time` ≤ 15h30, checado no timestamp de **abertura** da barra —
  nenhuma gera sinal na barra 15h45.

O `cancel_pending_entry` do fim da iteração mata apenas a ordem gerada **na
própria** última barra, que hoje nenhuma estratégia produz. A assimetria real
que resta é a de 15 vs 10 minutos de janela de fill, já da mesma família da
assimetria de 10 min do fill de saída listada em "Consequências". Fechá-la
exigiria dado intrabar, que o backtest não tem.

### 4. Reclassificação dos trades antigos: não feita na migração

O ADR deixava a escolha entre reclassificar no `analyze` ou reescrever as
linhas na migração, esta segunda dependendo de decisão do dono. Implementado o
**primeiro**: `Trade::effective_exit_reason()`
(`crates/trader-domain/src/trades.rs`) reconhece o flatten antigo pela marca
`journal.forced_exit = "session_flatten"` e o devolve como `EndOfDay`; as
métricas por motivo de saída usam esse método. A migração `0004` só amplia o
CHECK — **não escreve em trade nenhum**. O UPDATE opcional ficou em
`sql/maintenance/0004-reclassificar-flatten.sql`, com `ROLLBACK` no fim,
para quando o dono autorizar.

O `DROP CONSTRAINT` da migração é **por busca**, não por nome: o CHECK da
migração 0001 é anônimo, e derrubar pelo nome implícito falharia em silêncio
num ambiente que divergisse — o `ADD` criaria o novo, o antigo continuaria
valendo e o primeiro INSERT de `end_of_day` quebraria só em produção
(SQLSTATE 23514). Nome confirmado em dev (`trades_exit_reason_check`), mas o
DDL não depende disso.

### Onde o código ficou

| Arquivo | O quê |
|---|---|
| `crates/trader-domain/src/trades.rs` | `ExitReason::EndOfDay`, `ExitReason::as_str` (tabela única), `Trade::effective_exit_reason`, testes de serde e de journal legado |
| `crates/trader-core/src/session.rs` | `et_date` + teste da virada de data UTC × ET |
| `crates/trader-backtest/src/engine.rs` | `BacktestConfig.session_flatten_et`, `is_last_bar_of_session`, `flatten_session`, `cancel_pending_entry` + 5 testes |
| `crates/trader-backtest/src/metrics.rs` | `GroupMetrics`, `BacktestMetrics.by_exit_reason` + 2 testes |
| `crates/trader-backtest/src/report.rs` | tabela "Saídas por motivo" |
| `crates/trader-adapters/src/simulated/broker.rs` | `pending_entry_order_id` |
| `crates/trader-infra/src/config/mod.rs` | `SessionSettings` (`[session]`) |
| `crates/trader-infra/src/db/migrations/0004_exit_reason_end_of_day.sql` | CHECK ampliado |
| `crates/trader-infra/src/repositories/trade_repository.rs` | braço `end_of_day` na leitura; escrita via `as_str` |
| `crates/trader-cli/src/commands/paper.rs` | `EndOfDay` no live; janela de flatten vinda da config |
| `crates/trader-cli/src/commands/{mod,backtest,walkforward}.rs`, `main.rs` | `--no-flatten` |
| `config/default.toml` | seção `[session]` |

253 testes no workspace (241 antes), `cargo clippy --all-targets -- -D warnings` limpo.

### Pendências que este ADR abre e não fecha

- **Meio expediente no LIVE.** A janela de flatten do live é fixa
  (15h55–16h10 ET); nos 4 pregões de meio expediente ela roda com o mercado
  fechado e a posição atravessa a noite sem stop. O backtest agora trata esses
  dias corretamente; o live **não**. Buraco conhecido, fora do escopo deste ADR
  — precisa do calendário de pregão (candidato a §6.6).
- **`flatten_triggered` só em memória** (`LiveFillState`): restart entre o
  fechamento a mercado e a chegada do fill perde a marcação e o trade cai em
  `classify_exit_reason` sem o journal.
- **Duas regras de "dia" no mesmo laço**: o rollover de risco do motor usa
  data **UTC** (`engine.rs`), o flatten usa `et_date`. Coincidem em dado
  RTH-only; divergiriam em qualquer barra fora do RTH.
