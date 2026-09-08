# Plano de lucratividade — novas técnicas, estratégias e ativos (setembro/2026)

**Autor:** análise automatizada (coding agent, pesquisa multiagente de 06–07/09/2026) a pedido do dono do projeto
**Pergunta do dono:** "elaborar a documentação de implementação de novas técnicas e estratégias com potencial maior de lucro para testarmos — com liberdade para pensar em outros ativos ou até criptomoedas; a ideia é melhorar a capacidade lucrativa do bot."
**Estado do código analisado:** `main` em `d5e7279` (06/09/2026); banco dev `trader_db` (porta 5434) com candles 15m de 24/02/2025 → 02/09/2026; produção = app umbrelOS v1.1 com 8 instâncias
> ## Estado de execução — atualizado em 08/09/2026
>
> O plano abaixo é o documento de 07/09 e **não foi reescrito**: ele registra o
> que se sabia quando foi produzido. Esta caixa registra o que já saiu do papel.
>
> | Item do ranking §4 | Estado |
> |---|---|
> | 1 — flatten de fim de sessão (ADR-018) | ✅ **implementado** e verificado com dado real |
> | 5 — hotfix ET do veto de meio-dia (§5.4) | ✅ **implementado** |
> | 3 — harness de validação (ADR-019) | ✅ **implementado em parte** — o núcleo entrou; o que ficou de fora está na seção "Pendente" do `ADR-019` (lista e contagem canônicas), e inclui o Sharpe diário |
> | 4 — relatório estatístico (§5.3) | ✅ **implementado** em 08/09 (`trader-research/`). O critério "IC95 em blocos ≥ 1,0" passou a ser avaliável — números em `docs/reports/estatistica-gate-a-2026-09-08.md` |
> | 6 — ADR-020 (sizing/liquidez) | ✅ **implementado** em 08/09 — os seis itens. Os defaults reproduzem produção trade a trade; o cap de liquidez entrou **desligado** porque o feed do Gateway não entrega o volume real desde 07/08 (achado 7 do §2.3, agora medido em todos os pares — achado 8 abaixo). Números em `docs/reports/sizing-adr020-2026-09-08.md` |
> | 7 — higiene e custo real (§5.6) | ✅ **implementado** em 08/09 — comissão por ação da IBKR, desconto no fill do alvo, `tick_size` sem f64, runs rotulados. Falta reingerir os 6 símbolos parados (depende do servidor) e o casamento do `CommissionReport` em poll posterior (parcial: mede e loga, não fecha). Números em `docs/reports/custo-real-2026-09-08.md` |
> | 2, 8, 9, 9b — feed, screener, A9, sair de IWV | ⏳ pendentes (2, 9 e 9b dependem do servidor ou de decisão do dono) |
>
> **Nada foi enviado com push** — ver o aviso de deploy em `docs/HANDOFF.md`.
> O estado exato de `main` não é fixado aqui de propósito: um SHA envelheceria
> a cada commit. Use `git log --oneline`.
>
> **Oito coisas que a execução mostrou e que o texto abaixo ainda não sabia:**
>
> 1. **§5.4 diz que o efeito do hotfix ET seria "imensurável". Não é.** Medido:
>    a fade cai de PF 1,74 / avg R 0,218 / +4.560 para **PF 1,57 / 0,182 /
>    +3.618** in-sample (−21% no net), tudo em AVUV, com a **mesma** contagem de
>    trades — a correção troca *quais* sinais passam. No OOS, AVUV cai de avg R
>    0,256 para 0,159. O bug estava ajudando.
> 2. **§5.2 e §5.6 pedem índice único e dedupe de `backtest_runs`; a
>    implementação recusou os dois, com número.** No maior grupo "duplicado"
>    (24 linhas) há **sete valores distintos de `final_equity`**: não são
>    cópias. O `config_hash` cobre só o TOML da estratégia, não a versão do
>    motor, o slippage nem a régua de fim de sessão. Deduplicar apagaria
>    resultados diferentes entre si. A migração 0005 cria só o índice de busca.
> 3. **O PF em R, que o §2.3 achado 4 previa, é pior do que o plano estimava:**
>    a balance-area tem **PF_R 0,74 em AVUV e 0,97 em VBR** — abaixo de 1, ou
>    seja, sem edge em unidades de risco.
> 4. **A unidade em que o gate é lido decide o veredito.** Com o §5.3
>    implementado, os mesmos 214 trades reprovam nos **oito** pares isolados,
>    reprovam nas **três** estratégias agregadas e **passam** em todos os
>    critérios — os seis do ADR-010 e os três mensuráveis do ADR-019 §7 —
>    quando lidos como **um portfólio de oito pares**. Escolher a unidade não é
>    detalhe de apresentação; é a decisão.
> 5. **O esquema do bootstrap não é intercambiável.** Reamostrar só os pregões
>    *com trade*, em vez de todos como o ADR-019 §8 pré-registrou, fez a
>    balance em IJS **passar** (1,337) onde ela reprova (0,848). Com 17–32 dias
>    ativos em ~330 pregões, um bloco de 5 vira 16–29% da série e o bootstrap
>    circular degenera. Foi erro meu na primeira implementação, pego na revisão
>    adversarial.
>
> 6. **O simulador cobrava 1/10 da comissão real — e o segundo custo é maior
>    que a comissão.** US$ 0,35 fixos por perna contra os US$ 0,005 **por
>    ação** da IBKR: nos 214 trades OOS (234 a 1.311 ações, mediana 619), a
>    comissão vai de US$ 149,80 para US$ 1.489,36, **9,9×**. Mas o desconto no
>    fill do alvo — que o alvo não pagava — soma **US$ 1.760**, mais que a
>    comissão inteira: ele explica **56%** da queda de P&L, e a comissão, 43%.
>    Com o custo certo, **nenhum recorte passa o gate** — nem por par, nem por
>    estratégia, nem o portfólio dos oito, que reprova no avg R (0,107 contra
>    0,15). A `fade` em AVUV e em IWV passam a reprovar o profit factor; a
>    `balance-area` agregada fica com **PF em R 0,94 e avg R negativo**. Duas
>    das três estratégias ficam **negativas em 2026**. O veredito de manchete
>    **não depende** do desconto (com ele zerado o avg R do portfólio é 0,147,
>    ainda abaixo de 0,15); o do PF da fade em AVUV depende inteiramente.
> 7. **O `paper` simulado media com régua 5× mais cara que o backtest** —
>    10 bp de slippage contra 2 bp —, embaixo de um comentário que diz
>    "compartilhado com o backtest para garantir paridade de validação".
>    Corrigido: os dois vêm do mesmo `Default`.
>
> 8. **O achado 7 (feed esparso) medido em todos os pares — e a reingestão
>    não conserta.** Não é achado novo: o §2.3 já registra "3–10% do volume"
>    desde a troca para o Gateway em 07/08, com estimativas por símbolo
>    (AVUV ≈ 9%, IJS ≈ 23%, IWM ≈ 3–4%) tiradas dos dias gravados pelo
>    servidor. Ao calibrar o cap de liquidez do ADR-020 (08/09), a barra
>    mediana foi medida sobre **todos** os pregões: IWM guarda **3,2%** do
>    volume pré-Gateway, AVUV 7,1%, IWN 20%, IWV 32%, IJS 41%, VBR 51% e SLYV
>    **89%**. Duas coisas que a estimativa não tinha: (a) o dano é muito mais
>    disperso do que "3–10%" — SLYV quase intacto, IJS quase o dobro do
>    estimado; (b) os pregões de agosto foram **reingeridos em 03/09 e
>    voltaram com o mesmo volume baixo**, ou seja, não é barra parcial do poll
>    do live e **reingerir do Gateway não repara** — o histórico de 07/08 em
>    diante precisa vir de outra fonte, não só as barras futuras.
>    **O que isto contamina:** o cap de liquidez do ADR-020 (por isso ele
>    entrou desligado) e qualquer leitura de volume a partir de 07/08.
>    **O que NÃO contamina:** nenhuma das três estratégias vivas usa volume
>    nas regras — só a `breakout-first-pullback-v1`, arquivada, tem filtro por
>    volume —, então os vereditos de gate A não dependem disto.
>
> Veredito do gate A em `docs/reports/gate-a-com-flatten-2026-09-07.md`;
> estatística em `docs/reports/estatistica-gate-a-2026-09-08.md`; custo real em
> `docs/reports/custo-real-2026-09-08.md`; dimensionamento em
> `docs/reports/sizing-adr020-2026-09-08.md`.

**Regra de leitura:** nada aqui altera as **regras** das estratégias v1 em produção; as duas exceções são o hotfix do veto de meio-dia (§5.4, correção de bug com nota, precedente A2) e o piso de stop transversal (§6.9), se o dono aprová-lo. Toda mudança de regra é v2 validada do zero (framework §4); toda mudança de motor tem ADR proposto. Números de backtest são a 2 bp/lado e, salvo indicação, **sem** o flatten de fim de sessão que o live faz — ver §2.3, que é o achado central.

---

## 0. Sumário executivo

1. **A régua está errada antes de qualquer estratégia nova.** O motor de backtest deixa posições atravessarem a noite; o live encerra tudo às 15h55 ET. Com o flatten simulado, a `balance-area-breakout-v1` **cai de PF 1,92 para 1,55 e o avg R de 0,21 para −0,01** nos três pares vivos (20 de 96 trades overnight carregam 65% do P&L) — ela **reprova o gate A pelo avg R**. A `range-extreme-fade-v1` cai de PF 1,88 para 1,74 (32% do P&L era overnight) e continua passando. A `opening-reversal-v1` **sobe** de 1,23 para 1,74. O gate A de 04/09 e o gate B em curso comparam o live com um backtest que "ganha dormindo comprado". Corrigir isso é a primeira entrega (ADR-018).
2. **O edge do projeto é estreito e concentrado.** Reversão à média em ETFs small-cap value, com stops de 20–35 bp. Tudo com stop menor que ~20 bp morre no custo (tercil de stop ≤ 17 bp: WR 29%, PF 0,53; > 28 bp: WR 58%, PF 3,05). O P&L do portfólio de 8 pares em 18 meses (US$ 24k somando backtests isolados a 100k por par; US$ 19k com flatten; US$ 12k sob a regra de dinheiro real do ADR-017) veio **quase inteiro de junho–outubro de 2025**: na régua antiga os últimos 10 meses somam −US$ 178; na conta real (238k) com flatten, o cenário "paper hoje" rende US$ 27k/ano no histórico inteiro, mas só 7,7k/ano fora de jun–out/2025 e 13,4k/ano em 2026, com IC95 anual que **inclui zero** nos dois recortes pós-2025 (§2.5). Nenhuma estratégia sobrevive a um Deflated Sharpe com o histórico atual (todas ficam abaixo de 0,95 com qualquer N ≥ 2).
3. **A maior alavanca de amostra está dentro de casa:** o contexto global (`is_tradeable` exige Uptrend/Downtrend) rejeita por `NoContext` **24/18/31 sinais** (73 no total) da `range-extreme-fade-v1` em AVUV/SLYV/IWV contra 31/30/35 ordens de entrada enviadas (29/21/19 trades fechados) — os 48/36/62 eram exatamente o dobro, porque a rejeição é logada duas vezes no mesmo run (`execution/mod.rs:154` e `engine.rs:228`) e a medição contou linhas de log — a estratégia de dia de range é bloqueada justamente nos dias de range. Liberar `Neutral` para ela (política de contexto por estratégia) era a hipótese de maior potencial com menor custo; medida na auditoria de 07/09 (re-simulação dos 73 sinais bloqueados com a mecânica do simulador, replay validado contra o backtest oficial), ela é **negativa**: 54 trades, WR 33,3%, PF 0,70, avg R −0,321, −US$ 3.257 (com flatten, PF 0,56). Só AVUV isolado é positivo (PF 1,33 em 20 trades); o agregado é carregado por IWV (PF 0,28).
4. **Cripto, futuros e forex não pagam o edge que existe.** Cripto spot na IBKR Canada é, na melhor hipótese, sem stop server-side na API e a 12–18 bp/lado (Kraken: 40–80 bp/lado) contra stops de 20–30 bp; micro futuros de índice herdariam um edge que **não existe no subjacente** (IWM/SPY: PF 0,4–0,8 nas duas aprovadas, mesmo a 1 bp); forex em 15m tem barras de 5–8 bp contra piso de 20 bp de stop. O único caminho barato para "outros ativos" é o mesmo pipeline (ETFs US): ETFs setoriais de valor cíclico e ETFs de bitcoin (IBIT/ETHA) entram como tickers extras num screening **pré-registrado e agregado**, só depois do flatten no backtest.
5. **Ordem de execução:** Onda A (2–3 semanas) corrige a régua e a higiene (flatten, integridade do feed de produção, harness de validação, hotfix do veto de meio-dia, cap de liquidez, comissão real, screener SQL como Fase 0); Onda B (4–8 semanas) testa as duas v2 com evidência que sobreviveu à crítica (`range-extreme-fade-v2` com contexto Neutral; `balance-area-breakout-v2` com filtro de convicção), o piso de stop transversal, as janelas horárias e a varredura de alvo pareada como variantes pré-registradas, o replay de portfólio com conta compartilhada e a expansão de pares pré-registrada; Onda C guarda o que depende de B ou de regime novo (`opening-reversal-v2` short-only é hipótese de regime: 2025 PF 2,33, 2026 PF 0,82). Nada disto promete um número de lucro: promete parar de medir errado e testar, na ordem certa, o que tem chance.

## 1. Como este plano foi produzido

Pesquisa multiagente em duas fases (06–07/09/2026), toda ela sobre o repositório, o banco dev e a web:

| Fase | O quê | Resultado |
|---|---|---|
| Mapear | 5 leitores independentes: motor (execução/risco/backtest), estratégias (post-mortem das 9), livros (candidatos não implementados nas 6 análises), infra (IBKR Canada, mercados, deploy) e banco (estatísticas dos 136k candles) | ~150 fatos citados por `arquivo:linha` ou consulta SQL |
| Propor | 6 designers com lentes distintas: edge existente, setups novos, novos mercados, validação quant, execução/custo, portfólio/regime | 41 propostas com hipótese falsificável, fonte, teste e esboço de implementação |
| Criticar | 3 críticos adversariais por lente (estatística, código, operação), instruídos a **refutar**; 16 dos 18 rodaram | 41 vereditos por proposta: 33 "revisar", 7 "matar", 1 "manter" (109 vereditos individuais: 82 R, 19 X, 8 M); 12 itens acabaram em Tier D no ranking §4 |
| Consolidar | síntese + crítico de completude | este documento |

Os críticos **re-simularam** os trades a partir dos JSONs de backtest (`trader-cli backtest --output`) e dos candles do banco, reproduzindo o motor ao centavo antes de testar cada afirmação; os números deste plano são os reproduzidos por eles, não os dos designers. Os scripts e consultas usados ficaram no scratchpad da sessão; os que valem como ferramenta foram trazidos para o repositório em `sql/screens/` (§5.7).

Vereditos por proposta, com refutação e conserto, estão em `docs/reports/pesquisa-lucratividade-2026-09-07.md`.

---

## 2. Diagnóstico

### 2.1 Assinatura do que ganha e do que perde

Com o motor corrigido em 03–04/09 (ADR-015, A3, A4), 9 estratégias em 14 ativos dão um padrão nítido:

| Ganha (PF ≥ 1,9 OOS a 2 bp, sem flatten) | Perde (PF < 1 a 2 bp) |
|---|---|
| Fade de novo extremo em **dia de range**, com barra de sinal forte (`range-extreme-fade-v1`: 64 OOS, PF 2,10) | Continuação de tendência intraday em 15m (`pullback-trend-v1` 0,83; `low2-m2s-short-v1` 0,66; `breakout-first-pullback-v1` 0,92) |
| Rompimento aceito de **congestão multi-dia** (`balance-area-breakout-v1`: 91 OOS, PF 1,96 — mas ver §2.3) | Reversão "major" após trendline break (`trendline-break-test-v1` 0,91) |
| Lado **short** da `opening-reversal-v1` na 1ª hora (in-sample) | Alvo estrutural longe com stop no ruído (`value-area-reentry-v1` 0,65; `failure-test-long-v1` 0,61) |
| Stops de 20–35 bp do preço (≥ 1 barra de 15m) | Stops ≤ 15 bp (pullback: 15,5 bp mediano; IWV 9,3 bp) |
| 2–3 condições conjuntivas → 13–27 trades/ano/ativo | 5+ condições → 1–10 trades/ano/ativo (morre de amostra) |
| ETFs **small/mid-cap value** (IJS, VBR, AVUV, SLYV, IWN) | Índices growth/momentum (SPY, QQQ, IWM, IWO) para fade |

Fontes: `docs/reports/gate-a-revalidacao-2026-09-04.md`, `docs/decisions/ADR-016`, backtests de 06/09 a 2 bp (runs 529+ no banco dev).

### 2.2 O custo é o eixo de seleção — e o stop é o que o paga

Ida e volta = 4 bp. É 26% do R na pullback (stop 15,5 bp), 18% na balance (22 bp), 19% na range-fade (21 bp), 13% na opening-reversal (31,5 bp). A varredura de 04/09 (PF agregado de 0 → 2 bp, runs 337–412): balance 2,29 → 1,92; opening-reversal 1,41 → 1,23; range-fade 2,39 → 1,88; pullback 1,07 → 0,84 (a 5 bp: 1,52 / 1,02 / 1,46 / 0,60).

O achado mais sólido desta pesquisa vem de cruzar a distância do stop com o resultado, no OOS da balance-area (92 trades, 3 pares):

| Tercil de stop | WR | PF |
|---|---|---|
| ≤ 17 bp | 29% | 0,53 |
| 17–28 bp | 53% | 1,99 |
| > 28 bp | 58% | 3,05 |

Não é "acidente de sizing": é **stop estreito perde**. Grimes (Cap. 8) diz o mesmo em prosa, e mais forte do que este plano usa: stop inicial mais próximo que o range médio de 1 barra "significantly impaired whatever edge you might have had", com guideline de **raramente menos de 2 ATRs, às vezes mais de 4** (`docs/books/analysis/grimes-art-science-ta.md:150`) — o piso de ~1 × ATR14 proposto em §6.9 é mais frouxo que a fonte. Regra derivada para qualquer candidata: stop < 0,25% do preço nasce morta; e o "PF em dólares" esconde isso porque o sizing trava no notional (§2.3, achado 4).

Agravante de regime: agosto/2026 teve o menor range diário da amostra (0,78% médio nos pares vivos contra 1,05–1,87% nos demais meses; abr/2025 3,43%). Setembro começou em 1,05%.

### 2.3 Sete achados que os relatórios do projeto ainda não registram

**1. O backtest não fazia flatten; o live faz. ✅ CORRIGIDO em 07/09** (ADR-018).

> O motor passou a encerrar na última barra do pregão. Ele **reproduziu a
> estrutura** desta re-simulação — mesma contagem de trades (96/69/67) e
> exatamente as mesmas saídas overnight (20/9/8, IWV em 0) — mas **os valores
> em dólares da tabela abaixo estão superados**: o motor mediu +6.576 / +4.560 /
> +8.085 contra os +6.730 / +4.578 / +8.072 publicados aqui. PF e avg R
> coincidem. Valores válidos em `docs/reports/gate-a-com-flatten-2026-09-07.md`;
> a descrição do estado do motor daqui em diante vale como histórico, não como
> descrição do código de hoje. `BacktestEngine::run` (`crates/trader-backtest/src/engine.rs:136-240`) não tem fim de sessão; o live fecha a mercado às 15h55–16h10 ET (`paper.rs:2189-2198`). Re-simulando o flatten no close da barra 15h45 ET com 2 bp (três críticos independentes chegaram aos mesmos números):

| Estratégia (pares vivos, 2 bp, in-sample) | Sem flatten | Com flatten | Overnight |
|---|---|---|---|
| balance-area-breakout-v1 (96 t) | PF 1,92 · avgR 0,214 · +14.648 | **PF 1,55 · avgR −0,007 · +6.730** | 20 trades = +9.577 (65%) |
| — IJS / VBR / AVUV | 1,95 / 1,96 / 1,85 | 1,86 (avgR 0,27) / 1,47 (−0,04) / 1,43 (−0,17) | |
| range-extreme-fade-v1 (69 t) | PF 1,88 · avgR 0,297 · +6.015 | PF 1,74 · avgR 0,218 · +4.578 | 9 trades = +1.933 (32%) |
| opening-reversal-v1 IWM/IWN (67 t) | PF 1,23 · +3.400 | **PF 1,74 · +8.072** | 8 trades = −2.722 (6 stopados no dia seguinte) |

Consequências: a balance-area **reprova o gate A pelo avg R** (> 0,15) com a régua do live — só IJS passa sozinha; o gate B em curso compara o live com um backtest inflado; e os 20 trades overnight da balance são simétricos (10 long / 10 short, 14 alvos / 6 stops), o que sugere que o edge dela pode ser multi-dia — hipótese para a Onda C, não para a v1. A "subida" da opening-reversal vem de remover 8 trades overnight perdedores de 67; o bootstrap dá P(PF openrev > PF balance sob flatten) = 0,63 e o gate A OOS vigente dela continua em 1,11 — não é reabilitação. **Régua oficial daqui em diante:** flatten a mercado na última barra RTH **com** slippage; "ex-overnight" (descartar os trades) serve só como diagnóstico e dá números diferentes (balance PF 1,43/avg R −0,05).

**2. O edge da opening-reversal é do lado short e de 2025.** Somando 6 combinações a 2 bp: 125 shorts PF 1,59 (+13,4k) contra 71 longs PF 1,06. Mas por ano: shorts **2025 n=80 PF 2,33; 2026 n=45 PF 0,82**. O 2º trimestre de 2025 (crash e recuperação em V) responde por 88% do P&L short. Nos pares vivos o short é PF 1,24 (n=41), indistinguível de 1,0. É hipótese de regime, não edge estrutural — testável, mas só com pré-registro e controle.

**3. A range-fade vive do lado long e da manhã.** Nos pares vivos: long PF 3,34 (n=33) vs short 1,00 (n=36); 10h–11h ET = 100% do P&L. O bloco 12h–14h tem PF 0,87–1,29 conforme a re-simulação (n=30, avgR ≈ 0, t ≈ 0) — ruído, não perda; vetá-lo cortaria 43% da amostra por nada. **Bug real — ✅ CORRIGIDO em 07/09** (hotfix v1.0.1, §5.4; efeito medido e NEGATIVO, ver lá): o veto `midday_midrange` comparava o horário em UTC fixo (`range_extreme_fade_v1/context.rs:220-238`; TOML 15:30–18:00 UTC) e desliza 1h no horário de inverno — cobre 10h30–13h ET em vez de 11h30–14h; ~24% do **tempo** da amostra do gate A (≈ 20–25 trades da fade) foi medido com o veto deslocado.

**4. O sizing trava no notional e o risco real por trade é 0,15–0,32%, não 1%.** `qty = min(orçamento/dist_stop, capital/entry)` com cap de 1× equity hardcoded (`risk/mod.rs:239-279`). Com stops de 0,1–0,3% o cap prende sempre. Efeitos: (a) o $ arriscado é proporcional à distância do stop, e como stop largo ganha (§2.2), o **PF em dólares supera o PF em R**: balance OOS PF$ 2,09 vs PF_R 1,41 (AVUV 1,07, VBR 1,27, IJS 2,37; corr(risk, R) = 0,31); (b) o P&L absoluto é pequeno (~US$ 100/trade); (c) "risco 1%" só existiria com stop ≥ 1% do preço. A opening-reversal inverte (PF_R 1,28 > PF$ 1,11).

**5. O contexto global bloqueia a estratégia de range nos dias de range.** `is_tradeable = trend ∈ {Up, Down} ∧ vol ≠ High ∧ RTH` (`context/mod.rs:79-81`); o único lugar que rejeita por isso é `RiskManager::validate` (`risk/mod.rs:158-163`), e a `range-extreme-fade-v1` nunca lê `trend_state` — o detector dela exige EMA20 plana, o que **não** coincide com Neutral (44,4% das barras de dia de range são Neutral contra 41,9% nas de dia de tendência; "EMA20 plana" cobre 80–89% de todas as barras) — ver `docs/strategies/range-extreme-fade-v2.md` §3, que já registra a refutação. Medido com log de debug a 2 bp: **NoContext rejeitou 24 sinais em AVUV (55 sinais, 31 ordens), 18 em SLYV (48/30), 31 em IWV (66/35)** — os 48/36/62 publicados eram o dobro, porque a rejeição é logada duas vezes no mesmo run (`execution/mod.rs:154` e `engine.rs:228`) — só 3 dos 73 sinais são repetição do mesmo dia (23/18/29 dias distintos), então o que reduz o teto não é repetição e sim a taxa de fill da ordem stop: o teto real é ≈ 34 trades/ano nos três pares. Proxy nos candles: 39–42% de todas as barras e 44–49% das barras de dia de range são Neutral. O PF desses sinais bloqueados foi medido na auditoria de 07/09 e é **negativo**: 54 trades, WR 33,3%, PF 0,70, PF_R 0,62, avg R −0,321, −US$ 3.257 (com flatten, PF 0,56 e −US$ 3.985), negativo nos dois anos e nas duas direções; o conjunto cai de PF 1,90 para 1,16 (IC90 [0,80; 1,66]). AVUV isolado é positivo (PF 1,33, n=20); IWV carrega o negativo (PF 0,28). Confirmar no motor antes de descartar §6.1 (§6.1 passo 1).

**6. O P&L é concentrado em poucos dias e meses.** Balance-area OOS: 99% do P&L em dois meses (jul/25 +8.781, out/25 +6.985 de +15.957), 8 de 13 meses positivos; a variante de convicção (§6.2) tem 64% do P&L do pool num único dia (10/10/2025, 8 shorts simultâneos). Range-fade: top-2 meses = 40%, 11 de 16 meses positivos. O portfólio de 8 pares: jun–out/2025 = +24.564; nov/2025–ago/2026 = **−178**. Os dias de sinal em cluster são os dias bons **na balance-area** (dias com ≥ 2 entradas: PF 2,69; com 1 entrada: 0,70; no portfólio dos 8 pares o efeito cai para 2,19 contra 0,88, e em setups de fluxo medidos na auditoria ele **inverte**: 0,54 contra 0,93) — um teto por direção destruiria o edge, e o ADR-017 deve ser lido como "permitir o cluster".

**7. O feed de produção entrega barras esparsas.** Nos dias gravados pelo servidor (17–21/08, 28/08, 31/08–02/09) AVUV tem 60–100k ações/dia e range por barra de 1,6–3 bp, contra 780k–1M e 9–13 bp nos dias ingeridos pelo PC/TWS (≈ 6–13% do volume; pela mediana por barra, AVUV pós-Gateway ≈ 9% e IJS ≈ 23% do pré-Gateway; IWM ≈ 3–4%); IWM em 01/09 09:30 teve range real de 0,57% contra 0,19% gravado. A mesma conta, sem assinatura, via TWS no PC entregava barras completas com lag p50 de 29–54 s. O que mudou em 07/08 foi TWS → IB Gateway headless. Consequências que os críticos da lente de portfólio apontaram: os stops do live saem ~3× menores que os do backtest (a barra de sinal vista é menor), o critério "±30% do backtest" do gate B fica inatingível por construção, qualquer detector intradiário (tipo de dia, extremos do dia) vê dias de tendência como "normais", e **toda ingestão para screening tem de vir do PC/TWS, nunca do Gateway do servidor**. Enquanto isso não for diagnosticado, latência (ordem 17 enviada 6 min após a barra) e "custo por fill" medem principalmente o high/low que o feed não viu. Comissão: o simulador usa US$ 0,35/perna; a IBKR Canada cobra US$ 0,005/ação com mínimo US$ 1,00 (~US$ 4–5 por perna de 800–950 ações ≈ 0,9 bp ida e volta) e o casamento do `CommissionReport` só funciona se ele chegar no mesmo lote do poll (26 fills do dev com comissão zero).

### 2.4 Liquidez por barra: gargalo escondido

Equity da conta paper ≈ US$ 238k e cap de notional 1× → cada posição ≈ US$ 238k. Barra **mediana** de 15m no meio do dia (11h–13h ET), medida **antes de 07/08/2026** (depois disso o feed esparso do Gateway contamina o volume gravado): SLYV US$ 366k (p10 114k), IJS 527k (p10 164k), VBR ≈ 1,0–1,4M, IWN 1,6–1,8M, AVUV 2,6M. O bot já opera com 65% e 45% de uma barra mediana em dois dos oito pares (e com > 100% do p10); 2 bp não cobre isso, e a conta paper (fill a NBBO sem impacto) esconde. IWN tem PF igual ou maior na balance-area (2,23 com flatten, 41 trades) com 3–5× a liquidez de IJS.

### 2.5 Baseline de dinheiro, honesto

Tudo é paper. Dois níveis de leitura, ambos in-sample (24/02/2025 → 02/09/2026, 2 bp), replay em Python sobre os JSONs do motor de 06/09 (reproduz os críticos ao centavo; o motor com ADR-018 é a fonte de verdade). Detalhe completo, com IC95 e cenários, em `docs/reports/roadmap-decisao-2026-09-07.md` §1.

**Nível 1 — 8 backtests isolados com US$ 100k por par** (capital fictício de US$ 800k; é o que o gate A de 04/09 mediu):

| Régua | P&L 18,5 m | Observação |
|---|---|---|
| Sem flatten (régua antiga) | +24.063 | jun–out/2025 = +24.564; **nov/2025–ago/2026 = −178**; só 2026 = +432 |
| Com flatten 15h55 (régua do live) | +19.380 | PF 1,66; balance +6.730 · fade +4.578 · openrev +8.072 |
| Regra de dinheiro real do ADR-017 (1 posição, 100% notional), **sobre a régua com flatten** | +9.513 | 49 de 232 trades recusados (os +12.286 são a mesma trava aplicada à régua antiga, sem flatten — não encadear com a linha acima) |

**Nível 2 — na conta que existe (≈ US$ 238k), com flatten e as travas do ADR-017 como o live as executa** (cap 1× por instância, 3 posições, 200%, 4%/dia, comissão por ação):

| Cenário | Todo o histórico | Ex jun–out/2025 | Só 2026 |
|---|---|---|---|
| B — o que o paper faz hoje | **US$ 27,2k/ano (+11,4%)**, IC95 [+4,3k; +52,7k] | **7,7k/ano (+3,2%)**, IC95 [−12,4k; +28,6k] | **13,4k/ano (+5,6%)**, IC95 [−9,3k; +38,8k] |
| C — 100% de notional pela régua do **live** (o teto só olha as posições já abertas ⇒ 2 posições cabem) | 22,7k/ano | 6,3k/ano | 12,3k/ano |
| C' — 100% de notional como o ADR-017 está **escrito** (teto inclui a posição nova ⇒ 1 posição; 48 trades recusados) | 13,5k/ano | 2,4k/ano | 10,4k/ano |
| D — `capital_fraction` 1/3 (3 posições dentro de 100%) | 9,1k/ano (+3,8%) | 2,6k/ano (+1,1%) | 4,5k/ano (+1,9%) |

