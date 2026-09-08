# Estratégia: Opening Reversal v2 — short-only na primeira hora

**ID:** `opening-reversal-v2`
**Versão:** 2.0.0
**Status:** proposto / especificado — NÃO implementado (07/09/2026)
**Autor da especificação:** coding agent (pesquisa multiagente de 06–07/09/2026), a pedido do dono
**Origem:** `docs/cto-plano-lucratividade-2026-09.md` §6.3 e §7 (item 19 do ranking, **Tier C — Onda C**: hipótese de regime, spec pronta, **sem implementação** até haver OOS com regime distinto ou amostra de paper forward) e §2.3 achado 2
**Pesquisa de origem:** propostas `edge-existente-3` e `setups-novos-2` e as 6 críticas correspondentes (06–07/09/2026); vereditos em `docs/reports/pesquisa-lucratividade-2026-09-07.md`
**Relação com a v1:** `opening-reversal-v1` **não muda** (framework: "Nunca alterar uma estratégia em produção" — `docs/strategy-analysis-framework.md:268`). A v2 é uma estratégia nova, validada do zero (gates A e B completos), com **uma** regra a mais: `direction_filter = short_only`.

---

## 0. Leia antes de qualquer número

A v2 é uma **hipótese de regime, não um edge estrutural demonstrado**. Todo o P&L short da v1 nos 6 símbolos estudados vem de 2025 (crash das tarifas e recuperação em V): shorts 2025 n=80 PF 2,33 contra 2026 n=45 PF 0,82; o 2º trimestre de 2025 responde por 88% do P&L short; dois dias (23/04 e 12/05/2025) por ~57% do net da variante ≤ 10:15 (re-simulação dos críticos, 06/09/2026 — §13). Nos dois pares que estão em paper (IWM+IWN) o short é PF 1,24 com n=41 e t=0,89 — indistinguível de 1,0.

O critério de continuação que a proposta original declarava ("PF ≥ 1,4 e avg R ≥ 0,2 em 2025 **e** 2026") **já reprova** com os dados existentes (2026: n=45, PF 0,82) — foi por isso que o plano-mestre moveu a v2 para a Onda C (§6.3, §7): a spec fica pronta, mas **não há implementação** enquanto não existir um segmento OOS com regime distinto (bear ou alta volatilidade) ou amostra de paper forward que a justifique.

Por isso o desenho abaixo é de **pré-registro**: uma regra, três pares, v1 como controle, critério de falsificação em dados que não existiam quando a regra foi escolhida (holdout travado, segmento 2026, paper forward). Não há veredito neste documento: se o segmento 2026 e o holdout confirmarem PF < 1, o resultado correto é arquivar a hipótese de regime com número, não ajustar filtros.

## 1. Fonte

- **Livro:** Al Brooks — *Reading Price Charts Bar by Bar* (Wiley, 2009).
- **Capítulos:** Cap. 11 — "Opening Patterns and Reversals" / "Patterns Related to Yesterday" (opening reversals são fenômeno da **primeira hora**); Cap. 1 — "Signal Bars: Reversal Bars" e "Second Entries" (barra de sinal); Cap. 10 — "Protective Stops" (herdado da v1).
- **Análise completa do livro:** `docs/books/analysis/brooks-bar-by-bar.md` (setup 2.1, linhas 25–34; tabela 3.1, linhas 150–162).
- **Citação central (Cap. 11):** *"Breakouts, Failed Breakouts, and Breakout Pullbacks from yesterday's swing highs and lows [...] are the most reliable patterns in the first hour."*
- **O que o livro NÃO diz:** que o lado short seja melhor que o long. O filtro de direção é **interpretação nossa**, motivado por dado do projeto (§13), não pelo autor. Brooks trata os dois lados simetricamente.

## 2. Conceito em uma frase

Na primeira hora, quando o mercado testa a **máxima do dia anterior** e falha (barra de reversão bear forte), vendemos contra o teste — e **só** contra a máxima: o teste da mínima de ontem (lado long da v1) não é operado pela v2.

## 3. Fase 1 — Extração do conceito (8 perguntas)

