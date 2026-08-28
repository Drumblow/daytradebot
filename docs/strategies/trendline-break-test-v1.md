# Estratégia: Trendline Break Test v1

**Id:** `trendline-break-test-v1`
**Versão:** 1.0.0
**Status:** implementada; PF 1,38 no backtest mas amostra insuficiente — candidata retida, não aprovada (ver §16)
**Data:** 2026-08-28

---

## 1. Fonte

Al Brooks, *Reading Price Charts Bar by Bar* — Cap. 15 ("Major Reversals"),
Cap. 8 (abertura e "Trendline Break"), com elementos do Cap. 1 (barras de
reversão).

Extração prévia: `docs/books/analysis/brooks-bar-by-bar.md` §2.2 (as 8
perguntas) e §3.2 (tabela Subjetivo → Objetivo). É a **3ª colocada** do ranking
daquele documento; as duas primeiras (`range-extreme-fade-v1` e
`opening-reversal-v1`) já estão em produção, o que era a condição que a própria
análise colocava para esta entrar na fila.

## 2. Conceito em uma frase

Depois que uma tendência tem sua **linha de tendência rompida com momentum**, o
mercado volta para **testar o extremo antigo** — e é nesse teste, não no
rompimento, que se compra o fundo (ou se vende o topo).

Citação central (Cap. 15): *"The best trend reversal entries have a break of a
significant trendline before the setup... Enter on a pullback that tests the
trend extreme."*

E a regra que define quando NÃO operar (Fig. 8.3): *"don't trade Countertrend
until after there has been a trendline break."*

## 3. Fase 1 — Extração do conceito (8 perguntas)

| # | Pergunta | Resposta (do livro) |
|---|---|---|
| 1 | Nome | Trendline Break Reversal (reversão maior em duas pernas) |
| 2 | Contexto | Tendência de pelo menos algumas horas; primeiro movimento contrário rompe uma trendline significativa com momentum |
| 3 | Timeframe | 5min no livro (reversões de swings de horas) — **adaptado para 15m**, ver §9 |
| 4 | Entrada | No teste do extremo antigo, que pode *undershoot* (Higher Low) ou *overshoot* (Lower Low); stop além de uma barra de reversão forte, preferindo segunda entrada |
| 5 | Stop | Além do extremo do teste |
| 6 | Alvo | Esperam-se **duas pernas** contrárias; "swing part of your position" |
| 7 | Quando NÃO operar | Sem quebra prévia de trendline; em tendência forte com sinais fracos; *"make sure that it is perfect"* |
| 8 | Estatísticas | Nenhuma numérica. O autor afirma que **a maioria das reversões falha sem trendline break prévio**, e que teste com Higher High vs Lower High ocorre com frequência aproximadamente igual depois do break |

## 4. Contexto — tendência estabelecida

Antes de qualquer coisa é preciso existir uma tendência para reverter.

- Janela de `trend_lookback` barras (default 12 ≈ 3 h de 15m — o "algumas horas"
  do Cap. 15).
- Pivôs de swing = extremo com `pivot_bars` (default 2) barras de cada lado.
- **Tendência de baixa** (candidata a reversão *long*): ≥ 2 lower highs **e**
  ≥ 2 lower lows na janela.
- **Tendência de alta** (candidata a reversão *short*): ≥ 2 higher highs **e**
  ≥ 2 higher lows.
- Sem essa estrutura → `NoTrendToReverse`.

## 5. O rompimento da trendline (a pré-condição do livro)

Este é o filtro que o autor trata como inegociável. Componente **novo** no
projeto: nenhuma estratégia atual calcula trendline.

1. **A linha:** para tendência de baixa, a reta que passa pelos **dois últimos
   swing highs**; para tendência de alta, pelos **dois últimos swing lows**.
   Extrapolada barra a barra pela inclinação entre os dois pivôs.
2. **O rompimento:** um **fechamento** além da linha extrapolada (acima, em
   tendência de baixa). Fechamento, não pavio — mesma escolha das irmãs para
   reduzir ruído.
3. **Com momentum**, os três critérios do §3.2 da análise:
   - a perna contrária tem ≥ `break_min_bars` barras (default 3);
   - fecha além da EMA20 por ≥ `break_min_closes_beyond_ema` barras (default 2);
   - ultrapassa o **último swing point** da tendência antiga (num fundo: o rally
     supera o último lower high).
