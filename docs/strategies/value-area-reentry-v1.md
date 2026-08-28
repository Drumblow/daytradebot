# Estratégia: Value Area Reentry v1

**Id:** `value-area-reentry-v1`
**Versão:** 1.0.0
**Status:** implementada e REPROVADA no backtest (2026-08-28) — arquivada, ver §16
**Data:** 2026-08-28

---

## 1. Fonte

James F. Dalton, *Mind over Markets* — Cap. 4, "Special Situations — The
Value-Area Rule"; Cap. 2, "Organizing the Day" (definição de value area);
Apêndice 1, "TPO Value-Area Calculation" (algoritmo).

Extração prévia: `docs/books/analysis/dalton-mind-over-markets.md` §Setup B e §4
(tabela Subjetivo → Objetivo). O ranking daquele documento coloca este setup em
**2º lugar** entre as candidatas do livro, atrás apenas da
`balance-area-breakout-v1` (já em produção).

## 2. Conceito em uma frase

O mercado abre **fora** da área de valor do dia anterior, é rejeitado, **volta
para dentro dela e é aceito** — e quando isso acontece o leilão tende a
atravessar a área inteira até o lado oposto.

Citação central (Cap. 4): *"if price should be accepted (double TPO prints)
within the previous day's value area, there is a good possibility that the
market will auction completely through that value area."*

## 3. Fase 1 — Extração do conceito (8 perguntas)

| # | Pergunta | Resposta (do livro) |
|---|---|---|
| 1 | Nome do setup | Value-Area Rule |
| 2 | Contexto | Abertura fora da VA do dia anterior, seguida de retorno e aceitação dentro dela |
| 3 | Timeframe | Day timeframe (nosso operacional: 15m; contexto: diário) |
| 4 | Entrada | Aceitação dentro da VA ("double TPO prints"), na direção da travessia |
| 5 | Stop | Não explicitado no livro; a premissa invalida se o preço for rejeitado para **fora** da VA de novo — a borda de entrada é a referência lógica (**interpretação nossa**) |
| 6 | Alvo | Atravessar a VA **inteira** até o lado oposto — alvo estrutural, não múltiplo de R |
| 7 | Quando NÃO operar | Três filtros obrigatórios do autor: distância da abertura ao valor, largura da VA e direção do mercado |
| 8 | Estatísticas | Nenhuma. O autor é explícito: sem avaliar contexto, *"little better than a flip of a coin"* |

> **Nota de método:** a ausência de estatística no livro é o motivo de este setup
> entrar com os três filtros como **obrigatórios**, não opcionais. É a única
> defesa que a fonte oferece.

## 4. Value Area por proxy TPO (componente novo)

O livro define VA como *"the area where 70 percent of the day's business is
conducted"*. Não temos volume por preço; o **Apêndice 1 sanciona o cálculo por
tempo** (TPO). Adaptação para candles de 15m:

1. Tomar os candles RTH do **dia anterior completo** (26 barras de 15m).
2. Dividir o range do dia (`high − low`) em `va_buckets` faixas de largura igual
   (default 50). Largura mínima de faixa = `tick_size`.
3. Cada candle contribui **1 TPO** para toda faixa que seu `[low, high]` toca.
4. **POC** = faixa com mais TPOs (empate: a mais próxima do meio do range).
5. Expandir a partir do POC comparando a soma das **2 faixas acima** com a das
   **2 faixas abaixo**, incorporando o lado maior, até o acumulado cobrir
   ≥ `va_percent` (default 70%) do total de TPOs. É o algoritmo do Apêndice 1.
6. Saída: `va_high`, `va_low`, `poc`.

**Interpretações nossas (não estão no livro):** faixas de largura igual em vez
de níveis de tick (o livro usa o grid de ticks do pit); 15m em vez dos 30m do
TPO clássico — cada candle nosso é meio período do livro, o que torna a
contagem mais granular e não altera a forma da distribuição.

## 5. Setup de entrada

### 5.1 Pré-condição — abertura fora do valor

O **open do primeiro candle RTH de hoje** está fora da VA de ontem:

- `open > va_high` → viés de travessia **descendente** (short)
- `open < va_low` → viés de travessia **ascendente** (long)
- `va_low ≤ open ≤ va_high` → **sem setup** (`OpenInsideValueArea`)

### 5.2 Aceitação dentro do valor (o gatilho)

*"double TPO prints"* → **2 candles de 15m consecutivos fechando dentro de
`[va_low, va_high]`**, sendo o segundo o candle de sinal. Antes disso não há
setup (`NoValueAreaReentry`).

A direção da travessia é a do retorno: quem abriu abaixo do valor e voltou
opera **long** com alvo em `va_high`; quem abriu acima opera **short** com alvo
em `va_low`.

### 5.3 Os três filtros do autor (obrigatórios)

| Filtro | Citação | Regra objetiva |
|---|---|---|
| **Distância do valor** | *"The closer a market opens to the previous day's value area, the greater the chances of it penetrating and traveling through"* | `distância(open, borda mais próxima da VA) ≤ max_open_distance_atr × ATR_diário(14)` — senão `OpenTooFarFromValue` |
| **Largura da VA** | *"narrow value areas are more easily traversed than wider, high-volume value areas"* | `(va_high − va_low) ≤ max_va_width_atr × ATR_diário(14)` — senão `ValueAreaTooWide` |
| **Direção do mercado** | *"When price auctions up into value during a buying trend, the chances for continuation are much better than if the market were in a downward trend"* | inclinação da EMA20 na janela `trend_lookback` alinhada com a travessia (≥ 0 para long, ≤ 0 para short) — senão `TrendAgainstTraversal` |

## 6. Entrada, stop e alvo

| Elemento | Regra |
|---|---|
| **Entrada** | Ordem **stop** 1 tick além do extremo do candle de aceitação na direção da travessia (long: acima da máxima; short: abaixo da mínima). Convenção da ADR-009 — dá confirmação de que a travessia começou |
| **Stop** | 1 tick **fora** da borda da VA por onde entramos (long: abaixo de `va_low`; short: acima de `va_high`). É a invalidação literal da premissa: o preço foi rejeitado de volta para fora do valor |
| **Alvo** | **Borda oposta da VA** (`va_high` para long, `va_low` para short). Alvo estrutural — este é o único setup do portfólio que não usa múltiplo de R |
| **Validade da entrada** | `entry_validity_candles` (default 2) — se o gatilho não romper, cancela (`EntryExpired`) |

### 6.1 Guardas de risco

- `min_risk_reward` (default 1.2): como o alvo é estrutural, a relação R:R
  **varia por dia**. Se o stop ficar largo em relação à travessia restante, a
  operação é rejeitada com `PoorRiskReward`. É a guarda mais importante desta
  estratégia.
- `max_stop_atr`: stop mais largo que isso → `StopTooWide`.
- `StopWithinNoise`: stop a menos de 1 range médio de barra → rejeita.

## 7. Janela operacional

`trading_start_time` / `trading_end_time` em UTC, como nas irmãs. Default
**14:00–19:00 UTC (10h–15h ET)**:

- começa às 10h ET porque a aceitação exige 2 candles fechados e a abertura
  precisa ser conhecida;
- termina às 15h ET porque a travessia de uma VA inteira precisa de tempo — não
  faz sentido abrir posição às 15h50 com mandato de ficar flat no fechamento.

> ⚠️ Como nas outras estratégias, os horários são **UTC fixos por estação** e
> precisam ser ajustados na virada do DST.

## 8. Fase 2 — Tabela Subjetivo → Objetivo (consolidada)

| Conceito (livro) | Regra objetiva |
|---|---|
| "value area" | proxy TPO do dia anterior, 70% dos TPOs (§4) |
| "opens outside value" | `open` do 1º candle RTH fora de `[va_low, va_high]` |
| "accepted (double TPO prints)" | 2 candles de 15m consecutivos fechando dentro da VA |
| "auction completely through" | alvo = borda oposta da VA |
| "closer a market opens to value" | distância ≤ `max_open_distance_atr` × ATR diário |
| "narrow value areas" | largura ≤ `max_va_width_atr` × ATR diário |
| "during a buying trend" | inclinação da EMA20 alinhada com a travessia |
| "rejected back out of value" (stop) | 1 tick fora da borda de entrada |