| # | Pergunta | Resposta (com fonte) — diferença para a v1 em negrito |
|---|----------|----------------------|
| 1 | Nome | Opening Reversal / Failed Breakout da máxima de ontem (Cap. 11) — **só o lado short** |
| 2 | Contexto | Primeira hora; teste (ou rompimento com gap) da máxima do dia anterior |
| 3 | Timeframe | 5min no livro; aqui 15min (igual à v1) |
| 4 | Entrada | Barra de reversão bear forte na zona de teste; sell stop 1 tick abaixo da barra de sinal (Cap. 10) |
| 5 | Stop | 1 tick acima da máxima da barra de sinal; se risco > 1,5×ATR14, stop monetário = 60% do range da barra (Fig. 10.19) — igual à v1 |
| 6 | Alvo/saída | Alvo único 2R (adaptação da v1); flatten de fim de sessão pelo live (`paper.rs:2189-2198`) e pelo ADR-018 no backtest |
| 7 | Quando NÃO operar | Tudo o que a v1 já veta (barra fraca, momentum contra, 4+ trend bars contra) **+ qualquer setup long** (`DirectionFiltered`) |
| 8 | Estatísticas do autor | Um extremo do dia costuma se formar na primeira hora (Cap. 11). Nada sobre assimetria short/long |

## 4. O que muda em relação à v1 — e o que fica de fora

**Uma regra nova (princípio §3.6 do plano-mestre: uma mudança por variante):**

| Parâmetro | v1 | v2 | Fonte |
|---|---|---|---|
| `direction_filter` | (inexistente; long e short) | `short_only` | interpretação nossa sobre §13 |
| `trading_end_time` | `10:30:00` | `10:30:00` (**igual**); `10:15:00` só como **hipótese secundária** pré-registrada (§14) | Cap. 11 "first hour"; ver §12 |
| `entry_validity_candles` | 2 | 2 (**igual**; ablação com 1 contada no N) | ADR-009 |
| todos os demais | — | idênticos, byte a byte | — |

**Explicitamente fora da v2** (arquivados no plano-mestre §8; filtros post-hoc sobre os mesmos trades):

- **Open-Test-Drive (Dalton)** — redundante com a barra de sinal já exigida (close no terço inferior + sombra superior ≥ 1/3, `setup.rs:63-108`); em 15m só é detectável "após 2 candles" (`docs/books/analysis/dalton-mind-over-markets.md:405`), comprimindo a janela de 4 barras.
- **Segunda entrada (double top)** e **piso de stop em ATR** — a primeira segue candidata a versão futura (v1 §14); o segundo não tem motivo: o stop da openrev (mediano 31,5 bp, o maior das estratégias; custo 13% do R) não está na zona de ruído.
- **`time_exit`** — o tracker de `crates/trader-core/src/execution/time_exit.rs:123-145` **desarma para sempre** ao atingir `min_r` uma vez (`validated = true`, linhas 135–136 e 140–142) e **não fecha ao sino**; ele não substitui o flatten. O fechamento de fim de dia continua vindo do live (`paper.rs:2189-2198`, 15h55 ET) e, no backtest, do ADR-018. `dispatch.rs:134` fica `None` também para a v2.

## 5. Contexto de Mercado (filtro obrigatório)

Idêntico à v1 §4, com um item a mais:

1. **Janela:** barra de sinal com abertura entre `trading_start_time` e `trading_end_time` em America/New_York (`opening_reversal_v1/context.rs:62-84`; a comparação é `time > end`, linha 77, sobre o timestamp de **abertura** da barra — a primeira barra do dia é a 09:30).
2. **Zona de teste:** a barra toca ou cruza a máxima do dia anterior (zona de 0,3%, `level_zone_pct`).
3. **Níveis de ontem:** computados da própria série, por data ET (`context.rs:29`).
4. **Veto de momentum** e **veto de barras contra**: iguais à v1 (`context.rs:117,144`).
5. **Filtro de direção (novo):** setup resolvido como `Direction::Long` → `RejectionReason::DirectionFiltered`. O filtro entra em `setup.rs::detect_setup` **logo após** o `match` que resolve `(direction, level)` (`setup.rs:40-61`), antes da avaliação da barra de sinal. Consequência a documentar: o contador de `DirectionFiltered` no paper conta **toques na mínima de ontem**, não "longs que a v1 teria tomado" — para isso serve a instância v1 de controle (§14).