Leituras: (1) o "últimos 10 meses ≈ 0" da régua antiga vira +8,6k com flatten e 238k — porque a `opening-reversal-v1` evita 6 stops overnight e passa a carregar 2026; (2) **todo cenário pós-2025 tem IC95 anual que atravessa zero**; (3) em qualquer recorte pós-2025 os 2 melhores meses superam 100% do net; (4) risco real por trade mediano US$ 576 = 0,24% da conta; DD máximo 3,8% (1,3% com fração 1/3). Com feed íntegro e a régua certa, o cenário B é o número que o gate B tem de reproduzir dentro de ±30% antes de qualquer conversa de dinheiro real; e o cenário D (dinheiro real com 3 posições) é o que o dono deve confrontar com as horas: **US$ 2–5k/ano no regime de 2026**. Os "impactos" das Ondas B e C (§6–7) são hipóteses a testar, não somas a fazer sobre este baseline — o roadmap de decisão estima o valor esperado somado da Onda B em ≈ +US$ 9,6k/ano (tetos +30–45k, nenhum item com probabilidade > 0,40).

**O que fazer com as 3 instâncias da balance-area-v1 (IJS/VBR/AVUV) que reprovam com flatten:** manter em paper como **controle** (custam nada, geram amostra e o único OOS verdadeiro), bloqueadas para dinheiro real; ler o gate B delas contra o backtest com flatten; a decisão de desligar ou trocar pela v2 (§6.2) só depois do rerun do §5.1 e do replay de conta (§6.4). Não trocar as três de uma vez (§3.8).

### 2.6 O que o motor não expressa hoje

| Limitação (arquivo) | Consequência | Destrava |
|---|---|---|
| Alvo fixo obrigatório (`risk/mod.rs:146-155`) | Sem "deixar correr" | O7 (alvo opcional) |
| Sem modificação pós-fill (`ports.rs:44-56`) | Sem trailing/breakeven | O6 |
| Bracket com quantidade cheia nas 3 pernas (`ibkr/broker.rs:610-636`) | Sem parciais | O8 |
| Um timeframe, um símbolo (`crates/trader-domain/src/strategy.rs:50-55`) | Sem HTF, sem SPY/VIX como regime, sem breadth | O2 (`resample`), C4 |
| Estratégia stateless (`StrategyState` ignorado) | Sem cooldown, sem "uma tentativa por nível/dia" | O9 |
| `is_tradeable` rígido (`context/mod.rs:79-81`) | Range mudo em 40% das barras | **§6.1** |
| ~~Cap de notional 1× hardcoded (`risk/mod.rs:253`)~~ **resolvido em 08/09**: `max_notional_multiple`, `max_notional_usd`, `capital_fraction` e cap por liquidez em `RiskConfig`, com paridade automática (live, backtest e walk-forward passam pelo mesmo `build_risk_config`). O risco real continua ≪ 1% no modo de produção — agora medido: **100% dos 214 trades saem no teto de notional** | Risco real ≪ 1% (medido); cap por liquidez existe e está desligado | **§5.5** ✅ |
| ~~TIF Day + flatten só no live~~ **resolvido em 07/09**: o motor faz flatten por mudança de data ET, com a janela vindo de `[session]`. Só o swing (hold multi-dia) continua fora | Sem swing | **§5.1** ✅ |
| Walk-forward por contagem de candles, sem purge; ~~sem holdout~~ (**`--holdout-from` entrou em 07/09**: bloco travado, reportado à parte, com erro em vez de holdout desligado em silêncio); ~~sem DSR/PSR/IC~~ (**entraram em 08/09**, §5.3) | Viés de seleção das 42+ combinações agora tem número: DSR **0,45** no portfólio com N=42, contra o 0,95 convencional | **§5.2** ✅ **, §5.3** ✅ |
| Backtest single-symbol, capital fixo | Conta compartilhada (3 posições, 200%) não simulada | **§6.4** |
| Entrada limit no simulador enche na hora sem olhar o mercado (`simulated/broker.rs:522-575`) | Nenhum número de entrada limit vale (ADR-009 incluído) | infra |

Nota sobre o walk-forward: como as estratégias são funções puras de (contexto, série) e o walk-forward não re-ajusta nada, o "OOS" é a mesma rodada determinística após o primeiro bloco. Ele mede robustez temporal, não protege contra seleção de regra/par feita olhando o histórico inteiro. Por isso o harness (§5.2) precisa de um **holdout temporal travado** e de contagem de tentativas.

### 2.7 Mercados e ativos: o que a IBKR Canada permite (verificado 06/09/2026)

| Mercado | Disponível? | Esforço | Veredito desta pesquisa |
|---|---|---|---|
| ETFs US (setoriais, commodities, bonds, small-caps extras) | Sim | **S** (ingest + compose) | Único caminho barato; screening pré-registrado (§6.5) |
| ETFs de bitcoin/ether (IBIT, FBTC, ETHA) e alavancados/inversos/vol | Sim, com permissão "Complex or Leveraged ETPs" | **S/M** | IBIT/ETHA só como tickers extras do screening, só range-fade, só após flatten (§8) |
| Cripto spot | Fontes primárias negativas (.ca 410, sem permissão na lista, Paxos só EUA, OSC 2022); um review terceiro diz Zero Hash/Paxos exceto Quebec a 0,12–0,18% — **confirmar no Client Portal** | — | Mesmo se existir: API sem STP/bracket ("sempre stop" viola) e 12–80 bp/lado vs stops de 20–30 bp. **Arquivado** (§8) |
| Micro futuros CME (M2K/MES; MBT/MET) | Sim, permissão "Futures" | **XL** | Pré-teste negativo no subjacente; **arquivado** (§8) |
| Forex IDEALPRO | Sim (mín. US$ 25k/ordem) | **L** | Barra 15m de 5–8 bp vs piso 20 bp; **arquivado** (§8) |
| Ações individuais | Sim | **S** | Só como braço de controle do screening (4 nomes) |

Limites de escala: 3 client_ids por instância contra 32 por sessão do gateway (≈ 10 instâncias), pacing de histórico 60 req/10 min, 3 posições simultâneas na conta (ADR-017).

---

## 3. Princípios de decisão (aplicados a tudo abaixo)

1. **Régua do live.** Nenhum veredito de gate A antes do flatten no backtest (§5.1): flatten a mercado na última barra RTH, com slippage. Números "sem flatten" e "ex-overnight" só como comparação/diagnóstico, nunca misturados na mesma tabela.
2. **PF em R ao lado do PF em $**, e corr(risk_amount, R) no relatório — o gate A passa a olhar os dois.
3. **Pré-registro.** Universo, critério e N de tentativas declarados **antes** de rodar; resultado sob a hipótese nula reportado junto (ex.: com n=25 por combo, P(PF ≥ 1,3 | sem edge) = 0,20–0,28).
4. **Holdout temporal travado** (os últimos 4–6 meses do histórico) que nenhuma seleção toca e roda uma vez por família; e **paper forward** como único OOS verdadeiro.
5. **Concentração**: share do melhor dia e dos 2 melhores meses, e P&L por bloco/ano, obrigatórios no relatório. Com < 100 trades nenhuma estratégia deste projeto alcança DSR 0,95 — DSR/PSR viram relatório, não gate binário.
6. **Uma mudança por variante.** v2 = uma regra nova com fonte; ablações no harness, contadas no N.
7. **Regras do repo intactas:** v1 nunca muda (hotfix de bug documentado é exceção com nota, precedente A2); Decimal para dinheiro (f64 só em estatística); RejectionReason específico; paper only; sempre stop server-side; sem martingale.
8. **Amostra do gate B é o gargalo real** (0,60 trade/pregão — 232 trades em 384 pregões nos 8 pares vivos; o 0,74 vinha do relatório de 02/09, com 9 pares e a pullback já desligada; 0 trades das aprovadas desde 18/08). Toda troca de par ou de versão reinicia o relógio de 4 semanas — trocar tudo de uma vez zera a evidência.

---

## 4. Ranking

Tiers: **A** = fazer já (2–3 semanas; corrige a régua, custo baixo, sem mudar regra); **B** = próxima onda (v2 com evidência, 4–8 semanas, cada uma com gate A/B do zero); **C** = condicional a resultados de B; **D** = arquivado com número (não reabrir sem fato novo).

**✅ = entregue em 07/09/2026** (ver a caixa de estado no topo). O resto do
ranking continua valendo como lista de trabalho.

| # | Item | Tier | Esforço | Crítica (consenso) | Seção |
|---|---|---|---|---|---|
| 1 ✅ | Flatten de fim de sessão no backtest + `ExitReason::EndOfDay` + re-rodar gate A das 3 | **A** | S | manter (8/10) | §5.1 |
| 2 | Integridade do feed de produção (Gateway esparso vs TWS) + medição de lag por barra — pré-requisito do gate B e de qualquer instância nova | **A** | S/M | revisar (5–6); síntese: 2º lugar | §5.8 |
| 3 ✅ | Harness de validação: `--output/--slippage-bps/--label/--holdout-from/--strategy-config` no walkforward; PF_R, corr(risk,R), métricas por exit_reason/direção/hora/dia; `analyze` por (símbolo, estratégia, hash); `strategy_id` nos trades de backtest. **vários itens NÃO entraram** — entre eles `n_trials`, o Sharpe diário e o dedupe (recusado com número). Lista e contagem canônicas na seção "Pendente" do ADR-019 | **A** | M | revisar→manter (6–7) | §5.2 |
| 4 | Relatório estatístico em Python: PSR, IC95 por bootstrap em blocos, MC de drawdown, concentração mensal | **A** | S | manter (6–7) | §5.3 |
| 5 ✅ | Hotfix v1.0.1 da range-fade: veto de meio-dia em ET | **A** | S | consenso | §5.4 |
| 6 ✅ | Cap de notional por liquidez + `capital_fraction` + registro de equity/BUYING_POWER + trava de notional incluindo a posição prospectiva. **Implementado em 08/09**; o cap de liquidez fica desligado até o §5.8 (o feed não entrega o volume real desde 07/08 — achado 7) | **A** | S | revisar→manter | §5.5 |
| 7 | Higiene e custo real: reingerir 6 símbolos parados (pelo PC/TWS); ~~dedupe runs~~ (**recusado**, ver §5.6); rotular runs sem label; `tick_size` sem f64; comissão por ação no simulador; casamento do CommissionReport; slippage por faixa de liquidez | **A** | S | consenso | §5.6 |
| 8 | Fase 0 do framework: screener SQL com fill honesto (calibrado com controles) | **A** | S | manter (8) | §5.7 |
| 9 | A9: rejeição/timeout de confirmação de ordem como estados próprios + checagem de shortable + teste operacional de short na paper (dono da correção) | **A** | S/M | citado como bloqueador em 5 propostas | §5.9 |
| 9b | Retirar `range-extreme-fade-v1` de IWV | **A** | S | consenso | §5.10 |
| 10 | `range-extreme-fade-v2`: política de contexto `allow_neutral` (medir o delta antes de codar) | **B** | M | revisar (5–7) | §6.1 |
| 11 | `balance-area-breakout-v2`: filtro de convicção (stop ≥ 1,0–1,5 × ATR14) + segurar até o sino | **B** | M | revisar (4–6) | §6.2 |
| 12 | Replay de portfólio com conta compartilhada (regras exatas do live + flatten) como régua do gate B | **B** | M | revisar (5–7) | §6.4 |
| 13 | Piso de stop transversal no RiskManager (entrada−stop ≥ max(k × ATR14, ~20 bp), `RejectionReason::StopBelowFloor`) | **B** | S | síntese: manter | §6.9 |
| 14 | Janelas horárias por estratégia sob flatten (bab sinal ≤ 11:45, ref ≤ 12:45) como v2 pré-registradas, validadas só em paper | **B** | S | manter (6) / revisar (4): p≈0,21 | §6.10 |
| 15 | Varredura de alvo em desenho pareado (mesmas entradas): uma variante estrutural por estratégia, após o flatten | **B** | M | revisar (4–5) | §6.11 |
| 16 | Expansão de pares pré-registrada: balance-area em IWN; screening pooled de ETFs setoriais value + IBIT/ETHA + 4 ações como controle | **B** | S | revisar (5–6) | §6.5 |
| 17 | Instrumentação sem decisão: `day_type`, calendário (FOMC/opex), `range5`, stop_bp no `market_snapshot` | **B** | S | revisar (3–4) | §6.6 |
| 18 | Entrada STP LMT (paridade pós-envio) — só após medir `entries_cancelled_overshoot` e fills reais | **B/C** | M | revisar (4–6) | §6.7 |
| 19 | `opening-reversal-v2` short-only na 1ª hora — **hipótese de regime** (2025 PF 2,33 vs 2026 PF 0,82); spec pronta, implementação condicionada a OOS com regime distinto ou paper forward | **C** | S | manter (7) / matar (2): regime | §6.3 |
| 20 | Multi-estratégia por símbolo num processo + `analyze` por par (infra de escala) | **C** | L | revisar (4–5) | §6.8 |
| 21 | Swing v2 da balance-area (GTC, hold ≤ 1 noite) — só como estudo após #1 e #11 | **C** | L | revisar (4) | §7 |
| 22 | Timeframe 1h derivado (`resample`) — após #1; frequência antes | **C** | M | revisar (3–4) | §7 |
| 23 | Meta-labeling — só com ≥ 1.000 trades de estratégias com PF_R > 1; plumbing snapshot→journal entra em #3 | **C** | L | matar/revisar (2–4) | §7 |
| 24 | Escada de risco / Kelly — só após gate B com fills reais | **C** | S | revisar (3) | §7 |
| 25 | Micro futuros (M2K/MES) | **D** | XL | matar (1) | §8 |
| 26 | Cripto spot (IBKR ou Kraken) | **D** | M–XL | matar (1) | §8 |
| 27 | Forex IDEALPRO | **D** | L | matar (1) | §8 |
| 28 | ETFs 3× (TNA/TZA) como "stop largo" | **D** | S | matar (2–3) | §8 |
| 29 | Gate de regime por compressão de range | **D** | M | matar (2) | §8 |
| 30 | Teto por cluster / 1 posição por direção | **D** | M | revisar→descartar (4) | §8 |
| 31 | Entrada limit passiva na range-fade | **D** | M | matar (3) | §8 |
| 32 | Timeframe 5m para a opening-reversal | **D** | S | matar (1–2) | §8 |
| 33 | MOC em grupo OCA no flatten | **D** | M | matar | §8 |
| 34 | Veto integral 12h–15h na range-fade | **D** | S | matar (3–4) | §8 |
| 35 | Ações individuais como universo próprio | **D** | S | matar (2) | §8 |
| 36 | Instrument spec no domínio (sec_type/multiplier/expiry) — fatia 2 | **D** | L | revisar→descartar (sem cliente) | §8 |

---

## 5. Onda A — a régua certa (2–3 semanas)

### 5.1 Flatten de fim de sessão no backtest (ADR-018)

> ✅ **ENTREGUE em 07/09/2026** (commit `cbc8be5`). O roteiro abaixo foi
> executado; não o refaça. **Quatro** decisões de implementação divergiram do texto e
> estão em "Ajustes feitos na implementação" no ADR-018: o gatilho é **só** a
> mudança de data ET (o `ou hora ≥ 15:45` não entrou), e o **fim da série não é
> sino** — senão o walk-forward, que roda cada janela sobre um prefixo, geraria
> um `EndOfDay` fantasma em cada fronteira. Números do motor em
> `docs/reports/gate-a-com-flatten-2026-09-07.md`.

**Hipótese:** com o flatten replicado, a balance-area reprova o gate A pelo avg R, a range-fade continua passando e a opening-reversal melhora. **Já verificada por re-simulação** (§2.3) — a implementação é para o motor confirmar e para o gate B ter baseline coerente.