## 9. Fase 3 — Especificação técnica

```text
Inputs:
  - candles 15m (série contínua, >= 15 dias para ATR diário + dia anterior)
  - parâmetros do TOML (config/strategies/value-area-reentry-v1.toml)

Outputs:
  - Signal { direction, entry_price, stop_price, target_price, market_snapshot }
  - ou Rejected { reason, details }

market_snapshot (auditoria):
  va_high, va_low, poc, va_width, va_width_atr_ratio,
  open_today, open_distance, open_distance_atr_ratio,
  acceptance_bar_index, ema_slope, daily_atr, direction

Estado interno: nenhum (a estratégia é pura; posição/risco são do core)

Evento que dispara: fechamento de candle de 15m
```

## 10. Rejeições registradas

Novas em `RejectionReason`:

| Variante | Quando |
|---|---|
| `OpenInsideValueArea` | abertura de hoje dentro da VA de ontem — sem setup |
| `NoValueAreaReentry` | não há 2 fechamentos consecutivos dentro da VA |
| `ValueAreaTooWide` | VA larga demais (filtro do autor) |
| `OpenTooFarFromValue` | abertura longe demais do valor (filtro do autor) |
| `TrendAgainstTraversal` | travessia contra a direção do mercado (filtro do autor) |

Reaproveitadas: `IncompleteSetup` (sem dia anterior completo / série curta),
`OutsideTradingHours`, `PoorRiskReward`, `StopTooWide`, `StopWithinNoise`,
`SetupInvalidated`, `EntryExpired`.

## 11. Complementaridade com o portfólio

| Estratégia em live | Contexto que opera | Sobreposição |
|---|---|---|
| `pullback-trend-v1` | continuação de tendência | nenhuma — rejeita range |
| `balance-area-breakout-v1` | rompimento de congestão | **oposta**: opera a saída do valor; nós operamos o retorno |
| `opening-reversal-v1` | primeira hora | pouca — nossa janela começa às 10h ET |
| `range-extreme-fade-v1` | extremos do dia em dia de range | **atenção**: ambas são reversão à média. Diferença: a range-fade opera o extremo do dia **corrente**, com referência intradiária; nós operamos a área de valor de **ontem**, com alvo estrutural do outro lado. Podem coincidir num dia de range estreito — a validação precisa medir isso |

> **Item de validação obrigatório:** medir a sobreposição de sinais com a
> `range-extreme-fade-v1` no backtest (mesmo ativo, mesmo dia). Se for alta, a
> estratégia não agrega e deve ser reprovada mesmo com métricas boas.

## 12. Plano de testes unitários (candles sintéticos)

| # | Cenário | Esperado |
|---|---|---|
| 1 | Abre abaixo da VA, 2 fechamentos dentro, EMA plana, VA estreita | **Signal Long**, alvo = `va_high`, stop < `va_low` |
| 2 | Abre acima da VA, 2 fechamentos dentro | **Signal Short**, alvo = `va_low` |
| 3 | Abre dentro da VA | `OpenInsideValueArea` |
| 4 | Abre fora, só 1 fechamento dentro | `NoValueAreaReentry` |
| 5 | VA larga (> `max_va_width_atr` × ATR) | `ValueAreaTooWide` |
| 6 | Abertura distante (> `max_open_distance_atr` × ATR) | `OpenTooFarFromValue` |
| 7 | Long com EMA20 inclinada para baixo | `TrendAgainstTraversal` |
| 8 | Alvo perto demais do stop | `PoorRiskReward` |
| 9 | Série sem dia anterior completo | `IncompleteSetup` |
| 10 | Fora da janela de horário | `OutsideTradingHours` |
| 11 | Cálculo da VA: distribuição conhecida → `va_high`/`va_low`/`poc` esperados | teste direto do algoritmo do Apêndice 1 |

