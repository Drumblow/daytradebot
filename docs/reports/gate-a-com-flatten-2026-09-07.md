# Gate A re-rodado com a régua do live (flatten de fim de pregão) — 07/09/2026

**O que é:** a re-rodada que o ADR-018 exige, feita pelo **motor** (não por
re-simulação em Python). **Os números aqui substituem os de**
`docs/reports/gate-a-revalidacao-2026-09-04.md` — nenhum run anterior ao flatten
é comparável com estes, mesmo precedente do ADR-015 §4.

Substituir **formalmente** o gate A de 04/09 é ato do dono (decisão 2 do §10 do
plano) e ainda não foi feito. Esta página produz a evidência; §5 registra o que
ela não decide.

**Comando:** `trader-cli walkforward --symbol <S> --strategy <E> --from 2025-02-24 --to 2026-09-03 -w 6`
(flatten ligado por padrão) e `trader-cli backtest ... --slippage-bps 2` com e
sem `--no-flatten` para o delta. Binário: `target/release/trader-cli` do
working tree com o ADR-018 aplicado. Banco dev `trader_db` (5434).
Runs OOS persistidos: **725–732**.

---

## 1. O `--no-flatten` reproduz a régua antiga ao centavo

Primeiro critério de aceite: se a flag não reproduzir o baseline, o resto não
vale nada. In-sample, 2 bp, soma dos pares vivos:

| Estratégia | Baseline publicado (§2.3 do plano) | Motor com `--no-flatten` |
|---|---|---|
| balance-area-breakout-v1 (96 t) | PF 1,92 · avgR 0,214 · **+14.648** | PF 1,92 · avgR 0,214 · **+14.648** |
| range-extreme-fade-v1 (69 t) | PF 1,88 · avgR 0,297 · **+6.015** | PF 1,88 · avgR 0,297 · **+6.015** |
| opening-reversal-v1 (67 t) | PF 1,23 · **+3.400** | PF 1,23 · **+3.400** |

Σ|diff| = 0. A flag é fiel.

## 2. Com flatten: o motor confirma a re-simulação dos críticos

| Estratégia (in-sample, 2 bp) | ADR-018 previa | **Motor mediu** | Δ do net |
|---|---|---|---|
| balance-area-breakout-v1 (96 t) | PF 1,55 · avgR −0,007 · +6.730 | **PF 1,55 · avgR −0,007 · +6.576** | −2,3% |
| — IJS / VBR / AVUV (PF / avgR) | 1,86 / 0,269 · 1,47 / −0,035 · 1,43 / −0,174 | **1,85 / 0,269 · 1,47 / −0,036 · 1,43 / −0,174** | |
| range-extreme-fade-v1 (69 t) | PF 1,74 · avgR 0,218 · +4.578 | **PF 1,74 · avgR 0,218 · +4.560** | −0,4% |
| opening-reversal-v1 (67 t) | PF 1,74 · +8.072 | **PF 1,74 · +8.085** | +0,2% |

**Contagem de trades idêntica** (96 / 69 / 67) e, mais importante, a contagem
de saídas `end_of_day` bate exatamente com a contagem de trades overnight da
re-simulação: **20 / 9 / 8**, com IWV em **0** — o motor identificou os mesmos
trades, um a um.

O resíduo de 0,2–2,3% no net em dólares **não é sempre para baixo**: a
balance-area ficou −2,3%, a fade −0,4% e a opening-reversal **+0,2%**. A causa
não foi isolada, e as explicações fáceis não servem — a comissão do simulador é
fixa por trade e a contagem de trades é idêntica dos dois lados, então não há
comissão a mais; e slippage e comissão são ambos custo, logo nenhum produziria
um resíduo positivo. A hipótese mais provável, **interpretação nossa e não
verificada**, é o caminho da equity: fechar no sino muda a curva, e o sizing
depende dela, então as posições seguintes saem com tamanho diferente mesmo com
o mesmo número de trades.

PF e avg R, que são o que o gate lê, coincidem em 2–3 casas. Como o ADR-018 já
dizia, **o motor é a fonte de verdade**; os números desta tabela substituem os
da re-simulação.

## 3. Veredito do gate A (OOS do walk-forward, 6 janelas, com flatten)

Critérios do ADR-010: ≥ 50 trades · WR ≥ 40% · PF ≥ 1,3 · DD ≤ 10% ·
avg R > 0,15 · net > 0.

