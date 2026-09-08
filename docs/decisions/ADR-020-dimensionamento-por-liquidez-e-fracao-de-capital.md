# ADR-020 — Dimensionamento por liquidez e fração de capital

**Status:** proposto / especificado — NÃO implementado (07/09/2026)
**Data:** 2026-09-07
**Fecha:** item 6 do ranking (Tier A) e §5.5 de `docs/cto-plano-lucratividade-2026-09.md`; achado 4 de §2.3 e §2.4
**Contexto anterior:** ADR-017 (limite de risco da conta inteira — soma as posições; esta ADR trata do tamanho de **cada** posição e a complementa), ADR-018 (flatten no backtest) e ADR-019 (PF_R e corr no gate) — pré-requisitos dos modos de sizing (item 6); o cap, a fração e o registro de equity independem deles. A medição de slippage por fill que valida o cap depende do feed de produção íntegro (plano §5.8; sequenciamento §9, semana 2–3)
**Números:** re-simulação dos críticos (06/09/2026) sobre os JSONs de backtest a 2 bp e os candles do banco dev (`validacao-quant-4`, `portfolio-regime-1/2`, `validacao-quant-6`), consolidados no plano-mestre. Nada aqui é backtest novo do motor

---

## Contexto

### O sizing de hoje

`RiskManager::validate` dimensiona assim (`crates/trader-core/src/risk/mod.rs:239-279`):

```text
risk_budget     = capital × risk_per_trade_pct / 100     (linha 239)
qty_by_risk     = trunc(risk_budget / |entrada − stop|)   (241)
qty_by_notional = trunc(capital / entrada)                (253)
position_size   = min(qty_by_risk, qty_by_notional)       (254)
risk_amount     = |entrada − stop| × position_size        (279)
```

O cap de notional é **1× o capital, hardcoded** — a linha 252 carrega o
comentário "Melhoria futura: tornar o multiplicador de cap configurável no
RiskConfig", e `RiskConfig` (`risk/mod.rs:16-35`) não tem campo para isso.

`capital` é a equity que o broker devolve. No live é `NET_LIQUIDATION` da
**conta inteira** (`crates/trader-cli/src/commands/paper.rs:481`;
`crates/trader-adapters/src/ibkr/broker.rs:208`); no backtest, a equity do
`SimulatedBroker` (`crates/trader-backtest/src/engine.rs:186-191`, 100k
iniciais). As 8 instâncias dimensionam sobre a mesma equity. `BUYING_POWER` é
lido ao lado (`ibkr/broker.rs:209`) e nunca entra no sizing — fora do comando
`account`, o único uso é um stub de teste
(`crates/trader-core/src/execution/mod.rs:256`). A equity da conta paper
(≈ US$ 237.892) só existe numa linha de relatório (`docs/HANDOFF.md:215`);
`account_snapshots` existe desde a migração inicial
(`crates/trader-infra/src/db/migrations/0001_initial_schema.sql:278-291`) e
nada a escreve (ADR-014).

### Quatro fatos medidos

**1. O cap prende sempre; o risco real por trade é 0,15–0,32%, não 1%.**
Com stops de 0,1–0,3% do preço, `qty_by_risk` pede várias vezes o capital e o
cap de 1× corta (`notional/capital` mediano 1,05). O $ arriscado vira
proporcional à distância do stop: p10/p50/p90 = US$ 131/231/577 por 100k na
balance-area.

**2. PF em $ ≠ PF em R.** Stop largo ganha (tercis do OOS da balance-area,
92 trades: ≤ 17 bp WR 29% / PF 0,53; 17–28 bp 53% / 1,99; > 28 bp 58% /
3,05), e o sizing "acidental" põe mais dinheiro justamente nesses trades:

| Balance-area OOS (3 pares) | PF em $ | PF em R | corr(risk_amount, R) |
|---|---|---|---|
| pool | 2,09 | 1,41 | 0,31 |
| AVUV | 1,85 | 1,07 | 0,49 |
| VBR | 2,16 | 1,27 | 0,43 |
| IJS | 2,36 | 2,37 | — |

