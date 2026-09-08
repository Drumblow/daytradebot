# ADR-019 — Harness de validação e gate A estatístico

**Status:** **IMPLEMENTADO** em 07/09/2026, exceto o item 8 (relatório em
Python, `trader-research/`) e o dedupe do item 3 — ver "Ajustes feitos na
implementação" ao final. Números em `docs/reports/gate-a-com-flatten-2026-09-07.md` §7.
**Data:** 2026-09-07
**Fecha:** itens §5.2 e §5.3 (e a nota de §2.6 sobre o walk-forward) de
`docs/cto-plano-lucratividade-2026-09.md`; o "Deflated Sharpe ainda pendente"
de `docs/reports/gate-a-revalidacao-2026-09-04.md:143-145`
**Contexto anterior:** ADR-010 (gate composto), ADR-016 (sensibilidade ao
custo no veredito), ADR-018 (flatten no backtest — pré-requisito: todo número
deste ADR só vale com a régua do live)

---

## Contexto

O gate A (ADR-010) decide com seis números pontuais de `print_acceptance`
(`crates/trader-cli/src/commands/walkforward.rs:168-199`): ≥ 50 trades,
WR ≥ 40%, PF ≥ 1,3, DD ≤ 10%, avg R > 0,15, net > 0. A pesquisa de
06–07/09/2026 mostrou que a ferramenta que produz esses números não responde
às perguntas que decidem um gate:

- **O `walkforward` não exporta nem parametriza.** `Args` não tem `--output`,
  `--slippage-bps` nem `--label` (`commands/walkforward.rs:18-26`: símbolo,
  estratégia, período, timeframe e `windows`); `--output` e `--slippage-bps`
  existem só no `backtest` (`commands/backtest.rs:29,36`); o walk-forward usa
  `BacktestConfig::default()` (`walkforward.rs:86-91`; 2 bp fixos,
  `crates/trader-backtest/src/engine.rs:35-50`) e label fixo (`:156`).
- **Walk-forward por contagem de candles, sem holdout.** `split_windows`
  (`crates/trader-backtest/src/walkforward.rs:46-61`) corta em
  `len / (windows + 1)` candles (1.419 ≈ 55 pregões com 6 janelas), sem
  fronteira de pregão; cada janela roda o prefixo da série e separa IS/OOS por
  `entry_time` (`walkforward.rs:84-101`). Como as estratégias são funções
  puras de (contexto, série) e nada é re-ajustado, o "OOS" é a mesma rodada
  determinística após o primeiro bloco: **mede robustez temporal, não é OOS em
  relação ao desenho das regras** (plano §2.6), calibrado olhando o histórico
  inteiro (balance: teto de ATR 3× → 10×; fade: extensão 0,3 → 0,5) — trials
  que `backtest_runs` não registra.
- **`BacktestMetrics` tem 16 campos pontuais e nenhum de dispersão**
  (`crates/trader-backtest/src/metrics.rs:11-30`): sem PF em R, t-stat, nem
  quebra por `exit_reason`, direção, hora ou dia. O Sharpe
  (`metrics.rs:175-231`) anualiza pelo intervalo mediano da equity — por
  candle de 15 min o anualizador é ≈ 187 e a série inclui gaps overnight: os
  runs full-period das estratégias ativas dão **Sharpe −6 a −9 com PF > 1**
  (60 runs com PF > 1 e Sharpe < −5, mínimo −18,5); os de walk-forward gravam
  Sharpe 0 porque `from_trades` não tem série de equity.