4. O rompimento precisa ter ocorrido nas últimas `break_max_age` barras
   (default 20) — reversão testada muito depois já é outro contexto.

Falhas geram `NoTrendlineBreak` ou `BreakWithoutMomentum`.

## 6. O teste do extremo (o setup de entrada)

Depois do break, o preço volta ao extremo antigo (o low da tendência de baixa).

| Caso | Regra |
|---|---|
| **Undershoot** (Higher Low) | o teste **não** atinge o extremo antigo, chegando a ≤ `test_tolerance_pct` (default 0,3%) dele |
| **Overshoot** (Lower Low) | o teste **fura** o extremo antigo em até `max_overshoot_atr` (default 0,5) × ATR(15m) |
| **Anulação** | furar por **mais** que isso ⇒ *"the reversal is nullified, and the old trend has resumed"* (Cap. 8) ⇒ `ReversalNullified` |

Brooks é explícito de que os dois casos ocorrem com frequência parecida — por
isso **ambos** valem, e o Higher Low tem citação própria: *"It is imperative to
buy the first Higher Low because this reinforces the premise that a major bottom
is in"* (Cap. 8).

## 7. Barra de sinal

Mesma família das irmãs, espelhada para os dois lados:

- corpo ≥ `signal_body_min_pct` (default 0,30) do range da barra;
- sombra do lado testado ≥ `signal_wick_min_pct` (default 0,334) do range;
- fechamento no terço favorável (superior para long, inferior para short);
- direção da barra a favor da reversão.

Sem isso → `WeakConfirmation`.

> **Escopo da v1:** o livro aceita *"barra de reversão forte **OU** segunda
> entrada"*, e prefere a segunda entrada. A v1 implementa **apenas a barra de
> reversão**. Segunda entrada fica como candidata explícita de v2 — adicioná-la
> agora dobraria a superfície de calibração antes de sabermos se a premissa
> básica funciona.

## 8. Entrada, stop e alvo

| Elemento | Regra |
|---|---|
| **Entrada** | Ordem **stop** 1 tick além do extremo da barra de sinal, na direção da reversão (ADR-009) |
| **Stop** | 1 tick além do **extremo do teste** (o low do teste, numa reversão long) — literal do Cap. 8 |
| **Alvo** | `target_r_multiple` × risco (default **2,0**) — ver a adaptação em §9 |
| **Saída por tempo** | `time_exit` após `time_exit_candles` (default 12) barras sem alvo nem stop |
| **Validade da entrada** | `entry_validity_candles` (default 2) |

Guardas: `min_risk_reward` (1,5), `max_stop_atr` (2,0), `min_stop_atr` (0,15 —
lição direta da `value-area-reentry-v1`, cujo primeiro corte permitiu stops de
2 ticks e RR artificialmente alto).

## 9. Adaptações à nossa infra (interpretações nossas)

Três, e todas precisam ficar explícitas:

1. **Timeframe 15m em vez de 5m.** O livro fala de reversões de swings de horas;
   12 barras de 15m ≈ 3 h cobre o mesmo horizonte com um terço das barras.
2. **Alvo único em vez de duas pernas.** O Cap. 15 manda "swing part of your
   position" — sair parcial e carregar o resto. Nosso bracket na IBKR tem **um
   único TP**, então isso não é representável. A adaptação é um alvo de 2R
   (acima do 1,5R de scalp da `range-extreme-fade-v1`, porque aqui a expectativa
   é de duas pernas) mais a saída por tempo. **Esta é a maior distância entre o
   livro e a implementação, e é candidata a explicar um eventual fracasso.**
3. **Trendline por 2 pivôs, sem desenho.** O trader traça a linha no olho; nós
   usamos a reta pelos dois últimos pivôs do mesmo lado. É uma aproximação —
   linhas "significativas" no livro às vezes ligam pivôs não adjacentes.

## 10. Rejeições registradas

Novas em `RejectionReason`:

| Variante | Quando |
|---|---|
| `NoTrendToReverse` | sem estrutura de tendência na janela |
| `NoTrendlineBreak` | trendline não foi rompida em fechamento |
| `BreakWithoutMomentum` | rompeu, mas sem barras/EMA/swing point |
| `BreakTooOld` | rompimento fora da janela `break_max_age` |
| `NoExtremeTest` | preço não voltou para testar o extremo |
| `ReversalNullified` | overshoot além do limite — tendência antiga retomada |

