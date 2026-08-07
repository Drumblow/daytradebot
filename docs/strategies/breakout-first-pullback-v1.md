# Estratégia: Breakout — Primeiro Pullback v1

**ID:** `breakout-first-pullback-v1`
**Versão:** 1.0.0
**Status:** especificada — pronta para implementação
**Autor da especificação:** coding agent (a pedido do dono), 2026-08-06

---

## 1. Fonte

- **Livro:** Adam Grimes — *The Art and Science of Technical Analysis* (Wiley, 2012).
- **Capítulos:** Cap. 6 — "Breakouts, Entering on First Pullback Following" (template operacional); Cap. 5 (qualidade de breakouts: expansão de range/volume); Cap. 8 (stops e risco); Fig. 6.21 (reação que retrai o impulso = breakout falho).
- **Análise completa do livro:** `docs/books/analysis/grimes-art-science-ta.md` (seções 3.7 e 5.2).
- **Citação central:** *"It is really nothing more than a simple pullback trade."* — o setup é um pullback padrão, mas num contexto que a `pullback-trend-v1` não cobre: **o nascimento da tendência**, logo após o rompimento.

## 2. Conceito em uma frase

Após o rompimento válido (expansão de range e volume) de uma resistência testada várias vezes, o primeiro pullback controlado oferece entrada na nova tendência — com stop no pivô pré-breakout, porque o próprio nível rompido **não** é um bom stop.

## 3. Fase 1 — Extração do conceito (8 perguntas do framework)

| # | Pergunta | Resposta (com fonte) |
|---|----------|----------------------|
| 1 | Nome do setup | Breakout — First Pullback Following (Cap. 6) |
| 2 | Contexto | Breakout bem-sucedido de nível importante, com "boa atividade" (volume/volatilidade/price action) além do nível; o impulso inicial se exaure e o pullback começa (Cap. 6, template) |
| 3 | Timeframe | Qualquer (livro); aqui: 15min operacional |
| 4 | Sinais de entrada | Tratar como pullback padrão com gatilho padrão (Cap. 6) |
| 5 | Stop | Stops normais de pullback; referência de última instância = **pivô pré-breakout**, pois "the level is not a good stop" — continuações fortes voltam abaixo do nível (Cap. 6) |
| 6 | Alvo/saída | Alvos padrão de pullback; tendências nascidas de bons níveis "tend to be exceptional" — deixar espaço para o movimento maior (Cap. 6) |
| 7 | Quando NÃO operar | "Second breakout attempts" — restringir-se à **primeira** tentativa de rompimento do nível (Cap. 6); reação que retrai a maior parte do impulso = breakout falho (Fig. 6.21) |
| 8 | Estatísticas do autor | Nenhuma (o autor não publica win rate/R por setup; validação é 100% nossa) |

## 4. Contexto de Mercado (filtro obrigatório)

### Timeframes

- Operacional: 15min (candles OHLCV que chegam ~30s após o fechamento).
- Contexto: o próprio 15min — o nível, o breakout e o pullback são todos no operacional (não há timeframe maior nesta estratégia).

### Condições objetivas (15min)

1. **Nível R (resistência):** máxima de referência testada ≥ 2 vezes nos últimos 80 candles (tolerância de toque: 0,10% do preço). Interpretação nossa de "important level, clearly visible, tested multiple times".
2. **Breakout válido (barra B):** candle que fecha **acima de R + 0,25×ATR14** e ao mesmo tempo:
   - range(B) ≥ 1,5× range médio das últimas 20 barras;
   - volume(B) ≥ 1,5× volume médio das últimas 20 barras.
   (Literal: *"ranges of individual bars should expand"* + atividade além do nível, Cap. 5.)
3. **Janela de horário:** 09:45–15:30 ET (flat ao fim do dia; mesmo padrão das outras estratégias do bot).
4. **Sem clímax/volatilidade extrema:** ATR%14 ≤ 1,5% (limite global do RiskManager).

### Regras de rejeição de contexto

- Nível R não encontrado (menos de 2 toques em 80 candles).
- Breakout sem expansão (range ou volume abaixo de 1,5× a média).
- Breakout já consumido: qualquer breakout anterior do mesmo nível no dia (só a primeira tentativa vale).
- Fora da janela de horário.

## 5. Setup de Entrada (primeiro pullback)

### 5.1 O pullback

Após a barra de breakout B (que forma a máxima pós-breakout H):

- **Duração:** 2–6 candles após B com máximas descendentes ou fechamentos abaixo de H.
- **Controle (literal Fig. 6.21, limiar é interpretação nossa):** a retração do pullback deve ser ≤ 61,8% do impulso pós-breakout. Impulso = H − mínima da barra B. Retração = H − mínima do pullback até o momento.
- **Falha estrutural:** se o pullback fecha abaixo do pivô pré-breakout (maior mínima da base, ver §6), o breakout é considerado falho — sem trade.

### 5.2 Gatilho