**O que fazer**
- `crates/trader-domain/src/trades.rs:106-112`: variante `ExitReason::EndOfDay` (serde `end_of_day`) + teste.
- Migração `0004_exit_reason_end_of_day.sql`: ampliar o CHECK de `trades.exit_reason`. `trade_repository.rs:25-31` tem `match` exaustivo na **escrita** (quebra a compilação); a leitura (`:325-331`) termina em `_ => Target` e falha em silêncio — adicionar o braço `"end_of_day"` explicitamente e cobrir com teste de round-trip Trade→DB→Trade.
- `crates/trader-backtest/src/engine.rs`: `BacktestConfig.session_flatten_et: Option<(u32,u32)>` (default 15:45 — a última barra RTH; candles do banco são RTH-only, 26 barras/dia). Na última barra ET do dia (próxima barra é de outra data ET ou hora ≥ 15:45), após `set_market_candle`: cancelar entrada pendente e, se houver posição, `broker.close_position_at_market(symbol, candle.close, ExitReason::EndOfDay)` (`simulated/broker.rs:425-467` já existe) **com** o slippage de execução a mercado. Não bloquear sinal na última barra (o live coloca a entrada às 15h45 e só cancela às 15h55 — paridade).
- O horário vem de um `[session]` no `config/default.toml` usado por `paper.rs` e pelo engine (paridade por config); flag `--no-flatten` para reproduzir os runs 413–421 e medir o delta.
- `paper.rs:1783-1787, 1827-1831`: gravar `EndOfDay` em vez de `Manual` + journal `session_flatten`.
- `metrics.rs`: contagem e PF por `exit_reason`.
- Testes: posição aberta às 15:45 fecha como `EndOfDay`; trade que abre e fecha no mesmo dia não muda; testes existentes do engine com candles sintéticos em horários arbitrários precisam de revisão.

**Teste de aceite**
```bash
trader-cli walkforward --symbol IJS --strategy balance-area-breakout-v1 --from 2025-02-24 --to 2026-09-03 -w 6
# idem VBR, AVUV; range-extreme-fade-v1 em AVUV/SLYV/IWV; opening-reversal-v1 em IWM/IWN
```
Comparar com os runs 413–421 (label `walkforward-oos-6w`, 05/09; 414 é duplicata de 413). Esperado: balance agregada PF ~1,5 e avgR ~0 (reprova por avgR; IJS passa sozinha), range-fade PF ~1,7 e avgR ~0,2 (passa), opening-reversal PF ~1,7 em IWM/IWN.

**Decisões que isto abre (do dono):** (a) o gate B da balance-area continua? — a estratégia ganha dinheiro em paper só se carregar overnight, o que o live não faz; (b) o gate A passa a ser lido com flatten e a revalidação de 04/09 fica formalmente substituída.

**Não fazer:** MOC em grupo OCA. A NYSE/Arca (todos os 7 ETFs) não permite cancelar MOC após 15h45; um stop que encher entre 15h49 e 16h00 deixaria a MOC abrir posição invertida — a família do incidente de 03/09. Manter MKT às 15h55 e modelar 2 bp no flatten do backtest.

### 5.2 Harness de validação (ADR-019)

> ✅ **ENTREGUE EM PARTE em 07/09/2026** (commit `e1f1266`). O núcleo entrou;
> **boa parte do que este §5.2 pede não entrou** — a lista completa, com a
> contagem, está na seção "Pendente" do ADR-019 e não é repetida aqui — entre eles `n_trials`, o Sharpe/
> Sortino sobre retornos **diários** (o cálculo segue anualizando por candle de
> 15 min, que é justamente o defeito que este parágrafo mandava corrigir),
> `entries_triggered`/`entries_cancelled_overshoot`, e o índice único e o
> dedupe, que foram **RECUSADOS com número** (ver a correção logo abaixo).
> A lista canônica está na seção "Pendente" do
> `docs/decisions/ADR-019-harness-de-validacao-e-gate-estatistico.md` — não
> mantenha uma cópia dela aqui.
>
> O texto abaixo descreve `latest_by_strategy` recebendo filtros novos. Não foi
> assim: entrou um método irmão, `latest_for(strategy_id, symbol, config_hash)`,
> que também **ignora runs marcados `experimental`**; e o `analyze`, sem run
> compatível, **avisa e sai** em vez de pegar outro. O método antigo continua
> existindo para quem quiser "o último run, qualquer que seja".

Sem isto, cada ablação das v2 exige um módulo novo e o gate B pode ser contaminado em silêncio.

**O que fazer**
- `commands/walkforward.rs`: `--output` (já existe no backtest), `--slippage-bps` (Decimal, não `Option<u32>`), `--label` obrigatório quando houver override, `--holdout-from <data>` (bloco final travado, nunca entra em seleção, roda uma vez por família), `--strategy-config <path>` (TOML alternativo para a mesma struct — `load_strategy` casa o id por string e lê `config/strategies/<id>.toml`, então hoje qualquer variante exige match arm; validar `toml.strategy.id == id` e falhar fechado).
- `--set chave=valor` aplicado em `[strategy.parameters]` antes de `load_strategy` (o `config_hash` muda sozinho). **Obrigatório:** falhar se a chave não existir (`#[serde(deny_unknown_fields)]` em todos os `StrategyParameters`, ou abortar se o hash não mudar) — hoje uma chave errada é ignorada em silêncio.
- Persistência: runs com override marcados `experimental = true` (ou `strategy_id` sufixado) e `backtest_run_repository::latest_by_strategy` passa a filtrar por `(strategy_id, asset, config_hash da estratégia carregada)` — hoje `analyze` pega o run mais recente da estratégia em qualquer símbolo (`analyze.rs:75-78`), então a primeira ablação vira baseline do gate B.
- `metrics.rs`: `profit_factor_r`, `sharpe_r`, `corr_risk_result`; mapas por `exit_reason`, direção e hora ET; **P&L por dia ET** (n de datas, WR por dia, share do top-1 e top-5 dias), por bloco e por ano; Sharpe/Sortino sobre retornos **diários** (hoje anualiza por candle 15m e dá −6 a −9 com PF > 1); custo total e avg R bruto por trade; `entries_triggered` / `entries_cancelled_overshoot`.
- `metrics` jsonb ganha `slippage_bps`, `overrides`, `session_flatten` (feito;
  mais `experimental`, `windows`, `holdout_from` e `holdout_metrics`) — mas
  **`n_trials` não entrou**. ⛔ **O índice único e o dedupe foram recusados na
  implementação, com número:** dentro do maior grupo "duplicado" (24 linhas) há
  **sete valores distintos de `final_equity`**. Não são cópias — o `config_hash`
  cobre só o TOML da estratégia, não a versão do motor, o slippage nem a régua
  de fim de sessão. Deduplicar por essa chave apagaria resultados diferentes
  entre si. A migração `0005` cria **apenas** o índice de busca
  `idx_backtest_runs_baseline`. O texto original abaixo fica como registro da
  proposta: índice único em `(strategy_id, asset_id, config_hash, period, label)`; script de dedupe (413/414 e, pela mesma chave, 109 grupos com 279 linhas duplicadas em 687 runs — medido na auditoria de 07/09).
- `simulated/broker.rs:851-893`: preencher `strategy_id`/`config_hash` e o `market_snapshot` no `journal` do trade (hoje `unknown` e `{}`) — via `Order.metadata` → `Position.metadata`.
- `print_acceptance`: PF_R ao lado de PF$, t-stat do avg R, share do melhor dia e dos 2 melhores meses, N da família.

**Critério:** a tabela de buckets de §2.2 e os números de §2.3 reproduzidos pelo harness (Σ|diff| ≈ 0 contra a re-simulação dos críticos).

### 5.3 Relatório estatístico (Python, `trader-research/`)

> ✅ **IMPLEMENTADO em 08/09/2026.** Pacote em `trader-research/` (uv, numpy,
> tzdata; 97 testes), relatório em
> `docs/reports/estatistica-gate-a-2026-09-08.md`. Três coisas que o texto
> abaixo não sabia:
>
> 1. **O PSR muda conforme a série.** O texto abaixo dá um número por
>    estratégia (balance 0,938; fade 0,984; openrev 0,82) sem dizer sobre qual
>    série. São dois números diferentes: no pool da balance, PSR(0) = **0,942**
>    sobre o P&L diário e **0,574** sobre o R por trade — a mesma inflação por
>    sizing que o PF em $ tem sobre o PF em R. O relatório imprime os dois.
> 2. **A direção do erro do DSR é desconhecida, não "otimista".** Sem
>    `n_trials` registrado, V[SR] entre tentativas é substituída pela do
>    estimador de uma série; para tentativas *correlacionadas* — que é o caso
>    registrado — o DSR impresso é **pessimista**, não otimista.
> 3. **O esquema do bootstrap precisa do calendário de pregões.** O ADR-019 §8
>    pré-registra "todos os pregões, zeros incluídos"; sem a lista de sessões
>    no JSON isso é inexequível, e a diferença chega a virar veredito. O
>    `walkforward` passou a exportar `oos_sessions` e `initial_capital`.
>
> Os intervalos citados abaixo (balance OOS iid [1,24; 3,47], blocos
> [0,87; 4,91]; fade [1,20; 3,90] / [1,39; 3,57]; openrev [0,63; 1,97]) são da
> pesquisa de 06–07/09 e foram **medidos de novo** com a régua do live e o
> esquema pré-registrado — ver o relatório. Os limites inferiores em blocos
> ficam em 0,768 (balance), 0,959 (fade) e 0,842 (openrev).


Pesquisa em Python (uv, numpy/pandas) consumindo `--output`; o que virar critério é portado para `crates/trader-backtest/src/stats.rs` (f64 permitido: a regra do AGENTS.md proíbe f64 para **dinheiro**; `metrics.rs` já usa f64 no annualizer).

- PSR (t-stat, skew, kurtosis) por estratégia e do pool; DSR reportado como **faixa** [N=2 … N registrado] — nunca pass/fail (balance PSR 0,938; fade 0,984; openrev 0,82; DSR < 0,5 para todas com N ≥ 4).
- IC95 do PF por bootstrap estacionário sobre P&L diário (blocos de 5 e 10 pregões, seed fixa, 10k reamostras), lado a lado com iid. O veredito muda com o esquema — por isso pré-registrado: balance OOS iid [1,24; 3,47] mas blocos **[0,87; 4,91]**; fade [1,20; 3,90] / [1,39; 3,57]; openrev [0,63; 1,97].
- MC de drawdown reamostrando (R, risk_amount) juntos, por modo de sizing (§5.5). ~~Com o sizing atual p95: balance 3,8%, fade 1,9%, openrev 5,3%~~ — **medido de novo em 08/09 com a régua do live**, o p95 do sizing atual é bem menor: balance **1,28%**, fade **0,75%**, openrev **1,89%** no pool de cada estratégia. A conclusão sobrevive e fica mais forte: o critério de 10% **não morde em nenhum modo** exceto o C (0,5%/4×), onde o balance chega a 11,7% de p95 e 10,8% de probabilidade de estourar. O modo C, aliás, exige alavancagem que o teto de 200% da conta bloqueia.
- Concentração: share dos 2 melhores meses, meses positivos, share do melhor dia.
- CPCV **não** se aplica a regras sem fit (os 5 caminhos são permutações dos mesmos blocos) — fica reservado a qualquer componente ajustado (§7, meta-labeling).

**Gate A proposto (ADR-019):** além dos limiares atuais, com flatten: limite inferior do IC95 em blocos do PF ≥ 1,0; PF_R ≥ 1,2; share dos 2 melhores meses ≤ 60%; pass no holdout travado.

### 5.4 Hotfix v1.0.1 da range-extreme-fade (veto de meio-dia em ET)

`is_midday_midrange` (`range_extreme_fade_v1/context.rs:220-238`) compara `last.timestamp.time()` em UTC com `midday_start/end_time` fixos (15:30–18:00 UTC). Converter para America/New_York (mesmo `chrono_tz` já importado no arquivo), TOML `midday_start_time = "11:30:00"` / `midday_end_time = "14:00:00"` com comentário "ET", teste unitário com timestamp de janeiro e de julho. Restaura a regra documentada (`range-extreme-fade-v1.md` §4/§8: 11:30–14:00 ET); precedente A2 (`e4231f3`). O `config_hash` muda: nota formal de correção (não bump de versão) e re-rodar o walk-forward junto com §5.1 para atualizar o baseline do gate B. Efeito em P&L: ~~imensurável (subamostra EST ≈ 20 trades)~~ — **errado, e a
correção foi medida em 07/09**: in-sample, com flatten, a fade cai de
PF 1,74 / avg R 0,218 / +4.560 para **PF 1,57 / 0,182 / +3.618** (−21% no net),
com a **mesma** contagem de trades — a correção troca *quais* sinais passam.
Tudo concentrado em AVUV (PF 1,55 → 1,24); no OOS, AVUV cai de avg R 0,256 para
**0,159**, passando por 0,009 acima do limiar. **O bug estava ajudando.** Isso
não é argumento para mantê-lo — uma janela que anda uma hora duas vezes por ano
é acidente, não regra —, mas registra que o edge da fade é mais fino do que o
gate A de 04/09 mostrava. Detalhe em `docs/strategies/range-extreme-fade-v1.md`
§17. Continua sendo correção, não melhoria.

### 5.5 Dimensionamento por liquidez e fração de capital (ADR-020)

> ✅ **IMPLEMENTADO em 08/09/2026** — os seis itens. Relatório com as
> medições: `docs/reports/sizing-adr020-2026-09-08.md`; runs em `out/adr020/`
> (`trader-research/modos-sizing.ps1`); liquidez em
> `sql/stats/14-liquidez-por-barra-adr020.sql`.
>
> **Regressão primeiro:** com os defaults, o motor reproduz **trade a trade**
> os oito runs do §5.6. O texto abaixo continua valendo; o que ele não sabia:
>
> 1. **O feed do Gateway não entrega o volume real desde 07/08** (achado 7
>    do §2.3, medido em todos os pares — achado 8 do banner). O cap de
>    liquidez entrou **desligado** por causa disso: ligado hoje, mediria a
>    barra do IWM em 12M em vez de 151M. Enquanto o §5.8 não fechar, o teto
>    utilizável é o `max_notional_usd` estático — SLYV ≈ 104k, IJS ≈ 169k
>    (medidos, batem com a estimativa do texto abaixo).
> 2. **A janela ficou em 600 barras (≈ 23 pregões), não em 60 pregões.** A
>    ADR pedia 60 pregões e, na mesma frase, exigia fonte e N **iguais** no
>    live e no backtest. 60 pregões no live custariam triplicar a busca de
>    candles por poll. Entre os dois requisitos, o que não se negocia é a
>    paridade.
> 3. **O cuidado (a) estava certo e agora tem número.** Sem fracionar a base
>    do DD junto com o sizing, o mesmo prejuízo leria **1,01%** em vez de
>    2,88% — o critério de 10% do gate A ficaria 2,9× mais leniente sem
>    ninguém tê-lo mudado. A base fracionada é gravada no JSON
>    (`initial_capital = 33333.33` no modo de fração 1/3).
> 4. **O cuidado (c) estava certo, e o buraco era maior do que "quase nunca
>    morde".** Duas posições a 99,9% somam 199,8% e a terceira entrava
>    inteira: a conta ia a ~300% de notional sem trava nenhuma disparar. Quem
>    segurava era a contagem de posições. A checagem agora soma a posição
>    prospectiva. **É a única mudança de comportamento do live nesta ADR.**
> 5. **A fração de capital não é neutra em R** — o mínimo de US$ 1,00 por
>    ordem morde mais no lote menor, e o avg R cai de 0,107 para 0,102
>    (−4,7%). O texto abaixo previu "centavos"; centavos são 4,7% do edge
>    quando o edge é 0,107.
> 6. **Os modos foram medidos** (item 6 do texto abaixo). Com risco uniforme
>    (modo B), o PF em $ cai de 1,55 para 1,24 — colado no PF em R, que fica
>    em 1,21 nos dois — e **três dos oito pares viram negativos** (os três com
>    PF_R < 1). O modo C dá 7,45% de DD, perto do limite de 10% do gate.
>
> **Fica de fora:** ligar o cap de liquidez e pôr `max_notional_usd` nas
> instâncias de SLYV e IJS (ambos dependem do §5.8 e são mudança de
> produção), e agregar as recusas por `notional_above_liquidity_cap`.


