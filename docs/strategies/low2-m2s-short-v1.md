# Estratégia: Low 2 / M2S Short v1

**ID:** `low2-m2s-short-v1`
**Status:** especificação (Fases 1–3 do framework) — aguardando teste de frequência → implementação
**Criada:** 2026-08-17
**Fonte:** Al Brooks, *Reading Price Charts Bar by Bar* (2009) — setup 2.9 da análise `docs/books/analysis/brooks-bar-by-bar.md`; espelho da `pullback-trend-v1` para tendências de baixa

---

## 1. Fonte

- **Cap. 4, "High/Low 1, 2, 3, and 4":** "the first bar with a low below the low of the prior bar is a Low 1... Subsequent occurrences are called Low 2, Low 3, and Low 4"; M2S = Low 2 em que as barras tocam a EMA — "particularly reliable (a two-legged pullback to the EMA in a trend is a great setup)".
- **Cap. 4:** "Although a High 2 and a Low 2 are common reversal setups, they should never be traded unless the prior High 1 or Low 1 broke a trendline" — a perna anterior precisa ter rompido ao menos uma micro trendline.
- **Cap. 15, "Pullbacks in a Strong Trend":** M2S "have a very high probability of success during a strong bear trend day".
- **Cap. 9, "Failed High/Low 2":** "A Low 2 setup is not enough reason to take a Countertrend trade in the absence of a prior strong trendline break. In fact, it will almost always fail and turn into a great With Trend entry" — ou seja: Low 2 **só com tendência de baixa estabelecida**.
- **Cap. 9, Fig. 9.5:** não operar Low 2 longe demais da EMA.
- **Cap. 10, "Entering on Stops":** entrada sell stop 1 tick abaixo da barra de sinal; stop 1 tick acima.

## 2. Conceito em uma frase

Em tendência de baixa clara, o preço faz um pullback de duas pernas até a EMA 20 (Low 2); se surge uma barra de sinal bear, vendemos a continuação da queda com stop acima da barra de sinal e alvo múltiplo do risco.

É o **espelho exato** da `pullback-trend-v1` (High 2 long) para o lado vendido — dobra a cobertura direcional do portfólio, que hoje é estruturalmente long-only em tendência (a opening-reversal faz shorts apenas táticos na primeira hora).

## 3. Fase 1 — Extração do conceito (8 perguntas)

1. **Nome:** Low 2 / M2S (moving average, second entry, sell).
2. **Contexto:** bear trend (ou correção de duas pernas até a EMA dentro de bear). Regra-mestre: só com tendência de baixa estabelecida (Cap. 9).
3. **Timeframe:** 5min no livro → 15min no bot.
4. **Entrada:** sell stop 1 tick abaixo da barra de sinal, após Low 2 no pullback à EMA; o Low 1 anterior deve ter rompido uma micro trendline do pullback (Cap. 4).
5. **Stop:** 1 tick acima da barra de sinal.
6. **Alvo:** novo extremo do bear; na v1, 2R fixo (mesma adaptação da irmã long ao bracket de TP único). Swing parcial em trend day é candidato de v2 ("you should swing part of every short", Cap. 5, Fig. 5.8).
7. **Quando NÃO operar:** Low 2 contra bull forte sem trendline break (Cap. 9); Low 2 longe demais da EMA (Fig. 9.5); após Low 4 falho; em dia de range (território da range-extreme-fade-v1).
8. **Estatísticas do autor:** M2S com probabilidade "muito alta" em strong bear trend days (Cap. 15); failed Low 2 tende a gerar Low 4 (duas pernas a mais).

## 4. Contexto de Mercado (filtro obrigatório) — espelho da pullback-trend-v1

### Timeframes

```text
Timeframe operacional: 15min
Timeframe de contexto: 1h
```

### Condições no timeframe de contexto (1h) — invertidas

```text
[1] Preço ABAIXO da EMA 20 por pelo menos 10 candles consecutivos
[2] Máximas e mínimas DESCENDENTES (lower highs e lower lows)
[3] Pelo menos 60% dos últimos 20 candles fecharam abaixo da EMA 20
```

### Condições no timeframe operacional (15min) — invertidas

```text
[4] Preço abaixo da EMA 20
[5] Último movimento de baixa criou nova mínima (lower low) nos últimos 20 candles
[6] Não houve candle de fechamento acima da EMA 20 nos últimos 10 candles
```

### Regras de rejeição de contexto

```text
REJEITAR se preço estiver acima da EMA 20 no 1h ou no 15min
REJEITAR se não houver sequência de lower highs/lower lows
REJEITAR se o mercado estiver em trading range apertado (Barb Wire — território da irmã fade)
REJEITAR se houver clímax de venda exagerado (3+ barras bear grandes consecutivas sem pullback)
```