## 6. Setup de Entrada

### 6.1 Barra de sinal (reversal bar bear forte, Cap. 1)

Só o lado bear da v1 §5.1: close < open; fechamento no terço inferior do range; sombra superior ≥ 1/3 do range (`signal_wick_min_pct`); corpo ≥ 30% do range (`signal_body_min_pct`). Código herdado sem alteração (`setup.rs:63-108`).

### 6.2 Regras de rejeição do setup (ordem de avaliação)

1. Fora da janela → `OutsideTradingHours` (`mod.rs:101-104`).
2. Sem dia anterior / sem ATR → `IncompleteSetup` (`mod.rs:106-118`).
3. Barra não tocou a zona da máxima nem da mínima → `YesterdayLevelNotTested`; tocou as duas → `IncompleteSetup` (ambíguo) — igual à v1.
4. **Tocou a mínima (setup long) → `DirectionFiltered` (novo).**
5. Barra de sinal fraca → `WeakConfirmation`.
6. Vetos de momentum → `MomentumAgainst`.
7. RR < 1,5 → `PoorRiskReward` (`entry.rs:69-78`).

## 7. Entrada, Stop e Alvo

- **Entrada (literal, Cap. 10):** sell stop = low da barra de sinal − 1 tick. Validade: `entry_validity_candles = 2` (ADR-009; `orders.rs:234-236`, `stop_entry_expired(waited >= validity)`).
- **Stop (literal):** high da barra de sinal + 1 tick; se risco > 1,5×ATR14, stop monetário = entrada + 60% do range da barra (Fig. 10.19).
- **Alvo:** 2R. Vizinhos medidos pelo designer (short, entrada ≤ 10:30, flatten): 1,5R PF 2,44 / 2R 2,52 / 3R 2,27 — não re-simulados pelos críticos; manter 2R e não varrer.
- **A verdade sobre a barra 10:45.** Com sinal na barra 10:15 (fecha 10:30) e validade 2, a ordem fica viva durante as barras 10:30 **e 10:45**; com sinal na barra 10:30 (admitida pelo `trading_end_time = 10:30`), a entrada pode acontecer em 10:45 ou 11:00. As entradas na barra 10:45 são as piores da amostra: PF 0,26 nos pares vivos (n=10, designer), 0,49 (n=26) e 0,40 (n=36, 6 símbolos, net −6.518) nas re-simulações dos críticos. Na variante ≤ 10:15 elas são só 2 de 101 trades (net +288) — usar `entry_validity_candles = 1` custaria esses 2 trades. A v2 **mantém 2** (paridade com a v1, uma mudança por variante) e registra 1 como ablação.
- **Guard anti-latência:** como na v1, a entrada é estruturalmente além do último fechamento; `SetupInvalidated` só ocorre pela guarda de overshoot do ADR-015 (`config/default.toml:42`, 25% da distância do stop) — e é exatamente o caso-base da primeira hora (§15).

## 8. Gestão de Risco

- Risco por trade: 1,0% (global); limites por instância 2%/dia, 3 trades/dia, 3 perdas consecutivas; limites da conta: 3 posições simultâneas e 200% de notional (`config/default.toml:36-38`, ADR-017). O `RiskManager` recebe a mesma janela ET do TOML por `risk_config.rs:184-200` — a variante 10:15, se adotada, tem paridade automática.
- **Direção: só short.** Sempre com stop server-side (bracket, `ibkr/broker.rs:605-612`). Sem martingale. Paper only.
- **Cluster:** 6 instâncias short disparando entre 09:45 e 10:45 em ETFs com correlação 0,97 é o caso típico de teto da conta — o pior dia do replay de portfólio (16/03/2026, −US$ 2.032) foram dois shorts openrev IWM+IWN às 10:45 (plano-mestre §6.4). O P&L da v2 tem de ser lido **com o teto de 3 posições aplicado dia a dia**.
- **Sizing:** o `capital_fraction` e o cap por liquidez do ADR-020 (plano §5.5) mudam o P&L em US$ sem mudar PF nem avg R — a v2 é julgada em **R** (PF_R ao lado de PF$), nunca em dólares somados de backtests isolados.