## 13. Métricas mínimas de aprovação

Mesmo padrão das irmãs (ADR-010, gate A):

- backtest ≥ 6 meses, ≥ 50 trades no agregado
- win rate ≥ 40%, profit factor ≥ 1.3, avg R > 0.15, DD ≤ 10%
- walk-forward OOS ≥ 4 janelas positivas de 6 nos ativos selecionados
- **sobreposição com `range-extreme-fade-v1` < 30% dos sinais** (§11)

## 14. Onde vive no código

```text
crates/trader-core/src/strategies/value_area_reentry_v1/
  mod.rs        estrutura pública + trait Strategy + orquestração
  config.rs     parâmetros do TOML + config_hash
  context.rs    value area (TPO), ATR diário, filtros do autor, janela
  setup.rs      detecção da abertura fora + aceitação
  entry.rs      preços (entrada/stop/alvo) + construção do Signal
  tests.rs      os 11 cenários da secao 12
config/strategies/value-area-reentry-v1.toml
crates/trader-core/src/strategies/mod.rs      +2 linhas
crates/trader-cli/src/dispatch.rs             +1 variante, +9 braços
crates/trader-domain/src/signals.rs           +5 RejectionReason
```

Sem migration: `signals.rejection_reason` é `TEXT` sem constraint.

## 15. Checklist de validação

```text
[x] Documentação da estratégia preenchida
[x] Regras objetivas definidas
[x] Especificação técnica completa
[x] Implementação
[x] Testes unitários passando (13/13)
[x] cargo clippy/fmt limpos
[x] Backtest executado (14 ativos, 17,5 meses) — variantes A, B e C
[ ] Walk-forward OOS — NÃO executado (gate A reprova no in-sample)
[ ] Sobreposição com range-extreme-fade — não medida (só faz sentido se aprovar)
[ ] Métricas mínimas atingidas — REPROVADA (PF 0,76 / 0,84 / 0,89 nas 3 variantes)
[x] Versionada no git
```

## 16. Veredito da validação (2026-08-28) — REPROVADA na configuração especificada

Backtest sobre o mesmo dataset do relatório de 2026-08-20 (`trader_compare`,
134.635 candles, 2025-02-21 → 2026-08-20, 14 ativos, comissão $0,35 + slippage
0,1%). Duas variantes foram medidas.

### Variante A — stop na borda exata da VA (spec original)

| | trades | WR | PF | net |
|---|---|---|---|---|
| **Agregado 14 ativos** | 155 | 27,7% | **0,76** | **−$4.374** |

Melhores: IJS (12t, PF 1,51), IWO (12t, PF 1,44). Piores: VBR (2t, PF 0,00),
SCHA (5t, PF 0,34), IJR (9t, PF 0,37).

Diagnóstico: 112 stops × 43 alvos; RR planejado médio **3,80** mas realizado
**1,97** (ganho médio $315 ÷ perda média $160) — os setups de RR alto quase
nunca completam a travessia. RR máximo de **53,9** revelou entradas com stop de
2 ticks: a guarda de ruído original (`risk <= 1 tick`) era fraca demais.

### Variante B — stop com folga em ATR (calibração)

O livro **não especifica o stop** (§3, pergunta 5) — a borda exata foi
interpretação nossa e mostrou-se dentro do ruído. Adicionados
`stop_buffer_atr = 0,25` e `min_stop_atr = 0,15`.

| | trades | WR | PF | net |
|---|---|---|---|---|
| **Agregado 14 ativos** | 122 | 30,3% | **0,84** | **−$2.758** |

Melhorou em toda a distribuição (PF 0,76 → 0,84; prejuízo caiu 37%), e 5 ativos
passaram a positivo (IJS 2,10 · IWN 1,34 · SLYV 1,28 · QQQ 1,27 · IWO 1,10).
**Ainda assim reprova**: PF agregado < 1.

### Por que reprova (critérios da §13 / ADR-010)