- Primeira barra do pullback que **fecha acima do fechamento da barra anterior** vira a barra de sinal.
- Alternativa do próprio autor: rompimento de 2–3 máximas iguais no pullback — não usada na v1 (uma forma de gatilho basta; evita dupla contagem).

### 5.3 Regras de rejeição do setup

- Pullback com menos de 2 ou mais de 6 candles antes do gatilho.
- Retração > 61,8% do impulso.
- Pullback fechou abaixo do pivô pré-breakout (breakout falho).
- Segundo breakout do nível: qualquer barra anterior do lookback que tenha fechado acima do máximo das barras anteriores a ela (`breakout_already_taken`).

## 6. Entrada, Stop e Alvo

- **Entrada (adaptação nossa, semântica ADR-009):** buy stop 1 tick acima da máxima da barra de sinal, válida por 2 candles (`entry_validity_candles = 2`). O guard anti-latência das outras estratégias (`SetupInvalidated`) **não se aplica aqui por construção**: a barra de gatilho é sempre a última da série, então a entrada (máxima dela + 1 tick) está sempre acima do último fechamento; ordens stop que não rompem expiram por `entry_validity_candles`.
- **Stop (literal):** pivô pré-breakout − 1 tick. Pivô = a maior mínima da base nos 20 candles anteriores à barra B (interpretação nossa de "prebreakout pivot").
- **Sanidade do stop (Cap. 8):** rejeitar se distância entrada→stop < 1× range médio de barra (stop dentro do ruído) ou > 3×ATR14 (largo demais para day trade).
- **Risco-retorno:** rejeitar se RR < 1:1,5 (`min_risk_reward = 1,5`).
- **Alvo:** o **menor** entre:
  - **MMO (measured move objective):** H + (H − mínima da barra B) — projeção do impulso pós-breakout;
  - **2R** (duas vezes o risco inicial).
  (Interpretação nossa: o livro pede "deixar espaço para o movimento maior" com gestão por parciais; como nosso bracket tem TP único, o alvo conservador entre os dois preserva taxa de acerto.)
- **Sem saída ativa por tempo** na v1 (o livro não prescreve validação temporal para este setup — diferente do failure test).

## 7. Gestão de Risco

- Risco por trade: **1,0%** do equity (padrão do projeto; o "smaller size" do autor era específico do failure test).
- Limites globais do RiskManager: 2%/dia, 3 trades/dia, 3 perdas consecutivas, flat 15:30 ET.
- Sem reentrada: 1 breakout por nível por dia.

## 8. Fase 2 — Tabela Subjetivo → Objetivo (consolidada)

| Conceito subjetivo (livro) | Regra objetiva (15min OHLCV) | Origem |
|---|---|---|
| "important level, tested multiple times" | máxima testada ≥ 2× em 80 candles, tolerância 0,10% | interpretação |
| "successful breakout with good activity" | close > R + 0,25×ATR14; range ≥ 1,5× média(20); volume ≥ 1,5× média(20) | Cap. 5 (literal a expansão; limiares são interpretação) |
| "initial upthrust exhausts, pullback begins" | 2–6 candles com máximas descendentes ou closes < H | interpretação |
| "first reaction should be controlled" | retração ≤ 61,8% do impulso pós-breakout | Fig. 6.21 (limiar interpretação) |
| "standard pullback trigger" | buy stop na máxima da 1ª barra que fecha acima da anterior | Cap. 6 + adaptação stop (ADR-009) |
| "level is not a good stop; prebreakout pivot is" | stop = maior mínima da base (20 candles) − 1 tick | literal |
| "trends from good breakouts tend to be exceptional" | alvo = min(MMO, 2R) | interpretação (TP único) |
| "restrict to first breakout attempts" | 1 breakout por nível por dia | literal |

## 9. Fase 3 — Especificação Técnica

```text
Inputs:
  - candles 15min (janela ≥ 80)
  - ATR14, range médio(20), volume médio(20)
  - config da estratégia (todos os limiares acima)
  - estado do dia (nível já rompido hoje?)

Outputs:
  - Signal (long) | Rejected(RejectionReason, detalhes)
  - entrada (buy stop), stop (pivô − 1 tick), alvo (min(MMO, 2R))
  - snapshot auditável: R, toques, H, impulso, retração %, pivô, RR,
    range/volume da barra B vs médias

Estado interno:
  - nível R detectado e barra B (se houver) no dia
  - estágio: scanning → breakout_detected → in_pullback → triggered/done

Eventos:
  - fechamento de candle 15min
```

## 10. Rejeições Registradas pelo Bot

Reuso das existentes: `OutsideTradingHours`, `HighVolatility`, `SetupInvalidated`, `EntryExpired`, `StopMissing`, `MaxTradesReached`, `DailyLossLimitReached`, `ConsecutiveLosses`.

Novas (snake_case, a adicionar ao domínio):