## 9. Fase 2 — Tabela Subjetivo → Objetivo

| Conceito (livro) | Regra objetiva (15min) | Origem |
|---|---|---|
| "primeira hora" (Cap. 11) | barra de sinal com abertura em 09:30–10:30 ET (5 barras: 09:30 … 10:30, pela comparação `time > end`) | v1 + `context.rs:77` |
| "primeira hora" literal = 4 barras (09:30 … 10:15) | `trading_end_time = 10:15:00` | **hipótese secundária** (§14) — coincide com "os 4 primeiros candles" do doc v1 §4, mas foi escolhida após ver a tabela |
| "teste da máxima de ontem" | high da barra ≥ máxima de ontem × (1 − 0,3%) | interpretação (v1) |
| "barra de reversão forte" (Cap. 1) | critérios de §6.1 | Cap. 1 (v1) |
| "stop 1 tick além da barra de sinal" (Cap. 10) | sell stop = low − tick; stop = high + tick | literal (v1) |
| "risk about 60% of the height of the signal bar" | se risco > 1,5×ATR: stop = entrada + 60% do range | literal, Fig. 10.19 (v1) |
| "swing: pode ser o extremo do dia" | alvo único 2R + flatten 15h55 ET | adaptação (v1) + ADR-018 |
| *(sem conceito no livro)* "o lado long não paga" | `direction_filter = short_only` → `DirectionFiltered` | **interpretação nossa** sobre dado do projeto (§13) |

## 10. Fase 3 — Especificação Técnica

```text
Inputs:
  - candles 15min (dia anterior + dia atual), ATR14 (15min)
  - config da estratégia (config/strategies/opening-reversal-v2.toml)

Outputs:
  - Signal(short) | Rejected(RejectionReason, detalhes)
  - entrada (sell stop), stop, alvo 2R
  - snapshot da v1 (entry.rs:108-124) + "direction_filter": "short_only"

Estado interno: nenhum (stateless, como a v1)

Eventos: fechamento de candle 15min com abertura dentro da janela ET

Invariante de paridade com a v1 (testável): para qualquer série, o conjunto de
sinais short da v2 == o conjunto de sinais short da v1, com preços idênticos;
todo sinal long da v1 vira Rejected(DirectionFiltered) na v2.
```

## 11. Rejeições Registradas pelo Bot

Reuso (já no domínio, `crates/trader-domain/src/signals.rs:39-147`): `OutsideTradingHours`, `IncompleteSetup`, `YesterdayLevelNotTested`, `WeakConfirmation`, `MomentumAgainst`, `StopTooWide`, `PoorRiskReward`, `SetupInvalidated`, `MaxTradesReached`, `DailyLossLimitReached`, `ConsecutiveLosses`, `HighVolatility`, `PositionAlreadyOpen`.

Nova (a adicionar ao domínio, com o teste serde de `signals.rs:275-329`):

- `direction_filtered` — setup válido na direção bloqueada pelo `direction_filter` da estratégia. Detalhes: `{ "direction": "Long", "direction_filter": "short_only" }`.

## 12. Filtros de Horário e Ativo

- **Janela:** 09:30–10:30 ET declarada em NY (A2), como a v1 (`opening-reversal-v1.toml:46-47`). A variante 10:15 é hipótese secundária (§14) e só via override do harness (`--set trading_end_time=10:15:00`, ADR-019) — o `config_hash` muda sozinho.
- **Ativos de validação (pré-registrados, 3 e não 6):** IWM, IWN, VB.
- **Custo:** 2 bp/lado (ADR-016) em IWM/IWN/VB (tick medido pelos críticos: VB 0,35 bp; IWM 0,42 bp — plano §8; IWN **a medir** com a mesma query de preço). **SCHA** cota US$ 32,59 → tick 3,07 bp: só a `--slippage-bps 4`–5, ou excluído. SLYV (tick 0,96 bp) e IJR (tick **a medir**) só como sensibilidade, nunca no critério.
- **Dados:** VB, SCHA e IJR param em 06/08/2026 no banco dev — reingerir antes (`trader-cli ingest --symbol VB --timeframe 15m --days 40 --provider ibkr`, um por vez, pacing do gateway; plano-mestre §5.6), **pelo PC/TWS e nunca pelo Gateway do servidor**, que entrega barras esparsas (plano §2.3 achado 7; ranking item 7).
- **Símbolos compartilhados:** IWN é candidata também à balance-area (§6.5 do plano) — uma posição por símbolo na conta; medir o bloqueio mútuo antes de escalar.

