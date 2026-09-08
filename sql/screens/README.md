# `sql/screens/` — Fase 0: screener SQL com fill honesto

**Status:** proposto / especificado — **NÃO implementado como gate (07/09/2026)**. Os
arquivos `.sql` deste diretório rodam contra o banco dev e reproduzem os screens
executados em 06/09/2026; a **calibração obrigatória** (§4) ainda não foi executada.
Até lá o screener é ferramenta de pesquisa, não etapa do framework.
**Origem:** `docs/cto-plano-lucratividade-2026-09.md` §5.7 (item 8 do ranking, tier A,
crítica "manter (8)") e a proposta "Fase 0 — Screener com fill honesto" em
`docs/strategy-analysis-framework.md` §3 (pendente de aprovação do dono).
**Fontes dos números:** re-simulação dos críticos (06/09/2026) quando existe; caso
contrário, os screens do designer de setups (06/09/2026). Nenhum número deste README foi
produzido depois disso — o que falta medir está marcado como *a medir*.

---

## 1. Objetivo

Reprovar candidatos a estratégia **antes** de escrever Rust. Cada setup de livro que segue
da Fase 3 do framework (especificação) para as Fases 4–7 (implementação, testes, validação)
custa 1–2 semanas de código, doc, testes e walk-forward — foi o destino das 5 estratégias
arquivadas (`d5e7279`: "todas continuam reprovadas, e todas pioraram"). Em 06/09/2026, um
dia de consultas SQL sobre os ~136k candles de 15 min do banco dev (14 ativos; os screens
usaram 11 ETFs, 24/02/2025 → 02/09/2026) reprovou 7 candidatos que as análises de livro
ranqueavam no top-3: squeeze-breakout, pivot-point-intraday, opening-range-breakout,
gap-fade, "dia após clímax", double-top solto e o toque bruto de PDH (§8).

O screener é um **filtro de reprovação**: quem reprova aqui não vai para a Fase 3; quem
passa **não está aprovado** — segue para a Fase 3 e para o gate A com o N de variantes
testadas registrado (§7), porque cada variante conta como tentativa para o Deflated Sharpe
pendente (`docs/books/analysis/lopez-afml.md`, Cap. 14).

O ingrediente que separa este screener de um "scanner otimista" é o **fill honesto** (§2).
Exemplo do plano-mestre §5.7 (aqui com três casas): o rompimento de PDL vendido (Murphy
Cap. 16) dava avg R +0,063/+0,134 (2025/2026) com fill exato no nível (screen do designer
de setups, 06/09/2026); com fill honesto, −0,034/+0,034 (n = 499/344 — re-simulação dos
críticos, 06/09/2026). O "edge" inteiro era o gap atravessado no gatilho — o mesmo
artefato que a ADR-015 corrigiu no simulador.

## 2. Modelo de fill (o que o screener simula)

O modelo de referência — o do `_template.sql` e dos blocos "honestos" de `01`–`05` — espelha
o `SimulatedBroker` depois da ADR-015 (`crates/trader-adapters/src/simulated/broker.rs:245-283`).
Onde um screen de 06/09 diverge, o cabeçalho do arquivo registra: `02` não filtra risco
(stop = range da barra de release); `05` usa piso de 0,15% sem teto; `06` e `07` são
medições sem modelo de fill; `08` e os blocos "otimistas" de referência em `01`/`03`/`04`
não aplicam custo nem fill honesto (servem para mostrar o que um scanner ingênuo veria).

```text
entrada   primeira barra da janela de validade cuja high/low atravessa o gatilho;
          preço = max(open, gatilho) para long / min(open, gatilho) para short
          (abertura além do gatilho enche na abertura — nunca no gatilho)
stop      avaliado PRIMEIRO, a partir da barra do fill INCLUSIVE: se a barra do fill
          já toca o stop, o trade vale −1R (pior caso, como broker.rs:351-353)
alvo      k × R, com vários k por cross join (1; 1,5; 2; "99" = segurar até o fechamento)
saída     se nem stop nem alvo: fechamento da última barra do dia (15:45 ET)
custo     −0,0004 / risk_pct, subtraído em R (4 bp ida e volta, ADR-016: 2 bp por lado)
filtro    0,3% ≤ |fill − stop| / fill ≤ 1,5% (piso: o plano-mestre §2.2 diz "stop < 0,25%
          do preço nasce morta"; os screens de 06/09 usaram 0,3%, um pouco mais estrito —
          interpretação nossa; teto de 1,5% = "não é day trade de 15 min", interpretação nossa)
R         = |fill − stop| (em preço); r_net = resultado em R menos o custo
```