| Estratégia | Par | n OOS | WR | PF | DD | avg R | net | Veredito |
|---|---|---|---|---|---|---|---|---|
| balance-area-breakout-v1 | IJS | 23 | 60,8% | 2,54 | 0,52% | **0,383** | +3.384 | passa (menos n) |
| balance-area-breakout-v1 | VBR | 34 | 44,1% | 1,51 | 0,74% | **−0,013** | +1.851 | **reprova (avg R)** |
| balance-area-breakout-v1 | AVUV | 35 | **31,4%** | 1,43 | 2,84% | **−0,173** | +2.250 | **reprova (WR e avg R)** |
| range-extreme-fade-v1 | AVUV | 26 | 57,6% | 1,89 | 0,76% | 0,256 | +2.204 | passa (menos n) |
| range-extreme-fade-v1 | SLYV | 20 | 70,0% | 2,64 | 0,71% | 0,424 | +2.417 | passa (menos n) |
| range-extreme-fade-v1 | IWV | 18 | 55,5% | 1,31 | 0,78% | **0,043** | +422 | **reprova (avg R)** |
| opening-reversal-v1 | IWM | 32 | 59,3% | 1,72 | 2,00% | 0,450 | +3.773 | passa (menos n) |
| opening-reversal-v1 | IWN | 26 | 53,8% | 1,56 | 1,71% | 0,283 | +2.337 | passa (menos n) |

**Leitura, exatamente como o ADR-018 previu:**

1. **A `balance-area-breakout-v1` reprova o gate A pelo avg R** com a régua do
   live. Só IJS passa sozinha. Os 20 trades que carregavam 65% do P&L eram
   overnight — e o live nunca os teve.
2. A `range-extreme-fade-v1` continua passando em AVUV e SLYV, e **reprova em
   IWV** (avg R 0,043). Isso confirma §5.10 do plano de forma independente:
   IWV sai. Note que IWV tem **zero** trades `end_of_day` — a estratégia lá
   nunca segurou posição pela noite; o problema dela é o stop de 13 bp, não o
   flatten.
3. A `opening-reversal-v1` **sobe** com o flatten (PF 1,23 → 1,74 in-sample) e
   passa em IWM e IWN. Como o ADR já registra, isso vem de remover 8 trades
   overnight perdedores, não é reabilitação — e o bootstrap de §2.3 dá
   P(PF openrev > PF balance sob flatten) = 0,63.
4. **Nenhuma combinação tem 50 trades OOS.** O critério de amostra do ADR-010
   continua sendo o gargalo, e o paper forward segue sendo o único OOS
   verdadeiro.

## 3b. Efeito do hotfix ET (§5.4) — medido depois, e não é neutro

A tabela de §3 foi produzida **antes** do hotfix do veto de meio-dia. Com a
correção aplicada (`config_hash` `49ee6f045b4c35a7` → `818b53394244ca62`), a
`range-extreme-fade-v1` muda em **um** par:

| Par | n OOS | PF antes → depois | avg R antes → depois | Veredito |
|---|---|---|---|---|
| AVUV | 26 | 1,89 → **1,47** | 0,256 → **0,159** | passa por 0,009 |
| SLYV | 20 | 2,64 → 2,64 | 0,424 → 0,424 | inalterado |
| IWV | 18 | 1,31 → 1,31 | 0,043 → 0,043 | inalterado (reprova) |

In-sample o agregado da fade cai de PF 1,74 / avg R 0,218 / +4.560 para
**PF 1,57 / avg R 0,182 / +3.618** (−21% no net), com a mesma contagem de
trades. O plano previa efeito "imensurável"; não é. Detalhe e leitura na nota
de correção v1.0.1 em `docs/strategies/range-extreme-fade-v1.md` §17 — em
resumo, **o bug estava ajudando**, e o edge da fade é mais fino do que o gate A
de 04/09 mostrava.

**Os números de gate A que valem daqui em diante são os desta seção** para a
range-fade, e os de §3 para as outras duas.

## 4. Sinal de regime que a régua nova torna visível

Última janela (15/06 → 02/09/2026), balance-area: AVUV **9 trades, WR 0%,
PF 0,00, avg R −1,10, −2.360**; VBR 6 trades, PF 0,25, −678; IJS 6 trades,
PF 0,92, −78. Os três pares negativos no mesmo bloco. Com a régua antiga o
bloco aparecia amortecido pelo overnight. Isso alimenta o critério de
encerramento de 18/12/2026 do roadmap de decisão e deve ser reavaliado a cada
janela nova.

## 5. O que este relatório NÃO decide

- **Se o gate B da balance-area continua.** É a decisão 1 do dono (§10 do
  plano). A recomendação do plano segue de pé: manter em paper como controle,
  bloqueada para dinheiro real, lida contra o backtest **com** flatten.
- **Substituição formal do gate A de 04/09.** É a decisão 2 do dono. Este
  documento produz a evidência; a substituição é ato dele.