## 13. Evidência que motiva a v2 — re-simulação dos críticos (06/09/2026)

Todos os números abaixo são a 2 bp/lado, sobre os JSONs de backtest da v1 (runs 536/538/540/541/542/544) e candles do banco dev, reproduzidos por três críticos independentes. "Flatten" = fechamento no close da barra 15:45 ET. Nada disto é OOS em relação à escolha da regra.

**Pool de 6 símbolos (IWM, IWN, VB, SCHA, IJR, SLYV), short, sinal ≤ 10:15, flatten — a variante do designer:**

| Recorte | n | WR | PF | avg R | Net (US$) |
|---|---|---|---|---|---|
| short, sinal ≤ 10:15, entrada ≤ 10:30 (designer) | 99 | 57,6% | 2,52 | 0,49 | +19.363 |
| idem, com `entry_validity_candles = 2` real (crítico de código) | 101 | — | 2,52 | 0,490 | +19.651 (= 19.363 + 288 dos 2 trades a mais) |
| long-only, flatten (mesmo pool) | 71 | — | 0,93 | — | PF < 1 em 5 de 6 símbolos |
| por símbolo (short ≤ 10:15) | IWM 20 / IWN 15 / VB 13 / SCHA 20 / IJR 19 / SLYV 12 | — | 2,43 / 3,25 / 4,49 / 2,96 / 1,66 / 1,75 | — | — |

**Sem o corte 10:15 (regra primária da v2), 6 símbolos, short, sem flatten:** n=125, PF 1,59, avg R 0,29, t=2,19 (+13.434) contra longs n=71 PF 1,06 (+993). Com flatten o designer mediu PF 1,85 (não re-simulado por crítico).

**Por ano e por trimestre (short, 6 símbolos, sem corte de hora):**

| Segmento | n | PF | avg R | Net |
|---|---|---|---|---|
| 2025 | 80 | 2,33 | 0,42 | +15,4k |
| 2026 | 45 | 0,82 | 0,045 (t=0,21) | −2,0k |
| 2025Q2 (crash + V) | 23 | 5,69 | — | +11.832 = **88% do P&L short** |
| 2026Q1 / 2026Q2 | 16 / 18 | 0,59 / 0,38 | — | −2.051 / −3.421 |
| ex fev–abr/2025 | 109 | 1,25 | t=1,58 | — |
| pares vivos IWM+IWN, short | 41 | 1,24 | t=0,89 | IWM short 1,06 (n=23) vs long 1,41 (n=13); IWN short 1,53 / long 1,00 |

**Concentração (variante ≤ 10:15):** 99 trades em 54 datas; 23/04/2025 (+6.516) e 12/05/2025 (+4.494) = ~57% do net; top-5 dias = 52% do bruto. No pool **sem** corte de hora (125 shorts), os 5 maiores trades = 49% do P&L short. Bloco 1 do walk-forward (fev–mai/2025) = 48% do net (+9.285); blocos 2–7: 84 trades, PF 1,85, net +10.078 — mas blocos 3/5/6 = −225/+106/−636.

**Graus de liberdade já gastos:** 6 de 14 símbolos escolhidos após ver a tabela; direção escolhida após ver o resultado; corte 10:15 apoiado em 26 trades (6 nos pares vivos). A v1 reprovou o gate A com 58 OOS e PF 1,11 (`docs/reports/gate-a-revalidacao-2026-09-04.md:30-31,49`); com flatten sobe para PF 1,74 em IWM/IWN (plano-mestre §2.3, achado 1).