Métricas de saída, sempre por **ano** (2025 / 2026) e, quando faz sentido, por símbolo e
por hora ET do fill: `fills`, `avg_r_net`, `wr`, `risk_med_pct` (mediana da distância do
stop em % do preço), `sum_r`, `pf_r` = Σr⁺ / |Σr⁻|. O template (§9) acrescenta a agregação
por **data** (§5).

Fuso: todos os timestamps são convertidos com `at time zone 'America/New_York'`; janelas
(`10:00`–`14:30`, `< 10:30` = primeira hora, etc.) e horas de fill são ET. ATRd = média de
`(high − low) / close` dos 14 pregões anteriores (relativa, simples).

## 3. Diferenças em relação ao `SimulatedBroker` e ao engine de backtest

O screener é mais simples e, em geral, **mais pessimista** que o motor — aceitável para um
filtro de reprovação, mas cada diferença abaixo precisa ser conhecida antes de ler um
número. Quando for calibrar (§4), é esta tabela que se confere.

| Aspecto | `SimulatedBroker` / engine | Screener (screens de 06/09) | Efeito |
|---|---|---|---|
| **Gap de abertura no gatilho** | Se a abertura passa do gatilho mais que **25% da distância do stop**, a entrada é **cancelada** (`entry_overshoot_tolerance = 0.25`, `config/default.toml:42`; `broker.rs:262-283`; guarda pré-envio do live em `crates/trader-core/src/execution/mod.rs:93-117`) | **Enche no preço pior** (`max/min(open, gatilho)`), sem cancelar | Screener inclui trades que o bot nunca faria; mais pessimista em dias de gap e distorce o avgR por hora do fill. O template tem a flag `gap_cancel` para replicar a ADR-015 — *ainda não calibrada* |
| **Barra de sinal** | Toda estratégia v1 exige barra de sinal (corpo, sombra, fechamento no terço) e vetos (`momentum_veto`, `counter_trend_veto`, Barb Wire…) | Só nível + gatilho, salvo quando o screen modela explicitamente (05 modela corpo ≥ 30% e fechamento no terço; 06 mostra que o nível sozinho não paga) | Pode reprovar um setup cujo edge está na barra de sinal (falso negativo). Setup desse tipo **tem de modelar a barra no `sel`** |
| **Comissão** | US$ 0,35 por perna (`broker.rs:109`) + slippage de 2 bp por lado; produção IBKR Canada cobra US$ 0,005/ação, mín. US$ 1,00 (≈ 0,9 bp ida e volta, plano §2.3 achado 7) | 4 bp ida e volta, **sem comissão fixa** | Diferença pequena para 800–950 ações; irrelevante para o veredito |
| **ATR** | `daily_atr` = média simples do range (`high − low`) dos últimos N dias **completos**, em preço (`crates/trader-core/src/strategies/range_extreme_fade_v1/context.rs:59-89`); `atr` de barras usa true range (`context.rs:91-108`). Nada de Wilder em lugar nenhum | ATRd relativo = média de `(h − l)/close` dos 14 dias anteriores; "atr20" de barras = média de `high − low` de 20 barras, **sem true range** | Limiares em ATR não são idênticos aos do TOML; o efeito na classificação de "dia de range" está *a medir* (comparar o `sel` do controle positivo com os sinais do backtest, §4) |
| **Fim de sessão** | O engine **não tem** fim de sessão: posições atravessam a noite (`crates/trader-backtest/src/engine.rs:136-240`, plano §2.3 achado 1; ADR-018 proposta). O live faz flatten a mercado na janela 15h55–16h10 ET (`crates/trader-cli/src/commands/paper.rs:2189-2197`; loop em `paper.rs:925-941`; grava `Manual`, `paper.rs:1780-1787`) | **Sai no fechamento da barra 15:45 ET** — replica o **flatten do live**, NÃO o engine sem EOD | Um controle positivo lido do backtest atual falha por construção: a balance-area tem 20 de 96 trades overnight com 65% do P&L. Calibrar contra a re-simulação **com flatten** (plano §2.3, régua oficial: balance PF 1,55 / avg R ≈ 0) — "ex-overnight" (PF 1,43) só como diagnóstico — ou, quando a ADR-018 estiver no motor, contra o walk-forward com flatten. `hold_days` no template mede o delta overnight |
| **Ordem intrabar** | Stop primeiro, alvo depois, na barra do fill inclusive (`broker.rs:351-353`). Stop enche em `min/max(open, stop)` + slippage (gap no stop custa **mais** que −1R); alvo é limit e enche em `max/min(open, alvo)` (gap ajuda) | Stop primeiro, mesma barra; **−1R e +kR exatos** | Screener levemente otimista no stop com gap, pessimista no alvo com gap |
| **Toque do nível** | `low <= stop`, `high >= target` (`broker.rs:363-390`) | Screens de 06/09: gatilho e stop **estritos** (`low < PDL`, `high > stop`), alvo `<=`/`>=`; template: `<=`/`>=` em tudo, como o simulador | Diferença de 1 tick; irrelevante para o veredito, registrada por paridade |
| **Validade da entrada** | `entry_validity_candles` do TOML da estratégia (2 nas aprovadas, 1 na `pullback-trend-v1`; default do simulador 1, `broker.rs:111`), contado igual no simulador e no live desde a correção A3 (`docs/reports/gate-a-revalidacao-2026-09-04.md`) | Primeiro toque dentro da janela inteira (ex.: 10:00–14:30), ou N barras quando o screen modela (02, 05, 04/Q2b, 01/Q12b: 2 barras) | Screener gera **mais fills** que o motor para o mesmo setup |
| **Dinheiro e capacidade** | Sizing por risco com cap de notional 1× (`risk/mod.rs:239-279`; cap por liquidez e `capital_fraction` na ADR-020 proposta), 3 posições por conta (ADR-017), `max_trades_per_day` (`config/default.toml:31`), uma posição por símbolo | Só R; sem sizing, sem cap, sem limite de posições, um trade por (símbolo, dia) por regra | `fills` e `sum_r` **não** são comparáveis a trades e P&L em US$ do backtest |
| **Universo** | Um símbolo por run | 11 ETFs fixos (AVUV, IJS, IWM, IWN, IWO, IWV, SLYV, VBR, IJR, SCHA, VB); IJR, SCHA e VB param em 06/08/2026 no banco dev (plano §5.6) | n inflado por correlação — por isso a agregação por data (§5) |