- **O IC95 em blocos ainda não existe** — depende do relatório em Python
  (`trader-research/`, item 8 do ADR-019), que não foi implementado. **PF_R e
  concentração passaram a existir e estão na §7 desta página.** O **holdout
  travado** também existe como ferramenta (`--holdout-from`), mas **os runs
  desta página não foram rodados com ele** — nenhum número de holdout aparece
  aqui. O veredito da §3 usa só os seis critérios do ADR-010, que continuam
  sendo o gate vigente.
- **Custo real.** Tudo a 2 bp e com comissão de US$ 0,35/perna. A comissão por
  ação da IBKR Canada (§5.6) e a sensibilidade a 4–5 bp entram depois e
  empurram todos os números para baixo.

## 6. Ressalva que a auditoria de 07/09 impõe a ESTES números

`docs/reports/auditoria-2026-09-07.md` (item 4) registra que **o banco dev tem
o mesmo feed degradado do Gateway a partir de 07/08/2026** — barras com 3–10%
do volume e 15–25% do range reais. O período destes runs (24/02/2025 →
03/09/2026) inclui esse mês. Consequência concreta: as barras de 07/08 em
diante são menores do que foram de verdade, então os stops derivados delas
saem menores e o último bloco do walk-forward (15/06 → 02/09/2026) mede em
parte o feed, não o mercado.

Isso **não** invalida a comparação com/sem flatten desta página — os dois lados
leem os mesmos candles, e o delta é o que interessa. Mas contamina o **nível**
absoluto do último bloco, justamente o que §4 lê como sinal de regime. A
reingestão dos 6 símbolos parados e a correção do feed (§5.6 e §5.8 do plano)
vêm antes de qualquer baseline definitivo; até lá, o veredito de §3 vale como
"a régua certa aplicada aos dados que temos", não como número final.


---

## 7. Releitura com o harness do ADR-019 — PF em R e concentração

Os runs `gate-a-adr019` (07/09) refazem o gate A com o flatten, o hotfix ET e
as métricas novas. As duas colunas que o gate não tinha:

| Estratégia · par | n | PF em $ | **PF em R** | 2 melhores meses | t-stat avg R | corr(risco, R) | melhor dia |
|---|---|---|---|---|---|---|---|
| balance · IJS | 23 | 2,54 | 1,91 | 90% | 1,42 | 0,13 | 43% |
| balance · VBR | 34 | 1,51 | **0,97** | 113% | −0,07 | 0,27 | 74% |
| balance · AVUV | 35 | 1,43 | **0,74** | 115% | **−0,80** | **0,57** | 67% |
| fade · AVUV | 26 | 1,47 | 1,37 | 128% | 0,75 | 0,10 | 47% |
| fade · SLYV | 20 | 2,64 | 2,34 | **59%** | 1,86 | 0,04 | 22% |
| openrev · IWM | 32 | 1,72 | 2,03 | 110% | 1,88 | −0,28 | 39% |
| openrev · IWN | 26 | 1,56 | 1,56 | 74% | 1,08 | −0,02 | 31% |

**1. A balance-area perde em R nos dois pares que já reprovavam.** PF_R 0,74
em AVUV e 0,97 em VBR: em unidades de risco, essas instâncias não têm edge. O
PF em dólares acima de 1 vem da correlação entre tamanho e resultado (0,57 em
AVUV, a mais alta do conjunto) — o cap de notional põe posição maior
justamente nos trades de stop largo, e stop largo ganha. É o achado 4 de §2.3
do plano medido pelo motor, e reforça o veredito do ADR-018 por um caminho
independente.

**2. Só a fade em SLYV passa no critério de concentração** (59% contra o teto
proposto de 60%). Todas as outras ficam entre 74% e 128% — acima de 100%
significa que a soma dos demais meses é negativa.

**3. Nenhum t-stat chega a 2**; o de AVUV é **negativo**. Com 20–35 trades
isso é o esperado, e é o motivo de o plano tratar DSR/PSR como relatório e não
como gate. Mas fecha o quadro: não há aqui nenhuma combinação estatisticamente
distinguível de ruído.

**4. A openrev inverte o sinal** (PF_R acima do PF$, correlação negativa entre
risco e R), exatamente como a re-simulação previa.

**Se o gate proposto no ADR-019 §7 valesse hoje** — e ele ainda não vale, é
decisão do dono — o resultado seria: **nenhuma das sete combinações passa**, e
a que chega mais perto é `range-extreme-fade-v1` em SLYV, reprovada só pelos
20 trades contra os 50 exigidos. Isso não muda o que fazer no curto prazo (o
paper forward segue sendo o único OOS verdadeiro e a amostra é o gargalo), mas
muda a expectativa: a régua certa, aplicada aos dados que existem, não aprova
nada.