## 14. Pré-registro e critérios de validação

**Hipótese primária (H1):** `opening-reversal-v2` (= v1 + `short_only`, janela 09:30–10:30, validade 2), com flatten (ADR-018), em IWM, IWN e VB, walk-forward de 6 janelas: OOS agregado ≥ 50 trades, PF ≥ 1,3, avg R > 0,15, WR ≥ 40%, DD ≤ 10% **e** os critérios do gate A proposto (ADR-019, plano §5.3): PF_R ≥ 1,2, limite inferior do IC95 em blocos do PF ≥ 1,0, share dos 2 melhores meses ≤ 60%, pass no holdout travado.

**Hipótese secundária (H2):** H1 + `trading_end_time = 10:15:00`. Só é reportada ao lado de H1; nunca substitui H1 se H1 falhar.

**Falsificação — em dados que não existiam quando a regra foi escolhida:**
1. **Holdout travado** (últimos 4–6 meses do histórico, nunca tocado pela seleção; roda uma vez): avg R ≤ 0 → reprovada.
2. **Segmento 2026** reportado isolado. Hoje os dois recortes divergem e os dois entram no relatório: variante ≤ 10:15 com flatten, 29 trades, PF 1,64 (crítico estatístico); regra primária H1 (sem corte de hora), sem flatten, 45 trades, PF 0,82 (crítico de setups-novos-2). O segmento 2026 de H1 **com** flatten está **a medir** (harness, ADR-019). PF < 1,0 no segmento de H1 → hipótese de regime confirmada, arquivar.
3. **Paper forward** (gate B, 4 semanas por instância, ADR-010): único OOS verdadeiro.
4. **Controle:** v1 mantida em paper em símbolo distinto. Se o lado long da v1 der PF > 1,3 em ≥ 4 de 6 janelas com flatten, a assimetria que motiva a v2 está refutada.

**Relatório obrigatório (ADR-019):** P&L por dia ET com share do top-1 e top-5 dias; P&L **excluindo 23/04 e 12/05/2025**; por ano e por bloco; por direção (a v2 só tem uma — o controle v1 dá a outra); P&L com **teto de 3 posições aplicado dia a dia** (replay de portfólio, plano §6.4); corr(risk, R) e PF_R ao lado de PF$.

**Contador de tentativas (`n_trials`):** direção {both, short_only} × `trading_end_time` {10:30, 10:15} × `entry_validity_candles` {2, 1} = até 8 combinações, × 3 pares, mais os 6 símbolos já olhados — registrar todos, inclusive os deste documento.

**Sequenciamento (plano §9):** **Onda C** — linha "8+: Onda C conforme resultados", não a Onda B (o item 19 do ranking é Tier C). Pré-requisitos duros, todos da Onda A/B: §5.1 (flatten, ADR-018), §5.2 (harness, ADR-019), §5.6 (reingestão pelo PC/TWS), §6.4 (replay de portfólio); e a **condição de abertura** da Onda C (§6.3, §7): um segmento OOS com regime distinto (bear ou alta volatilidade) **ou** amostra de paper forward da v1 que justifique testar. Sem isso, nada deste documento é implementado. Cada instância nova reinicia o relógio de 4 semanas do gate B (§3.8) — se e quando a Onda C abrir, é **decisão do dono** onde a v2 entra em paper: recomendação é VB (onde a v1 não roda), mantendo IWM/IWN com a v1 como controle.

## 15. Risco operacional do short — teste obrigatório antes do gate B

O backtest não modela nada disto; o live já mostrou três dos quatro itens.