Reaproveitadas: `IncompleteSetup`, `OutsideTradingHours`, `WeakConfirmation`,
`PoorRiskReward`, `StopTooWide`, `StopWithinNoise`, `SetupInvalidated`,
`EntryExpired`.

## 11. Complementaridade com o portfólio

| Estratégia em live | Contexto | Relação |
|---|---|---|
| `pullback-trend-v1` | continuação de alta | **oposta**: opera a tendência que nós apostamos que acabou |
| `balance-area-breakout-v1` | rompimento de congestão | nenhuma — exige balance area, nós exigimos tendência |
| `opening-reversal-v1` | primeira hora | pouca — nossa janela começa depois |
| `range-extreme-fade-v1` | extremos em dia de range | **atenção**: ambas são contra-tendência. Diferença: a range-fade exige explicitamente um **dia sem tendência** (EMA flat, sem estrutura HH/HL); nós exigimos o **oposto** — tendência estabelecida e rompida. Por construção os contextos se excluem, mas a validação precisa confirmar |

**O ganho estrutural:** esta é a primeira estratégia do portfólio que opera
**short por desenho** (vender o primeiro Lower High no topo de uma alta), e não
por simetria oportunista. A única tentativa dedicada de short até hoje
(`low2-m2s-short-v1`) reprovou com PF 0,75.

## 12. Plano de testes unitários

| # | Cenário | Esperado |
|---|---|---|
| 1 | Baixa + break + teste undershoot + barra de reversão | **Signal Long** |
| 2 | Alta + break + teste + barra de reversão bear | **Signal Short** |
| 3 | Sem estrutura de tendência | `NoTrendToReverse` |
| 4 | Tendência, mas sem fechamento além da trendline | `NoTrendlineBreak` |
| 5 | Break sem momentum (1 barra, sem cruzar EMA) | `BreakWithoutMomentum` |
| 6 | Break antigo demais | `BreakTooOld` |
| 7 | Break, mas preço não volta ao extremo | `NoExtremeTest` |
| 8 | Overshoot > 0,5 ATR | `ReversalNullified` |
| 9 | Teste válido, barra de sinal fraca | `WeakConfirmation` |
| 10 | Alvo 2R com stop largo → RR < 1,5 | `PoorRiskReward` |
| 11 | Fora da janela de horário | `OutsideTradingHours` |
| 12 | Trendline: 2 pivôs conhecidos → valor extrapolado esperado | teste direto do cálculo |

## 13. Métricas mínimas de aprovação

Padrão do gate A (ADR-010): backtest ≥ 6 meses com ≥ 50 trades, WR ≥ 40%,
PF ≥ 1,3, avg R > 0,15, DD ≤ 10%, mais walk-forward OOS ≥ 4 janelas positivas
de 6 nos ativos selecionados.

**Critério extra desta estratégia:** medir a sobreposição de sinais com a
`range-extreme-fade-v1`. Os contextos deveriam se excluir por construção (§11);
se não se excluírem, a premissa de complementaridade está errada.

## 14. Onde vive no código

```text
crates/trader-core/src/strategies/trendline_break_test_v1/
  mod.rs  config.rs  context.rs  setup.rs  entry.rs  tests.rs
config/strategies/trendline-break-test-v1.toml
crates/trader-core/src/strategies/mod.rs      +2 linhas
crates/trader-cli/src/dispatch.rs             +1 variante, +9 braços
crates/trader-cli/src/risk_config.rs          +1 impl From
crates/trader-domain/src/signals.rs           +6 RejectionReason
```

Componente novo reaproveitável: **cálculo e extrapolação de trendline por
pivôs** (`context.rs`), que hoje não existe no projeto.

## 15. Checklist de validação

```text
[x] Documentação da estratégia preenchida
[x] Regras objetivas definidas
[x] Especificação técnica completa
[x] Implementação
[x] Testes unitários passando (14/14)
[x] cargo clippy/fmt limpos
[x] Backtest executado (14 ativos, 17,5 meses) — variantes A e B
[ ] Walk-forward OOS — NÃO executado (amostra por ativo sem poder estatístico)
[ ] Sobreposição com range-extreme-fade — não medida (só faz sentido se aprovar)
[ ] Métricas mínimas — PF ✅, WR ✗, amostra por ativo ✗
[x] Versionada no git
```

## 16. Resultado da validação (2026-08-28) — PROMISSORA, mas amostra insuficiente

