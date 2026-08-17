# Estratégia: Range Extreme Fade v1

**ID:** `range-extreme-fade-v1`
**Status:** especificação (Fases 1–3 do framework) — aguardando teste de frequência → implementação
**Criada:** 2026-08-17
**Fonte:** Al Brooks, *Reading Price Charts Bar by Bar* (2009) — setups 2.6 e 2.7 da análise `docs/books/analysis/brooks-bar-by-bar.md`

---

## 1. Fonte

- **Cap. 9, "Failed Higher High and Lower Low Breakouts":** "Most days are trading range days and offer many entries on failed swing high and low breakouts."
- **Cap. 10, "Selecting a Market":** ~80% dos dias são trading range days; as melhores 2–5 entradas do dia costumam ser "second entries in the form of reversals at new swing highs and lows on non-trending days".
- **Cap. 9:** "It is far more reliable if you wait for a signal bar with a strong bear close and beginners should restrict themselves to this type of fade setup."
- **Cap. 5, "Barb Wire":** veto cardinal — "Don't touch Barb Wire, or you will be hurt"; "never enter on a breakout" de tight range; regra da EMA ("never look to buy if the bars are mostly below the EMA, and never look to sell if the bars are mostly above the EMA").
- **Cap. 5, "Middle of the Day, Middle of the Range":** não operar no meio do dia E meio do range.
- **Cap. 10, "Entering on Stops" / "Protective Stops":** entrada stop 1 tick além da barra de sinal; stop 1 tick além do outro lado da barra de sinal.

## 2. Conceito em uma frase

Em dia de trading range (a maioria dos dias), o mercado rompe um swing high/low recente **sem momentum**, atraído por stops — e falha; entramos **contra** o rompimento (fade) com barra de sinal forte, mirando o retorno ao interior do range.

É a estratégia do contexto que o resto do portfólio **rejeita**: `pullback-trend-v1` exige tendência, `balance-area-breakout-v1` opera o rompimento *com* a direção, `opening-reversal-v1` só opera a primeira hora. Aqui operamos os dias "sem nada" — que são a maioria.

## 3. Fase 1 — Extração do conceito (8 perguntas)

1. **Nome:** Failed Higher High / Lower Low breakout fade ("Minor Reversal Scalps during Trading Range Days", Cap. 9) + veto de Barb Wire (Cap. 5).
2. **Contexto:** dia sem tendência (~80% dos dias, Cap. 10); swings de poucas barras a ~1h dentro de um range intradiário.
3. **Timeframe:** 5min no livro → 15min no bot (1 barra nossa ≈ 3 do livro).
4. **Entrada:** fade do rompimento falho de um swing high/low — sell stop 1 tick abaixo da barra de sinal bear (no topo) / buy stop 1 tick acima da barra de sinal bull (no fundo). Sempre com barra de sinal forte (Cap. 9: "beginners should restrict themselves to this type").
5. **Stop:** 1 tick além do extremo do novo high/low (o outro lado da barra de sinal).
6. **Alvo:** retorno ao interior do range — na v1, alvo fixo 1,5R–2R dentro do range (o livro opera scalp de volta ao interior; swing só com segunda entrada/wedge, fora da v1).
7. **Quando NÃO operar:** breakout com momentum forte (Cap. 9); meio do dia + meio do range (Cap. 5); Barb Wire puro sem falha (Cap. 5: "never enter on a breakout"); lado errado da EMA (regra da EMA, Cap. 5); dia de tendência sem trendline break.
8. **Estatísticas do autor:** ~80% dos dias são TR days (Cap. 10); "unusual for the second entry to fail" em fades de equilíbrio (Cap. 5); breakouts de tight ranges falham na maioria das vezes dentro de 1–2 barras (Cap. 5, "Tight Trading Ranges").

## 4. Contexto de Mercado (filtro obrigatório — o coração da estratégia)

Todos os critérios abaixo precisam ser verdade para o bot sequer procurar setup:

1. **Dia de range detectado (objetivo):**
   - EMA20 flat: inclinação média das últimas 12 barras < 0,05% do preço por barra (interpretação); **e**
   - sem estrutura de tendência: nenhuma sequência de 2+ HH/HL (ou LH/LL) por pivôs de 2 barras nas últimas 12 barras; **e**
   - range do dia até o momento < 1,5 × ATR14 diário médio (evita classificar trend day grande como range).