## 4. Calibração obrigatória antes de usar como gate

Os três críticos que revisaram a proposta pediram calibração antes do uso (critiques,
setups-novos-4, 06/09/2026); o crítico de validade estatística foi o mais direto: o screener
**nunca rodou um controle positivo** — só produziu negativos — e sua sensibilidade é
desconhecida. Antes de qualquer setup novo ser reprovado *por este instrumento*, rodar e
publicar:

1. **Controles positivos** (têm de passar o critério de §5):
   - `range-extreme-fade-v1` em **AVUV e SLYV** (gate A OOS: PF 1,95 e 2,95;
     `docs/reports/gate-a-revalidacao-2026-09-04.md`; com flatten às 15h45, PF 1,74 no
     agregado in-sample dos 3 pares, 69 trades — plano §2.3). Modelo a escrever no `sel`: nova mínima/máxima do
     dia a ≤ 0,5 × ATR14 em dia com range < 1,5 × ATRd + barra de sinal (corpo ≥ 30% do
     range, sombra ≥ 1/3) + entrada 1 tick além da barra de sinal + stop 1 tick do outro
     lado + alvo 1,5R (proposta setups-novos-4; regra completa em
     `docs/strategies/range-extreme-fade-v1.md`).
   - `balance-area-breakout-v1` em **IJS, VBR e AVUV, com flatten às 15h45** (a régua do
     live e do plano §5.7): PF 1,55 / avg R −0,007 no agregado in-sample dos 3 pares (IJS
     1,86 / VBR 1,47 / AVUV 1,43; 96 trades — re-simulação dos críticos, 06/09/2026, plano
     §2.3). Como o screener também sai no fechamento do dia, é este o número que ele tem de
     reproduzir. O "ex-overnight" (descartar os 20 trades: PF 1,43 / avg R −0,05) serve só
     como diagnóstico (plano §3 item 1). O PF 1,92 sem flatten **não é referência** — o
     screener não pode reproduzi-lo; calibrar o modelo até "bater" 1,92 seria calibrar para
     um artefato. Quando a ADR-018 estiver no motor, a referência passa a ser o walk-forward
     com flatten (plano §5.1: esperado PF ~1,5 e avg R ~0).