Backtest sobre o dataset do relatório de 2026-08-20 (`trader_compare`, 134.635
candles, 2025-02-21 → 2026-08-20, 14 ativos, comissão $0,35 + slippage 0,1%).

### Três correções de implementação durante o desenvolvimento

Todas encontradas pelos testes unitários, e nenhuma apareceria no backtest como
erro — apareceriam como "a estratégia quase não opera":

1. **A perna contrária destruía a estrutura que ela reverte.** `detect_trend`
   usava os dois últimos pivôs da janela; o rally que rompe a trendline cria um
   swing high novo e desfaz a sequência de lower highs. Corrigido: a estrutura é
   avaliada **até o extremo**, nunca incluindo a perna contrária.
2. **A barra de teste era eleita como extremo da tendência.** No caso de
   *overshoot* (Lower Low), que o Cap. 8 trata como metade dos casos válidos, o
   teste virava o mínimo da janela e o setup era descartado. Corrigido com recuo
   obrigatório para perna contrária + teste.
3. **Momentum medido no trecho errado.** A força da perna era medida do extremo
   até a barra que cruza a linha, exigindo ≥3 barras ANTES do cruzamento — o
   oposto do livro, onde rompimento rápido é mais força. Corrigido: momentum
   medido sobre a **primeira perna inteira**, do extremo ao ápice dela. Efeito:
   8 → 67 trades.

### Resultado

| Variante | trades | WR | PF | net |
|---|---|---|---|---|
| **A — tolerância do teste 0,3% (spec)** | **67** | 37,3% | **1,38** | **+$2.640** |
| B — tolerância 0,8% (calibração de frequência) | 250 | 31,2% | 0,92 | −$2.416 |

**A calibração de frequência falhou de forma reveladora.** Afrouxar a tolerância
quadruplicou a amostra e **inverteu o resultado**. Isso é evidência a favor da
regra do livro: o teste precisa ser *perto* do extremo antigo — com 0,8% entram
pullbacks que não são teste. A frequência baixa não é um parâmetro mal
calibrado; é a natureza do setup, que Brooks descreve como "major reversal".

### Por que não pode ser aprovada (critérios da §13 / ADR-010)

| Critério | Exigido | Variante A |
|---|---|---|
| Trades | ≥ 50 | 67 agregado, mas **2–9 por ativo** |
| Win rate | ≥ 40% | 37,3% |
| Profit factor | ≥ 1,3 | **1,38** ✅ |
| Max drawdown | ≤ 10% | ≤ 1,1% ✅ |

**Walk-forward não foi executado**: com ~4,8 trades por ativo em 17,5 meses,
cada janela OOS teria cerca de 1 trade. O teste não teria poder estatístico
nenhum e só somaria combinações à superfície.

### Situação: candidata retida, NÃO aprovada

É o melhor resultado das últimas cinco tentativas de estratégia — a única com PF
agregado acima de 1,3. Mas 67 trades distribuídos em 14 ativos não sustentam
nem seleção de ativo nem validação OOS, e o win rate fica abaixo do gate.

Diagnóstico de frequência (IWM, ~9.700 barras): `NoTrendlineBreak` 2.133 ·
`NoExtremeTest` 1.844 · `OutsideTradingHours` 1.672 · `NoTrendToReverse` 1.639 ·
`BreakWithoutMomentum` 1.511 · `BreakTooOld` 598 · `WeakConfirmation` 186 ·
`ReversalNullified` 74 → **11 sinais, 7 trades**. Nenhum filtro patológico: a
cadeia inteira é seletiva por desenho.

Caminhos possíveis, todos com custo, para decisão do dono:

1. **Ampliar o universo de ativos** (hoje 14). Aumenta a amostra agregada sem
   inventar histórico — mas aumenta a superfície de seleção.
2. **Ingerir mais histórico** de 15m da IBKR, se houver — o limite prático da
   API para intradiário costuma ser curto.
3. **Aceitar como candidata de acumulação forward**, como foi feito em 08-07 com
   as três estratégias de amostra < 50 (ver `docs/HANDOFF.md` §5 item 1). Aqui
   com uma ressalva importante: aquelas tinham walk-forward OOS; esta não tem.
4. **Arquivar** e voltar quando houver mais dados.

Nota de método: 2 configurações sobre 14 ativos = **28 combinações** somadas à
superfície de testes do projeto.