- `level_not_found` — resistência sem ≥2 toques em 80 candles
- `weak_breakout` — breakout sem expansão de range ou volume
- `breakout_already_taken` — segundo breakout do mesmo nível no dia
- `pullback_too_deep` — retração > 61,8% do impulso
- `pullback_too_long` — pullback passou de 6 candles sem gatilho
- `breakout_failed` — fechamento abaixo do pivô pré-breakout
- `stop_within_noise` — stop < 1× range médio de barra
- `stop_too_wide` — stop > 3×ATR14
- `poor_risk_reward` — RR < 1:1,5

## 11. Filtros de Horário e Ativo

- Janela: 09:45–15:30 ET (13:45–19:30 UTC no horário de verão).
- Ativos: os 6 validados no pipeline (SPY, QQQ, IWM, IWN, IJR, MDY) — o veredito vem do backtest por ativo.
- Long-only na v1 (a versão short — breakdown de suporte — fica para v1.1 se a v1 mostrar edge; flag `allow_short` já prevista na infra).

## 12. Métricas de Avaliação

### Métricas mínimas para aprovação em backtest (walk-forward OOS, mesmo padrão do projeto)

```text
- ≥ 50 trades OOS
- win rate ≥ 40%
- profit factor ≥ 1.3
- drawdown máximo ≤ 10%
- avg R > 0.15
- expectativa positiva (net P&L > 0)
```

### Métricas para aprovação em paper trading

```text
- ≥ 20 trades
- métricas dentro de ±30% do backtest
- zero violações de risco
```

## 13. Plano de Testes Unitários (candles sintéticos)

1. **Setup perfeito** → gera sinal (nível 2 toques, breakout com expansão, pullback de 3 candles, gatilho, stop no pivô, alvo = min(MMO, 2R)).
2. **Nível com 1 toque só** → `level_not_found`.
3. **Breakout sem expansão de volume** → `weak_breakout`.
4. **Breakout sem expansão de range** → `weak_breakout`.
5. **Retração de 70% do impulso** → `pullback_too_deep`.
6. **Pullback de 8 candles** → `pullback_too_long`.
7. **Pullback fecha abaixo do pivô** → `breakout_failed`.
8. **Stop dentro do ruído / largo demais** → `stop_within_noise` / `stop_too_wide`.
9. **RR < 1:1,5** (via config `min_risk_reward = 3.0`) → `poor_risk_reward`.
10. **Segundo breakout do nível** (rompimento prévio no lookback) → `breakout_already_taken`.
11. **Pullback de 1 candle só** (gatilho logo após o breakout) → `incomplete_setup`. (O caso `SetupInvalidated` não existe nesta estratégia por construção — ver §6.)
12. **Fora de horário** → `OutsideTradingHours`.

## 14. Decisões de Implementação

### Onde vive no código

```text
crates/trader-core/src/strategies/breakout_first_pullback_v1/
  mod.rs      → struct + trait Strategy (máquina de estados do dia)
  context.rs  → nível R, barra de breakout (expansão), filtros de horário
  setup.rs    → pullback (duração, retração, pivô), barra de sinal
  entry.rs    → entrada stop, stop no pivô, alvo min(MMO, 2R), snapshot
  config.rs   → todos os limiares (Deserialize)
  tests.rs    → os 12 casos acima
```

Registro: `strategies/mod.rs`, `dispatch.rs`, `config/strategies/breakout-first-pullback-v1.toml`.

### Interpretações nossas (não estão no livro)

- Todos os limiares numéricos (0,25×ATR de penetração, 1,5× range/volume, 61,8%, 80 candles, 2–6 candles, 20 candles do pivô).
- Entrada buy stop (livro: gatilho de pullback genérico; adaptação idêntica à das outras estratégias).
- Alvo min(MMO, 2R) como substituto da gestão por parciais.

## 15. Checklist de Validação

```text
[x] Documentação da estratégia preenchida
[x] Regras objetivas definidas
[x] Especificação técnica completa
[x] Código revisado
[x] Testes unitários passando (12 casos)
[x] Backtest executado e relatório gerado
[ ] Métricas mínimas atingidas — **REPROVADA**
[x] Nenhuma violação de regra de segurança financeira
[ ] Versionada no git
```

## 16. Veredito da validação (2026-08-06) — ARQUIVADA

- Backtest 17,5 meses × 6 ativos (runs 75–80): **1–2 trades por ativo** no período inteiro (9 trades no total). A conjunção (nível de 80 candles + primeira tentativa + expansão + pullback controlado + stop no pivô) é rara demais em 15min de ETF — amostra estatística impossível.
- Iterações documentadas: calibração de expansão (range 1,5→1,2 após medir p75=1,20 na distribuição real; volume 1,5→1,1; stop 3→4×ATR) e 3 formulações da regra de "primeira tentativa" até a fiel ao livro (rompimento anterior à janela do nível). Mesmo calibrada, a frequência não viabiliza validação.
- **Decisão:** arquivada. Não vai ao live. Revisão só com mudança estrutural (ex.: operar em timeframe menor ou universo de ações individuais — fora do escopo atual).