2. **Controle negativo** (tem de reprovar): `pullback-trend-v1` (PF 0,83 a 2 bp, plano
   §2.1; ADR-016). Se der tempo, as outras arquivadas com regra simplificada —
   `breakout-first-pullback-v1` 0,92, `trendline-break-test-v1` 0,91, `low2-m2s-short-v1`
   0,66, `value-area-reentry-v1` 0,65, `failure-test-long-v1` 0,61 (gate A, mesma fonte).
3. **Flag `hold_overnight`** (no template: `hold_days`, 0 = flatten do live; N = carrega
   N pregões). Serve para medir o delta overnight de um setup, não para aprová-lo com ele.
4. **Cancelamento por gap** (`gap_cancel = true`, tolerância 0,25) para bater com a
   ADR-015. Reportar também quantos fills foram cancelados.
5. **Agregação por DATA, não por símbolo-dia**: 11 ETFs a ~0,97 de correlação são o mesmo
   dia; 499 fills/ano de PDL-break são muito menos de 499 observações. O n efetivo, o
   t-stat e a participação do melhor trimestre saem da série de R somado por data.
6. **Publicar sensibilidade e especificidade** do template em `docs/reports/screens-<data>.md`:
   quantos positivos passaram, quantos negativos reprovaram. Se um positivo reprova ou um
   negativo passa, ajustar o modelo de fill/stop/barra de sinal (nunca o critério) e
   re-rodar tudo.

Só depois disso a Fase 0 entra no framework como etapa obrigatória (decisão do dono,
plano §10 item 6).

Sequenciamento e dependências (plano §9): os arquivos entram no repo na semana 1 sem
dependência de código; a calibração pode rodar antes da ADR-018 usando a re-simulação
dos críticos como referência, mas a referência definitiva do controle positivo da
balance-area é o walk-forward com flatten (§5.1, ADR-018); o N de variantes registrado
em §7 só vira insumo de verdade quando o harness gravar `n_trials`/`trial_group` (§5.2,
ADR-019). Nada aqui depende da ADR-020.

## 5. Critério de decisão (declarado antes de rodar)

Texto da proposta em `docs/strategy-analysis-framework.md` §3, "Fase 0" (reproduzido):

```text
Regra de decisão, declarada ANTES de rodar:
  - avg R líquido agregado com t ≥ 1,5;
  - mesmo sinal em 2025 e em 2026;
  - melhor trimestre ≤ 50% do P&L;
  - ≥ 40 fills/ano nos ativos vivos, contados por DATA (11 ETFs correlacionados são o mesmo dia);
  - o sinal não inverte ao mover o parâmetro principal ±20%.
Reprovou → não vai para a Fase 3. Passou → segue, e o N de variantes testadas
entra no relatório (docs/reports/screens-<data>.md) para a contagem de tentativas.
```

"Agregado" = todos os símbolos e todo o período; o t-stat do avg R sai da série de R
somado por **data** (§4 item 5), não da lista de fills por símbolo-dia — é o que o
template calcula na saída S2.