| Risco | Evidência | Teste / mitigação (antes do gate B) |
|---|---|---|
| Sell stop "cancelado" sem fill nem erro (**A9**) | VBR 28/08, ordem 17, sell stop 247,00; preço 35c abaixo do gatilho por 30 min; `cancelled` sem fill (`docs/reports/pregoes-2026-08-31_a_09-02.md:111-134`) | Correção do A9 (commit `82671e9`, confirmação de ordem) **em produção** — verificar no servidor, não no dev. Extrair dos `system_events` de produção as ordens sell-stop canceladas/rejeitadas por símbolo |
| Aluguel / shortable | `ibkr/broker.rs:605-608` mapeia `Sell` → `IbAction::Sell` sem checar disponibilidade | **3 sell stops fora de setup** na paper (IWM, IWN, VB), confirmando aceite e fill; adicionar checagem de shortable no envio (mudança de adapter — nota, não ADR) |
| SSR (Reg SHO 201) | dia após queda > 10% close-to-close; sem tratamento no bot | Registrar no `market_snapshot` a queda do dia anterior; comportamento a medir na paper — **a medir** |
| Latência da 1ª hora | Feed sem realtime: barra consolida 3–4 min (`docs/HANDOFF.md:172`), o bot espera estabilizar (`paper.rs:1125-1141`); ordem sai ≥ 4–5 min após o fechamento; nas barras de 5m de SPY, 12/20 (60%) dos gatilhos da 1ª hora já tinham sido cruzados nos 5 min seguintes (crítico operacional) | Medir por sinal na v1 em paper: (fechamento da barra → envio) e overshoot no envio (plano §5.8, `orders.metadata`). Se > 40% dos sinais da 1ª hora caírem em `SetupInvalidated`, o setup não é executável neste feed. Cenário pessimista no backtest: `entry_starts_next_candle` (§5.8) — **a medir** |
| Custo em SCHA | tick 3,07 bp vs 2 bp/lado calibrados para ETFs de US$ 120–435 | `--slippage-bps 4`–5 ou exclusão (§12) |

## 16. Plano de Testes Unitários (candles sintéticos)

Os 12 casos da v1 (`opening_reversal_v1/tests.rs:141-383`) são copiados para o módulo v2 e continuam valendo para o lado short. Novos:

1. **Setup long perfeito** (teste da mínima + bull reversal bar) → `Rejected(DirectionFiltered)` com `details.direction == "Long"` — e **não** `WeakConfirmation`.
2. **Setup short perfeito** → sinal short com entrada/stop/alvo **idênticos** aos da v1 na mesma série (paridade).
3. **`direction_filter = both`** → o setup long do caso 1 vira sinal (prova que o filtro é a única diferença).
4. **`direction_filter = long_only`** → setup short rejeitado com `DirectionFiltered` (simetria; não usado em produção).
5. **Toque na mínima com barra fraca** → `DirectionFiltered` (ordem de avaliação de §6.2: o filtro vem antes da barra de sinal).
6. **Toque duplo** (máxima e mínima na mesma barra) → continua `IncompleteSetup`.
7. **Janela:** barra de sinal 10:30 aceita com `trading_end_time = 10:30:00`; rejeitada (`OutsideTradingHours`) com `10:15:00`; barra 10:15 aceita nos dois — documenta a semântica `time > end` com timestamp de janeiro **e** de julho (DST).
8. **TOML da v2 faz parse**; `config_hash` ≠ hash da v1; chave desconhecida (`direction_filtre`) falha o parse (`deny_unknown_fields`, ADR-019).
9. **Serde:** `RejectionReason::DirectionFiltered` ↔ `"direction_filtered"` no round-trip de `signals.rs:275-329`.
10. **Snapshot:** `market_snapshot["direction_filter"] == "short_only"` no sinal.

## 17. Decisões de Implementação

### Onde vive no código (esforço S; sem mudança de motor, sem ADR próprio)

```text
crates/trader-core/src/strategies/opening_reversal_v2/     (cópia da v1; a v1 não é tocada)
  mod.rs      → OpeningReversalV2, id "opening-reversal-v2", version "2.0.0"
  context.rs  → idêntico
  setup.rs    → + filtro de direção após o match (direction, level)
  entry.rs    → + "direction_filter" no market_snapshot
  config.rs   → + DirectionFilter e campo direction_filter; deny_unknown_fields
  tests.rs    → 12 casos da v1 + os 10 de §16
crates/trader-core/src/strategies/mod.rs:7,17               → registrar o módulo e o re-export
crates/trader-domain/src/signals.rs:39-147, 275-329         → DirectionFiltered + teste serde
crates/trader-cli/src/dispatch.rs:28, 37-76, 83-88, 98-103, 113-118, 129-141, 150-211
                                                             → variante OpeningReversalV2 em todos os match; time_exit → None
crates/trader-cli/src/risk_config.rs:14, 77-88              → impl From<&OpeningReversalV2Params> for StrategyRiskParams
config/strategies/opening-reversal-v2.toml                  → novo
docs/strategies/opening-reversal-v2.md                      → este documento
```