2. **Preço no extremo do range:** o candle atual fez **nova máxima ou mínima do dia** (ou superou um swing high/low de pivô-2 recente) por **pouco** — extensão além do nível anterior ≤ 0,3 × ATR14(15min) (rompimento "sem energia"). Extensão maior = possível breakout real → não é fade.
3. **Regra da EMA (literal, Cap. 5):** rejeitar **long** se a maioria dos últimos 8 candles fechou **abaixo** da EMA20; rejeitar **short** se a maioria fechou **acima**. (No fade, queremos operar de volta NA DIREÇÃO da EMA: short acima dela, long abaixo.)
4. **Veto meio-do-dia/meio-do-range (literal, Cap. 5):** rejeitar se horário entre 11:30–14:00 ET **E** preço no terço central do range do dia.
5. **Veto Barb Wire (Cap. 5):** se as últimas 3+ barras têm sobreposição ≥ 50% do range médio(14) e ao menos 1 doji (corpo < 30% do range) → rejeitar tudo (equilíbrio total: "don't touch Barb Wire").

## 5. Setup de Entrada

### 5.1 Barra de sinal (reversal bar forte contra o rompimento, Cap. 9 + Cap. 1)

- **Bear (fade de nova máxima → short):** close < open; fechamento no terço inferior do range; sombra superior ≥ 1/3 do range; corpo ≥ 30% do range.
- **Bull (fade de nova mínima → long):** simétrico — close > open; fechamento no terço superior; sombra inferior ≥ 1/3 do range; corpo ≥ 30% do range.

(São os mesmos critérios da família `opening-reversal-v1` — reutilizar o código da barra de sinal.)

### 5.2 Nível de referência (o "extremo" sendo rompido)

- **Nova máxima/mínima do dia**, ou
- **Swing high/low de pivô-2** (pivô com 2 barras de cada lado) dentro do dia, superado por ≤ 0,3 × ATR14(15min).

### 5.3 Regras de rejeição do setup

- Dia não é de range (filtro 4.1) → `not_a_range_day` (nova).
- Rompimento com extensão > 0,3 × ATR (momentum demais para fade) → `breakout_too_strong` (nova).
- Regra da EMA violada → `wrong_side_of_ema` (nova).
- Meio do dia + meio do range → `midday_midrange` (nova).
- Barb Wire detectado → `barb_wire` (nova).
- Barra de sinal fraca → `WeakConfirmation` (existente).
- Fora da janela → `OutsideTradingHours` (existente).

## 6. Entrada, Stop e Alvo

- **Entrada (literal, Cap. 10):** stop 1 tick além da barra de sinal — sell stop = low(sinal) − $0,01 no fade de topo; buy stop = high(sinal) + $0,01 no fade de fundo. Validade: 2 candles (mesmo padrão das irmãs; ADR-009).
- **Stop (literal):** 1 tick além do extremo oposto da barra de sinal (o extremo do rompimento falho).
- **Alvo (adaptação):** 1,5R fixo na v1 — o livro opera scalp "de volta ao interior do range"; como nosso bracket tem TP único, usamos RR fixo em vez do lado oposto do range (alvo estrutural é candidato de v2: `min(2R, lado oposto do range)`).
- **RR mínimo:** 1,2 (mais baixo que as irmãs porque o alvo de scalp de range é naturalmente menor; validar no backtest — se PF/avgR não fecharem, sobe para 1,5).
- **Guard anti-latência:** estrutural — a entrada fica 1 tick além do extremo da barra de sinal (mesma situação da opening-reversal-v1).

## 7. Gestão de Risco

- Risco por trade: 1,0% (padrão do projeto).
- Limites globais por processo: 2%/dia, 3 trades/dia, 3 perdas consecutivas, flat 15:30 ET.
- Direções: **long e short** (infra simétrica desde a opening-reversal-v1).
- Janela operacional: 09:45–15:15 ET (fora a primeira hora — território da opening-reversal — e o último trecho, para dar tempo do fade voltar ao range).

## 8. Fase 2 — Tabela Subjetivo → Objetivo (consolidada)

