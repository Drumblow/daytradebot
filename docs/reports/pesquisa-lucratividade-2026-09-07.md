# Pesquisa de lucratividade — 41 propostas, 16 críticas e o que ficou de pé (06–07/09/2026)

**Por que existe:** o dono pediu "documentação de implementação de novas técnicas e
estratégias com potencial maior de lucro — com liberdade para pensar em outros ativos ou até
criptomoedas". A resposta curta está no plano-mestre
(`docs/cto-plano-lucratividade-2026-09.md`, fonte de verdade para tiers, números e
sequenciamento). Este relatório é o **arquivo** da pesquisa: as 41 propostas, os vereditos dos
críticos que as tentaram derrubar, os números re-simulados que sustentam cada veredito e as
linhas que ficaram fechadas com aritmética e condição explícita de reabertura. Serve para que a
pergunta "e cripto?" ou "e futuros?" seja respondida em um minuto daqui a seis meses, sem
reabrir a discussão do zero.

**Status de tudo o que está aqui:** proposto / especificado — **NÃO implementado (07/09/2026)**.
Nenhum arquivo de estratégia v1 foi tocado; nenhuma linha de motor foi escrita. Toda mudança de
regra é v2 validada do zero (framework §4, gates A e B completos); toda mudança estrutural de
motor tem ADR proposto (ADR-018, ADR-019 e ADR-020, criadas nesta sessão em `docs/decisions/` e ainda
não aprovadas nem implementadas).

**Estado do código analisado:** `main` em `d5e7279` (06/09/2026); banco dev `trader_db` (porta
5434) com candles 15m de 24/02/2025 → 02/09/2026 (136.204 candles 15m + 351 de 5m); produção =
app umbrelOS v1.1 com 8 instâncias. Números de backtest a 2 bp/lado e, salvo indicação, **sem**
o flatten de fim de sessão que o live faz — o achado central desta pesquisa (§5.1).

**Regra de leitura dos números:** quando um designer e um crítico divergem, vale o número do
crítico, marcado como "re-simulação dos críticos (06/09/2026)". Números que não existem em
nenhuma das fontes estão marcados "a medir", com o como.

---

## 1. Método

### 1.1 Três fases, sobre o repositório, o banco dev e a web

| Fase | Quem | O quê | Saída |
|---|---|---|---|
| Mapear | 5 leitores independentes | motor (execução/risco/backtest), estratégias (post-mortem das 9), livros (candidatos não implementados nas 6 análises), infra (IBKR Canada, mercados, deploy), banco (estatísticas dos 136k candles) | ~150 fatos citados por `arquivo:linha` ou consulta SQL (`mapas.md`, scratchpad da sessão) |
| Propor | 6 designers, uma lente cada | edge existente (7), setups novos (6), novos mercados (7), validação quant (7), execução/custo (7), portfólio/regime (7) | 41 propostas, cada uma com hipótese falsificável, fonte (livro/capítulo), impacto esperado, evidência, como testar, esboço de implementação, riscos, dependências (`designs.md`) |
| Criticar | 3 críticos adversariais por lente — estatística, código, operação — instruídos a **refutar** | re-simular os trades, conferir cada número no banco/JSON/código, procurar viés de seleção, checar viabilidade nas traits e paridade live==backtest, checar o que o live já mostrou | 16 dos 18 críticos rodaram; 109 vereditos individuais (`critiques.md`) |

### 1.2 Como os críticos re-simularam

Os críticos **não confiaram nos números dos designers**. Cada um reproduziu os trades a partir dos
JSONs de `trader-cli backtest --output` (binário de 06/09, 2 bp, 24/02/2025 → 02/09/2026) e dos
candles do banco dev, reimplementando a convenção do `SimulatedBroker` (stop avaliado primeiro
no pior caso, fill em `min(open, stop)`, slippage 2 bp em execução a mercado, alvo limit sem
slippage, US$ 0,35/perna). Só depois de reproduzir o motor ao centavo (Σ|diff| = 0,0 em 237
trades da balance-area; 0,13/10,5/0,06 em bab/ref/orv na re-simulação independente de outro
crítico) é que testaram cada afirmação — flatten, buckets de stop, direção, hora, ano, dia. Três
críticos independentes chegaram aos mesmos números de flatten (§5.1); é por isso que o plano os
adota.

Os scripts (`p1.py`…`p6.py`, `vq_crit2.py`, `vq_crit3.py`, `crit_eod.py`, `eod_sim.py`,
`nullsim.py`, `port.py`, `neutral.py`, `q1.sql`…`q6.sql`) ficaram no scratchpad temporário da
sessão. Os que valem como ferramenta (screens SQL com fill honesto) foram trazidos para
`sql/screens/` e `sql/stats/` (plano §5.7), ainda **sem calibração** contra controles.

### 1.3 Escala de vereditos

Cada crítico deu um veredito (**manter** / **revisar** / **matar**) e um score de 1 a 10. "Revisar"
não é aprovação branda: em quase todos os casos significa "a hipótese central sobrevive, mas o
desenho do teste ou o número de impacto não". Os 109 vereditos individuais: 8 manter, 82 revisar,
19 matar. Por proposta (maioria dos críticos): 1 "manter" limpo (edge-existente-1), 7 "matar"
(validacao-quant-7, novos-mercados-5/6/7, setups-novos-2/6, portfolio-regime-7) e 33 "revisar".

### 1.4 O que falta neste arquivo

A lente **portfólio/regime** só teve o crítico de estatística na primeira passagem; os críticos de
código e de operação dessa lente não rodaram por limite de sessão. A retomada os executou, mas
os vereditos deles **não estão em `critiques.md`** — estão no journal da retomada, e este
relatório não os inventa. Onde a tabela mostra um único veredito para portfolio-regime-1…7, é
por isso.

---

## 2. Tabela-resumo das 41 propostas