- `RiskConfig.max_notional_multiple` (default 1 = paridade) e `max_notional_usd: Option` por instância via `TRADER__RISK__MAX_NOTIONAL_USD` (o loader já aceita `TRADER__` com `__`); regra de elegibilidade: notional ≤ 1/3 da barra mediana de 15m dos últimos 60 pregões (calculável dos candles com paridade live/backtest): com os 60 pregões anteriores a 07/08/2026 (dado pré-Gateway, o que o próprio §5.6 manda usar): SLYV ≈ US$ 104k, IJS ≈ 169k, VBR ≈ 424k, IWN ≈ 713k, IWO ≈ 923k, AVUV ≈ 1,22M. Em SLYV/IJS isto **reduz** o tamanho atual — resultado desconfortável que precisa aparecer antes de qualquer escada de risco.
- `capital_fraction` (= 1/`max_concurrent_positions`) para que 3 posições caibam em 100% de notional (regra de dinheiro real do ADR-017) sem recusar o cluster. É política de risco: PF/avgR invariantes, P&L em $ cai ~3× por trade — declarar assim, sem prometer retorno. Três cuidados apontados pelos críticos: (a) o backtest calcula `max_drawdown_pct` sobre `initial_capital` fixo de 100k — com fração 1/3 o DD% do gate A fica 3× mais leniente sem ninguém mudar o critério; fracionar o capital inicial do backtest junto (e gravar `capital_fraction` em `metrics`); (b) decidir se `max_daily_loss_pct` por instância é sobre a conta ou sobre a fatia; (c) **a trava de notional do ADR-017 quase nunca morde hoje**: `exposure_limit_hit` (`paper.rs:1503-1513`) testa apenas a soma das posições **já abertas** ≥ teto, antes de somar a nova, e como o sizing é `trunc(equity/preço)` duas posições somam 199,9% < 200% e a terceira entra — o que morde é `positions.len() >= 3` (e, em agosto, a posição órfã). Incluir a posição prospectiva na checagem (`notional_existente + notional_novo >= teto`), com teste do caso 2 × 99,9%, e registrar a diferença como nota no ADR-017.
- Registrar a equity real e `BUYING_POWER` (lidos do broker, nunca usados) em `system_events`/`account_snapshots` a cada sessão — a equity de US$ 238k só existe num relatório.
- Modos de sizing para o harness (§5.2): A = atual; B = 0,15%/1× (risco uniforme sem alavancagem — 0,25%/1× é quase o status quo porque o cap prende para stop < 25 bp); B' = 0,25%/2×; C = 0,5%/4×. Reportar a fração de trades presos no cap por modo. C exige alavancagem intraday que o teto de 200% da conta bloqueia — só como estudo.

### 5.6 Higiene de dados e custo

> ✅ **IMPLEMENTADO em 08/09/2026**, menos dois itens (abaixo). Relatório em
> `docs/reports/custo-real-2026-09-08.md`. Quatro coisas que o texto abaixo
> não sabia:
>
> 1. **O efeito é maior do que "custo real" sugere, e vem em duas partes.**
>    Comissão: de US$ 149,80 para US$ 1.489,36 nos 214 trades (9,9×). Desconto
>    no fill do alvo: US$ 1.760 — **maior que a comissão inteira**. Juntos,
>    **nenhum recorte passa o gate**: nem por par, nem por estratégia, nem o
>    portfólio dos oito, que reprova no avg R (0,107 contra 0,15) depois de
>    passar em tudo com a régua anterior. A `fade` em AVUV e em IWV passam a
>    reprovar o PF; a `balance-area` agregada fica com **PF em R 0,94 e avg R
>    −0,035**; duas das três estratégias ficam **negativas em 2026**.
> 2. **"Spread cobrado no fill do alvo" descreve mal o mecanismo.** Uma ordem
>    limite parada no book é o lado **passivo** e não paga spread. O que o
>    desconto corrige é que **tocar não é encher**: o high do candle no seu
>    preço quase sempre significa que poucos lotes negociaram ali. O campo se
>    chama `limit_fill_haircut_pct` por isso, e é um valor único de 2 bp — a
>    calibração por ativo continua não feita.
> 3. **A causa dos runs sem label era o próprio comando `backtest`**, que
>    gravava `label = NULL` sempre (o `--label` só existia no `walkforward`).
>    Rotular os runs de 06–07/09 tratava o sintoma. A causa foi corrigida, e os
>    586 runs sem rótulo do banco dev foram marcados **pela data**, não por um
>    propósito que eu não poderia verificar run a run (`sql/maintenance/0006`).
> 4. **Achado fora do escopo:** o `paper` simulado usava 10 bp de slippage
>    contra os 2 bp do backtest, embaixo do comentário "compartilhado com o
>    backtest para garantir paridade de validação". Corrigido.
> 5. **A mudança criou, e fechou, um defeito próprio.** Com os dois custos no
>    banco, o `latest_for` do `analyze` passaria a escolher como baseline do
>    gate B o run mais recente — que era um `--legacy-cost` de paridade. O gate
>    compararia um paper que paga comissão real contra um backtest que paga
>    um décimo dela. `latest_for` passou a exigir os **três** eixos da régua —
>    comissão, slippage e desconto no alvo —, e run que não declara um deles
>    não casa com nada. O eixo do slippage já estava aberto desde antes: o
>    `--slippage-bps` nunca marcou o run como experimental, e o ADR-018 manda
>    rodar `--slippage-bps 4` em IJS e SLYV.
>
> **Fica de fora:** reingerir os 6 símbolos parados (depende do servidor) e o
> casamento do `CommissionReport` que chega em poll posterior — este último
> **parcialmente**: o adapter agora conta e loga as execuções que ficaram sem
> relatório, mas fechar o buraco exige adiar a emissão do fill, o que muda o
> caminho ao vivo e precisa do smoke test de 2 pregões que o §5.9 exige.

- Reingerir IJR, MDY, QQQ, SCHA, SPY, VB (parados em 06/08/2026) no servidor via workflow `ops` (1 símbolo por vez, pacing).
- ⛔ **NÃO deduplicar `backtest_runs`** — instrução revogada em 07/09/2026 pela
  própria implementação do ADR-019 (migração `0005`). Medido: no maior grupo
  "duplicado" há **sete valores distintos de `final_equity`**; não são cópias, e
  o dedupe apagaria resultados diferentes entre si. O que a linha original pedia
  fica como registro: deduplicar (413/414; pela chave do índice proposto na
  ADR-019 há **109 grupos e 279 linhas** duplicadas em 687 runs; 566 sem label).
  O que **vale** fazer é rotular os runs de 06–07/09 sem label (529–572 = backtests da pesquisa e pré-teste de futuros; 660–676 = 9 runs já rotulados `research-exit-policy-2026-09-07` (estudo de saídas) + 8 sem label do replay do roadmap: 660, 664, 666, 668, 671, 673, 674, 676).
- ✅ `ensure_asset` grava `tick_size` via `Decimal::from_f64_retain(0.01)` → `0.0100000000000000002…` em todos os 14 ativos: corrigir para `Decimal::new(1, 2)` e limpar o banco (viola "Decimal nunca f64"). **Feito**; os 14 ativos do banco dev limpos por `sql/maintenance/0005-corrigir-tick-size.sql`. O banco de produção é decisão do dono, como o 0004.
- ✅ Simulador: comissão por ação (US$ 0,005, mín. US$ 1,00, IBKR Canada) em vez de US$ 0,35 fixo; ~~`spread_bps` por ativo~~ **desconto único de 2 bp** cobrado no fill do alvo limit (o alvo não pagava nada — `simulated/broker.rs`), separado do slippage de mercado. **Feito, com o teste de paridade**: `--legacy-cost` reproduz os números de 08/09 dígito a dígito. O teto de 1% e o piso de US$ 1,00 da tabela IBKR estão implementados e **nunca mordem** nestes ativos.
- ⚠️ `ibkr/broker.rs`: casar o `CommissionReport` que chega em poll posterior ao fill (hoje só no mesmo lote). **Parcial**: o adapter conta e loga as execuções sem relatório, com os `exec_id`, para o buraco ser mensurável em produção. Fechá-lo exige adiar a emissão do fill — mudança no caminho ao vivo, com o mesmo smoke test do §5.9.
- ✅ Deixar `tick_size` no TOML da estratégia (regra v1) mas registrar divergência com `assets.tick_size` em log. **Feito**: o `paper` compara na largada e avisa.

### 5.7 Fase 0 do framework: screener SQL com fill honesto

Em um dia, um screener em SQL sobre os 136k candles reprovou 7 candidatos que as análises de livro ranqueavam no top-3 (squeeze-breakout, pivot-point-intraday, opening-range-breakout, gap-continuation/gap-fade, "dia após clímax", double-top solto, toque bruto de PDH) — cada um custaria 1–2 semanas de Rust, doc e walk-forward, o destino das 5 arquivadas. O ingrediente decisivo é o **fill honesto**: entrada em `max(open, gatilho)` (long) / `min(open, gatilho)` (short), stop avaliado primeiro na barra do fill, 4 bp. Exemplo: PDL-break short com fill exato no nível dava avg R +0,06/+0,13 (2025/2026); com fill honesto, −0,03/+0,03 — o "edge" era o gap atravessado no gatilho, o mesmo artefato que a ADR-015 corrigiu no simulador.

**Entregue nesta pesquisa:** `sql/screens/` com template, screens executados e README. **Antes de usar como Fase 0**, calibrar: controles positivos (range-fade em AVUV/SLYV; balance em IJS/VBR/AVUV **com flatten**, PF 1,55/avg R ≈ 0 — o screener sai no fechamento do dia, como o live), controle negativo (pullback-trend); flag `hold_overnight`; regra de cancelamento por gap (open além de 25% do stop → sem trade, como o simulador); agregação por **data** (n efetivo: 11 ETFs correlacionados são o mesmo dia); critério: avg R agregado com t ≥ 1,5, mesmo sinal em 2025 e 2026, melhor trimestre ≤ 50% do P&L. Registrar N de variantes em `docs/reports/screens-<data>.md`. Proposta de texto para o framework em `docs/strategy-analysis-framework.md` (Fase 0, marcada como proposta).

### 5.8 Feed de produção e latência

> ### 🔴 08/09/2026 — o custo do feed degradado está medido, e ele é o problema nº 1 do projeto
>
> Comparando, no banco de **produção**, o preço que a estratégia planejou
> (`signals.entry_price`) com o preço em que a ordem encheu
> (`trades.entry_price`), nos 7 trades reais:
>
> | trade | planejado | executado | desvio | **overshoot em R** | R final |
> |---|---:|---:|---:|---:|---:|
> | SPY (id 7) | 766,42 | 767,20 | +10,2 bp | **5,20** | −0,003 |
> | SPY (id 8) | 770,46 | 771,34 | +11,4 bp | **2,84** | −0,002 |
> | IWM (id 9) | 302,09 | 302,19 | +3,3 bp | 0,19 | −0,208 |
> | IWV (id 10) | 440,41 | 440,57 | +3,6 bp | **1,06** | +0,028 |
> | IWO (id 11) | 393,55 | 394,22 | +17,1 bp | **3,95** | −0,033 |
> | AVUV (id 12) | 125,78 | 125,40 | −30,2 bp | **1,09** | −0,609 |
> | SLYV (id 13) | 108,70 | 108,79 | +8,3 bp | **2,25** | −0,811 |
>
> **Seis dos sete trades entraram com overshoot maior que a distância inteira
> do stop planejado.** O trade nasce perdido: antes de o mercado se mexer, o
> risco planejado já foi consumido pela própria entrada.
>
> Em pontos-base o desvio parece pequeno (3 a 17 bp). O que o torna fatal é
> que **os stops do live são minúsculos**: 0,04 no SLYV a 108,70 é **3,7 bp**,
> contra os 12–30 bp que o backtest mede. É a cadeia causal do achado 7
> fechando: barra com 15–25% do range real → stop calculado sobre uma barra
> minúscula → stop 3–8× menor do que deveria → qualquer fill normal atravessa
> o risco inteiro.
>
> **Sintoma visível:** dois trades saem marcados como "alvo" **com prejuízo**
> (ids 11 e 13), porque a entrada encheu acima do próprio alvo e o trade fecha
> no alvo no instante seguinte, no vermelho. Nos 214 trades do backtest isso
> acontece **zero** vezes.
>
> **Por que a guarda do ADR-015 não pegou:** ela compara o preço de
> REFERÊNCIA no envio — que vem do mesmo feed degradado — contra o gatilho,
> com tolerância de 25% da distância do stop. Ela nunca olha o **fill**. Aqui
> os overshoots vão de 106% a 520%.
>
> **Três correções, em ordem de custo:**
> 1. **Guarda no fill** — ao encher a entrada, recalcular risco/retorno com o
>    preço real; se o trade já nasceu fora do plano, encerrar a mercado na
>    hora. Muda o caminho ao vivo: precisa do smoke test do §5.9.
> 2. **Piso de stop absoluto (§6.9)** — um sinal com stop de 3,7 bp não
>    deveria existir. É a guarda mais barata e não depende do servidor.
> 3. **O feed (esta seção)** — a causa raiz.
>
> **Isto reordena o projeto:** não adianta melhorar seleção de trade enquanto
> a entrada entrega preço fora do plano. As consultas que mediram isto estão
> no `host-check.yml`, só leitura.

Pré-requisito de qualquer comparação live × backtest e de qualquer instância nova: enquanto o Gateway entregar barras com 3–10% do volume e 15–25% do range, o gate B não mede o que o backtest mede (§2.3, achado 7).

> **Atualização de 08/09** (medição em `sql/stats/14-liquidez-por-barra-adr020.sql`): o dano é **muito mais disperso** do que "3–10%" — IWM guarda 3,2% do volume pré-Gateway, AVUV 7,1%, IWN 20%, IWV 32%, IJS 41%, VBR 51% e SLYV **89%**. E os pregões de agosto foram **reingeridos em 03/09 e voltaram com o mesmo volume baixo**: não é barra parcial do poll do live, é o que a fonte devolve. Consequência para o passo (1) desta lista: trocar a ingestão para o PC/TWS não basta para as barras futuras — **o histórico de 07/08 em diante precisa ser reingerido de outra fonte**, senão o banco fica com dois regimes de volume e qualquer estatística que use volume (o cap de liquidez do ADR-020 é a primeira) mistura os dois.

Ordem: (1) `debug-candles` no Gateway do servidor e no TWS do PC para a **mesma barra** — volume/dia e range/barra contra o histórico; tipo de market data (Realtime/Delayed/Frozen) que o Gateway/IBC entrega; (2) medir o lag de todas as barras em produção sem depender de ordens: `market_contexts.created_at − (timestamp + 15 min)` por símbolo; (3) gravar `signal_bar_close_ts` e `submit_latency_ms` em `orders.metadata` no envio (a query com `signals.timestamp` não serve: é `Utc::now()` no sinal); (4) só então alinhar o poll ao fechamento (20 s, nunca < 15 s entre requisições idênticas; o adapter abre um Client TCP por fetch) ou usar `subscribe_realtime_bars` como gatilho de fetch; (5) compartilhar dados em tempo real com a paper **só se** o TWS/PC não resolver sozinho (a paper herda as assinaturas do live; bundle US$ 10/mês, isenção exige US$ 30 de comissões que o projeto paper não gera). Cenário pessimista no backtest: `entry_starts_next_candle` (ordem só a partir do 2º candle) para bracketar o efeito de latência sem dado intrabar. Reforça: nada disso é "custo de 2 bp" — |close−open| mediano de uma barra de 15m é 6–11 bp conforme o ativo (8,5 bp no pool; 5,9 bp em IWV e 10,5 bp em IWM), ~1/3 do stop.

### 5.9 A9: confirmação de ordem e short (dono da correção)