Ex-overnight o pool cai a PF_R 0,99 / avg R −0,006: boa parte da covariância
tamanho × resultado é o overnight que a ADR-018 remove. Range-fade: PF_R 1,78
vs PF$ 2,10 (corr 0,16). Opening-reversal inverte: PF_R 1,28 > PF$ 1,11
(corr −0,23). O gate A em $ não enxerga nada disso.

**3. Liquidez por barra.** Barra mediana de 15m no meio do dia (11h–13h ET,
desde mar/2026): SLYV US$ 290k (p10 77k), IJS 370k, VBR 1,0M, IWN 1,6–1,8M,
AVUV 2,6M. Uma posição a 1× (≈ US$ 238k) é **82% de uma barra mediana em
SLYV e 64% em IJS**. A conta paper enche a NBBO sem fila nem impacto: os
2 bp do backtest nunca foram testados nesse tamanho.

**4. A regra de dinheiro real da ADR-017, como escrita, recusa o cluster — e
o cluster é onde está o edge.** Replay dos 232 trades dos 8 pares vivos
(fev/2025 → set/2026): US$ 24.063 isolados → 19.380 com flatten → **12.286 sob
1 posição / 100% de notional**; a 200% cabem 2 posições e caem 7 trades /
US$ 3.320; o máximo simultâneo em 18 meses foi 3. Ressalva dos críticos
(`portfolio-regime-1`, plano §5.5): esses cortes são do replay com a regra
**como a ADR-017 a descreve** (teto sobre a soma incluindo a posição nova). O
código do live testa só a soma das posições **já abertas** ≥ teto
(`exposure_limit_hit`, `paper.rs:1490-1517`), e como o sizing é
`trunc(equity/preço)`, duas posições somam 199,9% < 200% e a terceira entra —
hoje o que morde é `positions.len() >= 3`. O teto de 200% quase nunca morde;
o cluster passa por acidente, não por desenho.
Na balance-area, dias com ≥ 2 entradas
têm PF 2,69 (avg R +0,44, n=64) contra PF 0,70 (avg R −0,23, n=32) nos dias
de entrada única; fade 4,64 vs 1,46; opening-reversal 1,92 vs 0,96. A 2ª
entrada do dia tem PF 3,36 (balance) e 12,3 (fade) contra 1,52 / 1,57 da 1ª.
Tudo in-sample, mas consistente nas três estratégias.

## Decisão

Seis itens — os quatro primeiros por config, com paridade automática: live, backtest e
walk-forward passam pelo mesmo `build_risk_config`
(`crates/trader-cli/src/risk_config.rs:165-201`; chamado em `backtest.rs:126`,
`paper.rs:140`, `walkforward.rs:93`). Nenhum item aumenta risco após perda —
tudo é estático por config, então "sem martingale" segue valendo.

### 1. `max_notional_multiple` (default 1 = paridade)

`RiskConfig.max_notional_multiple: Decimal`; a linha 253 vira
`qty_by_notional = trunc(capital_ef × max_notional_multiple / entrada)`.
Default 1 para não mudar produção nem os runs existentes. Valores > 1 são
alavancagem intraday: paper only, e sujeitos ao teto de 200% da ADR-017 —
uma instância a 2× ocupa o teto sozinha e "3 posições" vira 1.

### 2. `max_notional_usd` por instância

`RiskConfig.max_notional_usd: Option<Decimal>`, teto absoluto aplicado depois
do multiplicador: `teto = min(capital_ef × multiple, max_notional_usd)`.
Vem do `[risk]` ou, por instância, da env `TRADER__RISK__MAX_NOTIONAL_USD` —
o loader já aceita prefixo `TRADER` com separador `__`
(`crates/trader-infra/src/config/mod.rs:171`) e cada instância tem o próprio
`env_file` (`deploy/home/docker-compose.yml:57-70`). Sem mecanismo novo de
configuração: só o campo e o plumbing em `build_risk_config`.

### 3. Elegibilidade por liquidez