Este critério substitui o da proposta original ("avgR ≥ +0,10 e PF_R ≥ 1,2 em cada ano"),
a pedido dos críticos: com ≥ 40 fills/ano e sd(R) ≈ 1,2–1,5 (medido nos trades do
projeto: range-fade 1,18, openrev (shorts) 1,46, balance 1,68), SE(avgR) ≈ 0,19–0,24 — um setup
com avgR verdadeiro +0,25 (≈ o OOS da range-fade, 0,30) passaria o limiar por ano só
55–60% das vezes. O critério agregado por data reduz o falso negativo; não o elimina. Com
< 100 fills nenhuma regra deste projeto alcança DSR 0,95 (plano §3 item 5) — o screener
reprova; não aprova.

## 6. Como executar

Pré-requisito: o container `trader-postgres` do `docker-compose.yml` (banco dev
`trader_db`; na máquina de dev a porta é 5434 via `POSTGRES_PORT` no `.env`, o default
do compose é 5433 — irrelevante para `docker exec`, que fala com o container direto) com
candles 15m ingeridos. Cada arquivo é autocontido (cria suas próprias tabelas
temporárias na sessão) e roda de ponta a ponta numa única chamada.

```bash
# Git Bash / Linux
docker exec -i trader-postgres psql -U trader -d trader_db < sql/screens/01-pdl-pdh-break.sql

# guardando a saída bruta para o relatório
docker exec -i trader-postgres psql -U trader -d trader_db < sql/screens/01-pdl-pdh-break.sql \
  | tee /tmp/screens-01.txt
```

```powershell
# PowerShell (não tem redirecionamento de stdin com <)
Get-Content sql/screens/01-pdl-pdh-break.sql | docker exec -i trader-postgres psql -U trader -d trader_db
```

Notas:
- Fuso **America/New_York** em toda agregação diária e em toda comparação de hora
  (padrão do repo). Nunca comparar hora em UTC: desliza 1h no horário de inverno — foi
  o bug do veto de meio-dia da range-fade (plano §5.4).
- Os arquivos usam `create temp table`; rodar dois arquivos **na mesma sessão** psql
  falha por tabela já existente — um `docker exec` por arquivo.
- Não há dependência externa: só `candles` e `assets` (`docs/DATA-MODEL.md`).
- O banco dev é somente leitura para o screener; nada é gravado em `backtest_runs`
  (os críticos descartaram o `trader-cli screen` gravando lá: o schema exige
  `asset_id`, `config_hash` e `final_equity` por run, que um screen agregado em 11
  símbolos não tem).

## 7. Como registrar

Cada sessão de screening gera **um** relatório `docs/reports/screens-<AAAA-MM-DD>.md`, com
o N de variantes testadas (regras distintas, limiares e alvos), mesmo as que não
apareceram em nenhuma proposta. Esqueleto:

```markdown
# Screens de <data> — <setups testados>

**Banco:** dev `trader_db`, candles 15m <de> → <até>, símbolos <lista>
**Modelo:** fill honesto (sql/screens/README.md §2), custo 4 bp, hold_days = 0, gap_cancel = <true|false>
**Calibração vigente:** <link para o relatório de calibração ou "não calibrado">

| Arquivo | Setup (fonte: livro/capítulo ou "interpretação nossa") | Variantes | Resultado (avg R por data, t, 2025/2026, melhor trim.) | Veredito |
|---|---|---|---|---|

**N acumulado de variantes desta família:** <n> (soma com os relatórios anteriores)
**Próximo passo:** <Fase 3 | arquivar | re-rodar com …>
```

O N acumulado é insumo do relatório estatístico de §5.3 do plano (DSR reportado como
faixa, nunca pass/fail) e, quando o harness existir, do `n_trials`/`trial_group` por
família (ADR-019 proposta, plano §5.2).

## 8. Screens executados em 06/09/2026

Fill honesto, 4 bp, 11 ETFs, 24/02/2025 → 02/09/2026, saída no fechamento do dia, **sem**
cancelamento por gap (a flag não existia) — salvo onde a linha indica (06 e 07 são
medições sem fill; 08 é modelo otimista). Números da re-simulação dos críticos
(06/09/2026) onde indicado; os demais são do designer de setups, mesma data. Fontes dos
livros via `docs/books/analysis/`.