## 5. Setup de Entrada (Low 2 Pullback) — espelho do High 2

### 5.1 Estrutura do pullback (invertida)

```text
[1] Pullback (subida corretiva) ocorre após impulso de baixa que fez nova mínima
[2] Pullback tem 2 a 6 candles (ideal: 3 a 5)
[3] A primeira perna para cima não quebra a última máxima de swing (mantém lower high)
[4] Entre as duas pernas, há uma pequena reação para baixo (mini-dip B)
[5] A segunda perna para cima forma um lower high em relação à primeira
[6] O pullback chega próximo à EMA 20 (toca ou fica até 0,3% acima/abaixo)
[7] Low 2: segunda barra do pullback com máxima abaixo da máxima da barra anterior...
    (contagem Low 1/Low 2 espelhada: Low 1 = primeira barra com high > high anterior
     dentro do pullback corretivo; Low 2 = a segunda ocorrência — ver nota §14)
```

> **Nota de contagem (interpretação):** na linguagem do livro, em bear trend a correção é para Cima e as contagens Low 1/Low 2 marcam as tentativas de retomada da queda dentro da correção. A forma operacional objetiva (espelho do nosso High 2): duas pernas de alta corretiva (com mini-dip entre elas) formando lower high, tocando a EMA, seguidas de barra de sinal bear.

### 5.2 Barra de sinal (reversal bar bear) — espelho da 5.1 da irmã

A barra de sinal deve ter **pelo menos 3 das 5 características** (invertidas):

```text
[1] corpo bear (close < open)
[2] fechamento no terço inferior do range
[3] sombra superior ≥ 1/3 do range (rejeição da EMA)
[4] corpo ≥ 30% do range
[5] pouca sobreposição com a barra anterior
```

## 6. Entrada, Stop e Alvo

- **Entrada (literal, Cap. 10):** sell stop = low(barra de sinal) − $0,01. Validade: 2 candles (ADR-009).
- **Stop (literal):** high(barra de sinal) + $0,01.
- **Alvo:** 2R fixo (adaptação ao bracket de TP único, mesma da irmã).
- **RR mínimo:** 1,5.
- **Guard anti-latência:** estrutural — entrada 1 tick além do extremo da barra de sinal.

## 7. Gestão de Risco

- Risco por trade: 1,0% (padrão). Limites globais: 2%/dia, 3 trades/dia, 3 perdas consecutivas, flat 15:30 ET.
- Direção: **short only** (infra simétrica desde a opening-reversal-v1; IBKR paper permite short em todos os ETFs do universo).
- Janela operacional: 09:45–15:15 ET (mesma das irmãs trend/range).

## 8. Fase 2 — Tabela Subjetivo → Objetivo (consolidada)

| Conceito (livro) | Regra objetiva (15min) | Origem |
|---|---|---|
| "bear trend" | espelho das condições §4 (EMA20 descendente, LH/LL, 60% closes abaixo) | Cap. 4/9 + interpretação |
| "two-legged pullback to the EMA" (M2S) | pullback de 2–6 candles com 2 pernas e lower high, a ≤ 0,3% da EMA20 | Cap. 4 + interpretação |
| "prior Low 1 broke a trendline" | a primeira perna do pullback superou a micro trendline da queda (proxy: rompeu a máxima da última perna de baixa) | Cap. 4 + interpretação |
| "barra de sinal bear" | 3 de 5 critérios de §5.2 | Cap. 5 |
| "stop 1 tick além da barra de sinal" | sell stop − $0,01; stop + $0,01 | literal, Cap. 10 |
| "não operar longe da EMA" | rejeitar se distância do pullback à EMA > 0,3% | Fig. 9.5 + interpretação |
| "novo extremo do bear" | alvo 2R fixo (v1) | adaptação (TP único) |

## 9. Fase 3 — Especificação Técnica

```text
Inputs:
  - candles 15min + candles de contexto 1h (resample)
  - EMA20 (15min e 1h), ATR14 (15min)
  - config da estratégia

Outputs:
  - Signal (short) | Rejected(RejectionReason, detalhes)
  - entrada (sell stop), stop, alvo 2R
  - snapshot: estrutura do pullback (pernas, lower highs), distância à EMA,
    métricas da barra de sinal, ATR, RR

Estado interno: nenhum (stateless)

Eventos: fechamento de candle 15min na janela 09:45–15:15 ET
```

## 10. Rejeições Registradas pelo Bot

Todas reutilizadas das irmãs: `NoContext`, `MarketLateral`, `IncompleteSetup`, `WeakConfirmation`, `StopWithinNoise`, `StopTooWide`, `PoorRiskReward`, `OutsideTradingHours`, `MaxTradesReached`, `DailyLossLimitReached`, `ConsecutiveLosses`, `HighVolatility`. Nenhuma nova (o espelho não introduz conceitos novos).