Citado como bloqueador em cinco propostas e sem dono em nenhuma: em 28/08 o sell stop de VBR (ordem 17) ficou 30 min com o mercado além do gatilho e o bot a deu por "cancelada" sem fill nem erro — hipótese A9: rejeição da IBKR (aluguel de short) engolida no timeout de confirmação (`ibkr/broker.rs:711-716` assumia aceite; a correção de `confirm_order` de 03/09 trata fim de stream como transmitida, não como rejeitada). Antes de qualquer v2 com short (§6.2 tem shorts; §6.3 é 100% short): (1) rejeição e timeout de confirmação viram estados próprios com `system_event` e alerta; (2) antes de expirar uma entrada, conferir o status real no broker; (3) checagem de "shortable" no envio; (4) teste operacional fora de setup na paper (3 sell stops em VB/SLYV/IWN, conferindo aceite e fill com `ops exposicao`). A regra "não tocar o protocolo de envio de `paper.rs`" (§5.8) vale para o bloco de envio/confirmação — este item é exatamente esse bloco e por isso entra sozinho, com smoke test de 2 pregões numa instância.

### 5.10 Retirar a range-fade de IWV

PF 1,15 in-sample a 2 bp, avg R −0,018, stop mediano 13 bp (o mesmo modo de falha da pullback), 61% dos dias com range < 1%, 0,8% das barras com volume zero. Libera o client_id 11. Formalizar com o walk-forward com flatten; não substituir por par novo na mesma semana (§3.8).

---

## 6. Onda B — extrair mais do edge que existe (4–8 semanas)

Todas as v2 seguem o framework (Fases 1–7), têm doc próprio em `docs/strategies/`, e só entram em paper depois de: flatten no backtest, harness, holdout e pré-registro. Cada uma reinicia o gate B — por isso entram em símbolos onde a v1 **não** roda, mantendo a v1 como controle.

### 6.1 `range-extreme-fade-v2` — política de contexto `allow_neutral` (doc: `docs/strategies/range-extreme-fade-v2.md`)

**Hipótese (falsificável):** os sinais que só existem com `trend_state = Neutral` têm ≥ 30 trades OOS nos 3 pares e PF ≥ 1,3, com avg R ≥ 0 por direção e por ano. **Se falhar**, a v2 fica só com o hotfix de §5.4.

**Passo 1 — medir sem módulo novo:** `RiskConfig.allow_neutral_context: bool` (default false; `RiskConfig` é `Copy`) plumbado por `StrategyRiskParams` (9 `impl From` mecânicos em `risk_config.rs`) e por override do harness; em `risk/mod.rs:158-163` rejeitar `NoContext` só se `!allow && !is_tradeable`, mantendo vol High e fase ≠ Regular como vetos **explícitos e independentes** (hoje estão dentro do `is_tradeable`; liberar Neutral sem isso passaria a operar vol High). Dividir `NoContext` em três razões auditáveis (`NeutralContext` / `HighVolatility` / `OutsidePhase`) para o count aparecer em `signals.rejection_reason` no paper. Rodar v1 com e sem a flag em AVUV/SLYV/IWV/IWN e diffar trades por `entry_time`: n, PF, avg R, hora e direção do incremento.

**Passo 2 — só se passar:** módulo `range_extreme_fade_v2` (cópia da v1 + parâmetro + versão 2.0.0), política gravada no `market_snapshot`, TOML, dispatch, doc. Sem ADX, sem oscilador, sem veto de hora novo (cada um é ablação separada, contada no N; a regra de Brooks Cap. 5 é "meio do dia **e** terço central" — alargar é regra sem fonte).

**Ressalvas dos críticos:** Neutral inclui transições e chop multi-dia (EMA20 × SMA200 cruzadas) — o detector da v1 e o veto Barb Wire continuam ativos; o long da v1 (PF 3,34) é "comprar nova mínima com close > EMA20 > SMA200", população diferente da que Neutral adiciona; mais sinais pressionam `max_trades_per_day = 3` e as 3 posições da conta (AVUV já tem duas instâncias que se bloqueiam). Fonte: Brooks Cap. 9/10 (~80% dos dias são de range) + Dalton Cap. 3 + Murphy Cap. 15.

### 6.2 `balance-area-breakout-v2` — filtro de convicção (doc: `docs/strategies/balance-area-breakout-v2.md`)

**Evidência (re-simulada com flatten, 2 bp, pool de 8 símbolos):** PF por bucket de stop/ATR14: [0;0,7) 0,29–0,6; [0,7;1,0) **1,02**; [1,0;1,5) **1,01**; ≥ 1,5 **4,45** (n=55, 100% do P&L filtrado). Stop mediano da v2 = 41 bp (p10 24 bp) contra 23 bp da v1 — sai da zona de custo. Com filtro ≥ 1,0 + alvo 3R + flatten: 107–130 trades, PF 2,3–2,6; 58% das saídas são `EndOfDay`, 11% alvo — a v2 é, na prática, "rompimento forte segurado até o sino"; o alvo 3R é decorativo.

**Por que é B e não A:** (a) um dia (10/10/2025, 8 shorts simultâneos) = 64% do P&L do pool; top-5 dias = 77%; com o teto de 3 posições da conta esse dia rende no máximo 3/8; (b) pool 2025 PF 3,93 vs **2026 PF 0,60** (após 01/12/2025: 64 trades, PF 0,88); (c) o "holdout" de 5 símbolos coincide em 27 de 46 datas com os pares vivos (mesmos eventos em ETFs a 0,97 de correlação); (d) ~20 tentativas nesta família; (e) 45 OOS em 3 pares — o gate A por estratégia exige ≥ 5 pares.

**Regra pré-registrada:** o edge medido está em stop ≥ 1,5 × ATR (barra de rompimento fechando ≥ 1,2 ATR além da borda). Se o limiar for 1,0 para caber no gate, declarar que [1,0;1,5) tem PF esperado ≈ 1 e julgar a v2 também pelo bucket ≥ 1,5 isolado. Manter alvo 2R como baseline e 3R como sensibilidade. Validar: holdout travado (os últimos 4–6 meses **são negativos** e precisam ser explicados, não ignorados), replay com teto de 3 posições (§6.4), relatório ex-10/10/2025, `--slippage-bps 4` em IJS/SLYV, paper forward em IWN/SLYV (onde a v1 não roda). Implementação: `entry.rs::evaluate_prices` após `let risk = …`, `RejectionReason::BreakoutWithoutConviction`, campo `min_stop_atr_mult`, TOML, dispatch, `risk_config`, tests. Fonte: Dalton Cap. 4 ("break-out precisa de atividade iniciativa"; "much bigger move") + Grimes Cap. 8.

### 6.3 `opening-reversal-v2` — short-only na 1ª hora (doc: `docs/strategies/opening-reversal-v2.md`) — **movida para a Onda C**

A síntese final classificou esta hipótese como **regime**, não edge: com os dados existentes o critério de continuação que a própria proposta declarava ("PF ≥ 1,4 e avg R ≥ 0,2 em 2025 **e** 2026") já reprova (2026: n=45, PF 0,82). A spec fica pronta em `docs/strategies/opening-reversal-v2.md` para ser testada quando houver um segmento OOS com regime distinto (bear ou alta volatilidade) ou amostra de paper forward — sem implementação agora. O texto abaixo registra a evidência e o desenho pré-registrado.

**Evidência (re-simulada, 6 símbolos, short, sinal ≤ 10:15 ET, flatten):** 99–101 trades, WR 57,6%, PF 2,52, avg R 0,49; por símbolo IWM 2,43 / IWN 3,25 / VB 4,49 / SCHA 2,96 / IJR 1,66 / SLYV 1,75; long-only PF 0,93 (< 1 em 5 de 6); entradas na barra 10:45 PF 0,26–0,49. Blocos 2–7 (excluindo fev–mai/2025): 84 trades, PF 1,85.

**Por que é B com cautela:** 48% do net vem do bloco 1 (crash e recuperação das tarifas); 23/04 e 12/05/2025 = 57% do net; shorts 2026 PF 0,82 (n=45); o corte 10:15 apoia-se em 26 trades; 6 de 14 símbolos escolhidos após ver a tabela. Operacional: latência da 1ª hora com feed sem realtime (barras maiores do dia, gatilho atravessado nos primeiros minutos); short na IBKR (aluguel, SSR, o sell stop de VBR 28/08 "cancelado" sem fill — A9); SCHA cota US$ 32,6 (tick 3,07 bp — rodar a 4–5 bp ou excluir); VB/SCHA/IJR param em 06/08 no banco.

**Regra pré-registrada:** v2 = v1 + `direction_filter = short_only` (+ opcionalmente `trading_end_time = 10:15`, como hipótese secundária; `entry_validity_candles = 2` deixa a entrada viva na barra 10:45 — documentar ou usar 1). Sem Open-Test-Drive, sem segunda entrada, sem piso de ATR (filtros post-hoc sobre os mesmos trades; o "drive back" é redundante com a barra de sinal). Validar em 3 pares (IWM, IWN, VB), v1 mantida como controle em símbolo distinto, critério em dados que não existiam quando a regra foi escolhida (holdout + paper forward + segmento 2026), P&L com teto de 3 posições, teste operacional de short antes do gate B (3 sell stops fora de setup na paper; checagem de shortable no envio; A9 em produção). Fonte: Brooks Cap. 11 (opening reversals são fenômeno da 1ª hora) + Cap. 1 (barra de sinal).

### 6.4 Replay de portfólio com conta compartilhada

`crates/trader-backtest/src/portfolio.rs`: função pura `replay(trades, PortfolioLimits{capital, capital_fraction, max_positions, max_notional_pct, one_per_symbol (incluindo janela de ordem pendente = entry_validity_candles), flatten}) -> {kept, dropped(reason), daily_pnl, max_dd, by_strategy, corr}` + comando `trader-cli portfolio --runs out/*.json`. Já medido em protótipo: 232 trades/US$ 24.063 isolados → US$ 19.380 com flatten → US$ 12.286 sob 1 posição/100%; 2 posições a 200% derrubam 7 trades/US$ 3.320; máximo simultâneo em 18 meses = 3; pior dia 16/03/2026 −US$ 2.032 (dois shorts openrev IWM+IWN às 10:45, corr 0,97). Corrigir o protótipo: preço do flatten = open da barra 15:45 ou close da 15:30 (não o print das 16:00); correlação condicionada aos dias em que ambas operam (com zeros em 85% dos dias a correlação é ≈ 0 por construção); **reusar a função `exposure_limit_hit` movida para `trader-core`** (a régua do replay tem de ser a do live: contagem → notional existente → perda diária, com equity no instante do sinal) e reportar a sensibilidade entre "≥ sobre existentes" (live, quase nunca morde) e "incluindo a nova" (o que o ADR-017 pretendia); descartar os trades cuja entrada pendente do dia anterior enche na barra 09:30 (o simulador expira só por contagem de candles — `orders.rs:234-236` — e um sinal às 15:30 fica válido por 2 candles; no live a perna é TIF Day e o flatten cancela a pendente). Preencher `strategy_id`/`config_hash` em `Order.metadata` no `build_bracket_order` (~5 linhas) elimina a gambiarra `--strategy-of` e corrige o `analyze`. Reproduzir os 232 trades/US$ 24.063 do protótipo com o binário Rust antes de trocar a régua. Passa a ser a referência do gate B (trades esperados por par **após** travas) e o número que falta para o gate C (DD e Sharpe de portfólio).

### 6.5 Expansão de pares pré-registrada

- **Imediato, uma troca só:** sair `range-fade` de IWV (§5.10) e entrar `balance-area-breakout-v1` em IWN no client_id liberado (PF 2,23 com flatten, 41 trades, US$ 1,6–2,7M por barra; IWN já tem a openrev 09:30–10:30 — medir o bloqueio mútuo por símbolo antes).
- **Screening pooled de ETFs setoriais/fator value cíclico** (KRE, XRT, XHB, XLE, XOP, GDX, XBI, IYT) + 3 controles growth (XLK, SOXX, ARKK) + IBIT/ETHA + 4 ações individuais de setores distintos (JPM, XOM, PG, CAT) como braço de controle, num único relatório pré-registrado (`docs/reports/screening-etfs-2026-XX.md`): hipótese ao nível de **classe** e agregada por estratégia (walk-forward 6 janelas, ≥ 50 OOS, PF ≥ 1,3, avg R > 0,15, com flatten), bootstrap da diferença de avg R candidatos vs controles, custo 3 bp para preços < US$ 60 (tick 1,7–2,5 bp), N = +16 no contador. Balance-area v1 (largura absoluta ≤ 2%) quase não forma em ativos com range diário 2–3%: o braço balance só nos nomes cuja largura mediana de 78 barras cabe em 2% (medir por SQL antes). Zero código: `trader-cli ingest --symbol KRE --timeframe 15m --days 560 --provider ibkr` no servidor (1 por vez), backtest, walkforward. Sob a hipótese nula P(≥ 3 de 8 "passam") = 0,73 — por isso só o desenho pooled decide. Cabem +2 instâncias (client_ids 12–13; atualizar o teste de faixas em `ibkr/broker.rs:1087-1105` e o compose) sem consolidar processos.

### 6.6 Instrumentação sem decisão

Gravar, no `market_snapshot` do sinal (paper **e** backtest), sem vetar nada: `day_type` (IB de 4 barras, Rotation Factor, extensão — Dalton Cap. 2/3/4), `range5_pct` (média do range diário dos 5 pregões anteriores), `stop_bp`, `calendar_tags` (FOMC/CPI/opex/reconstituição Russell, de `config/calendar.toml`). Implementar como `strategies/common/day_type.rs` (funções puras, padrão de `daily_atr`), sem tocar `MarketContext`/persistência. Motivo: FOMC parecia PF 4,76 (n=16) e sob flatten vira 2,61 — 3/4 do "efeito" era overnight; dia-após-FOMC n=7 e dia-da-semana (15 buckets) são extremos esperados de múltiplas comparações. Vetos só depois de ≥ 20–30 trades vetáveis em paper.

### 6.7 Entrada STP LMT (paridade pós-envio)

O simulador cancela a entrada quando a barra abre além de 25% do stop (`simulated/broker.rs:245-283`): 21,9% das entradas acionadas em IJS, 24% em IWV, 10% VBR, 3% AVUV. O live tem a guarda **pré-envio** do ADR-015 (funcionou em 01/09), mas nada pós-envio: um STP puro enche a qualquer preço. Primeiro medir (§5.2 `entries_cancelled_overshoot`; fração de fills reais com (fill−gatilho)/risco > 0,25 após o ADR-015) — o salto mediano entre barras é 1,6–3,3 bp e a tolerância ≈ 5,5 bp é excedida em 30% das barras de IJS (bounce bid-ask). Se > 2% dos fills, implementar `EntryOrderType::StopLimit` com `limit_offset = max(k × dist_stop, 2 × spread)` (k = 0,5 para rompimentos, 0,25 para fades), modo `cancel` no simulador (paridade exata), expiração no live (`expire_stale_stop_entry` hoje ignora ≠ Stop), fill parcial como caso suportado, ADR de execução v1.1. Esforço M (~9 arquivos), não S.

### 6.8 Multi-estratégia por símbolo (infra) — esforço L, não M

`paper.rs` (2.555 linhas) é construído em torno de **um** `LoadedStrategy` e de estado singleton por processo (`TimeExitTracker`, um `RiskManager`/`ExecutionEngine` com a janela da estratégia, `LiveFillState{open_order}` único, `rebuild_risk_state` por símbolo sem filtro de estratégia, `recover_open_order` sem como atribuir a ordem a uma de N estratégias). Consolidar é um refactor de 1–2 semanas — extrair um `InstanceRuntime{strategy, engine, risk_state, time_exit, fills}` por estratégia — num arquivo que acabou de produzir o incidente de 03/09 (identidade de ordem é o ponto fraco do live). Esperado 10–18% de `PositionAlreadyOpen` entre estratégias no mesmo símbolo (IJR 15%, IWN 11%, SLYV 6% no replay, contando só posições; a janela de ordem pendente de 30 min aumenta isso e a openrev 09:30–10:30 colide com o início da fade às 09:45). Antes: promover o bloqueio por exposição de `info!` a `system_event` para medir colisões reais em paper; `analyze` por (símbolo, estratégia) é pré-requisito de qualquer par novo no mesmo símbolo. Libera client_ids (teto ≈ 10 pares → 30) e reduz pacing. Não é "mais pares = mais lucro": os 18 pares seriam o mesmo fator (corr 0,96–1,00).