Legenda: vereditos na ordem **E**statística / **C**ódigo / **O**peração (M = manter, R = revisar,
X = matar); score médio dos críticos que rodaram; tier conforme plano-mestre §4 (A = fazer já,
B = próxima onda, C = condicional, D = arquivado com número) e o item do ranking (#).

| id | Título curto | Lente | E/C/O | Score | Tier (plano §4) |
|---|---|---|---|---|---|
| edge-existente-1 | Flatten de fim de sessão no backtest + `ExitReason::EndOfDay` | edge existente | M/M/M | 8,0 | **A** #1 |
| edge-existente-2 | balance-area-v2: filtro de convicção (stop ≥ 1,0×ATR14) + alvo 3R | edge existente | R/R/R | 5,0 | **B** #11 |
| edge-existente-3 | opening-reversal-v2: short-only, sinal até 10:15 ET | edge existente | R/M/R | 5,7 | **C** #19 |
| edge-existente-4 | range-fade: hotfix do veto de meio-dia em ET + v2 com veto integral 12–15h | edge existente | R/R/R | 3,7 | **A** #5 (hotfix) · **D** #34 (veto integral) |
| edge-existente-5 | range-fade-v2: permitir contexto Neutral | edge existente | R/R/R | 4,3 | **B** #10 |
| edge-existente-6 | Realocação de pares + cap de notional por liquidez | edge existente | R/R/R | 4,7 | **A** #6, #9b · **B** #16 (IWN) |
| edge-existente-7 | Harness de ablação (`--set`, métricas por exit_reason, n_trials) | edge existente | R/R/R | 6,0 | **A** #3 |
| setups-novos-1 | range-fade-v2: Neutral + ADX<20 + veto meio-dia em ET | setups novos | R/R/R | 5,7 | **B** #10 · **A** #5 (sem ADX/oscilador) |
| setups-novos-2 | opening-reversal-v2 short com Open-Test-Drive + 2ª entrada + piso ATR | setups novos | X/X/R | 2,7 | **D** (filtros); só o short-only sobrevive em #19 |
| setups-novos-3 | range-fade-h1 e balance-h1 em 1h derivado do 15m | setups novos | R/R/R | 3,7 | **C** #22 |
| setups-novos-4 | Fase 0 do framework: screener SQL com fill honesto | setups novos | R/M/R | 6,7 | **A** #8 |
| setups-novos-5 | Provedor de níveis e tipo-de-dia (IB, Rotation Factor, PDH/PDL/PDC) | setups novos | R/R/R | 3,3 | **B** #17 (instrumentação) · **C** (provedor) |
| setups-novos-6 | opening-reversal em 5m nos primeiros 60 min | setups novos | X/X/X | 1,7 | **D** #32 |
| novos-mercados-1 | Screening pré-registrado de ETFs setoriais value (KRE, XRT, XHB, XLE, XOP, GDX, XBI, IYT) | novos mercados | R/R/R | 5,3 | **B** #16 |
| novos-mercados-2 | ETFs de bitcoin/ether (IBIT, FBTC, ETHA) no pipeline atual | novos mercados | R/R/R | 3,0 | **B** #16 (2 tickers extras, só range-fade, só após #1) |
| novos-mercados-3 | Instrument spec + CostModel por instrumento | novos mercados | R/R/R | 4,0 | **A** #7 (fatia 1: custo por ativo, `tick_size`) · fatia 2 arquivada |
| novos-mercados-4 | Ações individuais large-cap (BRK.B, JPM, XOM, CVX, UNH, PG, HD, CAT) | novos mercados | X/R/R | 2,0 | **D** #35 (4 nomes só como controle em #16) |
| novos-mercados-5 | Micro futuros de índice (M2K/MES) RTH-only | novos mercados | X/X/X | 1,0 | **D** #25 |
| novos-mercados-6 | Cripto spot via exchange registrada (Kraken) | novos mercados | X/X/X | 1,0 | **D** #26 |
| novos-mercados-7 | Forex IDEALPRO (USD.CAD / EUR.USD) | novos mercados | R/X/X | 1,0 | **D** #27 |
| validacao-quant-1 | PSR/DSR sobre o OOS + modelo nulo de bracket aleatório | validação quant | R/R/R | 5,0 | **A** #4 (como relatório, não gate) |
| validacao-quant-2 | IC95 por bootstrap em blocos + Monte Carlo de drawdown | validação quant | R/M/R | 6,0 | **A** #4 |
| validacao-quant-3 | Meta-labeling com purged K-fold/CPCV | validação quant | R/X/R | 3,3 | **C** #23 (plumbing snapshot→journal em #3) |
| validacao-quant-4 | Sizing A/B/C: PF em R ao lado do PF em $, cap configurável | validação quant | M/R/R | 6,7 | **A** #3 (PF_R), #6 (cap) |
| validacao-quant-5 | Kelly fracionário + escada de risco | validação quant | R/R/R | 3,0 | **C** #24 |
| validacao-quant-6 | Correlação entre instâncias + teto por cluster | validação quant | R/R/R | 4,3 | **D** #30 (teto) · **B** #12 (replay) |
| validacao-quant-7 | Gate de regime por compressão de range / ADX / HMM | validação quant | X/X/R | 2,7 | **D** #29 |
| execucao-custo-1 | Medir custo real por fill em produção e recalibrar os 2 bp | execução/custo | R/M/R | 6,0 | **A** #2, #7 (comissão real) |
| execucao-custo-2 | Cortar latência de envio para < 60 s + `submit_latency` | execução/custo | R/R/R | 5,3 | **A** #2 (diagnóstico do feed primeiro) |
| execucao-custo-3 | Paridade de fim de sessão: flatten no backtest + MOC em OCA no live | execução/custo | R/R/R | 7,7 | **A** #1 (flatten) · **D** #33 (MOC/OCA) |
| execucao-custo-4 | Entrada STP LMT (paridade pós-envio) | execução/custo | R/R/R | 5,0 | **B/C** #18 |
| execucao-custo-5 | balance-area-v2 swing 1–3 pregões com bracket GTC | execução/custo | R/R/R | 4,0 | **C** #21 |
| execucao-custo-6 | Varredura de alvo (1,5R→4R) e alvo estrutural + métrica "custo em R" | execução/custo | R/R/R | 4,7 | **A** #3 (métricas); varredura não priorizada |
| execucao-custo-7 | Entrada limit passiva na range-fade + modelo realista de limit no simulador | execução/custo | X/R/R | 3,0 | **D** #31 (simulador de limit fica como infra, §2.6 do plano) |
| portfolio-regime-1 | Replay de portfólio com conta compartilhada | portfólio/regime | R/–/– | 5 | **B** #12 |
| portfolio-regime-2 | `capital_fraction` = 1/`max_concurrent_positions` | portfólio/regime | R/–/– | 4 | **A** #6 |
| portfolio-regime-3 | Detector de trend day em tempo real (veto para o fade) | portfólio/regime | R/–/– | 3 | **B** #17 (só instrumentação) |
| portfolio-regime-4 | Janelas horárias por estratégia sob flatten + calendário FOMC/opex | portfólio/regime | R/–/– | 4 | **B** #17 (calendário como tag); janelas só como ablação |
| portfolio-regime-5 | Rebalancear pares sob flatten + multi-estratégia por símbolo | portfólio/regime | R/–/– | 5 | **B/C** #20 (infra) · pares só por gate |
| portfolio-regime-6 | Transferência para ETFs macro (TLT, GLD, XLE, EEM, USO) | portfólio/regime | R/–/– | 3 | só dentro de #16 (screening pooled), plano §8 |
| portfolio-regime-7 | ETFs 3× (TNA/TZA) como "stop largo" e short sem aluguel | portfólio/regime | X/–/– | 2 | **D** #28 |

---

## 3. Por lente: hipótese, verificação, refutação, conserto

Cada subseção segue o mesmo esqueleto: **hipótese** do designer em 1–2 linhas; **o que os
críticos verificaram** (com número e origem); **refutação principal**; **conserto adotado** no
plano-mestre ou **motivo do arquivamento**. Arquivos citados como `caminho:linha` foram
conferidos no `main` em `d5e7279` (07/09/2026).

### 3.1 Lente "extrair mais do edge que já existe"

Contexto da lente: o designer re-simulou cada trade das duas aprovadas e da opening-reversal a
partir dos JSONs de 06/09 e dos candles do banco, reproduzindo o motor ao centavo (Σ|diff| = 0 em
96+67 trades), e testou políticas alternativas na mesma régua de `trader-cli backtest`.

#### edge-existente-1 — Flatten de fim de sessão no `BacktestEngine` + `ExitReason::EndOfDay`

- **Hipótese:** o gate A de 04/09 foi medido com um motor que deixa posições atravessarem a noite
  enquanto o live fecha tudo às 15h55 ET; com flatten no backtest, a balance-area deixa de cumprir
  avg R > 0,15, a range-fade continua passando e a opening-reversal passa a cumprir PF ≥ 1,3 em
  IWM/IWN.
- **Verificado (três críticos, independentes):** `crates/trader-backtest/src/engine.rs:136-240`
  não tem fim de sessão (o loop só chama `set_market_candle`, `evaluate_time_exit` e sync;
  `time_exit` = `None` nas três ativas, `crates/trader-cli/src/dispatch.rs:129-141`);
  `crates/trader-cli/src/commands/paper.rs:2189-2198` define a janela 15:55–16:10 ET e
  `paper.rs:1783-1787` grava o flatten como `ExitReason::Manual` + journal `session_flatten`;
  `crates/trader-domain/src/trades.rs:106-112` tem 5 variantes sem `EndOfDay`; o CHECK de
  `trades.exit_reason` (`crates/trader-infra/src/db/migrations/0001_initial_schema.sql:235`) não
  aceita `end_of_day`; `trade_repository.rs:28-30` tem `match` exaustivo. Re-simulação do flatten
  no close da barra 15:45 ET com 2 bp, pares vivos: balance-area 96 trades PF 1,92 → **1,55**,
  avg R 0,214 → **−0,007**, net 14.648 → 6.730 (20 overnight = +9.577, 65%); range-fade 69
  trades PF 1,88 → 1,74, avg R 0,297 → 0,218 (9 overnight = +1.933, 32%); opening-reversal
  IWM/IWN 67 trades PF 1,23 → **1,74**, net 3.400 → 8.072 (8 overnight = −2.722, 6 stopados no
  dia seguinte). Por par (balance): IJS 1,95 → 1,86 (avg R 0,27), VBR 1,96 → 1,47 (−0,04), AVUV
  1,85 → 1,43 (−0,17). Pool de 8 símbolos da balance: 237 trades PF 1,65/1,75 → 1,49, avg R
  −0,014.
- **Refutação principal:** nenhuma de fundo — os três críticos deram "manter" (8/10). Ressalvas
  registradas: o backtest sai no close da barra 15:45 (= print das 16:00, leilão) e o live manda
  MKT às 15:55 com ~US$ 238k de notional em IJS/SLYV contra barras medianas de meio-dia de US$
  370k/290k — o backtest com flatten continua otimista para os ilíquidos; o live também cancela
  a entrada pendente às 15:55 (paridade exige cancelar no backtest); o resultado "com flatten" da
  balance viva ainda depende de um dia (10/10/2025 = US$ 4.430 de 6.730); em blocos de 7, a
  balance viva com flatten tem bloco 7 (jun–set/2026) = −3.203 e o pool de 8 = −7.145.
- **Conserto adotado (plano §5.1, ADR-018 proposto):** implementar como descrito, mais
  `trade_repository.rs` e a migração 0004; `BacktestConfig.session_flatten_et` vindo de um
  `[session]` compartilhado com `paper.rs`; cancelar entrada pendente na última barra; **não**
  bloquear sinal na última barra (o live coloca a entrada às 15:45 e só cancela às 15:55); aplicar
  o slippage de fechamento a mercado ao flatten; rodar IJS/SLYV também a 4 bp; flag `--no-flatten`
  para reproduzir os runs 413–421; reportar net por bloco e share do melhor dia. Re-rodar o gate A
  das três.

#### edge-existente-2 — balance-area-breakout-v2: filtro de convicção (stop ≥ 1,0×ATR14) + alvo 3R

- **Hipótese:** só rompimentos cujo stop fica a ≥ 1,0×ATR14 da entrada pagam o custo; com o filtro,
  alvo 3R captura mais do "much bigger move" intradiário.
- **Verificado:** os números do designer reproduzem (crítico estatístico: pares vivos 54 trades
  PF 2,48 avg R 0,277; pool 8 = 130 trades PF 2,27; crítico de código: 43 trades PF 2,83 no
  conjunto vivo, 107 trades PF 2,59 no pool, com ATR14 calculado na barra de sinal). Stop mediano
  da v2 = 41 bp (p10 24 bp) contra 23 bp da v1 — sai da zona de custo da ADR-016. Com filtro ≥
  1,0 + 3R + flatten as saídas são 58% `EndOfDay`, 11% alvo, 31% stop.
- **Refutação principal:** (1) **os buckets não são monotônicos**: nos pares vivos R/ATR
  [0;0,7) 0,60 / [0,7;1,0) 0,96 / [1,0;1,5) 0,74 / ≥ 1,5 8,49 (designer) — re-simulação dos
  críticos (06/09/2026): 0,38 / 1,42 / 0,51 / 9,12; no pool de 8, [0,7;1,0) PF 1,02 (n=57) e
  [1,0;1,5) PF **1,01** (n=52) contra ≥ 1,5 PF **4,45** (n=55, 100% do P&L filtrado: +21.681 de
  +21.748). O limiar 1,0 foi escolhido para chegar aos 50 trades do gate, não porque [1,0;1,5)
  tem edge. (2) **Um dia carrega a v2**: 10/10/2025, 8 shorts simultâneos em 8 ETFs = US$ 15.617
  de 24.282 (64% do net do pool); top-5 dias = 77%; 130 trades = 55 datas, WR por dia 34,5%; com
  o teto de 3 posições da conta esse dia rende no máximo 3/8. (3) **Instabilidade temporal**:
  pool 2025 = 80 trades PF 3,93 (+28k) vs 2026 = 50 trades PF **0,60** (−3,8k); após 01/12/2025:
  64 trades PF 0,88. (4) O "holdout" de 5 símbolos coincide em 27 de 46 datas com os pares vivos.
  (5) O walk-forward proposto não é OOS (sem re-fit: "OOS" v2 pool = 119 trades PF 2,33 ≈
  in-sample 2,27). (6) Alvo 3R é decorativo: 2R/2,5R/3R/3,5R dão PF 2,15/2,14/2,30/2,44 — ruído.
  (7) ~20 tentativas nesta família sobre os mesmos 96 trades. (8) 55% dos trades terminam no
  flatten a mercado em IJS/SLYV, onde o notional é 64–82% da barra mediana.
- **Conserto adotado (plano §6.2, tier B):** hipótese pré-registrada com o limiar honesto —
  o edge medido está em stop ≥ 1,5×ATR (barra de rompimento fechando ≥ 1,2 ATR além da borda);
  se ficar em 1,0 para caber no gate, declarar que [1,0;1,5) tem PF esperado ≈ 1 e julgar também
  pelo bucket ≥ 1,5 isolado. Alvo 2R como baseline e 3R como sensibilidade. Validar em holdout
  travado (os últimos 4–6 meses são negativos e precisam ser explicados), replay com teto de 3
  posições, relatório ex-10/10/2025, `--slippage-bps 4` em IJS/SLYV, paper forward em IWN/SLYV
  (onde a v1 não roda), v1 mantida como controle. Varrer limiar × alvo no harness (#3) antes de
  escrever o módulo. Fonte: Dalton Cap. 4 + Grimes Cap. 8.

#### edge-existente-3 — opening-reversal-v2: short-only (fade da máxima de ontem), sinal até 10:15 ET

- **Hipótese:** o edge da opening-reversal está no lado short dentro da primeira hora; o lado long
  não paga o custo e as entradas na barra 10:45 são as piores.
- **Verificado:** re-simulação dos críticos (06/09/2026), 6 símbolos, short, sinal ≤ 10:15/entrada
  ≤ 10:30, flatten: 99 trades WR 57,6% PF 2,52 avg R 0,49, net 19.363; por símbolo IWM 2,43 / IWN
  3,25 / VB 4,49 / SCHA 2,96 / IJR 1,66 / SLYV 1,75; long-only + flatten 71 trades PF 0,93 (< 1
  em 5 de 6); barra de entrada 10:45 em 6 símbolos n=36 PF 0,40 (net −6.518). Detalhe corrigido
  pelo crítico de código: com `entry_validity_candles = 2` a entrada fica viva na barra 10:45
  (`orders.rs:234-236`) — a v2 real dá 101 trades PF 2,52 avg R 0,490 (2 trades a mais, +288).
- **Refutação principal:** (1) **48% do net vem do bloco 1** (fev–mai/2025, crash e recuperação
  das tarifas): +9.285 de 19.363; blocos 2–7 = 84 trades PF 1,85, mas blocos 3/5/6 = −225/+106/
  −636. (2) 23/04/2025 (+6.516) e 12/05/2025 (+4.494) = ~57% do net; top-5 dias = 52%. (3) Long
  PF 0,93 com 71 trades é indistinguível de 1,0 — a assimetria num histórico bull é compatível
  com "o edge foi o crash", não "o edge é o short". (4) O corte 10:15 apoia-se em 26 trades (6 nos
  pares vivos). (5) 6 de 14 símbolos escolhidos após ver a tabela. (6) Operacional: latência da
  1ª hora com feed sem realtime (barras maiores do dia; nos 5m de SPY, 12/20 barras da 1ª hora
  tiveram o gatilho cruzado nos 5 min seguintes); SCHA cota US$ 32,6 (tick 3,07 bp — o PF 2,96
  está medido com custo subestimado); short na IBKR (aluguel, SSR, sell stop de VBR 28/08
  "cancelado" sem fill — achado A9, `docs/reports/pregoes-2026-08-31_a_09-02.md` §4.2);
  VB/SCHA/IJR param em 06/08 no banco.
- **Conserto adotado (plano §6.3, tier C — Onda C: spec pronta, sem implementação):** v2 = v1 + `direction_filter = short_only`;
  `trading_end_time = 10:15` como hipótese secundária; `entry_validity_candles` documentado (ou
  = 1). Sem Open-Test-Drive, segunda entrada ou piso de ATR (§3.2, setups-novos-2). Validar em 3
  pares (IWM, IWN, VB), v1 como controle em símbolo distinto, critério em dados que não existiam
  quando a regra foi escolhida (holdout + paper forward + segmento 2026: 29 trades PF 1,64),
  P&L com teto de 3 posições, ex-23/04 e ex-12/05/2025; teste operacional de short antes do gate
  B (3 sell stops fora de setup na paper; checagem de shortable; A9 em produção); SCHA a 4–5 bp
  ou excluído. Fonte: Brooks Cap. 11 + Cap. 1.

#### edge-existente-4 — range-fade: hotfix do veto de meio-dia em ET (v1.0.1) e v2 com veto integral 12:00–14:59 ET

- **Hipótese:** (a) o veto `midday_midrange` compara UTC fixo e desliza 1h no inverno; (b) fades
  com entrada entre 12:00 e 14:59 ET têm PF ≤ 1 e vetar o bloco inteiro eleva o PF em ≥ 0,3.
- **Verificado:** (a) confirmado por três críticos:
  `crates/trader-core/src/strategies/range_extreme_fade_v1/context.rs:220-238` compara
  `last.timestamp.time()` em UTC com `midday_start/end_time` do TOML
  (`config/strategies/range-extreme-fade-v1.toml:28-29` = 15:30:00/18:00:00), enquanto
  `trading_start/end` já são ET (`session.rs`) desde a A2; 15:30–18:00 UTC = 11:30–14:00 EDT e
  **10:30–13:00 EST**; o doc da v1 especifica 11:30–14:00 ET; o comentário do código
  ("consistência com check_trading_hours") está obsoleto. ~24% da amostra do gate A
  (24/02–08/03/2025 e 02/11/2025–08/03/2026) foi medida com o veto deslocado; precedente A2
  (`e4231f3`) para hotfix que muda `config_hash` sem bump.
- **Refutação principal (parte b):** o bloco 12–14h ET dos pares vivos, na re-simulação dos
  críticos (06/09/2026) com flatten, é n=30, **PF 1,29, net +857** — positivo, não "≤ 1"; a
  tabela por hora do designer (12h 0,78 / 13h 0,94 / 14h 0,82) contradiz o próprio agregado dele
  ("30 trades removidos rendem +0,86k"); outro crítico mede o mesmo bloco a 2 bp sem flatten em
  n=30, −543, PF 0,87, avg R 0,00 (t = 0,02) — ruído, não perda. Teste de permutação das horas:
  p = 0,17. O sinal negativo (n=38, PF 0,61, −2.081) só existe em IJR/IWN/MDY, que não estão em
  produção. Cortar 43% da amostra derruba o OOS de 64 para ~37 (< 50). A hipótese "recupera P&L
  no inverno" é intestável: subamostra EST dos pares vivos ≈ 20–25 trades (14h EST n=5, PF 0,43).
- **Conserto adotado (plano §5.4, tier A) / arquivamento (§8):** fazer **só** o hotfix v1.0.1
  (ET via `chrono_tz` já importado no arquivo, TOML `11:30:00`/`14:00:00` com comentário "ET",
  teste unitário com timestamp de janeiro e de julho, nota formal de correção, re-rodar o
  walk-forward junto com o flatten — os runs 419–421 ficam obsoletos). **Veto integral 12–15h
  arquivado** (§4.10 deste relatório); no máximo override de ablação no harness, depois que #10
  recompuser a amostra.

#### edge-existente-5 — range-fade-v2: permitir contexto Neutral (política de contexto por estratégia)

- **Hipótese:** a range-fade só opera quando o contexto global classifica Uptrend/Downtrend, mas
  dias de range caem em Neutral; liberar Neutral para esta estratégia aumenta a amostra em ≥ 60%
  com PF do incremento ≥ 1,3.
- **Verificado:** `crates/trader-core/src/context/mod.rs:79-81` (`is_tradeable = trend ∈ {Up,
  Down} ∧ vol ≠ High ∧ phase == Regular`); o único lugar que rejeita por isso é
  `crates/trader-core/src/risk/mod.rs:158-163` (`NoContext`); `paper.rs` e `engine.rs` não
  consultam `is_tradeable`; `ContextAnalyzerConfig::default()` hardcoded em `engine.rs:127-128`
  e `paper.rs:155-156`; a range-fade nunca lê `trend_state` (a pullback e a low2 leem). Proxy
  nos candles (regra real EMA20/SMA200): Neutral = 38–44% de todas as barras RTH e 44–49% das
  barras com EMA20 plana (AVUV 49%, SLYV 48%, IWV 44%, IJS 48%, VBR 48%, IWN 47%). **Medição
  direta** (crítico de código de setups-novos, backtest com `RUST_LOG=debug` a 2 bp): NoContext
  rejeitou **48 sinais em AVUV (31 entradas executadas), 36 em SLYV (30), 62 em IWV (35)**.
- **Refutação principal:** o PF do incremento **não existe em lugar nenhum do projeto** — é uma
  aposta sem estimativa; "EMA20 plana" (< 0,05%/barra em 12 barras) cobre 80–89% de todas as
  barras, e 53–60% das barras planas são Up/Down — "o detector coincide com Neutral por
  construção" é falso; o long da v1 (PF 3,34 n=33, t=2,79; 2,22 com flatten) é "comprar a nova
  mínima com close > EMA20 > SMA200" — reversão a favor do contexto; Neutral inclui EMA20 e
  SMA200 cruzadas (chop multi-dia) e transições; toca o RiskManager de todas as estratégias;
  mais sinais pressionam `max_trades_per_day = 3` e as 3 posições da conta; o critério "≥ 30
  trades e PF ≥ 1,3" é subpotente (SE(avg R) a n=30 ≈ 0,22).
- **Conserto adotado (plano §6.1, tier B — "a medição de maior valor esperado do plano"):**
  passo 1 sem módulo novo — `RiskConfig.allow_neutral_context` (default false) plumbado por
  `StrategyRiskParams` e pelo harness; em `risk/mod.rs:158-163` rejeitar `NoContext` só se
  `!allow && !is_tradeable`, mantendo vol High e fase ≠ Regular como vetos independentes; dividir
  `NoContext` em `NeutralContext` / `HighVolatility` / `OutsidePhase`; rodar v1 com e sem a flag
  em AVUV/SLYV/IWV/IWN e diffar trades por `entry_time` (n, PF, avg R, hora, direção, ano do
  incremento). Passo 2 só se passar: módulo `range_extreme_fade_v2`, política no
  `market_snapshot`. Critério: ≥ 30 OOS, PF ≥ 1,3, avg R ≥ 0 por direção e por ano. Fonte:
  Brooks Cap. 9/10 + Dalton Cap. 3 + Murphy Cap. 15.

#### edge-existente-6 — Realocação de pares (balance em IWN, openrev-v2 em VB/SCHA, range-fade fora de IWV) + cap de notional por liquidez

- **Hipótese:** com as mesmas 8 instâncias, trocar os pares mais fracos pelos mais fortes e
  limitar o notional por liquidez eleva o P&L do portfólio em ≥ 40%.
- **Verificado:** os runs citados existem com os números citados (531 IWN 41t/1,89/0,229; 533
  SLYV 29t/1,64; 539 IWO 13t/2,29; 537 IJR avg R −0,076; 540 VB 29t/1,96/0,397; 544 SLYV
  22t/1,65; 542 SCHA 36t/1,27; 552 ref IWN 26t/1,60; 547 IWV 19t/1,15). Re-simulação com
  flatten: bab IWN **2,23** (n=41), bab IWO 0,68, ref IWN 1,09, ref IJR 0,94, ref MDY 1,10, ref
  IWV 1,15 / avg R −0,018 (stop mediano 13 bp). Liquidez por barra 15m 11–13h ET desde
  mar/2026: SLYV mediana US$ 290k (p10 77k; média 502k), IJS 370k (média 974k), VBR 1,0M, IWN
  1,6–1,8M, AVUV 2,6M; com equity ≈ US$ 238k (`docs/HANDOFF.md:215`) e cap 1×
  (`risk/mod.rs:250-253`), o bot opera com 82% e 64% da barra mediana em SLYV e IJS.
  `TRADER__RISK__MAX_NOTIONAL_USD` funciona sem código além do campo (loader com prefixo
  `TRADER` e separador `__`).
- **Refutação principal:** o "+42%" é soma de 8 backtests isolados com capital fixo — ignora o
  teto de 3 posições/200% e o bloqueio por símbolo (IWN teria balance e openrev na janela
  09:45–10:30); empilha três coisas não validadas (bab-v2, orv-v2, realocação), cada uma com gate
  A e 4 semanas de gate B; é "escolher ativos depois de ver a tabela"; SCHA tick 3,07 bp; VB/
  SCHA/IJR/MDY/SPY/QQQ param em 06/08; o cap de liquidez corta SLYV para ~1/5 e IJS para ~1/3 do
  tamanho atual e o P&L em $ cai na mesma proporção; cada par novo zera o relógio do gate B num
  portfólio com 0,60 trade/pregão (8 pares vivos) e 0 trades das aprovadas desde 18/08; client ids 1–3 só estão
  livres se a v1.2.0 do app estiver instalada (HANDOFF:210 registra como pendente).
- **Conserto adotado (plano §5.5, §5.10, §6.5):** separar em três: (A) agora, cap de notional
  configurável (`max_notional_multiple` + `max_notional_usd` por instância) — SLYV ≈ US$ 50–90k,
  IJS ≈ 85–120k, VBR ≈ 180k, AVUV/IWN/IWO 250–300k (regra: ≤ 1/3 da barra mediana de 15m dos
  últimos 60 pregões); registrar equity/`BUYING_POWER` reais; (B) uma troca só: sair range-fade
  de IWV (libera client_id 11) e entrar balance-area-v1 em IWN, medindo o bloqueio mútuo com a
  openrev antes; (C) VB/SCHA só após reingestão, SCHA a 4–5 bp, e após o teste operacional de
  short. Abandonar o "+42%" como número.

#### edge-existente-7 — Harness de ablação: `--set` no walkforward, métricas por exit_reason/direção/hora, registro de tentativas

- **Hipótese:** as ~30 variantes desta pesquisa não cabem em duas semanas se cada uma exige um
  módulo v2; com override de parâmetros por CLI, export JSON do walk-forward e métricas por
  exit_reason, cada ablação custa minutos e o gate ganha a contagem de tentativas.
- **Verificado:** `crates/trader-cli/src/commands/walkforward.rs:18-25` (Args sem `--output`/
  `--slippage-bps`; `backtest.rs:36,114` os tem); `metrics.rs` não separa por exit_reason/
  direção/hora; `backtest_runs.metrics` tem 16 chaves e nenhuma de slippage/parâmetros; runs
  413/414 duplicados (mesmo `config_hash`); 505 de 589 runs sem label; 22 cópias de walk-forward (auditoria de 07/09, chave completa: 109 grupos / 279 linhas em 687 runs, 566 sem label);
  60 runs com PF > 1 e Sharpe < −5 (mín −18,5 — `metrics.rs:175-231` anualiza por candle 15m);
  `dispatch.rs:36-72` mapeia id exato → struct; `load_strategy` recebe `toml_str`, então patchar
  `[strategy.parameters]` via `toml::Value` faz o `config_hash` mudar sozinho.
- **Refutação principal:** (1) nenhum `config.rs` usa `deny_unknown_fields` — um `--set` com
  chave errada é ignorado em silêncio, o hash não muda e o label mente; (2) `analyze` usa
  `latest_by_strategy` sem filtrar por label/hash/símbolo (`analyze.rs:75-78`) — como backtest e
  walkforward **sempre persistem**, a primeira ablação vira em silêncio o baseline do gate B da
  estratégia em produção; (3) DSR sobre o walk-forward do repo é DSR sobre in-sample (sem re-fit;
  "OOS" = mesma rodada após o bloco 1) — a ferramenta que falta é um **holdout temporal travado**;
  (4) `n_trials` será subcontado por construção (as ~30 variantes foram re-simulações em Python
  que nunca entram no banco); (5) faltam as métricas que decidiram os vereditos: P&L por dia ET,
  share do melhor dia, net por bloco e por ano.
- **Conserto adotado (plano §5.2, ADR-019 proposto, tier A):** `--output`, `--slippage-bps`
  (Decimal), `--label` obrigatório com override, `--holdout-from`; `--set` que **falha** se a
  chave não existir (`deny_unknown_fields` ou abortar se o hash não mudar); runs com override
  marcados `experimental` e `latest_by_strategy` filtrando por (strategy_id, asset, config_hash
  da estratégia carregada); métricas por exit_reason/direção/hora ET, P&L por dia (share top-1 e
  top-5), por bloco e por ano, Sharpe/Sortino sobre retornos diários, `profit_factor_r`,
  `corr_risk_result`; `slippage_bps`/`overrides`/`session_flatten`/`n_trials` no jsonb; índice
  único e script de dedupe; `strategy_id`/`config_hash`/`market_snapshot` no journal dos trades
  de backtest (`simulated/broker.rs:851-893`, hoje `unknown` e `{}`).

### 3.2 Lente "setups novos dos livros já analisados"

Contexto da lente: antes de propor, o designer rodou screens SQL no banco dev (11 ETFs, 15m,
fill honesto = `max(open, gatilho)` long / `min(open, gatilho)` short, stop avaliado primeiro na
barra de fill, 4 bp) e reprovou 7 candidatos que as análises de livro ranqueavam no top-3:
squeeze-breakout, pivot-point-intraday, opening-range-breakout/IB estreito, gap-continuation/
gap-fade, "dia após clímax", double-top/bottom solto, toque bruto de PDH. Conclusão da lente:
em 18 meses nenhum setup de continuação/rompimento sobrevive ao fill honesto; o que paga é fade
com confirmação.

#### setups-novos-1 — range-fade-v2: liberar Neutral + ADX<20 + veto de meio-dia em ET

- **Hipótese:** o detector de dia de range da v1 (EMA20 plana) e o filtro global de tendência
  são contraditórios; o subconjunto de sinais que só existe com `trend_state = Neutral` tem ≥ 30
  trades OOS e PF ≥ 1,3.
- **Verificado:** mesma mecânica de edge-existente-5 (é a mesma hipótese, por outro designer).
  O crítico de código **mediu**: NoContext 48/36/62 sinais em AVUV/SLYV/IWV contra 31/30/35
  entradas executadas — ~60% dos sinais que a estratégia aprova morrem no `is_tradeable`.
  **Corrigido pela auditoria de 07/09** (`docs/reports/auditoria-2026-09-07.md` §3, bloco A):
  a rejeição é logada duas vezes no mesmo run, então são **24/18/31 (73 no total), 43%** —
  e o delta desses sinais, medido, é **negativo** (54 trades, PF 0,70, avg R −0,321).
  Proxy SMA20/SMA200: 44,5% das barras em dia de range são Neutral (26.686 de 59.976), mas
  41,9% nas barras de dia de tendência — o proxy **não discrimina** dia de range; o argumento
  válido é o do detector contraditório. Erro factual corrigido: SLYV não roda balance-area
  (`trader-web/src/instances.rs:25-27`: IJS, VBR, AVUV) — só AVUV colide.
- **Refutação principal:** a proposta empacota três mudanças e só uma tem evidência. ADX de
  Wilder e estocástico lento entram "desligados por default" — código morto; o veto 12:00–14:00
  ET independente da posição no range **não é a regra do Cap. 5** de Brooks (meio do dia **e**
  terço central) — é calibração própria sem fonte, com −US$ 540 em 18 meses nos pares vivos
  (o −4.234 é nos candidatos IJR/IWN/MDY, que não rodam); "+80% de sinais" é teto teórico, não
  expectativa; a v2 substitui a v1 nos 3 símbolos e reinicia o gate B; ir para produção exige
  tocar `trader-web/src/instances.rs` e o `INSTANCES` do compose.
- **Conserto adotado (plano §6.1 + §5.4):** reduzir a v2 ao mínimo falsificável —
  `allow_neutral_context` (uma mudança) + hotfix do fuso como v1.0.1 separado; **nada** de ADX/
  estocástico/veto de hora na v2 (cada um é ablação separada, contada no N, e um veto alargado
  seria regra sem fonte). Medir o delta neutral-only por backtest A/B (mesmo TOML, política
  ligada/desligada) e o count de NoContext em `signals.rejection_reason` de produção.

#### setups-novos-2 — opening-reversal-v2 short com Open-Test-Drive, segunda entrada e piso 0,5×ATR14

- **Hipótese:** o edge está inteiro no lado short; uma v2 short-only com confirmação de "drive"
  de volta (Dalton), segunda entrada em double top (Brooks Cap. 11) e stop ≥ 0,5×ATR14 passa o
  gate A em walk-forward sobre 14 símbolos.
- **Verificado (dois críticos, dos JSONs do próprio designer, 6 símbolos, 2 bp):** shorts n=125
  PF 1,59 avg R 0,29 (+13.434); longs n=71 PF 1,06; **shorts 2025 n=80 PF 2,33 avg R 0,42; 2026
  n=45 PF 0,82 avg R 0,04** (t = 0,21); 2025Q2 n=23 PF 5,69 +11.832 = 88% do P&L short;
  2026Q1+Q2 n=34 −5.472; excluindo fev–abr/2025 n=109 PF 1,25 t=1,58; os 5 maiores trades = 49%
  do P&L short; nos pares em produção (IWM+IWN) short n=41 PF 1,24 t=0,89; IWM short PF 1,06
  (n=23) vs long 1,41 (n=13). O "PF 1,82" é o subconjunto de 4 símbolos escolhidos após a tabela.
- **Refutação principal:** o critério pré-registrado pelo próprio designer ("shorts PF ≥ 1,4 e
  avg R ≥ 0,2 em **ambos** os anos") **falha antes de qualquer linha de Rust**. É artefato de
  regime (crash e recuperação em V de 2025), não edge estrutural. Os filtros novos seriam
  ajustados sobre os mesmos trades depois de ver a tabela; o "drive back" é quase idêntico à
  barra de sinal já exigida (`setup.rs:60-100`); o Open-Test-Drive em 15m só é detectável "após
  2 candles" (análise do Dalton, linha 405); `time_exit` (candles=8, min_r=0,5) **não fecha antes
  do sino** — o tracker desarma para sempre ao atingir 0,5R uma vez (`time_exit.rs:123-145`).
- **Motivo do arquivamento (plano §8):** Open-Test-Drive + segunda entrada + piso de ATR
  arquivados como filtros post-hoc. O short-only puro sobrevive só como hipótese de regime em
  edge-existente-3 (#19), com critério em dados que não existiam quando a regra foi escolhida.
  Registrar N = 6 combos + este teste no contador do DSR.

#### setups-novos-3 — range-fade-h1 e balance-area-h1: a mesma família em 1h derivado do 15m

- **Hipótese:** em 1h a barra mediana é 0,31–0,43% (ex-1ª hora) contra 0,15–0,30% em 15m; o
  stop "além da barra de sinal" vai de ~21 para ~35 bp e o custo cai de 19% para ~11% do R.
- **Verificado:** os ranges de barra de 1h conferem exatamente (AVUV 0,398/0,358%, IJS
  0,376/0,337, IWM 0,460/0,413, IWV 0,266/0,242, SLYV 0,373/0,333, VBR 0,341/0,309); o código é
  agnóstico de timeframe (`operational_timeframe` no TOML; `TimeFrame::H1` mapeado); não há
  candle 1h no banco (só 15m e 351 de 5m); dados são RTH-only (26 barras/dia), então o bucket
  15:30 tem 2 candles e há 3–4 meios-pregões por símbolo.
- **Refutação principal:** (1) o INSERT com `source='derived-15m'` **quebra o carregamento**:
  `parse_source` (`trader-infra/src/repositories/mod.rs:39-48`) só aceita ibkr/polygon/manual/
  simulated e devolve `Err`, propagado em `candle_repository.rs:125`; (2) "lookbacks ÷4" mata o
  detector de range (`structure_lookback=3` faz `pivots()` avaliar 1 índice; a inclinação da
  EMA20 é por barra e anda ~4× mais em 1h); (3) **paridade quebra**: `paper.rs:968-973` pede
  `days(30)` → ~147 barras de 1h < 200 da SMA200 → `classify_trend` cai no ramo sem SMA200 e o
  `is_tradeable` do live fica mais permissivo que o do backtest; (4) a barra 15:30 de 1h "fecha"
  às 16:30, depois do scheduler parar (16:10) — nunca avaliada no live; (5) em 1h mais trades
  chegam ao flatten (hoje 19–36% duram ≥ 3h e 17–25% atravessam a noite); (6) a frequência "1/3"
  é palpite; a superfície (2 estratégias × 14 símbolos × 4 níveis de custo + recalibração) soma
  ~120 tentativas sem DSR; (7) IWV em 1h tem mediana 0,24–0,27%, quase o 15m de AVUV.
- **Conserto adotado (plano §7, tier C):** só depois de #1; `resample(candles, TimeFrame)` em
  `indicators` (O2) + `DataSource::Derived` (ou `source='manual'`); janela do live de `days(30)`
  → ≥ 70; excluir o bucket 15:30 e meios-pregões; reparametrizar por regra (`structure_lookback
  ≥ 8`, inclinação ×4), marcado "interpretação nossa"; teste de frequência **antes** de qualquer
  walk-forward; conjunto fixo de 8 ETFs value declarado antes; gate só no agregado a 2 bp.

#### setups-novos-4 — Fase 0 do framework: screener SQL com fill honesto

- **Hipótese:** a maior parte dos setups de livro pode ser reprovada em horas, sem Rust, com um
  mini-backtest em SQL que modele o fill como o simulador (ADR-015) e reporte avg R/PF por ano e
  por símbolo.
- **Verificado:** os screens existem (q1–q6.sql, 536 linhas, ~40 consultas auxiliares) e
  `q3.sql:39-49` implementa o que descreve (stop a partir da barra de fill inclusive, alvo k×R,
  custo −0,0004/risk_pct, filtro de risco 0,3–1,5%). Q12 re-executada: PDL-break short com fill
  exato dava avg R +0,063 (2025, PF_R 1,21) / +0,134 (2026, 1,50); com fill honesto **−0,034
  (0,90) / +0,034 (1,11)** — o "edge" era o gap atravessado no gatilho, o artefato que a ADR-015
  corrigiu. Q14 re-executada: 44,5%. Não toca motor, trait nem TOML. `docs/strategy-analysis-
  framework.md` só exige ≥ 50 sinais com custo na Fase 5.2 — depois da implementação; a Fase 0 é
  o que falta.
- **Refutação principal:** (1) o screener **nunca rodou um controle positivo** — só produziu
  negativos, e seus vereditos já são usados como evidência; (2) sai no fechamento do dia (replica
  o live, não o engine) — o retroteste contra a balance-area vai falhar (17–25% overnight
  respondem por 52–87% do P&L nos JSONs dos pares vivos: AVUV 6/35 com 2.577/4.953, IJS 5/25 com
  2.449/4.449, VBR 9/36 com 4.551/5.246); (3) o engine **cancela** a entrada quando o gap de
  abertura excede 25% do stop (`execution/mod.rs:93-117`; `simulated/broker.rs:245-283`), o
  screener enche com preço pior; (4) a regra de decisão é subpotente: com ≥ 40 fills/ano e sd(R)
  ≈ 1,2–1,5 (medido 1,18–1,68), SE(avg R) ≈ 0,19–0,24 → um setup com avg R verdadeiro +0,25
  passa "avg R ≥ +0,10 em 2025 E 2026" só ~55–60% das vezes; (5) 499 fills/ano em 11 ETFs são
  os mesmos dias — n efetivo por data; (6) o `trader-cli screen` gravando em `backtest_runs` não
  cabe no schema (`asset_id` NOT NULL etc.); (7) os arquivos vivem no scratchpad temporário e
  serão perdidos se não forem commitados.
- **Conserto adotado (plano §5.7, tier A):** `sql/screens/` com template, screens e README;
  calibrar **antes** de usar como Fase 0: controles positivos (range-fade em AVUV/SLYV; balance em
  IJS/VBR/AVUV **com flatten**, PF 1,55 / avg R ≈ 0 — o screener sai no fechamento do dia, como o live) e negativo (pullback-trend); flag `hold_overnight`;
  regra de cancelamento por gap (open além de 25% do stop → sem trade); agregação por **data**;
  critério: avg R agregado com t ≥ 1,5, mesmo sinal em 2025 e 2026, melhor trimestre ≤ 50% do
  P&L (teria pego os 88% de 2025Q2 de setups-novos-2). Registrar N em `docs/reports/screens-
  <data>.md`. Descartar `trader-cli screen`. Texto da Fase 0 no framework marcado como proposta.

#### setups-novos-5 — Provedor de níveis e tipo-de-dia (Dalton): IB, Rotation Factor, Nontrend/Nonconviction, PDH/PDL/PDC

- **Hipótese:** dias "Nonconviction" (|RF| ≤ 2 e range até 13:00 ET < 0,8×ATRd) têm PF de
  balance-area < 1,0 e de range-fade > 2,0 — veto para o breakout, habilitador para o fade à
  tarde; dias com IB estreito têm extensão de range em ≥ 70% dos casos.
- **Verificado:** estatísticas de IB conferem (IB = 55–68% do range; rompido em 91–97% dos dias;
  os dois lados em 14–27%; IB segura a máxima ou a mínima em 73–86%) — por isso a hipótese (2) é
  **trivialmente verdadeira pela taxa-base** e não é falsificável. `market_contexts.raw_values`
  é jsonb (sem migração); `ContextAnalyzerConfig` é hardcoded (`engine.rs:128`, `paper.rs:156`);
  `daily_atr`/`day_range` já são funções puras dentro da estratégia
  (`range_extreme_fade_v1/context.rs:61-89`) — o padrão barato existe. O título diz "IB de 6
  candles", o sketch diz 4 (a 1ª hora são 4 barras de 15m — o banco confirma).
- **Refutação principal:** a hipótese (1) não tem amostra: ~20% dos dias Nonconviction sobre 96
  trades da balance → ~19 trades no bucket, SE(avg R) ≈ 0,39 — impossível separar PF 1,0 de
  2,0; é filtro escolhido sobre os mesmos trades que seriam reavaliados ("filtros por intuição
  falharam 2×", doc da range-fade §14); o veto Nonconviction só é confiável após ~13:00 ET e
  incide nas entradas 13–15h da balance, que carregam +7,9k dos +14,6k mas dependem de overnight;
  o período que o habilitador abriria (12–14h) tem avg R 0,00 (n=30) nos pares vivos; o
  consumidor do provedor de níveis (setups-novos-2) morreu; Rotation Factor é sobre TPOs de 30
  min de futuros de pit dos anos 80.
- **Conserto adotado (plano §6.6, tier B):** instrumentação **sem decisão** — `strategies/common/
  day_type.rs` (funções puras: `initial_balance(n_bars=4)`, `rotation_factor`,
  `prior_day_levels`, `day_type_so_far`) gravadas no `market_snapshot` do sinal, paper e
  backtest, sem tocar `MarketContext`/persistência; screen de tipo-de-dia pré-registrado (Fase 0)
  cruzado com os trades reais das v1, mínimo 50 trades por bucket, validação 2025→2026; vetos só
  depois de ≥ 20–30 trades vetáveis em paper. Provedor de níveis (PDH/PDL/PDC) fica para a Onda C.

#### setups-novos-6 — opening-reversal em 5m nos primeiros 60 minutos

- **Hipótese:** em 5m a barra de sinal chega 10 min antes e o "drive back" é observável dentro da
  1ª hora; a v2 short-only mantém PF ≥ 1,3 e stop mediano ≥ 25 bp.
- **Verificado:** banco tem 351 candles de 5m (só SPY, 28/07–03/08/2026, 78/dia, 0 degeneradas);
  mediana da barra 15m 09:45–10:30: AVUV 0,325%, IJS 0,288, IWM 0,395, IWN 0,312, SLYV 0,261, VB
  0,331, VBR 0,284 (o "0,33–0,37%" vale só para IWM/VB/AVUV); razão 5m/15m na 1ª hora de SPY =
  0,71 → barras de sinal de 5m ≈ 0,19–0,28%, abaixo do piso em SLYV/IJS/VBR/IWN/AVUV;
  `ingest.rs:47-56` faz **uma** requisição com `--days` a partir de agora (sem `--from/--to`), e
  a IBKR limita 5 min a ~1 semana por requisição (~78 requisições/símbolo).
- **Refutação principal:** dependência dura de setups-novos-2, que já reprova; a premissa "a
  barra chega 10 min antes" **inverte** no feed atual: sem realtime a barra consolida em ~3–4 min
  e a guarda exige 2 polls de 30 s (`paper.rs:693,713`, `1036-1047`) — para 5m a decisão sai
  quando a barra seguinte já acabou e `MAX_BAR_SETTLE_POLLS=30` desiste após 15 min (3 barras
  de 5m); nos 5m de SPY, 60% das barras da 1ª hora tiveram o gatilho cruzado nos 5 min seguintes;
  a conversão dos vetos (`momentum_bars=2`, `counter_window=6`) para 5m muda o significado das
  regras; "live em 5m dobra polls" está errado (poll independe do timeframe).
- **Motivo do arquivamento (plano §8, #32):** ver §4.8 deste relatório.

### 3.3 Lente "novos mercados e ativos"

Contexto da lente: o pipeline é mono-mercado (`Contract::stock` em 5 call sites, `tick_size =
0.01` nos 9 TOMLs, sizing sem multiplicador, sessão NY em 5 camadas) — tudo que negocia como ação
US entra com esforço S; qualquer outra classe exige o refactor de instrumento (L) antes de um
único backtest. O designer rodou em 06/09 o pré-teste de futuros no subjacente (runs 561–572).

#### novos-mercados-1 — Screening pré-registrado de ETFs setoriais/fator "value cíclico" (KRE, XRT, XHB, XLE, XOP, GDX, XBI, IYT) com controles growth

- **Hipótese:** o edge é propriedade de cestas cíclicas/value que revertem à média, não das
  regras; ≥ 3 dos 8 candidatos reproduzem PF ≥ 1,3 com ≥ 25 trades em pelo menos uma das duas
  aprovadas, enquanto os 3 controles growth (XLK, SOXX, ARKK) ficam abaixo de 1,1.
- **Verificado:** runs 561–572 batem (IWM balance 0,84/0,78 a 1/2 bp; IWM fade 0,42/0,39; SPY
  fade 0,40/0,34; IWM openrev 1,26/1,18); runs 545/557 SLYV fade 3,04 (21 t), 543/556 AVUV 1,64
  (29 t); ingest de 528–560 dias numa requisição funcionou para os 14 ETFs; liquidez por barra
  11–13h: SLYV 0,53M, IJS 0,85M, VBR 1,83M, AVUV 2,63M (US$ 250k = 47% da barra média de SLYV;
  mediana 290k — o gargalo é pior); cada instância consome 3 client ids (24 em uso, teto 32 →
  cabem +2); permissão CLP **não** é necessária para ingest/backtest de TNA, só para ordem.
- **Refutação principal:** (1) o critério de falsificação não funciona: com n ≈ 25 por combo,
  sob PF verdadeiro 1,0 a probabilidade de cravar PF ≥ 1,3 é 0,20–0,28 (payoffs do projeto),
  P(≥ 3 de 8 candidatos "passam" | nulo) = **0,73**, e "6 controles < 1,1" tem P = 2,5% sob o
  nulo **e sob a alternativa** — o desenho responde "artefato" em ~97% dos mundos; (2)
  contaminação overnight (65% do P&L da balance) — setoriais carregam gaps de evento maiores
  (mediana de gap no banco 31–51 bp; 11–23% dos dias > 1%); (3) balance-area v1 usa largura
  **absoluta** ≤ 2% (`balance-area-breakout-v1.toml:14`) — em GDX/XBI/XOP com range diário 2–3%
  quase não forma; (4) sub-teste TNA sem poder (a diferença de custo 2→1 bp = +0,08 de PF é 0,2
  desvio-padrão do PF a n=36, SD ≈ 0,38; a base openrev reprovou o gate A); (5) tick de 1 centavo
  em GDX (~US$ 40) = 2,5 bp e KRE (~US$ 60) = 1,7 bp — decidir a 3 bp; (6) diversificação
  superestimada: KRE/XRT/XHB são os setores dominantes das cestas small-cap value; (7) hipótese
  "value cíclico" formulada depois de ver 14 ativos; (8) short em setorial repete o risco A9.
- **Conserto adotado (plano §6.5, tier B):** hipótese ao nível de **classe**, pooled por
  estratégia via walk-forward 6 janelas (≥ 50 OOS, PF ≥ 1,3, avg R > 0,15, **com flatten**),
  bootstrap da diferença de avg R candidatos vs controles (IC 90%), pré-registrado; critério por
  estratégia (balance só onde a largura mediana de 78 barras cabe em 2% — medir por SQL antes);
  3 bp para preço < US$ 60; matar o sub-teste TNA; +2 instâncias (client_ids 12–13; atualizar
  `ibkr/broker.rs:1087-1105` e o compose); N = +16 no DSR; ingest via workflow `ops` no servidor,
  1 símbolo por vez. Sob o nulo, P(≥ 3 de 8 "passam") = 0,73 — por isso só o desenho pooled decide.

#### novos-mercados-2 — Cripto passo 1: ETFs spot de bitcoin/ether (IBIT, FBTC, ETHA) no pipeline atual

- **Hipótese:** cripto spot não existe na IBKR Canada, logo cripto entra como ETF STK/SMART/USD;
  o detector de dia de range e o balance multi-dia identificam na sessão RTH do IBIT os mesmos
  regimes que em small-caps.
- **Verificado:** zero código (`Contract::stock` SMART/USD default; `ensure_asset` grava
  'stock'); permissão "Complex or Leveraged ETP" é mesmo exigida; IBIT (jan/2024) e ETHA
  (jul/2024) têm histórico suficiente; `max_atr_pct = 1.5` (15m) — IBIT ~0,4–0,6% passa; gap
  overnight mediano de AVUV 31,9 bp (> stop mediano 22 bp); balance v1 largura absoluta 2% →
  designer estima 0–5 sinais/ano em IBIT. **Premissa contradita:** um review terceiro
  (brokerchooser, 2026) diz que clientes elegíveis da IBKR Canada negociam cripto via Zero Hash/
  Paxos (Quebec excluído) a 0,12–0,18% (mín. US$ 1,75); as páginas .ca/.com devolveram 403 —
  **não verificado na fonte primária**; ação do dono no Client Portal.
- **Refutação principal:** prior negativo ignorado — a range-fade perde nos ativos "momentum"
  (IWM 0,39, SPY 0,34, QQQ 0,67, IWO 0,52) e BTC é o ativo-momentum canônico; a hipótese "fluxo
  ETF/CME em horário US reverte" não tem fonte no projeto (viola "nenhuma regra sem fonte");
  FBTC como "controle" mede o tick, não robustez (mesmo subjacente); o número do backtest é
  inválido sem flatten — o subjacente anda 17,5h com o ETF fechado, e o gap será múltiplos do dos
  small-caps; "criar balance-v2 com largura em ATR porque a v1 não forma em IBIT" é ajustar a
  regra ao ativo depois de olhar (a trendline inverteu o PF ao afrouxar 0,3→0,8%); com ~22–30
  trades P(PF ≥ 1,3 | nulo) ≈ 0,28 — o teste só reprova ou produz falso positivo.
- **Conserto adotado (plano §6.5 e §8):** não abrir "classe cripto"; IBIT e ETHA como **2
  tickers extras** do screening pooled, **só range-fade-v1**, **só após #1**, mesmo critério, sem
  v2 sob medida; kill pré-registrado: mediana do gap overnight > 3× a dos pares vivos (~35 bp) →
  arquivar sem backtest (a guarda ADR-015 e o cancelamento por gap descartariam a maioria das
  entradas). Corrigir a premissa nos docs após checagem no Client Portal. Ver §4.2.

#### novos-mercados-3 — Instrument spec (`sec_type`/multiplier/tick/lot/expiry) + CostModel por instrumento

- **Hipótese:** com o instrumento como entidade de domínio e o custo modelado por instrumento, o
  mesmo motor produz gate A comparável para qualquer mercado; efeito imediato: com custo por
  ativo (SLYV ≈ 3 bp, IWM ≈ 1,5 bp) o ranking de pares muda.
- **Verificado:** `Asset{symbol, asset_type, exchange, currency, tick_size, lot_size}` **já
  existe** em `trader-domain/src/entities.rs:200-211`; `ensure_asset` grava `tick_size` via
  `Decimal::from_f64_retain(0.01)` (`trader-infra/src/repositories/mod.rs:74`) → o banco tem
  `0.0100000000000000002081668171` nos 14 ativos (violação latente do "Decimal nunca f64");
  `slippage_bps` é `Option<u32>` (1,5 bp impossível; `backtest.rs:36`); walkforward não tem
  `--slippage-bps`; no simulador o slippage incide só em execução a mercado e **nunca no alvo
  limit** (`simulated/broker.rs:751-775`); a moeda-base da conta paper não está documentada em
  lugar nenhum (`ibkr/broker.rs:824` trata `BASE`; se for CAD, o cap de notional já está ~1,37×
  errado); os runs do gate A rotulados são 9 (413–421), não 6.
- **Refutação principal:** a "falsificação" da fatia 1 já está respondida pelo banco (ΔPF entre
  custos ≫ 0,1: IJS balance 2,53→2,22→2,08→1,95 na escada; ADR-016 tabela) — é engenharia
  disfarçada de teste; custo derivado de tick/spread continua premissa (só fills reais reduzem
  incerteza); re-rankear pares por 1 bp é ruído (variação 0,05–0,10 contra SD do PF 0,35–0,40 a
  n=20–40); a fatia 2 (L, 5 crates, 5–7 dias) não tem cliente — futuros/Kraken/forex estão
  falsificados — e arrisca regressão silenciosa no sizing que sustenta o gate A.
- **Conserto adotado (plano §5.6, tier A):** só a fatia 1, reescrita como "custo por ativo":
  `spread_bps` por ativo cobrado nos **dois** lados **inclusive no fill do alvo limit**, separado
  do slippage de mercado; comissão por ação (US$ 0,005, mín. US$ 1,00, IBKR Canada) em vez de US$
  0,35; `--slippage-bps` Decimal também no walkforward; teste de paridade reproduzindo os runs
  413–421 com STK/USD/2 bp; `tick_size = Decimal::new(1, 2)` e limpeza do banco; estender `Asset`
  em vez de criar `Instrument`, e só quando algum pré-teste de outra classe passar; documentar a
  moeda-base (`trader-cli account --provider ibkr`) antes de qualquer instrumento em CAD.

#### novos-mercados-4 — Ações individuais large-cap value (BRK.B, JPM, XOM, CVX, UNH, PG, HD, CAT)

- **Hipótese:** o edge é de cesta, não de ação — PF < 1,2 em ≥ 6 dos 8 nomes; resultado
  esperado negativo, "para encerrar a pergunta".
- **Verificado:** range mediano de barra 15m no banco: IJS 16,5 bp, SLYV 15,8, VBR 16,1, AVUV
  19,7, IWM 23,4 — barras de JPM/XOM/PG têm 15–25 bp como os ETFs, logo cost/R fica nos mesmos
  15–20% (o "6–10%" não decorre do tick); na IBKR a classe B da Berkshire é "BRK B" (com espaço)
  — `Contract::stock('BRK.B')` falha e o símbolo com espaço atravessa envs do compose; nenhum
  provedor de calendário no repo.
- **Refutação principal:** o teste não pode produzir um positivo credível (8 × 2 combos com n ≈
  25–40: P(≥ 3 nomes "passam" | nulo) ≈ 0,7) nem um negativo que mude decisão (negativo = o
  prior); "PF ex-earnings ±1 pregão" é grau de liberdade post-hoc; gaps de earnings/guidance
  (UNH em 2025 caiu dois dígitos em gap) dominam o P&L de backtest sem flatten; balance v1 (2%
  absoluto) quase não forma; 8 ingests no gateway de produção + calendário manual; compete por
  tempo com o único teste com chance (novos-mercados-1).
- **Motivo do arquivamento (plano §8, #35):** ver §4.11. Só 4 nomes de setores distintos (JPM,
  XOM, PG, CAT) como braço de controle do screening pooled, mesmo relatório, mesmo N; BRK.B fora.

#### novos-mercados-5 — Micro futuros de índice RTH-only (M2K/MES)

- **Hipótese (residual, após pré-teste negativo em balance/fade):** opening-reversal-v1 em M2K
  RTH com custo 1 bp e sizing por margem atinge PF ≥ 1,3 e DD ≤ 10% em walk-forward.
- **Verificado (três críticos, runs 561–572 no banco dev, sem label, criados 06/09 22:24–22:25
  UTC):** 2 bp → 1 bp: IWM balance 0,78 → 0,84 (23 t, avg R −0,24/−0,18), SPY balance 0,59 → 0,65
  (51 t, série até 06/08), IWM fade 0,39 → 0,42 (27 t, WR 18,5%), SPY fade 0,34 → 0,40, **IWM
  openrev 1,18 → 1,26** (36 t, DD 3,44%), SPY openrev 1,01 → 1,12. Gate A OOS da openrev em IWM a
  2 bp: run 417, 32 trades, PF 1,09/1,10 (`gate-a-revalidacao-2026-09-04.md:30`); agregado 1,11
  "não distinguível de 1,0". `main.rs:123-146`: walkforward sem `--slippage-bps`; cap 1×
  hardcoded (`risk/mod.rs:250-253`); flatten fixo (`paper.rs:2189-2190`); `classify_market_phase`
  fixo 13:30–21:00 UTC; scheduler com janela única.
- **Refutação principal:** o "passo 0" já está respondido — cortar o custo pela metade vale
  +0,08 de PF (0,2 desvio-padrão a n=36) e leva o OOS a ≈ 1,17 < 1,3; M2K RTH ≠ IWM (o nível de
  ontem que o mercado respeita inclui a Globex — outra regra sem fonte); sizing por margem com
  risco 1% leva o DD de 3,4% a 10–17% (viola ADR-010); margem intraday termina 15h45; MES = SPY
  (reprovado em tudo); XL (instrumento, sessão, rollover, margem, bundle US$ 10/mês, permissão,
  US$ 2.000 de Commodities NLV) para herdar um edge que o subjacente não tem.
- **Motivo do arquivamento (plano §8, #25):** ver §4.1. Rotular os runs 561–572.

#### novos-mercados-6 — Cripto passo 2: adapter de exchange registrada no Canadá (Kraken) com paper simulado local

- **Hipótese:** BTC/USD 15m tem estrutura de range/balance que paga 30 bp/lado com stop ≥ 3×ATR
  e alvo ≥ 2R (PF ≥ 1,3, ≥ 50 trades); expectativa do próprio designer: falha para a família
  intraday.
- **Verificado:** Kraken Pro tier 1 = **0,40% maker / 0,80% taker** (fee-schedule); tier 3 (US$
  10k+) 0,22/0,38; barra de 15m de BTC ≈ 30–50 bp; stop das v1 = 1 tick além da barra ou 0,3×ATR
  (~40 bp); OHLC da API devolve no máximo 720 candles (7,5 dias de 15m); sem sandbox (`validate=
  true` só valida); `--slippage-bps 30` como proxy de fee é otimista por construção (o alvo limit
  nunca paga slippage — `simulated/broker.rs:751-775`); sizing inteiro (`risk/mod.rs:239-263`,
  `trunc(capital/entry)`, `< 1` → `InsufficientBuyingPower`) rejeita 100% dos sinais com BTC >
  US$ 100k e capital fixo de 100k; `paper --provider kraken --broker simulated` não existe (Paper
  só tem `--mode simulated|replay|live`); 24/7 contradiz scheduler (entrypoint 09:25–16:10
  seg–sex), `classify_market_phase` (is_tradeable=false 17h/dia), flatten, rollover por dia UTC e
  circuit breaker (10 falhas → `live_stopped` de madrugada); ADR-003:25 já dizia "Binance/crypto
  fora do escopo inicial".
- **Refutação principal:** aritmética — ida-e-volta taker+taker = 160 bp ou taker+maker = 120 bp
  contra stop de ~40 bp → **cost/R 300–400%** (100–500% conforme o crítico); mesmo com stop ≥ 3×
  ATR (≥ 1,2%) e maker de 80 bp ida-e-volta, cost/R = 67%; a pullback morreu com 26%. Se a
  IBKR Canada tiver cripto via Zero Hash/Paxos a 12–18 bp/lado, a Kraken é dominada em custo — e
  12–18 bp/lado **ainda** matam stops de 20–30 bp. O gate B é impossível por definição sem
  sandbox. A variante "hold longo" não tem fonte analisada (Brooks Trading Ranges sem Fase 1) nem
  expressão no motor (TIF Day, sem GTC).
- **Motivo do arquivamento (plano §8, #26):** ver §4.2. Sem Fase 0/1.

#### novos-mercados-7 — Forex IDEALPRO (USD.CAD / EUR.USD)

- **Hipótese (já negativa no designer):** ATR(15m) de USD.CAD ≈ 4–7 bp contra ~1 bp/lado de
  custo → cost/R 30–50%; propunha um "spike de medição" após o Instrument spec.
- **Verificado:** `market_data.rs:54` fixa `what_to_show(Trades)`; `Contract::forex` e
  `WhatToShow::MidPoint` já existem no ibapi 3.1.0 (`contracts/mod.rs:345`, `historical/mod.rs:
  612`) — medir o range **não** precisa do Instrument spec (20 linhas num branch descartável de
  `debug-candles`); "nenhuma v1 é aplicável sem volume" é falso (as três ativas não usam volume
  em setup/entry/context; `relative_volume` devolve `None` sem bloquear); a sessão NY já é
  compatível com um spike; não há dados de forex no banco (assets = 14 ETFs); piso de 20 bp
  coerente com range mediano de barra 15m dos ETFs (11–23 bp).
- **Refutação principal:** o número é público e está 2–4× abaixo do piso — nenhum cenário
  plausível de medição reabre a linha; os outros bloqueios (sem volume/TRADES na IBKR, sessão
  24/5 fora do scheduler, nenhuma fonte de livro em forex 15m, relief OSC com sunset em ago/2026
  a confirmar, mínimo IDEALPRO US$ 25k/ordem) são independentes do número; um teste cujo
  resultado não pode mudar a decisão não deve ser executado; condicionar S a L é contabilidade
  errada.
- **Motivo do arquivamento (plano §8, #27):** ver §4.3. Fechado sem teste.

### 3.4 Lente "validação quantitativa e sizing" (AFML e afins)

Contexto da lente: pesquisa em Python (`trader-research/`, uv) consumindo o JSON de `backtest
--output`; o que virar critério de gate é portado para `crates/trader-backtest/src/stats.rs`.
Os três críticos concordam num ponto que muda a leitura de toda a lente: qualquer estatística
calculada **antes** do flatten no engine valida trades que a produção não executa.

#### validacao-quant-1 — PSR/DSR sobre o OOS + modelo nulo de bracket aleatório como gate A estatístico

- **Hipótese:** DSR ≥ 0,95 para a balance-area (edge sobrevive sem seleção de ativos) e < 0,95
  para a range-fade (aprovação depende de ter escolhido 3 entre 14 ativos correlacionados).
- **Verificado:** Sharpe = 0,00 nos 9 runs WF de 05/09 (`walkforward.rs` usa `from_trades`);
  `metrics.rs:175-231` anualiza por candle 15m; trials distintos (asset, config_hash): balance 30,
  fade 22, openrev 21, total 216; correlação open-to-close diária dos 7 ETFs ativos 0,805–0,996
  (IJS×SLYV 0,996). O "pool sem seleção" do designer **exclui IWM e SPY**, rodados com o mesmo
  hash e negativos: com eles, balance 10 ativos 311 trades PF 1,47; fade 8 ativos 229 trades PF
  1,06. Com a config atual nos 14 ativos: balance 447 trades PF$ 1,31 mas **PF_R 0,99**, avg R
  −0,007, PSR(0) 0,466 (ex-overnight PF$ 1,10); fade 383 trades PF$ 0,86, PF_R 0,84, PSR 0,095.
  OOS reconstruído dos 3 pares da balance (92 trades): σ_R 1,70, t = 1,52, **PSR 0,938** (ex-
  overnight 0,486; 19/92 overnight = 62% do P&L OOS); fade PSR 0,984 (ex-overnight 0,961);
  openrev 0,82. DSR: balance 0,12 (N=30), 0,56 (N=4), 0,81 (N=2); fade 0,007 (N=22), 0,36 (N=4),
  0,82 (N=2). P&L OOS da balance 99% em 2 meses (jul/25 +8.781, out/25 +6.985 de +15.957; 8/13
  meses positivos); fade top-2 meses = 40%, 11/16 positivos.
- **Refutação principal:** a hipótese está errada nos dois sentidos e o critério, aplicado hoje,
  **revogaria ambas as aprovadas** em qualquer N ≥ 2 (DSR 0,95 exigiria SR/trade ≈ 0,35, t ≈
  3,4 — com ≤ 100 trades nenhuma estratégia do projeto chega); a DSR é hipersensível ao conjunto
  de trials e o N é indefinível (as calibrações manuais dos docs — balance teto ATR 3×→10×, fade
  variantes A/B/C — são trials não registrados); o walk-forward sem re-fit não é OOS em relação ao
  desenho das regras; PSR/DSR assumem trades iid e não capturam a concentração mensal; o
  RandomBracket é o maior custo e é redundante com pool + bootstrap.
- **Conserto adotado (plano §5.3, tier A):** PSR (t, skew, kurtosis) por estratégia e do pool
  como **relatório obrigatório**; DSR reportado como faixa [N=2 … N registrado], **nunca**
  pass/fail; gate para dinheiro real = PSR ≥ 0,95 sobre os trades **live** do gate B; universo do
  pool e método de N_eff pré-registrados; contar como trials as calibrações documentadas;
  `slippage_bps` e `trial_group` no jsonb; f64 em `stats.rs` não viola AGENTS.md (§3.3 proíbe
  f64 só para dinheiro; `metrics.rs` já usa f64 no annualizer). Sequenciar depois do flatten.

#### validacao-quant-2 — IC95 por bootstrap em blocos + Monte Carlo de drawdown (e por que CPCV não se aplica)

- **Hipótese:** IC95 do PF da balance ≈ [1,3; 3,0] e da fade ≈ [1,25; 3,5], limite inferior
  encostado no gate; DD p95 por MC em 4–6% (balance) e < 3% (fade); CPCV é degenerado para regras
  sem fit.
- **Verificado:** IC analítico reproduzido — balance OOS [1,39; 3,15], fade [1,27; 3,48], openrev
  [0,66; 1,85] (inclui 1); bootstrap iid 1,22/1,19/0,62; MC de DD (100k, sizing atual) p95 balance
  3,8%, fade 1,8–1,9%, openrev 5,2–5,3%; ex-overnight balance vira [0,98; 2,51] analítico /
  [0,89; 2,65] bootstrap — inclui 1. CPCV degenerado confirmado de forma mais forte: o próprio
  walk-forward é degenerado (OOS = run completo menos bloco 1; `engine.rs:184` passa
  `StrategyState` default). 11 perdas seguidas em AVUV (run 416) somam ~2,4% de equity.
- **Refutação principal:** o veredito **flipa conforme o esquema de reamostragem**, que a
  proposta deixa em aberto: balance OOS PF 2,09 tem IC95 iid [1,24; 3,47] mas bootstrap em
  blocos (≈ 5 dias com trade) **[0,87; 4,91]** → reprovada; fade iid [1,20; 3,90] vs blocos [1,39;
  3,57] → aprovada nos dois. O designer errou a previsão justamente na balance, porque 99% do P&L
  vem de 2 meses. "DD p95 ≤ 10%" é não-vinculante com o sizing atual; "limite inferior ≥ 1,0" é
  fraco (PF 1,0 a 2 bp é break-even sobre custo calibrado por spread).
- **Conserto adotado (plano §5.3):** bootstrap estacionário (Politis-Romano) sobre P&L **diário**
  de todos os pregões (zeros incluídos), blocos de 5 e 10 pregões, 10k reamostras, seed fixa,
  lado a lado com iid — pré-registrado; reamostrar (R, risk_amount) juntos; concentração (share
  dos 2 melhores meses, meses positivos, share do melhor dia) obrigatória; MC de DD só com os
  modos de sizing de #6; descartar purge no walk-forward; Python primeiro. Gate A proposto: IC95
  em blocos do PF ≥ 1,0; PF_R ≥ 1,2; share dos 2 melhores meses ≤ 60%; pass no holdout.

#### validacao-quant-3 — Meta-labeling sobre balance-area e range-fade com purged K-fold/CPCV

- **Hipótese:** regressão logística com ≤ 8 features do `market_snapshot` atinge AUC OOS ≥ 0,60
  e, filtrando o tercil inferior, remove ≥ 40% dos stops e ≤ 20% dos alvos.
- **Verificado:** snapshots existem (`balance_area_breakout_v1/entry.rs:107-121`, `range_extreme_
  fade_v1/entry.rs:95-108`) mas os trades de backtest saem com `journal: {}` e `strategy_id
  'unknown'` (`simulated/broker.rs:851-893`); o dev tem 293 sinais, todos da pullback; dataset
  real ≈ 237–311 (balance) e 163–229 (fade) trades; `docs/books/analysis/lopez-afml.md:44`: meta-
  labeling serve para "primário com alto recall e baixa precisão" — não é o caso; "range dos 5
  pregões" não está no snapshot da balance.
- **Refutação principal:** (a) rótulo contaminado pelo overnight (19/92 OOS = 62% do P&L) — o
  modelo aprenderia "entre tarde e segure overnight"; (b) sem poder: SE(AUC) ≈ 0,04–0,08, a faixa
  "≥ 0,60 aprova / < 0,55 abandona" cabe em 1 SE; (c) o primário do pool de 14 não tem edge em R
  (PF_R 0,99 e 0,84) — AFML §3.7: meta-labeling não cria edge; (d) uma variável trivial já
  explica a estrutura: tercil de stop ≤ 17 bp WR 29%/PF 0,53, 17–28 bp WR 53%/PF 1,99, > 28 bp
  WR 58%/PF 3,05; (e) reduz o fluxo do gate B em 35–40% numa operação com 0 trades das
  aprovadas desde 18/08; (f) purged K-fold por dia não impede vazamento de regime (jul/25 e
  out/25).
- **Arquivamento condicional (plano §7, #23):** ver §4.12. O plumbing snapshot → journal +
  `strategy_id` nos trades de backtest (O16, ~0,5 dia) entra em #3 desde já.

#### validacao-quant-4 — Sizing A/B/C: PF em R ao lado do PF em $, `max_notional_multiple` configurável

- **Hipótese:** o cap de notional 1× prende em ~100% dos trades, logo o $ arriscado é ∝ distância
  do stop; o PF em R da balance OOS é ≈ 1,3–1,5, bem abaixo do PF em $ de 1,96; igualar o risco
  (modo B) sobe o Sharpe; modo C (0,5%/2×) dobra o $ com DD ≤ 10%.
- **Verificado (confirmado com folga — "o achado mais sólido do lote"):** `risk/mod.rs:239-279`
  (`qty = min(orçamento/dist, capital/entry)`, cap 1× hardcoded na :253 com comentário "Melhoria
  futura"; `RiskConfig` é `Copy`); balance OOS 3 pares **PF$ 2,09 vs PF_R 1,41**; VBR 1,27 (PF$
  2,16), AVUV **1,07** (1,85), IJS 2,37 (2,36); corr(risk_amount, R) = 0,43 VBR, 0,49 AVUV, 0,31
  pool (limiar 0,15); ex-overnight pool PF_R 0,99, avg R −0,006 — a covariância tamanho×resultado
  **é** o overnight; fade PF_R 1,78 vs 2,10 (corr 0,16); openrev inverte (PF_R 1,28 > PF$ 1,11,
  corr −0,23); risk$ p10/50/90 = 131/231/577; notional/capital mediano 1,05.
- **Refutação principal:** o desenho de B e C não faz o que promete — com cap 1× o risco máximo
  é stop% × equity, então "0,25% com cap 1×" só iguala o risco em stops ≥ 25 bp (mediana 22 bp,
  p10 12) e é quase o status quo; igualar o $ por trade coloca **mais** dinheiro nos stops
  estreitos, que perdem — "Sharpe sobe em B" tem boa chance de falhar. Modo C é inviável: com
  equity ~US$ 238k, 1× já é 87% da barra mediana de SLYV e 62% de IJS (a 2× vira 174%/124%);
  `exposure_limit_hit` (`paper.rs:1490-1517`) bloqueia a conta a 200% — a 1ª posição a 2× fecha a
  conta para as outras 7 instâncias; `BUYING_POWER` é lido e nunca usado; short a 2× dobra o
  aluguel que já falhou.
- **Conserto adotado (plano §5.2 e §5.5, ADR-020 proposto):** PF_R, sharpe_R e corr(risk, R)
  no relatório e no gate A desde já (AVUV/VBR ficam abaixo de 1,3 em R); modos A = atual, B =
  0,15%/1×, B' = 0,25%/2×, C = 0,5%/4× (só estudo), reportando a fração de trades presos no cap;
  cap de **liquidez** (≤ 1/3 da barra mediana) e `buying_power` como teto duro; rodar sobre o
  engine com flatten; cruzamento stop_bp × resultado como candidato a regra de v2 (stop ≥ 20 bp).

#### validacao-quant-5 — Kelly fracionário com shrinkage e escada de risco condicionada a PSR/DSR e gate B

- **Hipótese:** com edge real, ¼-Kelly de portfólio fica em 0,5–1,0% por trade; subir de 0,2%
  para 0,5% gera +US$ 500–900/mês (balance) e ≈ +1,7k/mês no total.
- **Verificado:** f* balance 0,137 (p 0,467, b 1,61) reproduz; fade b realizado = **1,07** (não
  1,23) → f* 0,27 (não 0,32); openrev f* 0,10; **ex-overnight a balance tem f* = −0,003** (PF_R
  0,99); `risk_per_trade_pct` é override por estratégia e **entra no `config_hash`**, e
  `analyze` usa `latest_by_strategy` sem filtrar hash — degraus diferentes se misturam; ADR-010
  gate C já prescreve 0,25–0,5% no primeiro mês real; ADR-017:74: padrões para "conta paper com
  margem".
- **Refutação principal:** a escada está morta na chegada pelas próprias regras — exige PSR/DSR
  ≥ 0,95 e a balance tem PSR 0,938 e DSR ≤ 0,81 (N=2); o único degrau que gera os +1,7k (0,5%/2×)
  é inviável em SLYV/IJS e reduz a conta a 1 posição; 0,25%/1× é matematicamente o status quo;
  tudo é paper; Kelly em 64–92 trades é dominado por erro de estimação e, após shrinkage, ÷3 por
  correlação e ¼, vira "fração fixa 0,25–0,5%" — o que Grimes recomenda sem Kelly; a conta paper
  enche a NBBO sem impacto, então a escada seria "validada" num broker que não cobra o tamanho.
- **Arquivamento condicional (plano §7, #24):** ver §4.13. `analyze` pode imprimir risco
  realizado médio, PF_R e f* shrunk como linha informativa; mover `risk_per_trade_pct` para fora
  do `config_hash` antes de qualquer degrau. Não viola "sem martingale" (anti-martingale).

#### validacao-quant-6 — Correlação entre instâncias e teto por cluster / alocação por contribuição de risco

- **Hipótese:** as 8 instâncias são ≈ 1 aposta (N efetivo ≈ 1,1); um teto por cluster (máx. 1
  posição por direção) reduz o max DD ≥ 30% perdendo ≤ 15% do $; a 2ª posição do dia tem PF menor.
- **Verificado:** correlação de preços confirmada (open-to-close diária 0,805–0,996). Mas o P&L
  diário **não** se comporta assim: dentro da balance corr 0,17–0,51 com só 7–10 dias em comum
  (outro crítico: 0,46–0,66); fade ≈ 0 (0,01–0,08); entre estratégias −0,05 a +0,21. 139 dias
  com trade, 56 com ≥ 2 entradas (40%; previsto ≥ 50%), 38 na mesma direção. **Dias com 1
  entrada: balance PF 0,70/avg R −0,23 (n=32); dias com ≥ 2 entradas (100% na mesma direção):
  PF 2,69/avg R +0,44 (n=64)**; fade 1,46 vs 4,64; openrev 0,96 vs 1,92; a 2ª entrada do dia tem
  PF 3,36 (balance) e 12,3 (fade) contra 1,52/1,57 da 1ª. `exposure_limit_hit` conta só posições
  (não ordens pendentes); o snapshot de posições da IBKR devolve `stop_price = 0`
  (`ibkr/broker.rs:886`); as 8 instâncias são processos independentes sem coordenador.
- **Refutação principal:** o rompimento simultâneo no cluster é **confirmação, não
  redundância** — um teto de 1 por direção manteria a pior entrada e jogaria fora a maior parte
  do P&L (o replay do designer: −42 trades/−US$ 11.556 = 48%); "priorizar o sinal de maior p" é
  impossível sem árbitro; "risco aberto ≤ 0,5%" não é computável; a ERC por estratégia que
  realoca de IWM+openrev/IWV+fade para IWN+balance/VB+openrev é re-seleção de pares após a tabela.
- **Conserto adotado (plano §6.4, §2.3 achado 6) / arquivamento (#30):** manter só o replay de
  portfólio com capital único e regras do ADR-017 (DD/Sharpe de portfólio — o número que falta
  para o gate C), com a corrida de 30 s modelada; **descartar o teto por cluster** (§4.6); ler o
  ADR-017 como "permitir o cluster"; hipótese invertida como candidata a filtro de breadth
  (exige contexto multi-símbolo, C4). Se algum dia adotado, "máx. N na mesma direção" é a única
  versão computável (`Position.direction` via `get_positions`).

#### validacao-quant-7 — Gate de regime por compressão de range (range5 < 1,0%) / ADX diário / HMM

- **Hipótese:** PF em compressão < 1,0 e em expansão ≥ 2,0; o gate remove ≥ 25% dos trades e ≥
  40% das perdas, subindo a balance de 1,96 para ≥ 2,3 e a openrev de 1,11 para ≥ 1,4.
- **Verificado:** a premissa "59–91% dos dias < 1% nos pares vivos" está errada no banco: AVUV
  26%, IJS 26%, VBR 33%, IWN 24%, SLYV 24,5%, IWM 20% — só IWV (61,5%) e SPY (57%); com média
  móvel de 5 pregões < 1,0%: AVUV 10%, IJS 9%, VBR 20%, IWM 7%, IWV 57%. Ago/26 confirmado como
  compressão (IJS 0,82% vs 1,07–3,54% nos demais meses; 61–91% dos dias de ago/26 < 1%).
  Juntando cada trade com o range5: balance compressão n=25 **PF 1,67, +US$ 2.947** (avg R
  0,01); 1,0–1,2% n=35 PF 2,38; expansão n=34 PF 1,92–2,16; fade compressão n=13 PF 1,02 (+18),
  expansão 1,74–2,00, tercil alto 1,28 (o pior); openrev compressão n=3, expansão PF 1,07–1,27,
  tercil alto PF 0,88 (o oposto do previsto). `validate()` não recebe candles
  (`risk/mod.rs:91-98`); ADX(14) diário precisa de ~28 pregões e o live entrega 23.
- **Refutação principal:** as três previsões pré-registradas falham; a cláusula de abandono do
  próprio designer ("se PF em compressão ≥ 1,3, o gate é abandonado") dispara para a balance
  (1,67); o gate removeria 27% dos trades e **perderia US$ 2.947**; relação não monotônica
  (tercil médio é o melhor) — assinatura de ruído com 13–35 trades por balde; limiar absoluto em
  % é filtro de ativo disfarçado; em ago/26 já houve 0 trades pelo `is_tradeable` — não há
  stop-out a evitar; um único episódio de compressão.
- **Motivo do arquivamento (plano §8, #29):** ver §4.5. Sobra só `range5_pct`/ADX como feature
  auditável no snapshot (#17), sem poder de veto.

### 3.5 Lente "execução e custo"

Contexto da lente: o "custo de 2 bp" não é spread — um centavo vale 0,23–0,83 bp nos ETFs
operados; o custo real vive em latência, gap/overshoot pós-envio e R pequeno demais (4 bp sobre
stop de ~22 bp = 0,18R). Os críticos redirecionam a lente: antes de latência, **integridade do
feed** (§5, achado 7).

#### execucao-custo-1 — Medir o custo real por fill em produção e recalibrar os 2 bp por símbolo, horário e perna

- **Hipótese:** a perna de entrada paga ≥ 3 bp e cresce com a iliquidez da barra; stop e alvo
  pagam < 1 bp; se a mediana ficar ≤ 2 bp em todos os pares, a calibração atual fica.
- **Verificado:** 26 fills do dev, todos `commission = 0`, todos de 04/08 (IWM fill 302,19 vs
  gatilho 302,09 = +3,3 bp = 19% do R; stop 301,56 vs 301,57 = 0,3 bp; SPY +10/+11 bp);
  sensibilidade ADR-016 −1.575/−983/−1.052 por bp; `apply_slippage` já distingue entrada/saída;
  filtro `max_spread_pct` morto por `quote = None` (`risk/mod.rs:185-196`); o casamento do
  `CommissionReport` **existe** desde `e9be836` (07/08) em `ibkr/broker.rs:366-399`, mas só se
  ele chegar no **mesmo lote** do poll; `signals.market_snapshot` não carrega bid/ask; produção
  tem 17 ordens e 6 trades até 02/09 (não "30–40 fills desde 18/08"); o simulador usa US$
  0,35/perna (`simulated/broker.rs:109`) — a IBKR Canada cobra US$ 0,005/ação, mín. US$ 1,00
  (~US$ 4–5 por perna de 800–950 ações ≈ 0,9 bp ida e volta); `ops.yml` não tem ação SQL.
- **Refutação principal:** o critério exige p50/p75 por símbolo×hora×perna com células de n=1–3
  — infalsificável com a amostra que vai existir; fills da paper são simulados contra o NBBO sem
  fila nem impacto — em SLYV/IJS a medição **subestima** sistematicamente; com feed esparso
  (§5.7), |fill − gatilho| mede o high/low que o feed não viu; a query de latência da proposta
  irmã usa `signals.timestamp` (= `Utc::now()` no sinal, `entry.rs:129`) e dá −899 s em todas as
  8 ordens do dev; "compartilhar dados com a paper" não é custo zero (herda o que o live assina;
  bundle US$ 10/mês; isenção exige US$ 30 de comissões) e pode nem ser a alavanca (o TWS no PC
  entregava barras completas sem assinatura).
- **Conserto adotado (plano §5.6 e §5.8, tier A):** provar o feed primeiro; regra de decisão por
  **faixa de liquidez** (> e < US$ 2M/barra), n ≥ 30 fills de entrada por faixa; paper como piso;
  modelo calibrável hoje (slippage de entrada = k × range da barra de gatilho, varrido em k);
  comissão por ação no simulador; corrigir o casamento do `CommissionReport` entre polls; gravar
  `signal_bar_close_ts`, `bid/ask`, `submit_latency_ms` em `orders.metadata`; ação read-only
  `sql` no `ops.yml` ou dump diário.

#### execucao-custo-2 — Cortar a latência de envio de 4–6 min para < 60 s (realtime na paper + poll alinhado)

- **Hipótese:** o live só entrega a entrada 4–6 min após o fechamento da barra; nesses minutos o
  preço anda ~13 bp, da ordem do stop; ≥ 30% dos gatilhos são tocados nesses minutos.
- **Verificado:** ordem 17 (VBR 28/08) enviada 16:06:15 UTC para barra fechada 16:00;
  `LIVE_POLL_SECS = 30`, `MAX_BAR_SETTLE_POLLS = 30` (`paper.rs:693,713`), guarda de 2 polls
  idênticos; no dev com TWS/PC a latência era 34–55 s (`market_contexts.created_at −
  fechamento` p50 29–54 s em 04–07/08); `subscribe_realtime_bars` implementado e não usado
  (`market_data.rs:76-120`); o adapter abre um `Client` TCP por fetch; |close−open| mediano de
  uma barra de 15m em 2026 é **5,4–7,4 bp** (não 13 — o designer usou o range), salto de
  abertura entre barras 1,6–3,3 bp; o CB de IWV derrubou uma instância em 04/09 por 10 falhas de
  fetch; o Gateway entrega barras com 3–10% do volume e 15–25% do range dos dias ingeridos pelo
  PC.
- **Refutação principal:** a ordem 17 **não** foi perdida por latência (o preço ficou 17–35c
  abaixo do gatilho por 30 min; é A9); "≥ 30% tocados" é intestável (351 candles de 5m); o
  "$200–600/mês" deriva de um overshoot médio 0,12R que ninguém mediu; poll de 15 s coincide com
  a regra "idênticas < 15 s" e 8 instâncias já fazem 160 req/10 min; o que mudou em 07/08 foi
  **TWS → IB Gateway headless**, não a assinatura — cortar o poll só entrega a mesma barra ruim
  mais cedo; `paper.rs` é o arquivo do incidente de 03/09.
- **Conserto adotado (plano §5.8):** reescopar como "integridade do feed + latência": (1)
  `debug-candles` no Gateway do servidor vs TWS do PC para a mesma barra; tipo de market data
  (Realtime/Delayed/Frozen); (2) lag de todas as barras via `market_contexts.created_at −
  (timestamp + 15 min)`; (3) `signal_bar_close_ts` e `submit_latency_ms` em `orders.metadata`;
  (4) só então poll alinhado (20 s, nunca < 15 s) ou `realtime_bars` como gatilho; (5)
  `BacktestConfig.entry_starts_next_candle` como cenário pessimista; (6) assinatura só se o
  TWS/PC não resolver.

#### execucao-custo-3 — Paridade de fim de sessão: flatten no backtest + MOC em grupo OCA no live

- **Hipótese:** com flatten no backtest a balance-area não passa o gate A em VBR/AVUV e a
  range-fade não muda; no live, MOC no lugar do MKT 15:55 faz o fill = close oficial.
- **Verificado:** idem edge-existente-1 (é a mesma proposta por outro designer), com correção de
  número: "PF 1,92 → ~1,43" era **descartar** os 20 overnight; **fechando-os** no close da 15:45
  com 2 bp, os overnight rendem +1.658 em vez de +9.577 e a balance fica PF 1,55 / WR 42,7% / avg
  R −0,007 (SE 0,13, t = −0,05) — reprova pelo avg R, não pelo PF; IJS sozinha passa. A
  range-fade **muda**: 9/69 overnight = 32% (5 longs no alvo, 4 shorts com 3 stops) → PF 1,74,
  avg R 0,218 (t = 1,7). Os 20 overnight da balance são 10 long/10 short, 14 alvos/6 stops. MOC:
  a IBKR documenta que NYSE/Arca recebem MOC até 15:50 e **não permitem cancelar após 15:45**
  (Nasdaq 15:55/15:50); todos os 7 ETFs operados são NYSE Arca; `oca_group` não aparece em lugar
  nenhum do adapter (pernas ligadas só por `parent_id`, `ibkr/broker.rs:668-705`); `Broker` não
  tem modify; o label `walkforward-oos-6w-eod` não bate (`walkforward.rs:156` grava label fixo).
- **Refutação principal (metade MOC):** uma MOC enviada às 15:49 é irrevogável; se stop ou alvo
  encher entre 15:49 e 16:00, a MOC executa no leilão e **abre posição invertida** — a família do
  incidente de 03/09 (3 vendas enfileiradas → −1.654 ações); a alternativa "cancelar TP/SL antes
  de 15:45 e mandar MOC" deixa a posição sem stop por 15 min; ganho ≈ 0,5–1 bp em ~20% dos trades
  de uma estratégia ≈ US$ 30–60/ano por instância.
- **Conserto adotado (plano §5.1) / arquivamento (#33):** flatten no engine como em
  edge-existente-1; reportar o gate A com avg R por estratégia; registrar que a range-fade também
  perde 32% e que o gate B em curso compara com um backtest que "dorme comprado"; **descartar
  MOC/OCA** (§4.9) — manter MKT às 15:55 e modelar 2 bp no flatten do backtest.

#### execucao-custo-4 — Entrada STP LMT com limite = gatilho + tolerância × risco (paridade pós-envio)

- **Hipótese:** o STP puro enche a qualquer preço após enviado onde o simulador cancela (open >
  gatilho + 0,25×risco); 5–10% das entradas são canceladas por overshoot no backtest; no live
  viram fills com risco 1,5–2×.
- **Verificado:** simulador cancela na abertura (`simulated/broker.rs:245-283`); IBKR rejeita
  StopLimit isolado (`ibkr/broker.rs:61-63`) mas o ibapi 3.1 aceita 'STP LMT' e `map_order_data`
  já o reconhece; a taxa de invalidação por overshoot no backtest não é 5–10%: **21,9% (IJS),
  10,0% (VBR), 2,8% (AVUV)** na balance e 3,3%/8,7%/24,0% (AVUV/SLYV/IWV) na fade (21 de 186
  acionadas); mas o live tem uma guarda **diferente**, pré-envio, sobre a barra em formação (`paper.rs:1352-1356`); a guarda pré-envio nunca dispara no backtest, porque `reference_price` é o close da barra de sinal, sempre aquém do gatilho (`engine.rs:200-203`; 0 disparos em 10 logs de backtest). São guardas assimétricas: cada lado invalida entradas que o outro aceita
  (`execution/mod.rs:93-117`), que funcionou em 01/09 (AVUV 0,05 > 0,04); os casos-símbolo (trade
  12 de 20/08, trades 7/8 de 04/08) são anteriores ao ADR-015; a tolerância ≈ 5,5 bp é excedida
  pelo salto de abertura entre barras em 30% das barras de IJS e 33% de SLYV (mediana 3,2–3,6 bp
  — bounce bid-ask); `expire_stale_stop_entry` ignora entradas ≠ Stop (`paper.rs:1630`);
  `build_bracket_order` não tem acesso à tolerância; ~9 arquivos (esforço M, não S).
- **Refutação principal:** a assimetria residual (salto entre o envio e o primeiro print) é
  intradiária, rara e **não foi medida**; a conta de impacto é inventada; um limite a 5 bp do
  gatilho num rompimento perde os rompimentos que correm; STP LMT de US$ 238k em SLYV produz
  fills parciais, que o live trata como anomalia; "v1 nunca muda" — muda o preço de entrada da
  v1 (config_hash). Argumento mais forte a favor, que a proposta não usa: com o feed esparso, o
  STP LMT seria a única proteção real contra encher a mercado num preço que o bot não viu.
- **Conserto adotado (plano §6.7, tier B/C):** instrumentar `entries_triggered`/
  `entries_cancelled_overshoot` no `BacktestReport`; medir em produção a fração de fills com
  (fill−gatilho)/risco > 0,25 **após** o ADR-015; só se > 2% dos fills, `EntryOrderType::
  StopLimit` com `limit_offset = max(k × dist_stop, 2 × spread)` (k = 0,5 rompimentos, 0,25
  fades), modo `cancel` no simulador, expiração no live, fill parcial como caso suportado, ADR de
  execução v1.1.

#### execucao-custo-5 — balance-area-v2 swing 1–3 pregões com bracket GTC e flatten condicional

- **Hipótese:** o edge da balance está no "much bigger move" multi-dia (20 overnight = +9.577;
  entradas 13–15h ET = +7,9k); com alvo 3–4R e hold ≤ 3 pregões o avg R sobe de 0,21 para ≥ 0,35.
- **Verificado:** overnight 20/96 simétrico (long 10 avg R 1,36 / short 10 avg R 1,06; 14
  alvos/6 stops; 13 duram 1 pregão, 3 duram 2, máx 5); intraday da balance avg R −0,013 (long) e
  −0,109 (short); os 14 alvos overnight têm R de 1,4 a 4,7 **só porque o alvo limit enche na
  abertura em gap a favor** (`max(open, target)`); os 6 stops −1,1 a −3,5R; gaps por dia ET
  (IJS/VBR/AVUV/SLYV): > 50 bp em 31,6–38,2%, > 100 bp em 10,7–12,3%, > 200 bp ~2%, máx 3,6–4,4%;
  TIF Day fixo em `execution/mod.rs:193` e `ibkr/broker.rs:615-634`; scheduler para às 16:10 e
  sobe às 09:25 (bot cego no pré-mercado); `flatten_session` incondicional; rollover por dia UTC;
  posição carregada conta nas 3 posições/200% às 09:30 e bloqueia a janela da openrev.
- **Refutação principal:** n = 20 num único regime bull (WR 70% com IC95 ≈ 46–88%; SE do avg R ≈
  0,5R); "os overnight são vencedores" é condicionamento post-hoc no `exit_time` (não é
  selecionável ex-ante); a conta de gap está errada por uma ordem de grandeza — segurar todos os
  trades multiplica as noites expostas ~6–10× (~190 noites/18 m vs 20 hoje), ~14 gaps > 1%/ano,
  metade adversos a −4,5R ≈ **−30R/ano contra +13R/ano** que a v1 inteira gera; a variante swing
  na re-simulação do designer rende só +15% sobre a v2 intradiária; Reg T overnight 50% limita o
  agregado em 200%; short overnight depende de aluguel (A9); esforço L com gate A do zero.
- **Conserto adotado (plano §7, tier C):** estudo de backtest sem motor, depois de #1 e #11:
  tabela P&L × dias de holding × hora de entrada × exit_reason com o modelo de gap que o simulador
  já tem; UMA variante pré-registrada (hold ≤ 1 noite, alvo 2R, flatten no dia seguinte 15:49) em
  ≥ 5 pares, ≥ 50 trades overnight, sensibilidade a 4 bp; stop alargado a 1×ATR diário como
  segunda variante única. Só abrir ADR-018-bis/GTC se avg R pooled − 2×SE > 0,15.

#### execucao-custo-6 — Varredura de alvo (1,5R → 4R) e alvo estrutural + métrica "custo em R"

- **Hipótese:** alvos maiores ou estruturais elevam o R capturado por bp pago; na fade, alvo =
  min(2R, lado oposto do range) sobe o avg R de 0,30 para ≥ 0,38; na balance intraday 3R não sobe.
- **Verificado:** RR planejado mediano 1,67–1,73 (balance) e 1,04–1,27 (fade; IWV 1,04); avg
  win/loss da fade ≈ 1,22–1,34 / −1,12 a −1,16; 4 bp/22 bp = 0,18R; alvo no lado oposto do range
  já é calculável em `analyze` (`day_extremes_before_signal`, `BalanceArea{high, low}`); a v1 já
  testou nos mesmos trades: lado oposto PF 1,82 vs 1,74 (vivos), 1,08 vs 1,02 (expansão), WR −9
  pontos, 23/69 terminam no flatten — marginal; `load_strategy` casa o id exato → `--strategy
  range-extreme-fade-v2-t20` falha ("estratégia desconhecida") e o CLI não tem `--strategy-
  config`; a sensibilidade ao custo do ADR-016 é **−18% (balance) e −25% (fade)** em P&L, não
  "29%/14%".
- **Refutação principal:** a previsão central (+0,08R) é indetectável — o avg R da fade tem SE ≈
  0,14 (n=69); ≥ 12 variantes × 8 pares reabre a superfície de seleção (42+42+66); o edge da fade
  é taxa de acerto (WR 61–70%) — alvo 3–5R sobre stop de 21 bp derruba o WR; a erosão do RR
  realizado vem do fill em `max(open, gatilho)` na entrada, que o alvo não toca; impacto ≈ +US$
  350/ano/instância, abaixo do ruído de 1 bp de calibração.
- **Conserto adotado (plano §5.2):** implementar já `cost_total`/`cost_r_per_trade`/
  `avg_r_gross`/`realized_rr` em `metrics.rs` (valem por si); `--strategy-config <path>` para
  varrer `target_r_multiple` sem código; no máximo 3 variantes por estratégia, pré-registradas,
  desenho **pareado** (mesmas entradas, diferença de R contra 2×SE); só com EOD e comissão por
  ação. A varredura em si não está na Onda A nem B.

#### execucao-custo-7 — Entrada limit passiva na range-fade-v2 + modelo realista de fill limit no simulador

- **Hipótese:** uma limit no close da barra de sinal compra ~7 bp mais barato numa barra de 21
  bp, eleva o RR realizado de 1,22 para ≥ 1,6 e perde ≤ 30% dos fills por seleção adversa.
- **Verificado:** o simulador enche qualquer entrada limit **na hora** em `order.price` ± slippage
  sem olhar o mercado (`simulated/broker.rs:522-575`) — os 8 trades limit do ADR-009 (SPY, PF
  5,42 vs 6,96) não valem nada; a fade entra em high+1 tick / stop em low−1 tick
  (`entry.rs:34-38`); Brooks *Trading Ranges* sem análise Fase 1 em `docs/books/analysis`.
  Reconstruindo a barra de sinal dos 69 trades da fade (3 pares, 2 bp) e uma limit no close:
  **todos os 27 perdedores teriam sido tocados (100%) e no máximo 31 dos 42 vencedores (74%;
  69% antes da barra de saída)** — limite superior, porque toque ≠ fill.
- **Refutação principal:** viola "nenhuma regra sem fonte" (contradiz a fonte da v1 — Brooks:
  stop 1 tick além da barra de sinal); a seleção adversa é mensurável e grande; 12–19 trades/ano/
  par × 0,7 → ~45 trades in-sample nos 3 pares, < 50 OOS mesmo agregado; o fill de uma limit de
  US$ 238k em SLYV não é derivável de OHLC; com latência de 30 s–6 min a limit descansa depois de
  o preço já ter voltado (lição do ADR-009); R cai 33% e o custo relativo **sobe** para 29% do R
  — contraria a lente; `expire_stale_stop_entry` nunca expiraria a limit; a guarda de overshoot
  isenta Limit → limit marketável enche a mercado.
- **Motivo do arquivamento (plano §8, #31):** ver §4.7. Salvar só a infra: simulador de entrada
  limit realista (fill se low ≤ limite − 1 tick / high ≥ limite + 1 tick, preço `min(open,
  limite)`, sem slippage, expira por validade) — pré-requisito de qualquer entrada limit futura e
  corrige o ADR-009 (`docs/cto-plano-lucratividade-2026-09.md` §2.6).

### 3.6 Lente "construção de portfólio e cobertura de regimes"

Contexto da lente: o livro é estruturalmente comprado, concentrado em small-caps com correlação
diária 0,96–1,00 e opera 0,60–0,74 trade/pregão. O designer replayou offline os 232 trades/US$
24.063 das 8 combinações vivas (runs 530/532/534/543/545/547/536/538) com as travas da conta.
**Só o crítico de estatística rodou nesta lente na primeira passagem** (§1.4); os vereditos de
código e operação estão no journal da retomada.

#### portfolio-regime-1 — Replay de portfólio com conta compartilhada como ferramenta oficial de gate

- **Hipótese:** aplicando as travas reais (3 posições, notional agregado, uma por símbolo,
  flatten 15:55) o P&L e a contagem divergem em > 10% do somatório dos backtests isolados.
- **Verificado:** 232 trades/US$ 24.063 = soma exata dos 8 runs; flatten reproduz US$ 19.380
  (PF 1,66; bab 6.730/1,55, ref 4.578/1,74, orv 8.072/1,74); 1 posição/100% → US$ 12.286 (−49
  trades/−11.777); 2 posições a 200% → −7 trades/−3.320; máximo simultâneo em 18 meses = 3; pior
  dia 16/03/2026 −US$ 2.032 (dois shorts openrev IWM+IWN às 10:45, corr 0,97); 20/01/2026
  −1.683 = balance IJS long segurado 5 dias (feriado MLK) — impossível no live; mensal: jun–out/
  25 +24.564, nov/25–ago/26 −178 (com flatten +14.947 e +4.356); 7/19 meses negativos; maxDD
  combinado US$ 5.346; JSON do backtest tem os campos necessários mas `strategy_id 'unknown'`;
  `find_exposure` bloqueia por posição **ou ordem aberta** (`paper.rs:2047-2066`).
- **Refutação principal:** H1 já está "confirmada" antes de construir a ferramenta (−19% só de
  flatten) — é infra, não hipótese; o maior efeito (flatten) é o O1 do motor (< 1 dia, com
  paridade) e o replay o reimplementa fora; colisões subestimadas (não modela a janela de ordem
  pendente); o preço de flatten usado é o close da 15:45 (= 16:00), não 15:55; a "correlação ≈ 0
  entre estratégias" é artefato de séries diárias com zeros em ~85% dos dias (bab 55 dias com
  trade, ref 59, orv 56 de 385; interseções 9–14).
- **Conserto adotado (plano §6.4, tier B):** O1 primeiro; replay só para as travas de conta,
  como função pura `portfolio.rs::replay(...)` + `trader-cli portfolio`, incluindo a janela de
  ordem pendente (= `entry_validity_candles`) e a corrida de 30 s; correlação condicionada aos
  dias em que ambas operam (n ≈ 9–14, admitir que é inestimável); flatten = open da 15:45 ou
  close da 15:30. Vira a referência do gate B (trades esperados **após** travas) e o número que
  falta para o gate C.

#### portfolio-regime-2 — `capital_fraction` = 1/`max_concurrent_positions` por instância

- **Hipótese:** dimensionar sobre equity/3 elimina os trades derrubados pelo teto de notional sem
  reduzir o P&L em mais de 5%; corolário: "máx. 1 posição por direção" destrói o edge (−42
  trades/−US$ 11.556).
- **Verificado:** `risk/mod.rs:239-254` (`qty_by_notional = (capital/entry).trunc()`; capital =
  `summary.equity` da conta inteira, `paper.rs:481`); `default.toml` [risk] 3 posições/200%;
  ADR-017:71-75 (dinheiro real: 100% notional); evento `portfolio_limit` existe e tem 0
  ocorrências no dev; o replay "max 3 pos derruba 1 trade" foi rodado com `max_notional = 1e12`
  — só testa "uma por símbolo"; a equity de ~240k não tem fonte em `docs/` além do HANDOFF:215
  (237.892).
- **Refutação principal:** H1 é falsa por aritmética — a fração reduz **todos** os trades: 2
  posições a 200% = 225 trades/US$ 20.743; fração 1/3 = 231 trades × 1/3 ≈ US$ 7,9k (−62%);
  vs regra de dinheiro real (12.286) −36%. É decisão de política de risco (PF/avg R invariantes),
  não melhoria de retorno; o corolário sobre direção é observação in-sample.
- **Conserto adotado (plano §5.5, tier A):** `capital_fraction` como política, declarada assim —
  "risco/trade e DD caem ~3×, P&L em $ cai ~60% vs sizing atual, 7/232 trades deixam de ser
  recusados; PF/avg R/DD% invariantes"; registrar equity real e `BUYING_POWER` em
  `system_events`/`account_snapshots` antes de calibrar o 0,333.

#### portfolio-regime-3 — Detector de trend day (Dalton) em tempo real: veto do fade contra a tendência

- **Hipótese:** IB de 4 barras + extensão de um lado só a partir das 11:00 ET identifica ≥ 50%
  dos trend days com ≤ 10% de falsos positivos; range-fade-v2 com veto mantém PF ≥ 1,74 removendo
  trades de PF < 0,5; perna 2: balance em trend day sem alvo rende mais.
- **Verificado:** a classificação usa max/min/close do **dia inteiro** (`q2.sql`; look-ahead
  total); range-fade contra trend day n=11, −US$ 1.433, PF 0,19, WR 36% (12 trades em trend day,
  −1.167); todos entram ≥ 11:15 ET, 9 ≥ 12:00, 6 ≥ 13:00; balance em trend day n=14, +6.580, PF
  4,91 (sob flatten +6.707, PF 9,5; 12 intraday); openrev em trend_dn n=5 (IWM×4 + IWN×1, corr
  0,97 → ≤ 4 dias independentes); trend_up 4,7–10,5% dos dias, trend_dn 5,0–8,9%; nada de IB/day
  type existe no contexto atual.
- **Refutação principal:** 4 parâmetros livres ajustados para separar 11 trades de 69 é
  overfitting por definição; ganho máximo in-sample ≈ +1,4k/18 m (≈ US$ 80/mês), metade em tempo
  real; o corte horário de portfolio-regime-4 já remove a maioria sem detector; a perna 2 não foi
  medida e depende de O1+O7.
- **Conserto adotado (plano §6.6, tier B):** `day_type` como diagnóstico puro gravado no
  `market_snapshot` (paper e backtest), medindo recall/precisão por hora contra a classe final;
  só vira veto se a versão em tempo real reproduzir ≥ 50% de recall às 12:00 e houver ≥ 30
  trades vetáveis em paper. Fonte: Dalton Cap. 3/4 (regra "range ≥ 2×IB" é interpretação nossa).

#### portfolio-regime-4 — Janelas horárias por estratégia sob flatten + calendário FOMC/opex como instrumentação

- **Hipótese:** balance 09:45–12:00 e range-fade 09:45–13:00 mantêm ≥ 90% do P&L com ≤ 65% dos
  trades; FOMC não deve ser vetado (PF 4,76) e o dia seguinte é negativo (PF 0,36).
- **Verificado (sob flatten):** balance por hora de entrada — 10h n=35 +2.776 PF 1,61; 11h n=26
  +4.030 PF 2,06; 12h n=9 −749; 13h n=15 −1.570; 14h n=3 +2.308; 15h n=8 −65; h < 12 = 61 trades
  +6.806 PF 1,81 avg R 0,11; range-fade 10h 17, 11h 15, 12h 8, 13h 13, 14h 9, 15h 7 → **h < 13 =
  40 trades (+4.478), não 53** (53 = h < 14, +4.520); janela já é parâmetro do TOML
  (`trading_start/end_time`); FOMC n=16 +6.045 PF 4,76 → **sob flatten +1.411 PF 2,61** (3/4 do
  "efeito" era overnight); dia-após-FOMC n=7 −1.654; opex n=12 +1.041; dia-da-semana: bab quinta
  n=30 PF 0,70 vs sexta n=10 PF 5,71; orv sexta n=15 PF 0,25 vs segunda n=14 PF 4,94 — 15 buckets.
- **Refutação principal:** o walk-forward não é OOS para a regra horária (bloco 1 contém 1 dos 61
  trades; "OOS" = 60 dos mesmos 61 que geraram a hipótese); permutação (5.000×): "alguma janela
  inicial com ≤ 65% dos trades captura ≥ 90% do P&L" ocorre em **21%** das permutações sob o nulo;
  buckets com n=3 (14h) e n=8–9 dominam; ref-v2 com corte 13:00 → 40 trades ≈ 37 OOS < 50 —
  reprova no próprio critério; dia-após-FOMC e dia-da-semana são extremos esperados de múltiplas
  comparações.
- **Conserto adotado (plano §6.6, tier B):** calendário (`config/calendar.toml`: FOMC/CPI/opex/
  reconstituição Russell) só como **tag** no journal, com FOMC recalculado sob flatten; janelas
  horárias só como hipóteses a validar com trades de **paper** (±30% do in-sample sob flatten
  após ≥ 20 trades por variante), contadas no N (2 variantes × 6 pares); reportar o p de
  permutação no doc.

#### portfolio-regime-5 — Rebalancear pares sob flatten + várias estratégias por símbolo num processo

- **Hipótese:** sob flatten a openrev (PF 1,74) merece mais instâncias que a balance (1,55);
  estratégias no mesmo símbolo quase nunca colidem; 6 símbolos × 3 estratégias elevam trades/
  pregão de 0,60 para ~1,3.
- **Verificado:** compose com 11 serviços, client_id 1–11 (1–3 desligados); `find_exposure`
  bloqueia por posição ou ordem aberta; VB/IJR/SCHA/MDY/SPY/QQQ param em 06/08; orv overnight 8
  trades (6 stop no dia seguinte), ex-overnight 59 t PF 1,56, com flatten 67 t PF 1,74;
  bootstrap (4.000×): P(PF_orv_flat > PF_bab_flat) = **0,63**; IC90% orv [1,12; 2,71] vs bab
  [0,99; 2,33] — indistinguíveis; colisões com 3 estratégias por símbolo (JSONs do designer): IJR
  18/122 (15%), IWN 11/98 (11%), SLYV 4/72 (6%), AVUV 1/64; com janela de ordem pendente de 30
  min 18%/14%/10%; `signals.rejection_reason` do dev: MaxTradesReached 250, OutsideTradingHours
  24, nenhum PositionAlreadyOpen.
- **Refutação principal:** o ranking repousa em 8 trades overnight da orv (−2.722 → +1.950);
  a orv continua reprovada no gate A (1,11); "quase nunca colidem" foi medido no único símbolo
  com 2 estratégias (AVUV) — IJR bate o limiar de 15% da própria H0; os candidatos (VB 29 t PF
  1,96, SLYV 22 t, IWN 41 t) foram escolhidos da tabela completa; mais trades/pregão não
  diversifica regime (18 pares = o mesmo fator, corr 0,96–1,00).
- **Conserto adotado (plano §6.8, tier B/C):** separar em (1) infra — `paper.rs` aceitando
  lista de estratégias por símbolo, `FillTracker`/risk state por `strategy_id`, `analyze` por
  (símbolo, estratégia) (O16), esperando 10–18% de `PositionAlreadyOpen`; (2) pares — só por
  gate A por estratégia a 2 bp **e** sob flatten, N acumulado; não reponderar a orv até passar o
  gate A. Libera client_ids (teto ≈ 10 pares → 30) e reduz pacing.

#### portfolio-regime-6 — Transferência pré-registrada para ETFs macro (TLT, GLD, XLE, EEM, USO/FXI)

- **Hipótese:** ao menos uma das três vivas obtém PF OOS ≥ 1,3 com ≥ 50 trades em ≥ 2 de 5 ETFs
  macro com correlação diária ≤ 0,5 com IWM; H0 forte: o edge é específico de small-cap value.
- **Verificado:** assets = 14 ETFs de equity, nenhum macro; runs a 2 bp fora de small-cap value:
  565 ref IWM PF 0,42, 571 ref SPY 0,40, 561 bab IWM 0,84, 567 bab SPY 0,65, 569 orv SPY 1,12
  (37 t); simulação nula (20.000 tentativas, magnitudes da fade): n=20 P(PF ≥ 1,3) = 0,35 →
  P(≥ 2/5) = 0,57; n=25 0,28 → **0,44**; n=30 0,23 → 0,33; n=50 0,17 → 0,20; `max_atr_pct = 1.5`
  e `VolatilityRegime::High` bloqueiam USO/GLD em dias voláteis; sessão de futuros 23h muda o
  significado de PDH/PDL para a openrev.
- **Refutação principal:** o critério é cara-ou-coroa (satisfeito ~metade das vezes sem edge
  algum); um ETF macro isolado nunca chega a 50 OOS e agregar TLT+GLD+XLE+EEM+USO mistura fatores
  heterogêneos (o pooling do gate A foi justificado por pares do mesmo fator).
- **Conserto adotado (plano §8, "ETFs macro"):** só dentro do screening pooled de #16 com
  critério de rejeição — nunca aprovação isolada; medir corr diária com IWM após ingest e manter
  só |corr| ≤ 0,5 (critério declarado, não visto); reportar no pré-registro o resultado esperado
  sob a nula (0,44) para que "passou" não seja lido como evidência.

#### portfolio-regime-7 — ETFs 3× (TNA/TZA) como "stop largo em % do preço" e perna short sem aluguel

- **Hipótese:** para a mesma estratégia e o mesmo movimento do Russell, TNA reduz o custo/R de
  4 bp/(3×stop) — openrev 12,7% → 4,2%, balance 18% → 6% — e o risco real sobe de ~0,2% para
  ~0,6% sem margem; TZA long replica o short sem aluguel.
- **Verificado:** `opening-reversal-v1.toml:11` `level_zone_pct = 0.003` (absoluto — vale 0,1%
  em termos de IWM dentro de TNA → amostra diferente, config_hash novo); `balance_max_width_pct
  = 0.02` idem; `risk/mod.rs:253` (risco escala com stop%); ATR% proxy 14 barras 15m em IWM p50
  0,267%, p90 0,454%, p99 1,00% — barras com 3×ATR% ≥ 1,5% = 6,5% (bloqueadas pelo regime High);
  TNA/TZA ausentes do banco; ticks/preços de TNA são da busca web do designer (não verificados);
  orv IWM 36 trades in-sample → ~31 OOS.
- **Refutação principal:** o stop em bp triplica mas o tick também — TNA a US$ 69,9 = 1,43 bp vs
  IWM ≈ 0,42 (3,4×): custo/R igual ou pior; assumir "2 bp em ambos" é viés embutido no desenho;
  "risco real 0,6%" é alavancagem, obtida em IWM com `max_notional_multiple` sem tick pior nem
  rebalanceamento diário do produto 3× nem permissão CLP; a orv está reprovada no gate A e um
  único ativo não chega a 50; "TZA long = short sem aluguel" responde a um incidente cuja causa
  registrada é A9 (timeout), não borrow.
- **Motivo do arquivamento (plano §8, #28):** ver §4.4.

---

## 4. Linhas fechadas com número — e a condição de reabertura

Cada linha abaixo está **arquivada** (tier D no plano §4/§8). Para cada uma: a aritmética que a
fecha, com origem, e a condição explícita que a reabre. "Fato novo" significa número novo, não
argumento novo.

### 4.1 Micro futuros de índice (M2K/MES; MBT/MET)

- **Aritmética:** o edge não existe no subjacente. Pré-teste no banco dev (runs 561–572, 2 bp →
  1 bp): IWM balance 0,78 → 0,84; SPY balance 0,59 → 0,65; IWM fade 0,39 → 0,42; SPY fade 0,34 →
  0,40; IWM openrev 1,18 → 1,26 (36 t); SPY openrev 1,01 → 1,12. Cortar o custo pela metade vale
  +0,05–0,10 de PF (0,2 desvio-padrão do PF a n=36). O gate A OOS da openrev em IWM a 2 bp é PF
  1,09 (32 t, run 417) — a 1 bp ficaria ≈ 1,17 < 1,3. Alavancagem para risco 1% leva o DD de
  3,4% para 10–17% (viola ADR-010). M2K RTH ≠ IWM (níveis de ontem formados na Globex); margem
  intraday termina 15h45; refactor XL + bundle US$ 10/mês + permissão + US$ 2.000 de Commodities
  NLV.
- **Reabre se:** uma estratégia passar o gate A OOS no **subjacente** (IWM ou SPY) a 1 bp com ≥
  50 trades e PF ≥ 1,3 — sem isso não há o que herdar. Passo mínimo se alguém insistir: expor
  `--slippage-bps` no walkforward (5 linhas) e um único run de IWM openrev a 1 bp.

### 4.2 Cripto spot (IBKR Canada ou exchange registrada)

- **Aritmética (Kraken):** Pro tier 1 = 0,40% maker / 0,80% taker; barra de 15m de BTC 30–50 bp;
  stop das v1 ≈ 40 bp; ida-e-volta 120–160 bp → cost/R 300–400% (a pullback morreu com 26%);
  mesmo com stop ≥ 3×ATR e maker nos dois lados, cost/R 67%. Sem sandbox → gate B impossível por
  definição. 24/7 contradiz scheduler, `classify_market_phase`, flatten, rollover e circuit
  breaker. Sizing inteiro rejeita BTC > US$ 100k.
- **Aritmética (IBKR Canada):** as fontes primárias dizem que não há (página .ca 410, lista de
  permissões sem "Cryptocurrencies", Paxos só EUA, OSC 04/08/2022); um review terceiro diz que
  há via Zero Hash/Paxos, exceto Quebec, a 0,12–0,18% (mín. US$ 1,75) — **ação do dono: conferir
  em Client Portal → Trading Permissions**. Irrelevante para o veredito: a API de cripto da TWS
  só aceita MKT-IOC e LMT (5 min), sem STP nem bracket — viola "sempre stop server-side"; e
  12–18 bp/lado contra stops de 20–30 bp é cost/R > 100%.
- **ETFs de bitcoin/ether (IBIT, ETHA):** não é "classe cripto"; entram como 2 tickers extras do
  screening pooled (#16), só range-fade-v1, só após o flatten no backtest, com kill
  pré-registrado (mediana do gap overnight > 3× a dos pares vivos ≈ 35 bp → arquivar sem
  backtest). Balance v1 é estruturalmente muda em ativo com range diário ~3% (largura absoluta
  2%).
- **Reabre se:** (a) existir estratégia de hold multi-dia com motor GTC (O15) e fonte de livro —
  outro projeto; ou (b) MBT (futuro CME na IBKR, ~1–2 bp), que depende da infra de futuros
  arquivada em §4.1; ou (c) o dono confirmar cripto spot na IBKR Canada **e** a API oferecer
  stop server-side — hoje não oferece.

### 4.3 Forex IDEALPRO (USD.CAD / EUR.USD)

- **Aritmética:** range diário 0,4–0,6% → barra de 15m em sessão NY de 5–8 bp contra o piso de
  20 bp de stop do projeto (range mediano de barra 15m dos ETFs vivos 11–23 bp; a única estratégia
  com stop abaixo de uma barra foi negativa já a 1 bp, ADR-016). Sem TRADES/volume na IBKR
  (`market_data.rs:54` fixo em `Trades`); sessão 24/5 fora do scheduler; nenhuma fonte de livro do
  projeto em forex 15m; relief OSC com sunset em ago/2026 a confirmar; mínimo US$ 25k/ordem.
- **Reabre se:** surgir estratégia v2 com fonte e stop ≥ 20 bp em outro timeframe (o que é outra
  família, com volume opcional e sessão própria). O número em si é público — não gastar o spike.

### 4.4 ETFs 3× (TNA/TZA) como "stop largo"

- **Aritmética:** o stop em bp triplica, mas o tick também: TNA a US$ 69,9 = 1,43 bp vs IWM 0,42
  bp (3,4×) — custo/R igual ou pior. O "risco real 0,6%" é alavancagem, obtida em IWM com
  `max_notional_multiple` (#6) sem tick pior, sem rebalanceamento diário e sem permissão CLP.
  Parâmetros absolutos (`level_zone_pct = 0.003`, `balance_max_width_pct = 0.02`) não escalam. A
  estratégia base (openrev IWM) reprova o gate A. Regime High bloquearia 6,5% das barras de IWM×3.
- **Reabre se:** a openrev passar o gate A em IWM **e** o cap de notional configurável (#6) não
  bastar para o risco por trade desejado — o que é uma decisão de alavancagem, não de instrumento.

### 4.5 Gate de regime por compressão de range (range5 < 1,0%) / ADX diário / HMM

- **Aritmética:** re-simulação dos críticos (06/09/2026), OOS dos pares vivos: balance em
  compressão n=25 PF 1,67, **+US$ 2.947** — o gate perderia dinheiro (P&L de 15.957 para 13.010)
  e 27% dos trades; fade compressão n=13 PF 1,02 (+18); openrev n=3; tercil alto de range5 tem
  PF 0,88 na openrev — sinal oposto ao previsto; relação não monotônica. A fração de dias < 1%
  nos pares vivos é 20–33% (não 59–91%); em ago/26 já houve 0 trades pelo `is_tradeable`.
  ADX(14) diário precisa de ~28 pregões e o live entrega 23 (paridade).
- **Reabre se:** houver um **segundo** episódio de compressão com trades, e a única versão
  falsificável que sobrou — "compressão prejudica trades de stop estreito" (interação range5 ×
  stop_bp) — for pré-registrada e testada só no OOS. Até lá, `range5_pct` só como feature no
  snapshot (#17).

### 4.6 Teto por cluster / 1 posição por direção

- **Aritmética:** dias com 1 entrada: balance PF 0,70/avg R −0,23 (n=32); dias com ≥ 2 entradas
  (100% na mesma direção): PF 2,69/avg R +0,44 (n=64); fade 1,46 vs 4,64; openrev 0,96 vs 1,92;
  a 2ª entrada do dia tem PF 3,36 (balance) e 12,3 (fade) contra 1,52/1,57 da 1ª. O replay do
  designer: "máx. 1 na mesma direção" derruba 42 trades/US$ 11.556 (48% do P&L). Correlação de
  P&L diário entre estratégias −0,05 a +0,21; dentro da fade ≈ 0. "Priorizar o sinal de maior p"
  é impossível (8 processos independentes, poll de 30 s, sem árbitro); "risco aberto ≤ 0,5%" não
  é computável (snapshot da IBKR devolve `stop_price = 0`).
- **Reabre se:** o replay de portfólio (#12) mostrar DD de portfólio p95 > 10% com as regras do
  ADR-017; mesmo então a única forma computável é "máx. N na mesma direção" via
  `Position.direction`. O ADR-017 deve ser lido como "permitir o cluster".

### 4.7 Entrada limit passiva na range-fade

- **Aritmética:** reconstruindo a barra de sinal dos 69 trades da fade (3 pares, 2 bp) e uma
  limit no close: **27/27 perdedores tocados (100%), ≤ 31/42 vencedores (74%; 69% antes da barra
  de saída)** — limite superior, porque toque ≠ fill. Amostra: 12–19 trades/ano/par × 0,7 → < 50
  OOS mesmo agregado. R cai 33% e o custo relativo sobe para 29% do R. Contradiz a fonte da v1
  (Brooks: stop 1 tick além da barra de sinal); fonte alternativa sem Fase 1. O simulador enche
  qualquer limit na hora (`simulated/broker.rs:522-575`) — os 8 trades limit do ADR-009 não valem.
- **Reabre se:** (1) o simulador de limit realista existir (infra, fica); (2) a Fase 1 de Brooks
  *Trading Ranges* estiver feita; (3) com dados de 5m/1m para AVUV/SLYV/IWV, a fração de
  vencedores tocados **e enchidos** ficar ≥ ~60%. Sem (3) o número do backtest não tem valor.

### 4.8 Timeframe 5m para a opening-reversal

- **Aritmética:** barras de sinal de 5m ≈ 0,19–0,28% (razão 5m/15m na 1ª hora de SPY = 0,71 sobre
  medianas 15m de 0,26–0,40%) — abaixo do piso de 20–25 bp em SLYV/IJS/VBR/IWN/AVUV; 60% dos
  gatilhos da 1ª hora (12/20 nos 5m de SPY) são atravessados nos 5 min seguintes; sem realtime a
  decisão sai quando a barra seguinte já acabou; `ingest` não tem `--from/--to` e a IBKR limita 5m
  a ~1 semana/requisição (~78 requisições/símbolo). Dependência dura da openrev-v2 short com
  filtros, que reprova (shorts 2026 PF 0,82).
- **Reabre se:** (1) um edge short em 15m sobreviver a um segmento OOS bear; (2) O8 (realtime
  compartilhado com a paper) habilitado e `debug-candles --timeframe 5m` mostrar barra
  consolidada no 1º poll por 5 pregões; (3) `ingest --from/--to` com fatiamento de 1 semana; (4)
  logs de produção mostrarem entradas da openrev perdidas por overshoot/latência em número
  relevante. Nunca como trilha de estratégia; só como infraestrutura de dados.

### 4.9 MOC em grupo OCA no flatten

- **Aritmética:** todos os 7 ETFs operados são NYSE Arca; a NYSE não permite cancelar MOC após
  15:45 ET (recepção até 15:50). Uma MOC enviada às 15:49 é irrevogável: se stop ou alvo encher
  entre 15:49 e 16:00, a MOC executa no leilão e **abre posição invertida** — a família do
  incidente de 03/09. `oca_group` não existe no adapter; `Broker` não tem modify. Ganho ≈ 0,5–1 bp
  em ~20% dos trades de uma estratégia ≈ US$ 30–60/ano por instância.
- **Reabre se:** nunca como OCA. A única alternativa é "cancelar TP/SL antes de 15:45 e mandar
  MOC" (posição sem stop por 15 min) — viola "sempre stop server-side"; decisão explícita do dono
  se um dia o slippage medido do flatten a mercado (§5.1, 4 bp em IJS/SLYV) justificar.

### 4.10 Veto integral 12h–15h na range-fade

- **Aritmética:** re-simulação dos críticos (06/09/2026) com flatten, pares vivos: o bloco
  12–14h é n=30, PF 1,29, net **+857** (outro crítico, sem flatten: −543, avg R 0,00, t = 0,02 —
  ruído); permutação das horas p = 0,17; 15h ET tem PF 2,23 (n=7). Vetar corta 43% da amostra
  (64 → ~37 OOS < 50) por nada; o sinal negativo só existe em IJR/IWN/MDY (n=38, PF 0,61), que
  não rodam. A regra de Brooks Cap. 5 é "meio do dia **e** terço central" — alargar é regra sem
  fonte.
- **Reabre se:** depois que #10 recompuser a amostra, como override de ablação pré-registrado
  no harness, e só se o OOS agregado ficar ≥ 50 trades com PF ≥ 1,5 em ≥ 4/6 janelas **sem** o
  P&L cair. O hotfix do fuso (#5) é outra coisa e fica.

### 4.11 Ações individuais como universo próprio

- **Aritmética:** edge de cesta, não de ação; gaps de earnings/guidance (UNH 2025: dois dígitos)
  dominam o P&L sem flatten e não têm calendário completo; balance v1 (2% absoluto) quase não
  forma; barras de 15m de JPM/XOM/PG têm 15–25 bp como os ETFs → cost/R igual; com 8 × 2 combos e
  n ≈ 25–40, P(≥ 3 "passam" | nulo) ≈ 0,7; BRK.B é "BRK B" na IBKR. 8 ingests no gateway de
  produção + calendário manual por trimestre.
- **Reabre se:** as cestas cíclicas de #16 transferirem (se não transferirem, ações individuais
  não precisam ser testadas). Até lá, 4 nomes (JPM, XOM, PG, CAT) só como braço de controle do
  screening pooled, mesmo relatório, mesmo N.

### 4.12 Meta-labeling agora

- **Aritmética:** primário sem edge em R no pool de 14 (balance PF_R 0,99, fade 0,84 — AFML §3.7:
  meta-labeling não cria edge); labels contaminados pelo overnight (19/92 OOS = 62% do P&L);
  SE(AUC) ≈ 0,06 com ~240 trades — a faixa "≥ 0,60 aprova / < 0,55 abandona" cabe em 1 SE; a
  estrutura já é explicada por stop_bp (tercis: PF 0,53 / 1,99 / 3,05); reduz o fluxo do gate B
  em 35–40%.
- **Reabre se:** existirem ≥ 1.000 trades rotulados de estratégias com PF_R > 1, com flatten no
  engine, e o baseline manual pré-registrado (stop_bp + hora + direção) **não** esgotar o ganho;
  então CPCV por 6 blocos temporais, nunca K-fold por dia. O plumbing snapshot → journal entra em
  #3 desde já.

### 4.13 Escada de risco / Kelly agora

- **Aritmética:** Kelly ≈ 0 na balance ex-overnight (PF_R 0,99 → f* = −0,003); fade f* 0,27 (b
  realizado 1,07); PSR balance 0,938 e DSR ≤ 0,81 (N=2) — o pré-requisito "PSR/DSR ≥ 0,95" da
  própria escada não é atingido; o degrau 0,25%/1× é o status quo (cap prende para stop < 25 bp);
  0,5% exige cap 2–4× — inviável em SLYV/IJS (87%/62% da barra mediana já a 1×) e reduz a conta a
  1 posição; tudo é paper, com 0 trades das aprovadas desde 18/08 e fills a NBBO sem impacto.
- **Reabre se:** gate B fechado com fills reais medidos (entry vs gatilho, saída vs stop), PF_R ≥
  1,3 no OOS com flatten, cap de liquidez elegível por ativo (AVUV/IWM/IWN sim; SLYV/IJS/IWV não
  até medir slippage), `risk_per_trade_pct` fora do `config_hash`, e PSR ≥ 0,95 sobre os R do
  **live**. Degraus em risco real (risk_amount/equity), descida automática após 2 janelas de 20
  trades abaixo de −30% do backtest.

### 4.14 Também fechados nesta pesquisa (sem seção própria)

- **Open-Test-Drive + segunda entrada + piso de ATR na openrev** (setups-novos-2): filtros
  post-hoc sobre um edge que é de regime (shorts 2025 PF 2,33 / 2026 PF 0,82). Reabre com
  segmento OOS bear ou amostra forward de paper, sem os filtros.
- **Instrument spec fatia 2** (novos-mercados-3): sem cliente (todos os mercados novos
  falsificados); reabre com um pré-teste positivo de outra classe.
- **ETFs macro isolados** (portfolio-regime-6): P(≥ 2 de 5 "passam" | nulo) = 0,44; só dentro
  do screening pooled com critério de rejeição.
- **Alternativas de saída na balance** (breakeven em 1R PF 1,68; parcial 50% em 1R + runner
  2,02; saída por tempo 3/4/6/8 barras 1,66/1,63/1,52/1,47; trailing 1,0/1,5 ATR 1,83/1,78 —
  todas piores que 3R+filtro nos mesmos 96 trades, designer edge-existente-2) e **na fade** (lado
  oposto PF 1,82 vs 1,74; tempo/breakeven/trailing/parcial 1,02–1,76 vs 1,74). Reabrem só como
  ablação pré-registrada no harness.

---

## 5. Achados transversais

Os sete achados do plano §2.3, que nenhum relatório anterior do projeto registrava, mais três que
os críticos acrescentaram.

1. **O backtest não faz flatten; o live faz.** `engine.rs:136-240` sem fim de sessão; `paper.rs:
   2189-2198` fecha às 15h55–16h10 ET. Com flatten no close da 15:45 + 2 bp (três críticos
   independentes): balance-area PF 1,92 → 1,55, avg R 0,214 → −0,007 (20/96 overnight = 65% do
   P&L; reprova o gate A pelo avg R, só IJS passa); range-fade 1,88 → 1,74 (32% overnight;
   passa); opening-reversal IWM/IWN 1,23 → 1,74. Os 20 overnight da balance são 10 long/10 short,
   14 alvos/6 stops — hipótese de edge multi-dia para a Onda C, não para a v1. O gate B em curso
   compara o live com um backtest que "ganha dormindo comprado".
2. **O edge da opening-reversal é do lado short e de 2025.** 6 combinações a 2 bp: 125 shorts PF
   1,59 (+13,4k) vs 71 longs PF 1,06; shorts 2025 n=80 PF 2,33; 2026 n=45 PF 0,82; 2025Q2 = 88% do
   P&L short; nos pares vivos short PF 1,24 (n=41) — indistinguível de 1,0. Hipótese de regime.
3. **A range-fade vive do lado long e da manhã.** Long PF 3,34 (n=33, t=2,79) vs short 1,00
   (n=36); 10h–11h ET = 100% do P&L; o bloco 12–14h é ruído (avg R ≈ 0), não perda. **Bug real:**
   o veto `midday_midrange` compara UTC fixo (`range_extreme_fade_v1/context.rs:220-238`; TOML
   15:30–18:00) e cobre 10h30–13h ET no inverno; ~24% da amostra do gate A foi medida com o veto
   deslocado.
4. **O sizing trava no notional e o risco real por trade é 0,15–0,32%, não 1%.** `qty = min(
   orçamento/dist, capital/entry)` com cap 1× hardcoded (`risk/mod.rs:239-279`); o $ arriscado é
   ∝ distância do stop e, como stop largo ganha, o PF em $ supera o PF em R: balance OOS PF$ 2,09
   vs PF_R 1,41 (AVUV 1,07, VBR 1,27, IJS 2,37; corr(risk, R) = 0,31); a openrev inverte (PF_R
   1,28 > PF$ 1,11). Tercis de stop no OOS da balance: ≤ 17 bp WR 29%/PF 0,53; 17–28 bp 53%/1,99;
   > 28 bp 58%/3,05 — "stop estreito perde", regra de triagem: stop < 0,25% do preço nasce morto.
5. **O contexto global bloqueia a estratégia de range nos dias de range.** `is_tradeable` exige
   Up/Down (`context/mod.rs:79-81`); só `RiskManager::validate` (`risk/mod.rs:158-163`) rejeita
   por isso; a range-fade nunca lê `trend_state`. Medido com debug a 2 bp: NoContext rejeitou
   48/36/62 sinais em AVUV/SLYV/IWV contra 31/30/35 entradas. O PF desses sinais é desconhecido —
   o teste de maior valor esperado do plano (#10). **Superado pela auditoria de 07/09**
   (`docs/reports/auditoria-2026-09-07.md` §3, bloco A): a contagem era o dobro (são 24/18/31,
   73 no total) e o PF do delta foi medido — 54 trades, PF 0,70, avg R −0,321, −US$ 3.257;
   com flatten, PF 0,56. O item deixa de ser o de maior valor esperado.
6. **O P&L é concentrado em poucos dias e meses.** Balance OOS: 99% em 2 meses (jul/25 +8.781,
   out/25 +6.985 de +15.957), 8/13 meses positivos; a variante de convicção tem 64% do P&L num
   dia (10/10/2025, 8 shorts simultâneos); fade top-2 meses = 40%, 11/16 positivos; portfólio de 8
   pares: jun–out/2025 +24.564, nov/2025–ago/2026 **−178**. O IC95 do PF da balance vira [0,87;
   4,91] com bootstrap em blocos. Por isso concentração (share do melhor dia e dos 2 melhores
   meses, P&L por bloco/ano) entra no relatório obrigatório e DSR/PSR viram relatório, não gate.
7. **O feed de produção entrega barras esparsas.** Nos dias gravados pelo servidor (17–21/08,
   28/08, 31/08–02/09) AVUV tem 60–100k ações/dia e range por barra de 1,6–3 bp, contra 780k–1M
   e 9–13 bp nos dias ingeridos pelo PC/TWS (24–27/08); a mesma conta, sem assinatura, via TWS no
   PC entregava barras completas com lag p50 de 29–54 s; o que mudou em 07/08 foi TWS → IB
   Gateway headless. Enquanto isso não for diagnosticado, latência e "custo por fill" medem
   principalmente o high/low que o feed não viu. Comissão: simulador US$ 0,35/perna vs IBKR
   Canada US$ 0,005/ação mín. US$ 1,00 (≈ 0,9 bp ida e volta); o `CommissionReport` só casa no
   mesmo lote do poll (26 fills do dev com comissão zero).
8. **O cluster é confirmação, não redundância.** Dias com ≥ 2 entradas (100% na mesma direção)
   têm PF 2,69 na balance contra 0,70 nos dias com 1 entrada; a 2ª entrada do dia tem PF 3,36
   (balance) e 12,3 (fade). Qualquer teto por direção destrói o edge; o ADR-017 deve ser lido como
   "permitir o cluster" e o replay de portfólio (#12) precisa modelar a corrida de 30 s entre
   instâncias, que passam juntas pela trava.
9. **Liquidez por barra é gargalo escondido.** Equity paper ≈ US$ 238k (HANDOFF:215) e cap 1× →
   cada posição ≈ US$ 238k; barra mediana 15m 11–13h ET desde mar/2026: SLYV 290k (p10 77k), IJS
   370k, VBR 1,0M, IWN 1,6–1,8M, AVUV 2,6M — 82% e 64% de uma barra mediana em dois dos oito
   pares; a conta paper (fill a NBBO sem impacto) esconde isso; IWN tem PF igual ou maior na
   balance (2,23 com flatten) com 5× a liquidez de IJS.
10. **O walk-forward do repo não é OOS e o Sharpe está errado.** Estratégias são funções puras
    de (contexto, série) e o walk-forward não re-ajusta nada (`walkforward.rs:46-61, 84-103`):
    o "OOS" é a mesma rodada determinística após o primeiro bloco — mede robustez temporal, não
    protege contra seleção de regra/par; CPCV é degenerado pelo mesmo motivo. `metrics.rs:175-
    231` anualiza por candle 15m (√35040 ≈ 187) e dá Sharpe −6 a −18,5 com PF > 1 (60 runs); os 9
    runs WF de 05/09 têm Sharpe 0,00. O harness (#3) precisa de holdout temporal travado, Sharpe
    sobre retornos diários e contagem de tentativas.

---

## 6. Higiene pendente no banco e no código

Tudo abaixo é tier A (#7 do plano) e não muda regra nenhuma.

| Item | Onde | O que fazer |
|---|---|---|
| Runs 413/414 duplicados | `backtest_runs`, label `walkforward-oos-6w`, bab IJS 23 t PF 2,36, mesmo `config_hash` | Deduplicar (manter o maior id); índice único em (strategy_id, asset_id, config_hash, period, label). Inflam o WF da balance de 91 para 114 trades se somados |
| 22 cópias de walk-forward; 505 de 589 runs sem label; 49 runs com o mesmo hash da balance sem label | `backtest_runs` | Script de dedupe; `--label` obrigatório com override; `experimental` flag |
| Runs 553–572 sem label (06/09 18:2x e 22:24–22:25 UTC) | 553–558 repetem 530–534; 561–572 = pré-teste de futuros (IWM/SPY a 1 e 2 bp) | `UPDATE backtest_runs SET label='pre-teste-futuros-2026-09-06' WHERE id BETWEEN 561 AND 572`; rotular ou apagar 553–558 e os pools de 06/09 |
| `tick_size` gravado via f64 | `trader-infra/src/repositories/mod.rs:74` (`Decimal::from_f64_retain(0.01)`) → `0.0100000000000000002081668171` nos 14 ativos | `Decimal::new(1, 2)` e limpar o banco (viola AGENTS.md §3.3); registrar divergência com o `tick_size` do TOML em log |
| 6 símbolos parados em 06/08/2026 | IJR, MDY, QQQ, SCHA, SPY, VB (9.430–9.480 candles vs 9.936 dos vivos) | Reingerir no servidor via workflow `ops`, 1 símbolo por vez (pacing 60 req/10 min); pré-requisito de #16 e de qualquer pool de 14 |
| Sharpe negativo com PF > 1 | `metrics.rs:175-231` (annualizer por candle); 60 runs com Sharpe < −5, mín −18,5; 9 runs WF com 0,00 | Sharpe/Sortino sobre retornos diários (#3) |
| Trades de backtest com `strategy_id 'unknown'` e `journal {}` | `simulated/broker.rs:851-893` | `Order.metadata` → `Position.metadata` → `Trade.journal` (O16, ~0,5 dia) |
| `analyze` usa `latest_by_strategy` sem filtrar símbolo/hash | `analyze.rs:75-78`; `backtest_run_repository.rs:67-95` | Filtrar por (strategy_id, asset, config_hash da estratégia carregada) — hoje pega o run mais recente da estratégia em qualquer símbolo |
| Comissão zero nos 26 fills do dev; `CommissionReport` só casa no mesmo lote | `ibkr/broker.rs:366-399` | Casar em poll posterior; comissão por ação no simulador (`simulated/broker.rs:109`) |
| `cancelled_at IS NULL` com `status = 'cancelled'` | ordem 17 (pregões §4.2) | Carimbar a data na transição |
| `backtest --help` diz "10 bp" para o default de slippage | `backtest.rs:114-119` (default 2 bp) | Corrigir o texto |
| `slippage_bps: Option<u32>` | `backtest.rs:36`, `main.rs:121` | Decimal (1,5 bp impossível hoje); expor no walkforward |
| Comentário obsoleto "em UTC (como as demais)" | `range_extreme_fade_v1/context.rs:233-234` | Sai com o hotfix #5 |
| Moeda-base da conta paper não documentada | nenhum doc/HANDOFF/runbook menciona CAD/NetLiquidation; `ibkr/broker.rs:824` trata `BASE` | `trader-cli account --provider ibkr`; se CAD, o cap de notional já está ~1,37× errado |
| Equity real e `BUYING_POWER` lidos e nunca usados | `ibkr/broker.rs:204-213`; único uso em stub de teste | Registrar em `system_events`/`account_snapshots` a cada sessão (#6) |
| `sql/screens/` e `sql/stats/` criados em 07/09 a partir dos screens da pesquisa | feito | Calibrar contra controles (plano §5.7) antes de usar como Fase 0 |
| `ops.yml` sem ação SQL read-only | `.github/workflows/ops.yml:15-27` | Adicionar, ou usar o dump diário, para qualquer medição em produção |

---

## 7. Limites

- **Um regime só.** Todo o histórico (24/02/2025 → 02/09/2026) tem um episódio de volatilidade
  extrema (abr/2025) e um de compressão (ago/2026); o P&L está concentrado em jun–out/2025. Nenhum
  walk-forward do repo é OOS em relação ao desenho das regras; o único OOS verdadeiro é o paper
  forward, que tem 0 trades das aprovadas desde 18/08.
- **Re-simulações, não o motor.** Os números dos críticos são Python sobre JSONs do binário de
  06/09 e candles do banco dev; reproduzem o motor ao centavo no baseline, mas o flatten "no close
  da barra 15:45" difere do MKT às 15:55 do live em 10 min e no tipo de fill. O motor com #1 é a
  fonte de verdade; até lá, nenhum número "com flatten" deste relatório é oficial.
- **Banco dev ≠ produção.** O dev tem 3 trades e 26 fills de live (04/08, pullback); tudo sobre
  fills, comissão real e latência em produção precisa do dump do servidor ou de uma ação `sql`
  no workflow `ops`. 6 dos 14 símbolos param em 06/08/2026 no dev.
- **A conta paper enche a NBBO sem fila nem impacto.** Qualquer custo medido nela é piso; em
  SLYV/IJS o piso está longe do teto.
- **Vereditos incompletos numa lente.** Os críticos de código e de operação de portfólio/regime
  rodaram na retomada e seus vereditos estão no journal da retomada, não aqui (§1.4).
- **Fontes web não confirmadas na origem.** Cripto na IBKR Canada (Zero Hash/Paxos, 0,12–0,18%),
  fee da Kraken, regras de MOC da NYSE/Nasdaq, preço/tick de TNA e ADV dos ETFs macro vêm de
  fetch/busca de 05–06/09/2026; as páginas da IBKR devolveram 403/410. As decisões que dependem
  disso (cripto spot) foram tomadas por aritmética que vale nas duas hipóteses, mas a premissa
  precisa de checagem humana no Client Portal antes de virar ADR.
- **Contagem de tentativas.** As ~30 variantes desta pesquisa (e as ~150 anteriores: 42+42+66)
  foram re-simulações que não estão em `backtest_runs`; o N inicial de qualquer DSR futuro será
  uma estimativa declarada, não um registro.
- **Nada aqui é código.** Esboços de implementação nas propostas são intenção; cada um passa por
  ADR (motor) ou pelo framework Fases 1–7 (estratégia) antes de existir.

---

**Documentos relacionados:** `docs/cto-plano-lucratividade-2026-09.md` (plano-mestre);
`docs/reports/gate-a-revalidacao-2026-09-04.md` (régua que este relatório propõe substituir);
`docs/reports/pregoes-2026-08-31_a_09-02.md` (A9, overshoot 01/09, feed);
`docs/reports/incidente-2026-09-03-ordens-duplicadas.md` (família de risco do MOC/OCA);
`docs/decisions/ADR-015`, `ADR-016`, `ADR-017`; `docs/strategy-analysis-framework.md`.