Uma posição só é elegível se `notional ≤ 1/3 da barra mediana de 15m` do
símbolo nos últimos 60 pregões (mediana de `close × volume` das barras RTH;
`candles.volume` existe, `0001_initial_schema.sql:28`). Se o cap de liquidez
for o menor dos tetos, a quantidade é **reduzida** (não rejeitada); se a
redução ficar abaixo de 1 ação, rejeita com `RejectionReason` próprio (ex.:
`NotionalAboveLiquidityCap`) — não reutilizar `InsufficientBuyingPower`
(`crates/trader-domain/src/signals.rs:143`), que é outro fato. Valores
implicados hoje:

| Símbolo | 1/3 da barra mediana | Efeito sobre o 1× atual (≈ 238k) |
|---|---|---|
| SLYV | ≈ US$ 50–90k | **reduz** (era 82% da barra) |
| IJS | ≈ 85–120k | **reduz** (era 64%) |
| VBR | ≈ 180k | reduz |
| AVUV / IWN / IWO | 250–300k | ≈ sem efeito |

O "1/3" é **interpretação nossa** — nenhum livro do projeto dá o número; a
fração fica em config (`max_pct_of_median_bar_notional`) para a varredura.
Paridade: a mediana deve sair da mesma fonte e do mesmo N no live e no
backtest. A janela do live é `days(30)` / 600 barras ≈ 23 pregões
(`paper.rs:966-993`); ou a janela sobe para cobrir 60 pregões, ou os dois
modos leem a mediana do banco. Decisão de implementação — mas fonte e N iguais
são obrigatórios, senão o cap vira uma assimetria nova.

### 4. `capital_fraction` = 1 / `max_concurrent_positions`

`RiskConfig.capital_fraction: Decimal` (default 1). No início do sizing,
`capital_ef = capital × capital_fraction`, o que altera `risk_budget` e
`qty_by_notional` de uma vez. Com 3 posições e fração 1/3, três posições
cheias cabem em 100% de notional — a regra de dinheiro real da ADR-017 sem
recusar o cluster (fato 4). `risk_amount = dist × qty` (linha 279) continua
sendo o risco real, então `result_in_r` e o avg R do gate não mudam de
significado. Falha fechado (padrão do `build_risk_config`): fração fora de
(0, 1] aborta a subida; `capital_fraction × max_concurrent_positions >
max_portfolio_notional_pct / 100` gera aviso na subida.

Junto (item 6 do ranking): a trava de notional da ADR-017 passa a **incluir a
posição prospectiva** — `notional_existente + notional_novo >= teto` em
`exposure_limit_hit` (`paper.rs:1490-1517`), com teste do caso 2 × 99,9%, e a
diferença registrada como nota na ADR-017. Sem isso, com fração 1/3 o teto de
200% vira letra morta e a checagem `>=` sobre as posições existentes segue
deixando a soma passar do teto por uma posição inteira (`portfolio-regime-1`).

### 5. Registrar equity e `BUYING_POWER` por sessão

No `live_started` (`paper.rs:877`) e no flatten, gravar `equity`,
`buying_power` e `cash` em `account_snapshots` (tabela pronta, sem
repositório; não tem coluna de moeda — a moeda da conta vai no `metadata`
JSONB, `0001_initial_schema.sql:288`) e um
`system_events` `account_snapshot` com o mesmo payload (`record_event`,
`paper.rs:740`). Motivo: o número que calibra tudo isto só existe num
relatório; e o painel (ADR-014) passa a ter equity real. `buying_power`
**não** entra no sizing por esta ADR — é candidato a teto duro, não decisão.

### 6. Modos de sizing no harness (ADR-019)

| Modo | `risk_per_trade_pct` | `max_notional_multiple` | O que testa |
|---|---|---|---|
| A | 1,0% | 1× | status quo: cap prende, risco ∝ stop |
| B | 0,15% | 1× | risco uniforme sem alavancagem (iguala para stop ≥ 15 bp) |
| B' | 0,25% | 2× | risco uniforme com alavancagem moderada |
| C | 0,5% | 4× | **só estudo**: exige alavancagem que o teto de 200% bloqueia |