| Arquivo | Setup (fonte) | Variantes | Resultado em 06/09/2026 | Veredito |
|---|---|---|---|---|
| `01-pdl-pdh-break.sql` | Rompimento de PDL (short) / PDH (long), abertura dentro do range de ontem — Murphy Cap. 16, análise §3.2 (`pivot-point-intraday`) | 8 regras × alvos 1 / 1,5 / 2 / fechamento | PDL-break short: fill no nível avg R **+0,063 / +0,134** (2025/2026, PF_R 1,21 / 1,50) → fill honesto **−0,034 / +0,034** (PF_R 0,90 / 1,11; n = 499 / 344 — re-simulação dos críticos, 06/09/2026). PDH-break long PF 0,64 em 2026. Condição TO < PDC do livro piora. Variante confirmada por fechamento (Q12b): *a medir* — sem número registrado | **Reprovado** — o "edge" era o gap no gatilho |
| `02-squeeze-release.sql` | Release de squeeze (BB20 dentro do canal SMA20 ± 1,5 × range20 por ≥ 6 barras) — Rosewag Cap. 12, análise §3.7 (`ttm-squeeze-breakout`); proxy BB/KC com SMA e range simples = interpretação nossa | 1 regra de trade (fade) × 3 alvos + 2 medições forward | O release **reverte**: retorno em 8 barras na direção do rompimento **−0,30% / −0,13%** (2025/2026). Fade do release com stop na barra: PF **0,15–0,52** | **Reprovado nas duas direções** |
| `03-orb-ib-estreito.sql` | Opening range / Initial Balance estreito, rompimento após 10:30 — Rosewag Cap. 16 (análise §3.2) + Dalton Cap. 2 (análise §2.1); limiares de "estreito" = interpretação nossa | 5 regras (limiar absoluto 0,5%; relativo 0,45 / 0,35 / 0,30 × ATRd; short por símbolo) | Limiar relativo < 0,45 × ATRd: short **+0,02 / −0,05**, long **+0,08 / −0,17** (2025/2026). H1 < 0,30 × ATRd: short −0,20 / +0,10. Limiar absoluto 0,5%: *a medir* (sem número registrado) | **≈ 0 — reprovado**; IB entra só como contexto (plano §6.6) |
| `04-gap-fade.sql` | Gap-down ≥ 1% com "drive back" na 1ª hora, long acima da barra das 10:15 — Dalton Setup C invertido (análise "Setup C — Gap"); a inversão e os limiares 1% / 0,8 × ATRd = interpretação nossa | 2 regras + estatística por bucket de gap | Entrada acima da 10:15, stop na mínima da 1ª hora: avg R **+0,09 (2025 ex-crash) / −0,04 (2026)**. Buckets de gap (Q1) e entrada na máxima da 1ª hora (Q2): *a medir* | **≈ 0 — reprovado** |
| `05-double-top-range.sql` | Double-top / double-bottom "solto" (2ª entrada) em dia de range, com barra de sinal — Brooks, análise §2.4; tolerâncias em ATRd = interpretação nossa | 1 regra (2 direções) × 3 alvos | PF **0,37–0,86** com stops de ~0,3% | **Reprovado** — o edge da range-fade está no detector de dia de range + barra de sinal, não em padrões soltos |
| `06-pdh-touch-first-hour.sql` | Toque bruto da PDH nas barras 09:45–10:15 (abre abaixo, high ≥ 0,997 × PDH) — base da `opening-reversal-v1` (Brooks Cap. 11 / Dalton Cap. 4); tolerância 0,3% = interpretação nossa | 1 medição (sem modelo de fill) | n = **1.119** (587 em 2025, 532 em 2026; ~70/símbolo/ano); retorno short até o fechamento **+0,03% / −0,06%**; **78%** das barras de toque são rompidas depois; MFE mediano 0,44–0,46%; range da barra de toque 0,33–0,37% (stop de 1 barra ≈ 33 bp) | O nível sozinho **não tem edge**; se existe, está na barra de sinal + vetos (plano §6.3) |
| `07-neutral-share-range-days.sql` | Proxy de contexto (close/SMA20/SMA200 do 15m) dentro de dias de range (range < 1,5 × ATRd) — não é setup | 1 medição + variante | **44,5%** das barras de dias de range são `neutral` (26.686 de 59.976; Q14 re-executada pelos críticos) contra ~41–42% nos dias de tendência (41,1% no screen do designer; 41,9% na recomputação de um crítico — o proxy **não** discrimina dia de range; o argumento válido é o do detector contraditório, plano §2.3 achado 5). Regra real EMA20/SMA200 por símbolo: 41–47% (re-simulação dos críticos). Produção: 36% das barras `neutral` (`docs/reports/pregoes-2026-08-31_a_09-02.md`, 195 barras; o designer mediu o mesmo 36% em `market_contexts` de 31/08–04/09) | Motiva a `range-extreme-fade-v2` (`allow_neutral`, plano §6.1) — o PF dos sinais bloqueados é **desconhecido** |
| `08-dia-apos-queda.sql` | Dia seguinte a queda close-to-close ≥ 2% ("dia após clímax") — Murphy Cap. 4, análise §3.6; limiar 2% e mecânica de entrada = interpretação nossa; sem custo nem fill honesto (modelo otimista — já negativo em 2026 assim mesmo) | 2 regras × alvos + estatística por bucket | avg R **+0,55 (2025 ex-crash) / −0,09 (2026)** | **Regime, não edge** |