### 6.9 Piso de stop transversal no RiskManager

A regra que teria vetado a pullback antes do gate e que protege qualquer candidata futura: rejeitar sinal se `entrada − stop < max(k × ATR14, piso_bp)` com `RejectionReason::StopBelowFloor`, `k` e `piso_bp` no `[risk]` (defaults a calibrar: 1 × ATR14 ou ~20 bp — a mediana da barra de 15m nos ETFs vivos é 11–23 bp). Fonte: Grimes Cap. 8 ("stop inicial nunca menor que um range médio de barra"); evidência do projeto: tercil de stop ≤ 17 bp WR 29%/PF 0,53 (§2.2); pullback negativa já a 1 bp (ADR-016). Implementação S (`risk/mod.rs`, `risk_config.rs`, TOML, teste), paridade automática. **Cuidado:** em `[risk]` o piso muda a amostra das v1 **sem** mudar o `config_hash` (o hash é SHA-256 só da config da própria estratégia — `range_extreme_fade_v1/config.rs:95-100`), então a mudança de regime fica invisível na trilha de auditoria; e o tercil que motiva a regra só vale na balance-area (medido: na range-fade o tercil mais estreito é o melhor, PF 2,50, e um piso de 20 bp apagaria 33 dos 69 trades, +5,7R). Medir — medir primeiro no harness quantos trades das aprovadas cairiam (esperado: os de PF < 1) e só então decidir se entra como regra global (ADR) ou como parâmetro das v2.

### 6.10 Janelas horárias por estratégia (v2 pré-registradas, validação só em paper)

Sob flatten, balance-area por hora de entrada: 10h +2.776 (PF 1,61), 11h +4.030 (2,06), 12h −749 (0,39), 13h −1.570 (0,24), 14h +2.308 (n=3), 15h −65 — as entradas 12h–13h só pagavam quando atravessavam a noite. Range-fade: 10h–11h = 100% do P&L; 12h–14h ≈ 0. **Ressalva estatística:** teste de permutação dos buckets dá p ≈ 0,21 — o padrão não se distingue de ruído; os buckets foram vistos antes da hipótese; o walk-forward não é OOS para isto. Por isso: v2 com **uma** mudança (`trading_end_time = 11:45:00` na balance — a janela é checada no timestamp de abertura da barra de sinal, inclusivo; `12:45:00` na fade), critério pré-registrado "PF e avg R dentro de ±30% do in-sample sob flatten após ≥ 20 trades de **paper**", 2 tentativas contadas no N, sem afrouxar se o OOS ficar em 45–49 trades. Exige match arm em `dispatch.rs` para o id v2 (ou mecanismo `id@variant` com checagem `toml.strategy.id == args.strategy`). Se §6.1 avançar, **não** criar duas v2 da fade: um único TOML v2 com janela + política de contexto, contado como tentativas separadas no harness.

### 6.11 Varredura de alvo em desenho pareado

As variantes de alvo compartilham as **mesmas entradas**: calcular por trade a diferença de R entre v1 e a variante e testar a média das diferenças contra 2 × SE pareado, agregado por estratégia — muito mais potente que comparar PFs de variantes independentes (avg R da fade tem SE ≈ 0,14 com n=69; uma diferença de 0,08R é indetectável sem pareamento). **Uma** variante estrutural por estratégia: fade → alvo `min(lado oposto do range, 2R)` (candidato já registrado no TOML da v1; re-simulado: PF 1,82 vs 1,74, marginal); balance → nada intraday até o rerun com flatten (o intraday da balance tem avg R ≈ −0,01: 3R não ajuda por construção). Pré-requisitos: flatten (§5.1), métricas `cost_total`, `cost_r_per_trade`, `avg_r_gross`, `realized_rr` no relatório (§5.2), comissão por ação (§5.6). RR planejado mediano hoje: balance 1,67–1,73; fade 1,04–1,27 (IWV 1,04). Sensibilidade ao custo pelo ADR-016: P&L −18% (balance) e −25% (fade) de 0 para 2 bp.

---

## 7. Onda C — condicionais

- **`opening-reversal-v2` short-only.** Ver §6.3: hipótese de regime; spec pronta; sem implementação até haver OOS com regime distinto ou paper forward.
- **Swing v2 da balance-area (GTC, hold ≤ 1 noite).** Depois de §5.1 e §6.2, como estudo de backtest com o modelo de gap que o simulador já tem: tabela P&L × dias de holding × hora de entrada × exit_reason; ≥ 50 trades overnight em ≥ 5 pares; sensibilidade a 4 bp. Aritmética contra: segurar todos os trades multiplica as noites expostas ~6–10×; P(|gap| > 1%) = 11–17% por noite; a −4,5R por gap adverso ≈ −30R/ano contra +13R/ano da v1 — só com stop alargado (o que muda R, RR e sizing). Se avançar: ADR-018-bis com pré-mercado cego, `outside_rth`, notional overnight ≤ 1×, long-only até A9, ADR-017 revisado (posição carregada conta nas 3 às 9h30).
- **Timeframe 1h derivado dos 15m.** `resample(candles, TimeFrame)` em `indicators` (O2) + `DataSource::Derived` (o `parse_source` rejeita fonte desconhecida); janela do live de `days(30)` → ≥ 70 (senão SMA200 diverge e `is_tradeable` fica mais permissivo no live); excluir o bucket 15:30 (parcial) e meios-pregões; reparametrizar por regra (pivôs exigem `structure_lookback ≥ 8`; inclinação da EMA ×4); teste de frequência antes (estimativa de 1/3 é palpite). Tese de custo válida (barra 1h mediana 0,31–0,43% → custo 11% do R) mas sem evidência de que o fade mantém a taxa de acerto.
- **Meta-labeling.** Só quando existirem ≥ 1.000 trades rotulados de estratégias com PF_R > 1 (hoje o pool de 14 ativos tem PF_R 0,99 e 0,84; SE(AUC) ≈ 0,06 com 240 trades; labels contaminados pelo overnight). O plumbing snapshot → journal entra em §5.2. Baseline manual (stop_bp + hora + direção) provavelmente esgota o ganho.
- **Escada de risco / Kelly.** Kelly ≈ 0 na balance ex-overnight; degraus só com gate B fechado com fills reais, PF_R ≥ 1,3, cap de liquidez elegível por ativo, descida automática. O degrau "0,25%/1×" é o status quo.
- **Provedor de níveis (PDH/PDL/PDC, pivôs, IB)** como funções puras — barato, mas seu consumidor (openrev-v2 com Open-Test-Drive) foi arquivado; fazer só se §6.6 mostrar separação.

---

## 8. Cripto, futuros, forex e outros ativos — resposta direta

**Cripto.** Três caminhos, três vereditos:
- *Spot na IBKR Canada.* As fontes primárias (página .ca 410, lista de permissões sem "Cryptocurrencies", disclosure Paxos restrito a EUA, decisão OSC de 04/08/2022) dizem que não há; um review terceiro (2026) diz que há via Zero Hash/Paxos, exceto Quebec, a 0,12–0,18%. **Ação do dono:** conferir em Client Portal → Settings → Trading Permissions. Irrelevante para o veredito: a API de cripto da TWS só aceita MKT-IOC e LMT (5 min), sem STP nem bracket — viola "sempre stop server-side"; e 12–18 bp/lado contra stops de 20–30 bp é cost/R > 100%.
- *Exchange registrada (Kraken; Coinbase).* Fee de Kraken Pro no tier básico: um crítico leu 0,40% maker / 0,80% taker na página de tarifas; o crítico de completude aponta que a tabela pública do Pro é 0,25% / 0,40% (o 0,40/0,80 pode ser a tarifa do app não-Pro) — **a confirmar**; Coinbase Advanced 0,40/0,60. Em qualquer das leituras são 25–80 bp/lado contra uma barra de 15m de BTC de 30–50 bp; stop de 1 barra paga 1,5–5× o próprio risco em fee. Sem sandbox (gate B impossível), 24/7 contra scheduler/flatten/circuit breaker, sizing inteiro rejeita BTC > US$ 100k. **Arquivado.** Reabre só como estratégia de hold multi-dia com motor GTC (outro projeto) ou via MBT (futuro CME), que depende da infra de futuros também arquivada.
- *ETFs de bitcoin/ether (IBIT, FBTC, ETHA).* Mesmo pipeline, permissão "Complex or Leveraged ETPs" (ação do dono, herdada pela paper). Mas: o subjacente anda 17,5h com o ETF fechado — o backtest sem flatten seria dominado por overnight que o live nunca captura; a balance-area v1 (largura absoluta 2%) é estruturalmente muda em ativo com range diário ~3%; range diário 2,5–3,5% dá stops de 40–50 bp (bom para custo) mas gaps que a guarda ADR-015 vai cancelar em massa. **Só como 2 tickers extras do screening pooled de §6.5, só range-fade, só após §5.1**, com kill pré-registrado: mediana do gap overnight > 3× a dos pares vivos → arquivar sem backtest.

**Micro futuros (M2K/MES; MBT/MET).** Pré-teste feito (runs 561–572, 2 bp → 1 bp): IWM balance 0,78 → 0,84, SPY balance 0,59 → 0,65, IWM fade 0,39 → 0,42, SPY fade 0,34 → 0,39, IWM openrev 1,18 → 1,26 (36 trades; OOS a 2 bp = 1,09, gate A reprovado). Cortar o custo pela metade vale +0,05–0,10 de PF; o resto é ausência de edge em índices amplos. Mais: M2K RTH ≠ IWM (níveis de ontem formados no Globex), margem intraday termina 15h45, alavancagem para risco 1% leva o DD de 3,4% a 10–17% (viola ADR-010), refactor XL (instrumento, sessão, rollover, margem, dados CME US$ 10/mês). **Arquivado com número.**

**Forex.** USD.CAD/EUR.USD em 15m: barra de 5–8 bp contra piso de 20 bp de stop; sem TRADES/volume na IBKR (`what_to_show` fixo em Trades); sessão 24/5 fora do scheduler; nenhuma fonte de livro do projeto; relief OSC com sunset em ago/2026 a confirmar. **Fechado sem teste** (o número é público).

**ETFs 3× (TNA/TZA).** O stop em bp triplica, mas o tick também: TNA a US$ 69,9 = 1,43 bp vs IWM 0,42 (3,4×) — custo/R igual ou pior. O "risco real 0,6%" é alavancagem, obtida em IWM com `max_notional_multiple` (§5.5) sem tick pior nem rebalanceamento diário. A estratégia base (openrev IWM) reprova o gate A. **Arquivado.**

**Ações individuais.** Edge de cesta, não de ação; gaps de earnings/guidance; balance v1 muda. Só 4 nomes como braço de controle do screening (§6.5), BRK.B fora (símbolo com espaço na IBKR).

**ETFs macro (TLT, GLD, XLE, EEM, USO).** Com 15–31 OOS por combo, P(PF ≥ 1,3 | sem edge) = 0,28 e P(≥ 2 de 5 passarem) = 0,44 — cara ou coroa. Só dentro do screening pooled com critério de rejeição; nunca aprovação isolada. Se entrarem: só balance e fade (a "máxima de ontem" da opening-reversal não faz sentido em ETF cujo subjacente negocia 23h), ingest pelo PC/TWS, conferir se USO exige a permissão CLP (commodity pool) e o gap ex-dividendo mensal de TLT (~0,3%, que a balance de 3 dias lê como rompimento).

**Refactor "Instrument spec" (sec_type/exchange/currency/multiplier/expiry no domínio).** Sem cliente: as três classes que o exigiriam (futuros, cripto, forex) estão arquivadas. `Asset` já existe no domínio (`entities.rs:200-211`) — se um dia voltar, estender `Asset`, não criar entidade paralela. O que sobra e vale a pena é a **fatia 1** (custo por ativo: `spread_bps`/comissão por ação no simulador, §5.6).

**Outros arquivados desta pesquisa** (números em `docs/reports/pesquisa-lucratividade-2026-09-07.md`): gate de regime por compressão (bucket de compressão da balance tem PF 1,67 e +US$ 2.947 — o gate perde dinheiro); teto por cluster (dias de cluster são os melhores); entrada limit passiva (todos os 27 perdedores da fade teriam sido tocados, ≤ 31/42 vencedores); 5m para a openrev (60% dos gatilhos da 1ª hora atravessados em 5 min; ingest sem `--from/--to`); MOC/OCA; veto integral 12h–15h; Open-Test-Drive + segunda entrada + piso de ATR na openrev.

---

## 9. Sequenciamento e esforço

| Semana | Entregas | Depende de |
|---|---|---|
| 1 | ✅ §5.1 flatten + ADR-018; §5.8 diagnóstico do feed (Gateway vs TWS, mesma barra); ✅ §5.4 hotfix ET; §5.6 ~~dedupe~~ (**⛔ revogado**, ver §5.6)/rótulos/tick_size; §5.7 `sql/screens` já está no repo desde a pesquisa, mas a **calibração** que o próprio §5.7 exige antes de usá-lo como Fase 0 não foi feita | — |
| 1–2 | ✅ §5.2 harness (walkforward `--output/--slippage/--label/--holdout`, PF_R, métricas por dia, `strategy_id` nos trades, `analyze` por par) — **parcial**, ver ADR-019 "Pendente"; §5.6 comissão por ação + spread no alvo | §5.1 |
| 2 | ✅ Re-rodar gate A das 3 estratégias com flatten + hotfix (`docs/reports/gate-a-com-flatten-2026-09-07.md`); ✅ §5.3 relatório estatístico (`docs/reports/estatistica-gate-a-2026-09-08.md`); ⏳ decisão do dono sobre o gate B da balance-area | §5.1, §5.2 |
| 2–3 | §5.5 cap de liquidez + `capital_fraction` + registro de equity (ADR-020); §5.9 A9 (confirmação de ordem e short) com smoke test; §5.10 sair de IWV; §5.8 correção do feed (poll alinhado / realtime como gatilho) conforme o diagnóstico | §5.8 |
| 3–4 | §6.1 passo 1 (medir o delta Neutral) — **a medição de maior valor esperado do plano**; §6.4 replay de portfólio | §5.1, §5.2 |
| 4–6 | §6.1 passo 2 (v2) se passar; §6.2 como v2 pré-registrada em símbolos onde a v1 não roda; §6.9 piso de stop medido no harness; §6.10 janelas e §6.11 alvo pareado como variantes pré-registradas; §6.6 instrumentação | §6.1, §6.4 |
| 5–8 | §6.5 reingestão + screening pooled (setoriais, IBIT/ETHA, 4 ações); IWN na balance-area | §5.6, §5.1 |
| 8+ | §6.7/§6.8 conforme medições; Onda C conforme resultados | B |

Esforço total da Onda A: ~2–3 semanas de calendário com dedicação integral — o roadmap de decisão (`docs/reports/roadmap-decisao-2026-09-07.md` §5) estima **19–29 dias-pessoa** com revisão, testes, deploy e um incidente; Onda B: 27–42 dias-pessoa, mais gates de 4 semanas de paper por v2 (podem correr em paralelo em símbolos distintos, respeitando 3 posições e ~10 instâncias). A 20 h/semana, A + B levam 5–7 meses; os marcos datados (M0 11/09 → M1 25/09 → M2 23/10 → M3 20/11 → gate B fev–mar/2027) e os critérios de encerramento (18/12/2026; 31/03/2027) estão no roadmap de decisão.

---

## 10. O que só o dono decide