"0,25% / 1×" não é modo: é quase o status quo — com stop mediano de 22 bp
(p10 12) o cap segue prendendo em mais da metade dos trades. Cada modo
reporta a **fração de trades presos no cap** (`position_size < qty_by_risk`,
já detectada na linha 265), e o gate A imprime PF_R ao lado de PF$ e
corr(risk_amount, R). Rodar só sobre o engine com flatten (ADR-018): sem ele
o modo A "vence" por causa do overnight.

## O que esta ADR NÃO é

- **Não promete retorno.** `capital_fraction = 1/3` corta o P&L em $ ≈ 3× por
  trade com PF e avg R invariantes (o DD em % só fica invariante se a base do
  backtest for fracionada junto — ver Consequências): no replay dos críticos,
  ≈ US$ 7,9k
  contra 20.743 (2 posições a 200%, −62%) ou contra 12.286 (1 posição a
  100%, −36%). O que se compra é 3 posições dentro de 100% de notional, os
  7 trades / US$ 3.320 do cluster de volta e o pior dia (16/03/2026, dois
  shorts IWM+IWN a 0,97 de correlação, −US$ 2.032) dividido por 3. É política
  de risco, e é assim que deve ser lida.
- **O cap de liquidez REDUZ o tamanho** em SLYV e IJS — e o P&L em $ dessas
  instâncias na mesma proporção. O ganho esperado (menos slippage real) não
  tem dado próprio: medi-lo faz parte de "como aplicar".
- **O modo C não é operável.** 0,5% de risco com stop de 22 bp pede cap ≥ 4×:
  alavancagem intraday que o teto de 200% bloqueia e que a conta paper não
  valida. Entra no harness como estudo, não como candidato.