### Esboços (não são código final)

```rust
// config.rs (v2)
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DirectionFilter { Both, LongOnly, ShortOnly }

impl DirectionFilter {
    pub fn allows(self, direction: Direction) -> bool { /* Both → true; LongOnly → Long; ShortOnly → Short */ }
}
// StrategyParameters: + pub direction_filter: DirectionFilter  (default ShortOnly na v2)
```

```rust
// setup.rs::detect_setup (v2), imediatamente após o `match (touches_high, touches_low)`:
if !params.direction_filter.allows(direction) {
    return Err((
        RejectionReason::DirectionFiltered,
        json!({ "direction": format!("{direction:?}"), "direction_filter": params.direction_filter }),
    ));
}
```

```toml
# config/strategies/opening-reversal-v2.toml (diferenças em relação à v1)
[strategy]
id = "opening-reversal-v2"
version = "2.0.0"
source = "Al Brooks - Reading Price Charts Bar by Bar, Cap. 11 (Opening Reversals) + Cap. 1 (Signal Bars)"

[strategy.parameters]
# ... todos os parâmetros da v1, valores idênticos ...
# Única regra nova (interpretação nossa; plano-mestre §6.3). Valores: both | long_only | short_only
direction_filter = "short_only"
# Paridade com a v1. Com validade 2 a entrada pode acontecer na barra 10:45 (doc §7).
entry_validity_candles = 2
trading_start_time = "09:30:00"
# Hipótese secundária 10:15:00 só via override do harness (ADR-019), nunca editando este arquivo.
trading_end_time = "10:30:00"
```

Regras do repo respeitadas: `Decimal` para preços; toda regra vem do TOML; `RejectionReason` específico; paridade live==backtest (o filtro é função pura da série, avaliada igual nos dois caminhos); stop server-side no bracket; paper only; sem martingale.

### Interpretações nossas (não estão no livro)

- O filtro de direção inteiro (§13 é dado do projeto, não Brooks).
- A leitura de "primeira hora" como 5 barras (v1, `time > end`) versus 4 barras (H2).
- Tudo o que a v1 já listava (zona 0,3%, vetos como rejeição, alvo 2R, 5min→15min).

## 18. Checklist de Validação

```text
[x] Documentação da estratégia preenchida (Fases 1–3)
[x] Regras objetivas definidas (uma regra nova, pré-registrada)
[x] Especificação técnica completa
[ ] Pré-requisitos da Onda A/B: flatten no backtest (ADR-018), harness (ADR-019), reingestão de VB/SCHA/IJR pelo PC/TWS, replay de portfólio (plano §6.4)
[ ] Condição de abertura da Onda C (plano §6.3/§7): segmento OOS com regime distinto ou amostra de paper forward — decisão do dono
[ ] Código implementado (módulo v2; v1 intacta)
[ ] Testes unitários passando (12 herdados + 10 novos)
[ ] Walk-forward 6 janelas com flatten em IWM, IWN, VB (H1) e H2 ao lado; n_trials registrado
[ ] Holdout travado rodado uma vez; segmento 2026 reportado isolado
[ ] Relatório de concentração (ex-23/04 e 12/05/2025; top-5 dias; por ano/bloco) e P&L com teto de 3 posições
[ ] Teste operacional de short na paper (3 sell stops fora de setup; shortable; A9 em produção; SSR)
[ ] Latência da 1ª hora medida na v1 em paper (fração de SetupInvalidated)
[ ] Métricas mínimas atingidas (gate A com a régua do ADR-019)
[ ] Gate B: 4 semanas de paper forward, v1 como controle em símbolo distinto
[ ] Nenhuma violação de regra de segurança financeira
[ ] Versionada no git
```