| Conceito (livro) | Regra objetiva (15min) | Origem |
|---|---|---|
| "dia de trading range" (~80% dos dias) | EMA20 flat (<0,05%/barra em 12) + sem 2+ HH/HL ou LH/LL em 12 barras + range do dia < 1,5×ATR diário | interpretação de Cap. 9/10 |
| "fade de novo swing high/low sem momentum" | nova máx/mín do dia (ou pivô-2) com extensão ≤ 0,3×ATR14 + barra de sinal contra | Cap. 9 + interpretação |
| "beginners devem esperar barra com fechamento forte contra" | barra de sinal obrigatória (critérios 5.1), sem exceção na v1 | literal, Cap. 9 |
| "meio do dia, meio do range: não operar" | veto 11:30–14:00 ET ∧ preço no terço central do range do dia | literal, Cap. 5 |
| "regra da EMA no Barb Wire" | long só se maioria dos últimos 8 closes abaixo da EMA20; short só se acima | literal, Cap. 5 |
| "Barb Wire: 3+ barras sobrepostas com doji" | veto: 3+ barras com sobreposição ≥ 50% do range médio(14) ∧ ≥1 doji (corpo < 30% do range) | literal, Cap. 5 |
| "stop 1 tick além da barra de sinal" | entrada stop ± $0,01; stop no outro extremo ∓ $0,01 | literal, Cap. 10 |
| "scalp de volta ao interior do range" | alvo 1,5R fixo (v1); lado oposto do range é v2 | adaptação (TP único) |

## 9. Fase 3 — Especificação Técnica

```text
Inputs:
  - candles 15min do dia corrente (precisa do dia inteiro até o candle atual)
  - EMA20 (15min), ATR14 (15min), ATR14 diário médio (proxy: média dos ranges diários na série)
  - pivôs de 2 barras (swing points intradiários)
  - config da estratégia

Outputs:
  - Signal (long|short) | Rejected(RejectionReason, detalhes)
  - entrada (stop), stop, alvo 1,5R
  - snapshot: extremo rompido, extensão do rompimento em ATR, inclinação da EMA,
    range do dia, métricas da barra de sinal, flags de cada veto

Estado interno: nenhum (stateless — tudo deriva da série a cada candle)

Eventos: fechamento de candle 15min na janela 09:45–15:15 ET
```

## 10. Rejeições Registradas pelo Bot

Reuso: `OutsideTradingHours`, `IncompleteSetup`, `WeakConfirmation`, `StopTooWide`, `PoorRiskReward`, `MaxTradesReached`, `DailyLossLimitReached`, `ConsecutiveLosses`, `HighVolatility`.

Novas (a adicionar ao domínio):

- `not_a_range_day` — filtro de contexto: dia com estrutura de tendência / EMA inclinada / range grande demais
- `breakout_too_strong` — extensão do rompimento > 0,3×ATR (momentum real, não é fade)
- `wrong_side_of_ema` — regra da EMA do Cap. 5 violada
- `midday_midrange` — veto meio do dia + meio do range
- `barb_wire` — equilíbrio total detectado (3+ barras sobrepostas com doji)

## 11. Filtros de Horário e Ativo

- Janela exclusiva: 09:45–15:15 ET (13:45–19:15 UTC no DST; ajustar na virada).
- Ativos do pipeline: os 14 com histórico validado (small-caps primeiro: IWM, IWN, IJR, IJS, VBR, AVUV, SLYV, SCHA, VB, IWO, IWV; depois QQQ/MDY/SPY só para confirmar o padrão "edge em small-caps").

## 12. Métricas de Avaliação

Critérios do projeto (gate A, walk-forward OOS 6 janelas): ≥ 50 trades, WR ≥ 40%, PF ≥ 1,3, DD ≤ 10%, avg R > 0,15, expectativa positiva. **Expectativa de frequência (a checar no teste de frequência ANTES da implementação completa):** sendo a estratégia dos ~80% de dias de range, espera-se sinal na maioria dos dias por ativo — se o scanner mostrar < 1 setup/ativo/semana, os filtros estão apertados demais e devem ser recalibrados medindo a distribuição real (lição das estratégias arquivadas).

### 12.1 Teste de frequência — EXECUTADO 2026-08-17 ✅

Scanner de calibração (`target/tmp/frequency_scanner.py`, aproximação dos filtros desta spec em Python sobre os 133k candles do banco, 17,5 meses × 14 ativos). Resultado por variante de filtros:

| Variante | Sinais totais | Por ativo (17,5m) | Sinais/sem agregado |
|---|---|---|---|
| A — base (ext ≤ 0,3×ATR, regra da EMA, corpo ≥ 30%) | 382 | 21–38 (~27) | ~5,1 |
| B — A com ext ≤ 0,5×ATR | 492 | ~35 | ~6,6 |
| C — B sem a regra da EMA | 725 | ~52 | ~9,7 |
| D — C com corpo ≥ 20% | 961 | ~69 | ~12,9 |