> ### Decisões tomadas em 08/09/2026 (delegadas pelo dono)
>
> O dono delegou os itens 1, 2, 4 e a leitura do gate B ("tome as decisões que
> você achar mais correta"). Ficam registradas aqui, com o motivo, para
> poderem ser revertidas por quem discordar.
>
> **1. O gate B é lido POR ESTRATÉGIA, não por portfólio.** O §5.3 mostrou
> que a unidade decide o veredito: os mesmos 214 trades reprovam nos oito
> pares, reprovam nas três estratégias e passam tudo como portfólio. Escolher
> a unidade permissiva **depois** de ver qual passa é escolher a resposta. O
> número do portfólio continua sendo reportado ao lado, como informação.
> Custo aceito: 4 a 5,5 meses de amostra em vez de 7 semanas.
>
> **2. O gate A de 04/09 é formalmente substituído** pelo de 07–08/09
> (`gate-a-com-flatten-2026-09-07.md` + custo real). O de 04/09 foi medido
> com um motor que segurava posição pela noite — coisa que o live nunca faz.
> Manter os dois seria manter um número oficial que descreve um mundo que não
> existe.
>
> **3. As 3 instâncias da balance-area CONTINUAM em paper, como controle** —
> não como candidatas. Em R elas perdem (avg R −0,035; −2,5 R/ano). Ficam
> porque desligar apaga a única medição contínua de live × backtest da
> estratégia. **Com data para revisar:** elas ocupam vagas do teto de 3
> posições simultâneas da conta e podem bloquear entradas da
> `opening-reversal`, que é a única com edge medido (avg R 0,332; +14,7
> R/ano). Quanto isso custa só o replay de portfólio (§6.4) responde — e
> enquanto ele não existir, a decisão de manter é provisória.
>
> **4. A `range-extreme-fade` sai de IWV — no MESMO deploy do hotfix.** O
> hotfix do veto de meio-dia já muda o `config_hash` da fade e, por §3.8,
> já reinicia o relógio de 4 semanas dela. Tirar IWV junto custa zero a mais;
> tirar depois custaria outro reinício.
>
> **5. O modo de dimensionamento continua o A (produção).** Trocar o sizing
> agora mudaria a régua no meio de um gate B que já vai reiniciar, e a
> escolha certa depende de dois números que só o dono tem (capital real e
> tolerância a drawdown). Registrado que o A é o menos honesto dos modos: o
> lucro em $ dele vem da covariância entre tamanho e resultado (§5.5).
>
> **6. Short com dinheiro real: não se decide agora.** Nenhuma estratégia
> chegou ao gate C, então a pergunta não está no caminho crítico. O que
> **está** é o teste de 3 sell stops na paper (§5.9) — depende do servidor.


1. **Gate B da balance-area-v1** continua como está, sabendo que o backtest que a aprovou não tem flatten? (Recomendação: continuar em paper, mas ler o gate B contra o backtest **com** flatten e não contar overnight como edge.)
2. **Gate A passa a ser lido com flatten, PF_R, IC em blocos, holdout e concentração** (ADR-019) — substitui formalmente a revalidação de 04/09.
3. **Client Portal:** conferir permissão de cripto (para corrigir a premissa nos docs), pedir "Complex or Leveraged ETPs" (se quiser IBIT/ETHA no screening), verificar assinaturas de market data do usuário live e a moeda-base da conta paper (`trader-cli account --provider ibkr`) — se for CAD, o cap de notional já está ~1,37× errado.
4. **Trocas de par:** aceitar que cada troca reinicia 4 semanas de gate B; sequência recomendada: IWV → IWN agora; v2 em símbolos novos; nada mais até a amostra andar.
5. **Cripto/futuros/forex:** aceitar os arquivamentos com número (§8) e as condições de reabertura, para a pergunta não voltar a cada mês.
6. **Fase 0 no framework** (§5.7) e a regra transversal "stop mínimo ≥ 20 bp / ≥ 1 range médio de barra" como critério de triagem de qualquer candidata.
7. **As 11 perguntas do roadmap de decisão** (`docs/reports/roadmap-decisao-2026-09-07.md` §6): capital real pretendido, tolerância a drawdown, horas por semana, prazo máximo em paper, retorno anual que justifica continuar, moeda-base, aceitação de short com dinheiro real, permissões no Client Portal, leitura do gate B por estratégia ou por portfólio, destino das 3 instâncias da balance-area, backup off-site. Sem as respostas 1, 3 e 5 o calendário do §9 não é verificável.

---

## 11. Limites honestos

- Todo o histórico (24/02/2025 → 02/09/2026) é um regime só, com um episódio de volatilidade extrema (abr/2025) e um de compressão (ago/2026). O P&L está concentrado em jun–out/2025. Nenhum walk-forward do repo é OOS em relação ao desenho das regras; o único OOS verdadeiro é o paper forward, que tem 0 trades das aprovadas desde 18/08.
- Os números deste plano são re-simulações em Python sobre JSONs do binário de 06/09 e candles do banco dev; reproduzem o motor ao centavo no baseline, mas o flatten "no close da barra 15:45" difere do MKT às 15:55 do live em 10 min e no tipo de fill. O motor com §5.1 é a fonte de verdade.
- **O motor já rodou (07/09) e a fonte de verdade agora existe.** Ele confirmou
  a re-simulação: mesma contagem de trades (96/69/67) e exatamente as mesmas
  saídas overnight (20/9/8, IWV em 0). PF e avg R batem em 2–3 casas; o net em
  dólares tem resíduo de **0,2% a 2,3% em módulo** — e não é sempre para baixo:
  a balance-area ficou −2,3%, a fade −0,4% e a opening-reversal **+0,2%**. A
  A causa **não foi isolada**, e as explicações fáceis não servem: a comissão do
  simulador é fixa por trade (US$ 0,35/perna) e a contagem de trades é a mesma
  dos dois lados, então não há comissão a mais; e slippage e comissão são ambos
  custo, logo nenhum dos dois produziria o **+0,2%** da opening-reversal. A
  hipótese mais provável — **interpretação nossa, não verificada** — é o
  caminho da equity: fechar no sino muda a curva, e o sizing depende dela, de
  modo que as posições seguintes saem com tamanho diferente mesmo com o mesmo
  número de trades. Onde os dois divergirem, vale o motor.
- **⚠️ Todo número da `range-extreme-fade-v1` neste plano é anterior ao hotfix
  ET do §5.4.** Medido **na régua com flatten**, o agregado dela cai de
  PF 1,74 / avg R 0,218 / +4.560 para **PF 1,57 / 0,182 / +3.618** — ou seja,
  −9,8% no PF, −16,5% no avg R e −21% no net. **O −21% é do net agregado, não
  um desconto que se aplique a qualquer número.** E o efeito é 100% AVUV:
  SLYV (2,75) e IWV (1,15) ficam idênticos — esses três são **in-sample** e
  estão em `docs/strategies/range-extreme-fade-v1.md` §17; os equivalentes OOS
  (2,64 e 1,31) estão no §3b do relatório. Além disso, os números da fade em
  §2.1 e §2.3 são **sem flatten**, régua na qual o delta do hotfix nunca foi
  medido — não existe fator de correção para eles. Os valores válidos, com as
  duas correções aplicadas, estão em
  `docs/reports/gate-a-com-flatten-2026-09-07.md` §3b.
- **O PF em R, previsto no achado 4 do §2.3, é pior do que o plano estimava.**
  Medido: balance-area com **PF_R 0,74 em AVUV e 0,97 em VBR** — abaixo de 1,
  ou seja, sem edge em unidades de risco. Nenhum t-stat do conjunto chega a 2, e
  só a fade em SLYV passa no critério de concentração de 60%.
- O banco dev tem 3 trades e 26 fills de live (04/08, pullback); tudo sobre fills, comissão real e latência em produção precisa do dump do servidor ou de uma ação `sql` read-only no workflow `ops`.
- 6 dos 14 símbolos param em 06/08/2026 no banco dev (§5.6).
- A conta paper enche a NBBO sem fila nem impacto: qualquer custo medido nela é piso, e em SLYV/IJS o piso está longe do teto.
- Dois dos 18 críticos (lente portfólio/regime: código e operação) rodaram numa retomada em 07/09; seus vereditos estão incorporados (§2.3 achado 7, §5.5, §6.4, §6.8) e no relatório de pesquisa.
- Convenção de janela: `trading_end_time` é checado sobre o timestamp de **abertura** da barra de sinal, inclusivo (`session.rs:39-48`); "entradas até 12:00" exige `trading_end_time = 11:45:00`. Qualquer tabela por hora deve usar a mesma convenção.

---

## 12. Documentos produzidos por esta pesquisa

| Documento | Conteúdo |
|---|---|
| `docs/cto-plano-lucratividade-2026-09.md` | este plano |
| `docs/reports/pesquisa-lucratividade-2026-09-07.md` | as 41 propostas, os vereditos dos críticos e os números que os sustentam; arquivo das linhas fechadas |
| `docs/reports/roadmap-decisao-2026-09-07.md` | baseline de $ na conta real (cenários A–D com IC95), teto e valor esperado por item da Onda B, árvore de decisão com marcos datados, critérios de encerramento, orçamento em dias-pessoa e as 11 perguntas que só o dono responde |
| `docs/strategies/range-extreme-fade-v2.md` | Fases 1–3 do framework: política de contexto `allow_neutral` (+ hotfix v1.0.1) |
| `docs/strategies/balance-area-breakout-v2.md` | Fases 1–3: filtro de convicção do rompimento |
| `docs/strategies/opening-reversal-v2.md` | Fases 1–3: short-only na 1ª hora (Onda C — hipótese de regime; spec pronta, sem implementação) |
| `docs/decisions/ADR-018-paridade-fim-de-sessao-backtest.md` **✅ IMPLEMENTADO** | flatten no engine, `ExitReason::EndOfDay`, releitura do gate A |
| `docs/decisions/ADR-019-harness-de-validacao-e-gate-estatistico.md` **✅ IMPLEMENTADO, com exceções** | walkforward `--output/--slippage/--label/--holdout`, PF_R, concentração, `analyze` por par. **Vários itens não entraram** — lista e contagem na seção "Pendente" do próprio ADR; não replicar aqui |
| `docs/decisions/ADR-020-dimensionamento-por-liquidez-e-fracao-de-capital.md` **✅ IMPLEMENTADO (08/09)** | cap por liquidez (implementado e **desligado**), `max_notional_multiple`, `max_notional_usd`, `capital_fraction`, registro de equity e moeda em `account_snapshots`, trava de notional com a posição prospectiva, modos de sizing no harness. O banner do topo do ADR lista os três desvios do que estava especificado |
| `sql/screens/` | template de screener com fill honesto, screens executados, README com regra de calibração |
| `sql/stats/` | 14 consultas de estatística descritiva do banco (liquidez por barra e por hora, range diário e por barra, gaps, perfil intradiário) — base de §2.2, §2.4 e da ADR-020 §3. A `14-liquidez-por-barra-adr020.sql` (08/09) calibra o cap de liquidez e mede o feed esparso nas duas janelas |
| `docs/reports/estudo-politicas-de-saida-2026-09-07.md` | estudo pareado pré-registrado de políticas de saída (breakeven, trailing, stop além da barra, parcial): nenhuma supera alvo fixo + flatten com t > 2 em 2025 **e** 2026. Usa a comissão real da IBKR, por isso a política A dá avg R −0,037 / PF_R 0,94 / US$ 6.062 onde a ADR-018 publica −0,007 / +6.730 (a diferença é só o modelo de comissão) |
| `docs/strategy-analysis-framework.md` | seção "Fase 0 — Screener" marcada como proposta |
| `docs/HANDOFF.md` | entrada de 07/09 apontando para este plano, e as entradas da execução |
| `docs/reports/gate-a-com-flatten-2026-09-07.md` **(novo, 07/09)** | o gate A re-rodado pelo MOTOR com flatten, hotfix ET e as métricas do ADR-019. **Os números dele substituem os de `gate-a-revalidacao-2026-09-04.md`** — nenhum run sem flatten é comparável (precedente ADR-015 §4). A substituição **formal** do gate A continua sendo a decisão 2 do dono (§10). Ressalva do próprio relatório (§6): o período inclui o feed degradado do Gateway a partir de 07/08/2026, então o **nível** do último bloco está contaminado; o delta com/sem flatten não está |
| `trader-research/` **(novo, 08/09 — §5.3 implementado)** | pacote Python (uv, numpy, tzdata; 97 testes) que consome o `--output` do walkforward: PSR, DSR como faixa, IC95 do PF por bootstrap estacionário sobre o calendário completo de pregões, MC de drawdown por modo de sizing, concentração e P&L por ano. Falha fechado em seis situações — réguas diferentes, run experimental, divergência contra o `metrics.rs`, run sem calendário, arquivo repetido e `--por portfolio` sem `--capital` |
| `docs/reports/estatistica-gate-a-2026-09-08.md` **(novo, 08/09)** | o critério "IC95 em blocos ≥ 1,0" medido pela primeira vez. Achado: **a unidade em que o gate é lido decide o veredito** — reprova nos oito pares, reprova nas três estratégias, passa no portfólio dos oito. Registra também que o esquema errado do bootstrap chegou a **virar um veredito** (balance·IJS), e a rodada adversarial de 63 achados que corrigiu o pacote antes da publicação |
| `docs/reports/custo-real-2026-09-08.md` **(novo, 08/09 — §5.6 implementado)** | o custo do simulador trocado pelo real. Achados: a comissão antiga era **1/9,9** da real, e o **desconto no fill do alvo é maior que toda a comissão nova** (US$ 1.760 contra 1.489), respondendo por 56% do efeito; com o custo certo **nenhum recorte passa o gate** — o portfólio dos oito, que passava tudo, reprova no avg R. Traz o teste de paridade (`--legacy-cost` reproduz os números antigos dígito a dígito), a decomposição em três cenários e a §0 registrando os números que a primeira versão do próprio relatório errou |
| `docs/reports/sizing-adr020-2026-09-08.md` **(novo, 08/09 — §5.5 implementado)** | o tamanho da posição deixa de ser acidente. Achados: com risco uniforme o PF em $ cai de 1,55 para 1,24 (colado no PF em R, 1,21) e **três dos oito pares viram negativos**; o cap de 1/3 da barra corta **37% dos trades** no notional de produção (SLYV −54%, IJS −35%); a fração de capital **não é neutra em R** (avg R 0,107 → 0,102, por causa do mínimo de US$ 1,00 da comissão); e sem fracionar a base do DD junto, o critério do gate A ficaria 2,9× mais leniente. Traz a regressão trade a trade contra os runs do §5.6 |
| `trader-research/modos-sizing.ps1` **(novo, 08/09)** | reproduz os 64 runs dos modos de dimensionamento em `out/adr020/` — inclusive o modo A, que serve de regressão contra os `wf56_*` do §5.6 |
| `sql/maintenance/0005-corrigir-tick-size.sql` **(novo, 08/09 — ✅ RODADO na produção em 08/09)** | corrige o `tick_size` gravado a partir de f64. 14 ativos no dev, **24 na produção**; nenhum sobrou errado. Efeito prático hoje é nulo (o motor lê o TOML), e o script diz isso |
| `sql/maintenance/0006-rotular-runs-sem-label.sql` **(novo, 08/09 — ✅ RODADO na produção em 08/09)** | rotula os runs sem `label` **pela data**, não por propósito inferido. 586 no dev, **126 na produção** (2 de 03/08, 3 de 04/08, 23 de 05/08, 97 de 06/08, 1 de 18/08). A causa (o `backtest` gravava NULL sempre) foi corrigida no código |
| `sql/maintenance/0004-reclassificar-flatten.sql` **(novo, 07/09 — ✅ conferido na produção em 08/09: NÃO SE APLICA)** | UPDATE opcional dos flattens gravados como `manual` antes do ADR-018. O script roda em modo de conferência (termina em `ROLLBACK`) e a produção devolveu **zero linhas**: nenhum trade tem `exit_reason = 'manual'` junto de `forced_exit = 'session_flatten'`. Não há o que reclassificar, e a decisão do dono deixa de ser necessária |