## 11. Filtros de Horário e Ativo

- Janela: 09:45–15:15 ET (13:45–19:15 UTC no DST; ajustar na virada).
- Ativos do pipeline: os 14 — com atenção ao padrão já medido duas vezes neste projeto: **edge em small-caps**; SPY/QQQ/MDY tendem a reprovar.

## 12. Métricas de Avaliação

Critérios do projeto (gate A, walk-forward OOS 6 janelas): ≥ 50 trades, WR ≥ 40%, PF ≥ 1,3, DD ≤ 10%, avg R > 0,15. **Teste de frequência antes da implementação** (passo formalizado em 08-17): se < ~1 setup/ativo/semana, recalibrar limiares medindo a distribuição — não intuir.

### 12.1 Teste de frequência — EXECUTADO 2026-08-17 ✅

Scanner de calibração (`target/tmp/frequency_scanner_low2.py`, aproximação dos filtros desta spec sobre os 133k candles do banco, 17,5 meses × 14 ativos):

- **83–134 sinais/ativo** (1,1–1,8/semana) — **3–4× a frequência da irmã long** no mesmo período;
- total 1.512 sinais (~20/semana agregado);
- distribuição uniforme entre os ativos (sem outliers mortos).

**Veredito do teste: frequência amplamente suficiente** — mesmo com fill rate de 50% nas entradas stop, passa de 50 trades/ativo no histórico. Nenhuma recalibração necessária; limiares herdados da irmã (0,3% da EMA, 2–6 candles de pullback) seguem como default.

## 13. Plano de Testes Unitários (candles sintéticos)

Espelho dos testes da `pullback-trend-v1`:

1. **Setup perfeito** (bear trend + pullback de 2 pernas à EMA + Low 2 + barra bear) → sinal short com preços corretos.
2. **Sem tendência de baixa** (EMA ascendente) → rejeição de contexto.
3. **Pullback não tocou a EMA** (> 0,3% de distância) → rejeição.
4. **Pullback com uma perna só** (sem Low 2) → rejeição.
5. **Barra de sinal fraca** (< 3 de 5 critérios) → `weak_confirmation`.
6. **Pullback quebrou a máxima de swing** (estrutura de baixa invalidada) → rejeição.
7. **Clímax de venda** (3+ barras bear grandes) → rejeição de contexto.
8. **Fora da janela** → `OutsideTradingHours`.
9. **RR < 1,5** (via config) → `poor_risk_reward`.
10. **Série insuficiente** → `IncompleteSetup`.

## 14. Decisões de Implementação

### Onde vive no código

```text
crates/trader-core/src/strategies/low2_m2s_short_v1/
  mod.rs / context.rs / setup.rs / entry.rs / config.rs
```

**Estratégia de implementação — máximo reuso sem tocar na irmã em produção:** a `pullback-trend-v1` está em live e não pode ser alterada (regra de ouro de versionamento). Duas opções:
- **(a) Módulo novo com lógica espelhada** (copiar e inverter comparações) — seguro, zero risco para a irmã, mas duplica código;
- **(b) Extrair o motor High 2/Low 2 para um núcleo parametrizado por direção** e fazer a irmã delegar — mais limpo, mas toca código em produção (exigiria bateria de regressão completa da pullback-trend-v1 antes de qualquer deploy).

**Decisão: (a) na v1.** A extração (b) fica como refatoração futura quando ambas estiverem estáveis em produção. Registrar em `dispatch.rs`, `risk_config.rs` e TOML próprio.

### Interpretações nossas (não estão no livro)

- Contagem operacional do Low 2 (§5.1 nota) — a forma em duas pernas com lower high é o espelho fiel do nosso High 2, não a contagem barra-a-barra literal do livro.
- "Micro trendline rompida" → proxy de rompimento da máxima da última perna de baixa.
- Distância máxima à EMA (0,3%) — mesmo limiar da irmã, a calibrar na medição.
- Alvo 2R fixo — swing parcial é v2.

## 15. Checklist de Validação

```text
[x] Fase 1 — extração do conceito com citações (este documento)
[x] Fase 2 — tabela subjetivo → objetivo (§8)
[x] Fase 3 — especificação técnica (§9)
[x] Teste de frequência (scanner no histórico) — **FEITO 2026-08-17 (§12.1): 83–134 sinais/ativo, 3–4× a irmã long. APROVADO sem recalibração**
[ ] Calibração dos limiares por distribuição medida
[ ] Implementação (Fase 4 — espelho da irmã, opção (a))
[ ] Testes unitários (§13) passando
[ ] Backtest 17,5 meses nos 14 ativos
[ ] Walk-forward OOS 6 janelas + veredito com critérios fixos
[ ] Paper live (novas instâncias) — só após aprovação
```

## 16. Veredito da validação

*(a preencher após o pipeline)*