**Leitura:**
- A configuração base (A) cairia na **armadilha de amostra** das arquivadas (< 50 OOS/ativo) — o teste de frequência fez seu trabalho antes de uma linha de Rust.
- Com extensão 0,5×ATR e sem a regra da EMA estendida (C), a estratégia passa de ~50 sinais/ativo — viável por ativo; agregada, sobra.
- A regra da EMA do Cap. 5 é, no livro, específica do contexto **Barb Wire** — estendê-la a todo fade foi interpretação conservadora nossa. Decisão: implementar todos os limiares como **configuração TOML** e deixar o backtest decidir o ponto do trade-off frequência × qualidade (lição: medir, não intuir).
- Direções equilibradas com leve viés short (~2:1) — esperado: fades de topo em dias de range têm a gravidade da EMA a favor.

## 13. Plano de Testes Unitários (candles sintéticos)

1. **Setup perfeito short** (dia flat, nova máxima do dia por 0,1×ATR, bear reversal bar) → sinal short com entrada/stop/alvo corretos.
2. **Setup perfeito long** (nova mínima por 0,1×ATR, bull reversal bar) → sinal long.
3. **Dia de tendência** (EMA inclinada + HH/HL) → `not_a_range_day`.
4. **Rompimento forte** (extensão 0,6×ATR) → `breakout_too_strong`.
5. **Lado errado da EMA** (short com maioria dos closes acima da EMA) → `wrong_side_of_ema`.
6. **Meio do dia + meio do range** → `midday_midrange`; mesmo horário no extremo do range → passa.
7. **Barb Wire** (3 barras sobrepostas com doji) → `barb_wire`.
8. **Barra de sinal fraca** (doji no extremo) → `weak_confirmation`.
9. **Fora da janela** (sinal às 9:30 ou 15:30 ET) → `OutsideTradingHours`.
10. **RR < mínimo** (via config) → `poor_risk_reward`.
11. **Extensão exatamente 0,3×ATR** (borda) → sinal válido.
12. **Nova máxima do dia mas dia ainda cedo (9:45, range do dia pequeno)** → comportamento definido e testado (range do dia mínimo para operar — ver §14).

## 14. Decisões de Implementação

### Onde vive no código

```text
crates/trader-core/src/strategies/range_extreme_fade_v1/
  mod.rs       → struct + trait Strategy
  context.rs   → detector de dia de range (EMA flat, pivôs, range do dia) + vetos
  setup.rs     → extremo rompido, extensão, barra de sinal
  entry.rs     → entrada/stop/alvo + RR
  config.rs    → parâmetros (TOML em config/strategies/range-extreme-fade-v1.toml)
```
Registrar em `trader-cli/src/dispatch.rs` e no registry; config TOML própria.

### Reuso explícito

- Critérios de barra de sinal: **mesma família da `opening-reversal-v1`** (extrair para helper compartilhado se as duas ficarem idênticas — avaliar na implementação; não quebrar a irmã que está em produção).
- Pivôs de 2 barras: mesmo padrão usado nas demais estratégias.
- Shorts: infra pronta desde a opening-reversal-v1.

### Interpretações nossas (não estão no livro)

- Limiares numéricos de "dia de range" (inclinação da EMA, 12 barras, 1,5×ATR diário) — **calibrar medindo a distribuição real antes de fixar** (lição documentada: filtros por intuição falharam 2× neste projeto).
- Extensão máxima do rompimento (0,3×ATR) — idem.
- Alvo 1,5R fixo (o livro opera de volta ao interior do range / lado oposto) — candidato óbvio de v2.
- Janela 09:45–15:15 ET — o livro não restringe (além do veto do meio do dia); a janela é nossa adequação operacional.
- Range do dia mínimo para operar cedo (9:45): o detector de range precisa de histórico intradiário mínimo — definir na implementação (sugestão: só operar a partir da 3ª barra do dia e exigir range do dia ≥ 0,5×ATR diário para o alvo caber).

## 15. Checklist de Validação

```text
[x] Fase 1 — extração do conceito com citações (este documento)
[x] Fase 2 — tabela subjetivo → objetivo (§8)
[x] Fase 3 — especificação técnica (§9)
[x] Teste de frequência (scanner no histórico: setups/ativo/dia) — **FEITO 2026-08-17 (§12.1): viável com limiares calibrados (variante C+); base A seria rara demais**
[ ] Calibração dos limiares por distribuição medida
[ ] Implementação (Fase 4)
[ ] Testes unitários (§13) passando
[ ] Backtest 17,5 meses nos 14 ativos
[ ] Walk-forward OOS 6 janelas + veredito com critérios fixos
[ ] Paper live (novas instâncias na VM) — só após aprovação
```

## 16. Veredito da validação

*(a preencher após o pipeline)*