- **`backtest_runs` não sabe a que custo nem com que parâmetros rodou** (o
  jsonb `metrics` só tem as 16 chaves do struct) e o índice é não-único
  (`crates/trader-infra/src/db/migrations/0003_backtest_runs.sql:21`): os runs
  413/414 (walk-forward IJS, 05/09) são cópias idênticas. Medido na auditoria
  de 07/09 pela chave completa do índice proposto: **109 grupos com 279 linhas
  duplicadas em 687 runs**, e **566 runs sem `label`** (as "22 cópias em 9
  grupos" eram um recorte parcial). Como `label` é nulável, um índice único
  simples não impede duplicata em 82% dos runs: a migração precisa de
  `NULLS NOT DISTINCT` ou de `label NOT NULL DEFAULT ''`.
- **A primeira ablação vira baseline do gate B em silêncio.** `analyze` pega
  o backtest de comparação por `latest_by_strategy` (`commands/analyze.rs:75-78`),
  que filtra só por `strategy_id` e devolve o mais recente em qualquer símbolo
  e `config_hash` (`crates/trader-infra/src/repositories/backtest_run_repository.rs:66-91`).
  Um walk-forward com override mantém o `strategy_id` da v1.
- **Os trades do simulador nascem anônimos.** `close_position_to_trade` grava
  `strategy_id`/`config_hash` como `"unknown"` e `journal` como `{}`
  (`crates/trader-adapters/src/simulated/broker.rs:851-868, 893`): o
  `build_bracket_order` só põe `entry_order_type` em `Order.metadata`
  (`crates/trader-core/src/execution/mod.rs:194-196`) e o `market_snapshot`
  do sinal (`crates/trader-domain/src/signals.rs:189`) nunca chega ao trade.
- **Chave errada de parâmetro é ignorada em silêncio.** Nenhum dos 9
  `StrategyParameters` tem `#[serde(deny_unknown_fields)]` (ex.:
  `crates/trader-core/src/strategies/balance_area_breakout_v1/config.rs:25-57`).
- **O N de tentativas não é rastreável.** `backtest_runs` tem 216 trials
  distintos `(strategy_id, asset_id, config_hash)` — balance 30, fade 22,
  openrev 21 — mas as ~150 combinações históricas (42 pares de agosto, +42 na
  value-area, +66 na trendline) e as ~30 re-simulações em Python desta
  pesquisa não têm registro. Qualquer DSR hoje parte de um N chutado.

## Evidência (re-simulação dos críticos, 06/09/2026)

Os críticos da lente `validacao-quant` reconstruíram o OOS dos pares vivos a
2 bp (balance n=92, fade n=64, openrev n=58). Números **sem** flatten salvo
indicação; com o ADR-018 todos mudam e devem ser recalculados.

| Métrica | balance-area-breakout-v1 | range-extreme-fade-v1 | opening-reversal-v1 |
|---|---|---|---|
| PSR (SR* = 0; t-stat, skew, kurtosis) | 0,938 (t 1,52; σ_R 1,70); **0,486 ex-overnight** | 0,984 (t 2,27); 0,961 ex-overnight | 0,820 |
| DSR como faixa [N=30/22 · N=4 · N=2] | 0,12 · 0,56 · 0,81 | 0,007 · 0,36 · 0,82 | — |
| IC95 do PF, iid por trade | [1,24; 3,47] (PF 2,09) | [1,20; 3,90] | [0,63; 1,97] |
| IC95 do PF, bootstrap em blocos (~5 pregões) | **[0,87; 4,91]** | [1,39; 3,57] | — |
| IC95 do PF ex-overnight (analítico / bootstrap) | [0,98; 2,51] / [0,89; 2,65] | [1,14; 3,40] / [1,07; 3,75] | — |
| MC de drawdown, p95 (sizing atual) | 3,8% | 1,9% | 5,3% |
| Share dos 2 melhores meses · meses positivos | 99% (jul/25 +8.781, out/25 +6.985 de +15.957) · 8/13 | 40% · 11/16 | — |
| PF em $ vs PF em R · corr(risk_amount, R) | 2,09 vs **1,41** (AVUV 1,07, VBR 1,27, IJS 2,37) · 0,31 | 2,10 vs 1,78 · 0,16 | 1,11 vs 1,28 · −0,23 |

Leituras que motivam a decisão:

1. **Nenhuma estratégia alcança DSR 0,95 com qualquer N ≥ 2**; com ≤ 100
   trades isso exigiria SR/trade ≈ 0,35 (t ≈ 3,4). DSR/PSR viram relatório,
   não gate (plano §3.5). O designer previa PSR ≈ 0,95 e DSR ≥ 0,95 para a
   balance; a re-simulação dos críticos (06/09/2026) deu 0,938 e ≤ 0,81.
2. **O veredito do IC flipa com o esquema de reamostragem**: a balance passa
   por iid e reprova por blocos, porque 99% do P&L OOS está em dois meses e só
   o bootstrap em blocos enxerga isso. O designer previa [1,3; 3,0]; o crítico
   mediu [0,87; 4,91]. Por isso o esquema é pré-registrado aqui.
3. **PF em $ esconde o PF em R**: o sizing trava no notional
   (`risk/mod.rs:239-279`) e o dinheiro arriscado cresce com a distância do
   stop; como stop largo ganha (tercil ≤ 17 bp: WR 29%, PF 0,53; > 28 bp:
   WR 58%, PF 3,05 — plano §2.2), PF$ 2,09 vira PF_R 1,41. A openrev inverte.
4. **DD p95 de 3,8/1,9/5,3%**: o critério de 10% não morde com o sizing atual;
   só morde com os modos do ADR-020, onde o MC deve ser re-rodado.
5. **CPCV não se aplica**: para regra sem fit os 5 caminhos são permutações dos
   mesmos blocos (o walk-forward já é degenerado: OOS = run completo menos o
   bloco 1). Fica reservado a componente ajustado (plano §7).

## Decisão

Um harness único — Rust no motor e na persistência, Python na estatística.
Nada aqui altera regra de estratégia; v1 continua intacta.

### 1. CLI do `walkforward` (`commands/walkforward.rs`, `main.rs`)

- `--output <arquivo>` com os `oos_trades` (já `pub`,
  `trader-backtest/src/walkforward.rs:36`), métricas por janela e agregadas,
  no formato JSON do `backtest`.
- `--slippage-bps` como `Decimal` (não `Option<u32>` como em `backtest.rs:36`;
  2,5 bp precisa existir), gravado no run.
- `--label` **obrigatório quando houver override**; sem override mantém
  `walkforward-oos-{n}w`.
- `--holdout-from <data ET>`: bloco final travado (padrão: últimos 4–6 meses,
  plano §3.4). Nunca entra em seleção, aborta com `--set`, roda **uma vez por
  família** de hipótese, marcado `holdout = true` no jsonb; rodar de novo
  exige nota no relatório.
- `--no-flatten` só para reproduzir os runs 413–421 (ADR-018).
- `--strategy-config <path>` (plano §5.2): TOML alternativo para a **mesma**
  struct — `load_strategy` casa o id por string e lê
  `config/strategies/<id>.toml` (`dispatch.rs:37-52`), então hoje qualquer
  variante exige match arm; validar `toml.strategy.id == id` e falhar
  fechado. Mesmas obrigações de `--set`: `--label`, `experimental`, contagem.

### 2. `--set chave=valor` com validação

Aplicado em `[strategy.parameters]` do TOML (via `toml::Value`) antes de
`load_strategy` (`crates/trader-cli/src/dispatch.rs:37-52`); o `config_hash`
muda sozinho porque é SHA-256 da config inteira
(`balance_area_breakout_v1/config.rs:62-69`). **Obrigatório falhar** se a
chave não existir: `#[serde(deny_unknown_fields)]` nos 9 `StrategyParameters`
**e** abortar se o hash resultante for igual ao do TOML sem override. `--set`
sem `--label` é erro de CLI, não aviso. Nota: o atributo é a única linha
deste ADR dentro de `strategies/*_v1/config.rs`; é infra de parse, não regra
— não muda a serialização nem o `config_hash`, e um TOML válido hoje continua
válido. Se o dono preferir não tocar nos arquivos v1, a comparação de hash
sozinha atende (plano §5.2 admite as duas formas).

### 3. Persistência e proteção do baseline do gate B

- Runs com override recebem `experimental = true` no jsonb (a alternativa,
  `strategy_id` sufixado, quebraria a contagem de `n_trials` por família).
- `latest_by_strategy` passa a filtrar por `(strategy_id, asset, config_hash
  da estratégia carregada do TOML)` e a ignorar experimentais; `analyze`
  compara por par. Sem run compatível, avisa em vez de pegar outro.
- O jsonb `metrics` ganha `slippage_bps`, `overrides` (chave → valor),
  `session_flatten` (horário ou `null`), `holdout`, `n_trials`, `trial_group`.
- Migração `0005_backtest_runs_unique.sql` (a 0004 é do ADR-018): índice
  único em `(strategy_id, asset_id, config_hash, period_start, period_end,
  label)` precedido de dedupe que mantém o maior `id` (413/414 e as 22
  cópias) e rotula os runs 553–572 (plano §5.6).

### 4. Métricas novas em `metrics.rs`

`Decimal` para dinheiro e R; f64 só nas estatísticas (item 8):

- `profit_factor_r`, `sharpe_r` (SR por trade em R), `t_stat_avg_r`,
  `corr_risk_result` (entre `risk_amount` e `result_in_r`).
- Mapas `(n, PF, PF_R, net, avg R)` por `exit_reason` (inclui `EndOfDay`),
  por direção e por hora ET de entrada.
- **P&L por dia ET**: n de datas com trade, WR por dia, share do top-1 e do
  top-5 dias, net por bloco do walk-forward e por ano civil.
- Sharpe e Sortino sobre **retornos diários** (agregar `daily_pnl_series` por
  data ET, pregões sem trade incluídos, anualizar por √252); o cálculo por
  candle fica como campo legado até a remoção.
- Custo total (comissão + slippage) e **avg R bruto** ao lado do líquido —
  4 bp ida e volta são 18% do R na balance e 19% na fade (plano §2.2) e não
  aparecem em lugar nenhum.
- `entries_triggered` e `entries_cancelled_overshoot` (a guarda de 25% em
  `simulated/broker.rs:245-283` cancela 21,9% das entradas acionadas em IJS e
  24% em IWV — plano §6.7 precisa desse número).

### 5. Journal dos trades do simulador

`Signal.market_snapshot`, `strategy_id`, `strategy_version` e `config_hash`
entram em `Order.metadata` (`execution/mod.rs:194-196`) → `Position.metadata`
→ `Trade.journal` e campos do trade (`simulated/broker.rs:851-893`). O
`paper.rs` já grava isso (`paper.rs:1826`, `1972`); é paridade de dado, não
de regra. Sem isso não há PF por bucket de `stop_distance_atr` sem Python.

### 6. `print_acceptance` e contagem de tentativas

Além dos seis checks, imprimir PF_R ao lado de PF$, t-stat do avg R, share do
melhor dia e dos 2 melhores meses, meses positivos, custo em R e o **N da
família**: runs distintos `(asset_id, config_hash)` da estratégia desde a
adoção do harness **+** o N histórico declarado no `trial_group`. O N
anterior ao harness é estimativa e é impresso como tal.

### 7. Gate A proposto (se aprovado, substitui formalmente a revalidação de 04/09 — decisão 2 do dono, plano §10)

Todos obrigatórios, **com flatten** (ADR-018), a 2 bp, por estratégia:

| Critério | Limiar | Origem |
|---|---|---|
| Os seis atuais (trades, WR, PF, DD, avg R, net) | ADR-010 | mantidos |
| Limite inferior do IC95 do PF por bootstrap em blocos | ≥ 1,0 | critério do designer, esquema pré-registrado pelo crítico validacao-quant-2; plano §5.3 |
| PF em R | ≥ 1,2 | métrica: crítico validacao-quant-4; limiar: plano §5.3, interpretação nossa |
| Share dos 2 melhores meses | ≤ 60% | métrica: crítico validacao-quant-2; limiar: plano §5.3, interpretação nossa |
| Holdout travado | passa nos seis atuais, rodado uma vez | crítico edge-existente-7; plano §3.4 |
| Sensibilidade ao custo | reportar PF a 4–5 bp (ADR-016) | interpretação nossa |

PSR, DSR (faixa), MC de DD e corr(risk, R) são **relatório obrigatório**, não
gate. Sob a hipótese nula, com n=25 por combinação, P(PF ≥ 1,3 | sem edge) =
0,20–0,28 (plano §3.3) — esse número acompanha todo veredito.

### 8. Relatório estatístico em Python (`trader-research/`, novo)

Diretório gerido por `uv` (numpy/pandas) consumindo o `--output`; o que virar
critério é portado para `crates/trader-backtest/src/stats.rs` (inexistente;
`lib.rs:6-9`). Pré-registrado aqui para não virar botão de aprovar:

- **PSR** (t-stat, skew, kurtosis) por estratégia e do pool; **DSR como
  faixa** [N=2 … N registrado], nunca pass/fail.
- **IC95 do PF** por bootstrap estacionário (Politis–Romano) sobre o **P&L
  diário de todos os pregões, zeros incluídos**, blocos médios de 5 e 10
  pregões, 10.000 reamostras, seed fixa; iid por trade ao lado. Reamostrar
  `(R, risk_amount)` juntos.
- **MC de drawdown** reamostrando `(R, risk_amount)` por modo de sizing
  (ADR-020), 10.000 caminhos; p50/p95 em $, % e R.
- **Concentração**: share do melhor dia, dos 2 melhores meses, meses positivos,
  net por bloco e por ano.
- **CPCV: não se aplica** a regras sem fit (Evidência, item 5).

**f64 é permitido aqui.** O `AGENTS.md:47` proíbe f64 para **dinheiro**;
estatística já é f64 no repo (`metrics.rs:226-228`, anualizador) e no parse
de `[risk]`. `stats.rs` recebe R e P&L em `Decimal`, converte para f64 dentro
da função e devolve estatística; nenhum valor monetário volta de f64.

## Consequências

- **Cada ablação custa minutos, não um módulo.** As variantes das v2 (plano
  §6.1–6.3) rodam com `--set` + `--label`, sem TOML novo nem `dispatch.rs`,
  até virarem regra com fonte; só então ganham módulo `_v2` e doc próprio.
- **Ablações contam no N.** Todo run com override entra em `n_trials` e o
  `print_acceptance` mostra o N e o DSR correspondente — o preço da varredura
  barata.
- **O gate B não é contaminado**: `analyze` só compara com o run do mesmo
  `(estratégia, par, config_hash)` e ignora experimentais; o baseline muda
  apenas quando o TOML da produção muda.
- **O gate A de 04/09 fica substituído** pela releitura com flatten, PF_R, IC
  em blocos, holdout e concentração — decisão 2 do dono (plano §10). Pela
  Evidência, a balance-area tende a reprovar e a range-fade a passar; o
  veredito é do motor com o ADR-018, não desta tabela (a medir).
- **Aceite do harness:** reproduzir a tabela de buckets de stop (plano §2.2)
  e os números de flatten de §2.3 com Σ|diff| ≈ 0 contra a re-simulação dos
  críticos; `GROUP BY strategy_id, asset_id, config_hash, period_start,
  period_end, label HAVING count(*) > 1` (a chave do índice único) devolve
  zero linhas após o dedupe.
- Métricas anteriores ao harness (Sharpe por candle, custo não registrado)
  deixam de ser comparáveis — precedente do ADR-015.

## Alternativas rejeitadas

- **DSR ≥ 0,95 como gate binário** (designer): com ≤ 100 trades reprova as
  duas aprovadas com qualquer N ≥ 2 — nem N_eff = 3 chega a 0,95 — e o
  veredito é hipersensível ao conjunto de trials (fade: 0,007 com N=22 contra
  0,82 com N=2), que é indefinível (crítico validacao-quant-1, lentes
  estatística e código). Critério cujo resultado depende de uma escolha do
  analista não serve como gate.
- **Modelo nulo `RandomBracket`** (200 runs por par): maior custo da proposta,
  redundante com pool sem seleção + bootstrap; `rand` não é dependência
  direta de nenhum crate (só transitiva no `Cargo.lock`).
- **CPCV e purge no walk-forward**: inertes para estratégias stateless.
- **`n_trials` só do banco**: subconta por construção; daí o `trial_group`.
- **`--set` sem `deny_unknown_fields`**: é a ferramenta de "afrouxar para
  ganhar amostra" que o framework proíbe após trendline e value-area; a
  proteção tem de estar no código, não na disciplina.

## O que este ADR NÃO resolve

- **Viés de seleção histórico.** O N das ~150 combinações é estimativa
  declarada; nada o corrige de verdade. O único OOS verdadeiro segue sendo o
  paper forward (plano §3.4), com 0 trades das aprovadas desde 18/08.
- **Um regime só.** Bootstrap não inventa regimes que a amostra
  (24/02/2025 → 02/09/2026) não tem.
- **Custo real.** 2 bp é calibração por spread, não por fills; a sensibilidade
  a 4–5 bp é reportada, o custo realizado vem de §5.6/§5.8.
- **Sizing.** PF_R e MC por modo dependem do ADR-020; aqui só se mede.

## Riscos

- **Data-mining facilitado.** `--set` barato multiplica tentativas; a
  mitigação é código, não convenção: `--label` e `n_trials` obrigatórios,
  holdout que aborta com `--set`, `experimental` gravado. Se o N da família
  crescer além do registrado hoje (balance 30, fade 22, openrev 21) sem
  hipótese nova com fonte, o relatório deve dizer isso.
- **N histórico é estimativa** (~150 combinações sem registro estruturado —
  crítico edge-existente-7) e o erro dessa estimativa não é quantificável; o
  DSR é impresso como faixa justamente por isso.
- **Bloco do bootstrap muda o veredito** (Evidência, item 2): 5 e 10 pregões
  ficam fixos por este ADR; outro tamanho é sensibilidade, não critério.
- **Paridade de dado.** O `journal` do simulador passa a ter o que o live já
  grava; se divergirem em chave ou formato, `analyze` compara campos
  diferentes — teste de igualdade de chaves entre um trade de paper e um de
  backtest da mesma estratégia.
- **Migração destrutiva.** O dedupe apaga linhas de `backtest_runs`: dump
  antes, ids removidos no relatório de higiene.

## Como aplicar

1. Depois do ADR-018 (flatten), junto com o hotfix ET da fade (plano §5.4).
2. Ordem: flags do `walkforward` (½ dia) → `--set` com `deny_unknown_fields`
   → `latest_by_strategy` por par + `experimental` → métricas novas → journal
   do simulador → migração + dedupe → `print_acceptance`. Cada passo com
   teste; nenhum altera regra de v1 (o único toque em `strategies/*_v1` é o
   atributo de parse do item 2, sem efeito no `config_hash`).
3. Re-rodar o gate A das três estratégias nos pares vivos com
   `--holdout-from` e `--output`; rodar o relatório Python; registrar em
   `docs/reports/gate-a-<data>.md` com N da família e o resultado sob a
   hipótese nula.
4. Só então medir o delta de contexto Neutral (plano §6.1) e as demais
   ablações — todas com `--label`, todas contadas.
5. Atualizar `docs/strategy-analysis-framework.md` (Fase 7: critérios do
   item 7 e relatório do item 8) e `docs/runbooks/go-live-checklist.md`
   quando o dono aprovar este ADR.

---

## Ajustes feitos na implementação (07/09/2026)

### O que entrou

| Item do ADR | Onde |
|---|---|
| 1. `--output`, `--slippage-bps` (Decimal), `--label`, `--holdout-from`, `--strategy-config` no `walkforward` | `commands/walkforward.rs`, `main.rs` |
| 2. `--set chave=valor` + `#[serde(deny_unknown_fields)]` nos 9 `StrategyParameters` | `strategy_source.rs` (novo), `strategies/*/config.rs` |
| 3. `latest_for(estratégia, par, config_hash)` ignorando experimentais; `slippage_bps`/`session_flatten`/`experimental`/`overrides`/`windows`/`holdout_from` no jsonb `metrics`; índice `0005` | `backtest_run_repository.rs`, `commands/analyze.rs`, migração 0005 |
| 4. `profit_factor_r`, `t_stat_avg_r`, `corr_risk_result`, quebra por direção e por hora **ET**, concentração por dia e por mês, `cost_total` | `metrics.rs` |
| 5. `strategy_id`/`strategy_version`/`config_hash`/`market_snapshot` do sinal chegando ao `Trade.journal` do simulador | `execution/mod.rs`, `simulated/broker.rs` |
| 6. `print_acceptance` com PF_R, concentração, t-stat, corr, long/short e o aviso de amostra | `commands/walkforward.rs` |
| 7. Gate proposto impresso **como proposta**, separado do veredito do ADR-010 | idem |

### Três decisões diferentes do texto

**1. O gate do §7 é impresso, não aplicado.** Os critérios novos (PF_R ≥ 1,2,
2 melhores meses ≤ 60%) aparecem sob a linha
`--- proposta ADR-019 §7 (ainda não é o gate vigente) ---`. Adotá-los
formalmente é a decisão 2 do dono (plano §10) e substitui o ADR-010; até lá o
veredito que vale é o dos seis critérios de cima. Imprimir os dois lado a lado
é o que permite ao dono decidir olhando o efeito real.

**2. O dedupe de `backtest_runs` NÃO foi feito, e a migração explica por quê.**
O ADR pede índice único em
`(strategy_id, asset_id, config_hash, period_start, period_end, label)`
precedido de dedupe. A auditoria de 07/09 mediu o que isso apagaria: dentro do
maior grupo duplicado (24 linhas) há **sete valores distintos de
`final_equity`**. Não são cópias — o `config_hash` cobre só o TOML da
estratégia, e não a versão do motor, o slippage nem a régua de fim de sessão.
Deduplicar por essa chave apagaria resultados diferentes entre si e destruiria
a evidência de que o motor mudou. A migração `0005` cria só o **índice de
busca**; o único e o dedupe ficam para quando a identidade do run for
recuperável (os runs a partir daqui já gravam `slippage_bps` e
`session_flatten`) **e** o dono autorizar apagar linhas.

**3. `--set` não confia no parser de TOML para tipar o valor.** TOML tem
literal de hora: `x = 11:45:00` faz parse como `Datetime`. Todos os campos de
horário das estratégias são `String`, então o valor tipado quebraria o parse
com uma mensagem incompreensível — justamente em `trading_end_time`, o
override que as v2 de janela horária (§6.10) vão usar. Só inteiro, float,
booleano e string entre aspas são tipados; o resto vira string. Coberto por
teste.

### As travas contra "ablação barata vira baseline"

Verificadas com dado real em 07/09:

- `--set` sem `--label` → aborta.
- `--set` numa chave inexistente → aborta no parse, listando as chaves válidas.
- `--set` com o mesmo valor do arquivo → aborta ("o `config_hash` continua …").
- `--set` junto com `--holdout-from` → aborta.
- `--holdout-from` com data inválida → **erro**, não holdout desligado em
  silêncio (o padrão `.ok()` de `--from`/`--to` faria isso).
- TOML de `--strategy-config` cujo `strategy.id` não bate com `--strategy` →
  aborta (o `dispatch` casa pelo argumento, não pelo arquivo).
- Uma ablação real (`--set target_r_multiple=3`, run 741) ficou como o run
  **mais recente** da estratégia e o `analyze` continuou escolhendo o run 740,
  o de produção. Sob o código antigo ela teria virado o baseline.

### O que o PF em R mostrou, e não era esperado nesta intensidade

Gate A OOS com flatten, hotfix ET e o harness (runs `gate-a-adr019`):

| Estratégia · par | PF em $ | **PF em R** | 2 melhores meses | t-stat | corr(risco, R) |
|---|---|---|---|---|---|
| balance · IJS | 2,54 | 1,91 | 90% | 1,42 | 0,13 |
| balance · VBR | 1,51 | **0,97** | 113% | −0,07 | 0,27 |
| balance · AVUV | 1,43 | **0,74** | 115% | **−0,80** | **0,57** |
| fade · AVUV | 1,47 | 1,37 | 128% | 0,75 | 0,10 |
| fade · SLYV | 2,64 | 2,34 | **59%** | 1,86 | 0,04 |
| openrev · IWM | 1,72 | 2,03 | 110% | 1,88 | −0,28 |
| openrev · IWN | 1,56 | 1,56 | 74% | 1,08 | −0,02 |

Leituras:

1. **A balance-area em AVUV e VBR tem PF em R abaixo de 1** — ou seja, em
   unidades de risco ela perde. O PF em dólares acima de 1 vem da correlação
   entre tamanho e resultado (0,57 em AVUV, a mais alta do conjunto), que é o
   cap de notional produzindo tamanho maior justamente nos trades de stop
   largo. Isto reforça, por um segundo caminho independente, o veredito do
   ADR-018.
2. **Só a fade em SLYV passa no critério de concentração.** Todos os outros
   ficam entre 74% e 128% (acima de 100% significa que os demais meses somam
   negativo).
3. **Nenhum t-stat chega a 2.** O de AVUV é negativo.
4. A openrev **inverte** (PF_R > PF$, corr negativa), como a re-simulação
   previa.

### Pendente

- **Item 8 — relatório estatístico em Python (`trader-research/`).** PSR, DSR
  como faixa, IC95 por bootstrap estacionário em blocos, MC de drawdown. É o
  §5.3 do plano e agora é possível, porque o `--output` do walk-forward passou
  a existir. Sem ele, o critério "limite inferior do IC95 em blocos ≥ 1,0" do
  §7 não tem como ser avaliado.
- **`n_trials` / `trial_group`** não são gravados: o N da família continua
  sendo estimativa declarada em relatório, como o próprio ADR admite.
- **Custo por ativo** (comissão por ação, spread no alvo) é o §5.6 do plano e
  não entrou aqui; todos os números acima usam US$ 0,35/perna.