- Não é escada de risco, não é Kelly, não é kill-switch (ADR-017 §"O que este
  ADR NÃO resolve") e não muda nenhuma estratégia v1.

## Alternativas rejeitadas

| Alternativa | Por que não |
|---|---|
| Teto por cluster / 1 posição por direção (ranking #30, Tier D) | Destruiria o edge: dias de cluster PF 2,69 vs 0,70; 2ª entrada do dia PF 3,36 vs 1,52 (balance). No replay do designer (`portfolio-regime-2`, in-sample), "máx. 1 na mesma direção" derruba 42 trades / US$ 11.556. Os pares aprovados são o mesmo cluster, então "1 por direção" = "1 posição de cada vez", num gate B que já roda a 0,60 trade/pregão (8 pares vivos; o 0,74 era com 9 pares, com a pullback já desligada). A ADR-017 deve ser lida como "permitir o cluster". |
| ETFs 3× (TNA/TZA) como "stop largo" (#28, Tier D) | O stop em bp triplica, mas o tick também: TNA a US$ 69,9 = 1,43 bp vs IWM 0,42 (3,4×) — custo/R igual ou pior. O "risco real 0,6%" é alavancagem, obtida em IWM com `max_notional_multiple` sem tick pior, sem rebalanceamento diário do produto e sem permissão CLP. A estratégia-base (openrev IWM) reprova o gate A. |
| Escada de risco / Kelly fracionário (#24, Tier C) | f* da balance-area ex-overnight ≈ 0 (−0,003); o 0,137 só existe porque o tamanho covaria com o overnight. PSR da balance 0,938 e DSR < 0,95 em todas as estratégias: pela regra da própria proposta nenhum degrau sobe. Só após gate B com fills reais medidos, e como teto informativo (¼-Kelly com shrinkage), não como número. Grimes (Cap. 8) já descarta Kelly/optimal f como agressivos demais para trades que não são independentes. |
| `buying_power` como único teto | Conta paper com margem: é múltiplo da equity e não diz nada sobre a liquidez do ativo. Fica registrado (item 5) como candidato a teto adicional, não substituto. |
| Tabela nova de "capital por instância" | Mesmo motivo da ADR-017: terceiro lugar guardando o mesmo fato. A fração é config; a equity vem do broker. |

## Consequências e riscos

- **Paridade.** Os campos novos vivem no `RiskConfig` compartilhado;
  `risk_amount` continua dist × qty. A mediana de liquidez é a única peça
  com risco de assimetria (item 3).
- **A conta paper enche a NBBO.** Todo custo medido nela é piso; o cap de
  liquidez não se "valida" em paper — só o slippage real por fill (plano
  §5.8 / §6.5) diz se 1/3 é o número certo.
- **Moeda-base da conta a confirmar.** O cap trata `NET_LIQUIDATION` como
  dólares. Se a conta paper for em CAD, o cap de 1× já está ≈ 1,37× errado
  hoje — conferir com `trader-cli account --provider ibkr` antes de fixar
  qualquer teto em dólares (ação do dono, plano §10.3). O item 5 grava a
  moeda para isso.
- **Fills parciais ficam MENOS prováveis**, não mais: lotes menores enchem
  melhor (US$ 238k em SLYV é 47% da barra média de 11h–14h, US$ 502k, e 82%
  da mediana, US$ 290k; a fração e o cap dividem isso por 3 ou mais). O live
  trata quantidade divergente como anomalia (`ensure_stop_protection`,
  `paper.rs:2283`); o cap reduz a chance de cair nesse caminho.
- **DD% do backtest fica 3× mais leniente se só o sizing mudar.** O
  `max_drawdown_pct` parte de `initial_capital` fixo (100k) e mede o DD sobre
  o pico da curva (`crates/trader-backtest/src/metrics.rs:64-97`); com fração
  1/3 o P&L cai 3× e a base não — o critério de DD do gate A afrouxa sem
  ninguém mudar o critério (plano §5.5, cuidado a). Fracionar o
  `initial_capital` do backtest junto e gravar `capital_fraction` em
  `metrics`.
- **`max_daily_loss_pct` por instância: sobre a conta ou sobre a fatia?**
  Hoje é % do `capital` que a instância vê (`RiskConfig.max_daily_loss_pct`,
  `risk/mod.rs:19`). Com `capital_fraction` a leitura muda de sentido —
  decisão do dono antes de implementar (plano §5.5, cuidado b); esta ADR não
  fixa.
- **`config_hash`.** `risk_per_trade_pct` entra no hash da estratégia
  (override em `risk_config.rs:33-35`); os campos desta ADR ficam no `[risk]`
  da aplicação e não mudam o hash. Os modos A/B/B'/C exigem `--label`
  obrigatório (ADR-019) para não virar baseline do gate B por acidente
  (`analyze.rs:75-78`).
- **Comissão por ação.** Com US$ 0,005/ação e mínimo US$ 1,00 (plano §5.6),
  lotes menores pagam proporcionalmente mais no mínimo — centavos, mas o
  simulador precisa cobrar por ação para o harness enxergar.
- **Medido na entrada.** Como na ADR-017, posição aberta acima do cap (equity
  caiu, liquidez secou) não é reduzida.
- **Gate B.** Trocar sizing em produção não reinicia o relógio do gate B
  (PF/avg R são invariantes ao tamanho), mas muda `$/mês` e o DD em $ dos
  relatórios — anotar a data da troca em `system_events` e no HANDOFF.

## Como aplicar (quando aprovado)

1. `crates/trader-core/src/risk/mod.rs`: campos `max_notional_multiple`,
   `max_notional_usd: Option<Decimal>`, `capital_fraction` e
   `max_pct_of_median_bar_notional: Option<Decimal>` em `RiskConfig` (é
   `Copy`; `Option<Decimal>` também é), defaults 1 / `None` / 1 / `None`
   para que os runs existentes reproduzam. Sizing: `capital_ef = capital ×
   capital_fraction`; `teto = min(capital_ef × multiple, max_notional_usd,
   cap_liquidez)`; `qty_by_notional = trunc(teto / entrada)`; `risk_amount`
   inalterado. Log de **qual** teto prendeu. No backtest, `initial_capital ×
   capital_fraction` como base do `max_drawdown_pct` (`metrics.rs:64-97`) e
   `capital_fraction` gravado em `metrics`.
2. `crates/trader-infra/src/config/mod.rs:74-110` (`RiskSettings`, f64 com
   `serde(default)`), `config/default.toml:26-42` (`[risk]`, comentário
   citando esta ADR), `crates/trader-cli/src/risk_config.rs:165-201`
   (`build_risk_config`, falha fechado: fração ∉ (0, 1], multiple < 1,
   usd ≤ 0, pct ∉ (0, 100]). `StrategyRiskParams` (`risk_config.rs:27-36`,
   9 `impl From`) **não** muda: nada disto é parâmetro de estratégia.
3. Liquidez: função pura `median_bar_notional(candles, n_pregoes)` em
   `trader-core` (padrão de `daily_atr`), consumida pelo `RiskManager`;
   janela do live (`paper.rs:966-993`) revista para o N escolhido, ou leitura
   do banco nos dois modos.
4. `RejectionReason` novo em `crates/trader-domain/src/signals.rs` e no parse
   de `crates/trader-infra/src/repositories/signal_repository.rs:319`.
5. Repositório de `account_snapshots` + chamada no `live_started` e no
   flatten em `paper.rs`; evento `account_snapshot`; moeda no `metadata`.
   `exposure_limit_hit` (`paper.rs:1490-1517`) recebe o notional da posição
   prospectiva; teste puro do caso 2 × 99,9% (a 3ª deve ser recusada por
   notional, não só por contagem); nota na ADR-017.
6. Testes em `risk/mod.rs`, ao lado de
   `risk_amount_reflects_actual_position_risk_under_notional_cap` (linha 408):
   capital 240.000, `capital_fraction` 1/3, entrada 100, stop 99,78 →
   **qty 800, notional 80.000**, `risk_amount` 0,22 × 800 = 176;
   `max_notional_usd` 50.000 com entrada 108 → qty 462; mediana 290.000 com
   fração 1/3 → teto ≈ 96.667 → qty 895 a 108; fração 1,5 → erro na subida.
7. Compose: `TRADER__RISK__MAX_NOTIONAL_USD` no `env_file` das instâncias
   SLYV e IJS (`/data/trader/env/instances/*.env`,
   `deploy/home/docker-compose.yml:57+`; no app umbrelOS, o compose da store)
   com o valor **a medir** nos candles de produção (faixa implicada: SLYV
   50–90k, IJS 85–120k). Fora do pregão, uma instância por vez, com a
   medição de slippage por fill antes/depois (`fills` × `orders` do tipo
   stop por símbolo — plano §5.8; query esboçada em `edge-existente-6`).
8. Harness (ADR-019): `--risk-pct` / `--notional-cap` / `--capital-fraction`
   no backtest e no walkforward; tabela por modo com PF$, PF_R, avg R, DD% e
   fração presa no cap.

## Fontes

- Grimes, Cap. 8 — fração fixa de risco; stop inicial nunca menor que um range
  médio de barra (base do fato 2); Kelly/optimal f descartados
  (`docs/books/analysis/grimes-art-science-ta.md:145-160`). Murphy, Cap. 16 —
  exposição por grupo correlacionado ≤ 20–25% do equity
  (`docs/books/analysis/murphy-technical-analysis.md:216-218`): base do
  `capital_fraction`; aqui o grupo é o livro inteiro e o valor 1/3 é
  **interpretação nossa**, não o do livro. AFML, Cap. 10 — bet sizing
  (`docs/books/analysis/lopez-afml.md:115-119`; base dos modos do harness).
- "1/3 da barra mediana de 15m", "60 pregões" e a grade A/B/B'/C são
  **interpretação nossa**, calibráveis por config e por harness.
- Plano-mestre §2.3 (achado 4), §2.4, §5.5, §6.4, §8; ADR-010, ADR-014,
  ADR-017; `docs/reports/pesquisa-lucratividade-2026-09-07.md` (vereditos).