Total: **19 regras de entrada/saída distintas** (sem contar alvos, limiares e direções)
em 8 arquivos — é o N mínimo a somar no primeiro `docs/reports/screens-<data>.md`.
Conclusão da lente de setups, confirmada pelos críticos: em 18 meses de amostra **nenhum
setup de continuação/rompimento sobrevive ao fill honesto**; o que paga é fade com
confirmação em dia de range, com stop ≥ 20 bp.

## 9. Arquivos

| Arquivo | Conteúdo |
|---|---|
| `_template.sql` | Template parametrizado: blocos `p` (parâmetros) → `bars` → `days` → `dd` → `bs` → `sel` → `f` → `fb` → `fb2` (cancelamento por gap) → `st` → `tg` → `ex` (fechamento do último pregão permitido) → `r` → saídas S1–S7: por ano, por data (t-stat), melhor trimestre, por símbolo, por hora do fill, por direção e cancelados por gap. Inclui `gap_cancel` e `hold_days`. Vem com um `sel` de exemplo (PDL-break short); para um setup novo, normalmente só `sel` muda |
| `01-…` a `08-…` | Screens de 06/09/2026, um arquivo por setup, com cabeçalho (setup, fonte, hipótese, parâmetros, resultado, N de variantes). São **cópias fiéis** das consultas que produziram os números de §8 — não foram "melhoradas" para que os números continuem reproduzíveis |
| `../stats/` | Consultas de **estatística** do banco (liquidez, range, gaps, autocorrelação, proxy de tendência) — não são screens |

Regras do repo que se aplicam aqui: o screener não toca motor, trait, TOML nem estratégia
v1 (v1 nunca muda; v2 nasce validada do zero); toda regra num `sel` tem fonte
(livro/capítulo) ou está marcada como *interpretação nossa*; f64/numeric é aceitável
porque o screener mede R, não dinheiro (`Decimal` continua obrigatório em código de
produção); paper only; sempre stop server-side; sem martingale.

## 10. Limites honestos

- Um regime só (24/02/2025 → 02/09/2026), com um episódio de volatilidade extrema
  (abr/2025) e um de compressão (ago/2026, menor range diário da amostra: 0,78%). "2025 ex-crash"
  nos screens = excluindo mar–abr/2025.
- O screener facilita testar 50 variantes por tarde; sem o registro de N (§7) ele vira um
  acelerador de data-mining. O N só tem valor quando existir o consumidor (DSR do plano §5.3).
- Números por símbolo-dia inflam o n (§4 item 5). Os screens de 06/09 ainda reportam
  assim; o template corrige.
- O modelo não vê barra de sinal, latência de feed (plano §2.3 achado 7), liquidez por
  barra (§2.4) nem o teto de 3 posições da conta. Um "passa" aqui é uma hipótese para a
  Fase 3, não um resultado.