| Critério | Exigido | Obtido (B) |
|---|---|---|
| Trades | ≥ 50 | 122 agregado, mas **≤ 11 por ativo** |
| Win rate | ≥ 40% | 30,3% |
| Profit factor | ≥ 1,3 | **0,84** |
| avg R | > 0,15 | negativo no agregado |

Somar só os 5 ativos positivos dá 50 trades e +$2.695 — mas escolher os
vencedores depois de ver o resultado é exatamente o viés de seleção que o
processo existe para evitar, e não há amostra OOS que o sustente. **Não foi feito
walk-forward**: o gate A falha ainda no in-sample, e rodá-lo só adicionaria
combinações à superfície de testes.

### O gargalo real: a entrada, não o setup

Contagem de rejeições em IWM (~9.700 barras): `NoValueAreaReentry` 4.122 ·
`OpenInsideValueArea` 2.325 · `OutsideTradingHours` 2.039 · `ValueAreaTooWide`
799 · `PoorRiskReward` 69 · `StopTooWide` 9 · `OpenTooFarFromValue` 4 ·
`TrendAgainstTraversal` 1.

Os filtros obrigatórios do autor **quase não mordem** (5 rejeições somadas). O
que limita a amostra é outra coisa: **29 sinais gerados em IWM produziram apenas
9 trades** — 69% das ordens de entrada nunca acionaram.

A causa é uma decisão nossa: a entrada é uma ordem **stop além do extremo da
barra de aceitação** (convenção da ADR-009), enquanto o livro define a entrada
como **a própria aceitação** dentro da VA. Exigir um rompimento adicional depois
da aceitação descarta a maioria dos casos e, quando aciona, piora o preço.

### Variante C — entrada na própria aceitação (executada 2026-08-28)

Autorizada pelo dono. `entry_order_type = "limit"`: a entrada passa a ser o
**fechamento da barra de aceitação**, leitura literal do Cap. 4, em vez de
esperar o rompimento do extremo dessa barra (convenção da ADR-009).

| | trades | WR | PF | net |
|---|---|---|---|---|
| **Agregado 14 ativos** | **306** | 27,5% | **0,89** | **−$3.907** |

A amostra **triplicou** (122 → 306) e o PF subiu de 0,84 para 0,89 — mas o
prejuízo aumentou, porque mais trades sobre expectativa negativa perdem mais.

**Este é o resultado que fecha o caso.** A objeção "amostra pequena demais para
concluir" deixa de valer: com 306 trades em 17,5 meses o agregado continua
abaixo de 1. A amostra maior **reforçou** o veredito em vez de resgatá-lo.

### A instabilidade entre variantes é a evidência final

| Ativo | PF variante B | PF variante C |
|---|---|---|
| SPY | 0,63 | **1,39** |
| QQQ | **1,27** | 0,86 |
| IJR | 0,25 | **1,20** |
| MDY | 0,59 | 0,40 |
| IJS | **2,10** | **1,66** |

Trocar apenas o mecanismo de entrada vira o sinal de vários ativos. Um edge real
não deveria depender disso. **Um único ativo (IJS) fica acima de PF 1,3 nas duas
variantes** — e com 14 ativos testados, um sobrevivente é o que se espera por
acaso.

### Situação

**ARQUIVADA.** Três configurações testadas, nenhuma com edge agregado. O código
permanece no repositório (como `failure-test-long-v1` e `low2-m2s-short-v1`),
registrado no `dispatch.rs`, com os 13 testes passando — mas **não vai para o
paper live**. O TOML fica com `entry_order_type = "stop"`, o default da ADR-009.

O que fica de aproveitável: o **cálculo de value area por proxy TPO**
(`context.rs::compute_value_area`, algoritmo do Apêndice 1, com teste unitário
sobre distribuição conhecida). É um componente reutilizável — a Value-Area Rule
falhou, mas value area como *filtro de contexto* para outras estratégias segue
disponível.

Nota de método: esta estratégia consumiu **3 configurações** (A, B e C) sobre 14
ativos, somando **42 combinações** à superfície de testes do projeto. Ver a
observação sobre risco de seleção em §13.
