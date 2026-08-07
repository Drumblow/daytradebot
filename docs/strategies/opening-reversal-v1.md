# Estratégia: Opening Reversal v1

**ID:** `opening-reversal-v1`
**Versão:** 1.0.0
**Status:** especificada — pronta para implementação
**Autor da especificação:** coding agent (a pedido do dono), 2026-08-06

---

## 1. Fonte

- **Livro:** Al Brooks — *Reading Price Charts Bar by Bar* (Wiley, 2009).
- **Capítulos:** Cap. 11 — "Opening Patterns and Reversals" / "Patterns Related to Yesterday"; Cap. 1 — "Signal Bars" e "Second Entries"; Cap. 10 — "Protective Stops".
- **Análise completa do livro:** `docs/books/analysis/brooks-bar-by-bar.md` (setup 2.1, tabela 3.1).
- **Citação central:** *"Breakouts, Failed Breakouts, and Breakout Pullbacks from yesterday's swing highs and lows, large flags, trendlines, or trading ranges are the most reliable patterns in the first hour"* (Cap. 11).

## 2. Conceito em uma frase

Na primeira hora, quando o mercado testa a máxima ou mínima do dia anterior e falha (barra de reversão forte), entramos contra o teste — essas reversões "frequentemente formam a máxima ou a mínima do dia" (Cap. 11).

## 3. Fase 1 — Extração do conceito (8 perguntas)

| # | Pergunta | Resposta (com fonte) |
|---|----------|----------------------|
| 1 | Nome | Opening Reversal / Failed Breakout de high/low de ontem (Cap. 11) |
| 2 | Contexto | Primeira hora; teste (ou rompimento com gap) da máxima/mínima do dia anterior |
| 3 | Timeframe | 5min no livro; aqui 15min (1 barra nossa ≈ 3 dele) |
| 4 | Entrada | Barra de reversão forte na zona de teste; stop 1 tick além da barra de sinal (Cap. 10) |
| 5 | Stop | 1 tick além do extremo oposto da barra de sinal; se risco grande, "risk about 60% of the height of the signal bar" (Fig. 10.19) |
| 6 | Alvo/saída | Pode ser o extremo do dia → swing; parcial em 2–3× risco inicial (Cap. 11). Nossa adaptação: alvo único 2R |
| 7 | Quando NÃO operar | Sem barra de sinal forte; momentum contra muito forte (4+ barras contra → segunda entrada preferível, Cap. 1); breakout de opening range sem falha |
| 8 | Estatísticas do autor | Um extremo do dia costuma se formar na primeira hora (Cap. 11) |

## 4. Contexto de Mercado (filtro obrigatório)

1. **Janela:** 09:30–10:30 ET — os 4 primeiros candles de 15min do dia (13:30–14:30 UTC no horário de verão). A barra de gatilho deve estar nessa janela.
2. **Zona de teste:** o candle corrente toca ou cruza a **máxima do dia anterior** (setup short) ou a **mínima do dia anterior** (setup long). "Zona" = até 0,3% além do nível (interpretação).
3. **Níveis de ontem:** computados dos candles do dia anterior (data em America/New_York) presentes na própria série.
4. **Veto de momentum (Cap. 1):** rejeitar se os últimos 2 candles fecharam além da zona com corpos > 60% do range — contra-movimento forte demais para fade direto (o livro pediria segunda entrada; a v1 rejeita, conservador).
5. **Veto de barras contra:** 4+ das últimas 6 barras são trend bars contra a direção pretendida → rejeitar (o livro preferiria segunda entrada).

## 5. Setup de Entrada

### 5.1 Barra de sinal (reversal bar forte, Cap. 1)

- **Bull (long na mínima de ontem):** close > open; fechamento no terço superior do range; sombra inferior ≥ 1/3 do range; range não-doji (corpo ≥ 30% do range).
- **Bear (short na máxima de ontem):** simétrico — close < open; fechamento no terço inferior; sombra superior ≥ 1/3 do range; corpo ≥ 30% do range.

### 5.2 Regras de rejeição do setup

- Candle não tocou a zona do nível de ontem (`yesterday_level_not_tested`).
- Barra de sinal fraca (não atende 5.1) → `weak_confirmation`.
- Veto de momentum ou de barras contra → `momentum_against`.
- Fora da janela 09:30–10:30 ET → `OutsideTradingHours`.
- Sem dados do dia anterior na série → `IncompleteSetup`.

## 6. Entrada, Stop e Alvo

- **Entrada (literal, Cap. 10):** stop 1 tick além da barra de sinal — buy stop = high + $0,01 (long); sell stop = low − $0,01 (short). Validade: 2 candles (ADR-009).
- **Stop (literal):** 1 tick além do extremo oposto da barra de sinal. **Exceção do livro (Fig. 10.19):** se o risco > 1,5×ATR14(15min), usar stop monetário = 60% do range da barra de sinal a partir da entrada.
- **Alvo:** 2R (adaptação do "parcial em 2–3× risco" ao bracket de TP único).
- **RR mínimo:** 1,5.
- **Guard anti-latência:** entrada é sempre 1 tick além do extremo da última barra → estruturalmente além do último fechamento; o caso `SetupInvalidated` não ocorre (mesma situação da breakout-first-pullback).

## 7. Gestão de Risco

- Risco por trade: 1,0% (padrão do projeto).
- Limites globais: 2%/dia, 3 trades/dia, 3 perdas consecutivas, flat 15:30 ET (brackets cancelados/fechados pelo broker ao fim do dia como nas demais).
- Direções: **long e short** (primeira estratégia do bot com short — o simulador, o RiskManager (RR em valor absoluto) e o bracket do domínio já são simétricos).

## 8. Fase 2 — Tabela Subjetivo → Objetivo (consolidada)

| Conceito (livro) | Regra objetiva (15min) | Origem |
|---|---|---|
| "primeira hora" | 4 primeiros candles do dia (09:30–10:30 ET) | Cap. 11 |
| "teste da máxima/mínima de ontem" | candle toca ou cruza o nível, zona de 0,3% | interpretação |
| "barra de reversão forte" | critérios de 5.1 (terço, sombra ≥ 1/3, corpo ≥ 30%) | Cap. 1 |
| "stop 1 tick além da barra de sinal" | stop entry ± $0,01 | literal |
| "risk about 60% of the height of the signal bar" | se risco > 1,5×ATR: stop = entrada ∓ 60% do range da sinal | literal (Fig. 10.19) |
| "4+ trend bars contra → segunda entrada" | rejeitar (conservador) | Cap. 1 |
| "swing: pode ser o extremo do dia" | alvo único 2R | adaptação (TP único) |

## 9. Fase 3 — Especificação Técnica

```text
Inputs:
  - candles 15min (precisa cobrir o dia anterior + o dia atual)
  - ATR14 (15min)
  - config da estratégia

Outputs:
  - Signal (long|short) | Rejected(RejectionReason, detalhes)
  - entrada (stop), stop, alvo 2R
  - snapshot: high/low de ontem, zona tocada, métricas da barra de sinal,
    ATR, RR, flags de veto

Estado interno: nenhum (stateless — níveis derivados da série a cada candle)

Eventos: fechamento de candle 15min dentro da janela 09:30–10:30 ET
```

## 10. Rejeições Registradas pelo Bot

Reuso: `OutsideTradingHours`, `IncompleteSetup`, `WeakConfirmation`, `StopTooWide`, `PoorRiskReward`, `MaxTradesReached`, `DailyLossLimitReached`, `ConsecutiveLosses`, `HighVolatility`.

Novas (a adicionar ao domínio):

- `yesterday_level_not_tested` — candle não tocou a zona do nível de ontem
- `momentum_against` — momentum contra demais (veto de 2 barras ou 4/6 trend bars)

## 11. Filtros de Horário e Ativo

- Janela exclusiva: 09:30–10:30 ET (13:30–14:30 UTC no DST; 14:30–15:30 UTC fora do DST — ajustar na virada).
- Ativos: os 6 do pipeline (SPY, QQQ, IWM, IWN, IJR, MDY).

## 12. Métricas de Avaliação

Mesmos critérios do projeto: ≥ 50 trades OOS, WR ≥ 40%, PF ≥ 1.3, DD ≤ 10%, avg R > 0.15, expectativa positiva (walk-forward 6 janelas).

## 13. Plano de Testes Unitários (candles sintéticos)

1. **Setup perfeito long** (teste da mínima de ontem + bull reversal bar) → sinal long com preços corretos.
2. **Setup perfeito short** (teste da máxima de ontem + bear reversal bar) → sinal short.
3. **Sem toque no nível** → `yesterday_level_not_tested`.
4. **Barra de sinal fraca** (doji/corpo pequeno) → `weak_confirmation`.
5. **Momentum contra** (2 barras fortes além da zona) → `momentum_against`.
6. **4+ trend bars contra** → `momentum_against`.
7. **Fora da janela** (sinal às 11h ET) → `OutsideTradingHours`.
8. **Sem dia anterior na série** → `IncompleteSetup`.
9. **Stop monetário 60%** (barra de sinal gigante, risco > 1,5×ATR) → stop = entrada − 60% do range.
10. **RR < 1,5** (via config) → `poor_risk_reward`.
11. **Toque apenas na zona de 0,3% sem cruzar** → sinal válido (borda da zona).
12. **Gap grande acima da máxima de ontem sem falha** → sem sinal (sem barra de reversão).

## 14. Decisões de Implementação

### Onde vive no código

```text
crates/trader-core/src/strategies/opening_reversal_v1/
  mod.rs      → struct + trait Strategy
  context.rs  → níveis de ontem (dia ET via chrono-tz), janela, vetos
  setup.rs    → zona de teste + barra de sinal (5.1)
  entry.rs    → entrada/stop/alvo (incl. regra dos 60%), build_signal
  config.rs   → parâmetros (Deserialize)
  tests.rs    → os 12 casos acima
```

### Interpretações nossas (não estão no livro)

- Zona de teste de 0,3%; janela estrita de 4 candles; vetos transformados em rejeição (o livro preferiria segunda entrada — candidata a v1.1); alvo 2R; conversão 5min→15min.

## 15. Checklist de Validação

```text
[x] Documentação da estratégia preenchida
[x] Regras objetivas definidas
[x] Especificação técnica completa
[x] Código revisado
[x] Testes unitários passando (12 casos)
[x] Backtest executado e relatório gerado
[~] Métricas mínimas atingidas — **APROVADA EM QUALIDADE em 3 ativos; amostra insuficiente**
[x] Nenhuma violação de regra de segurança financeira
[ ] Versionada no git
```

## 16. Veredito da validação (2026-08-06) — APROVADA PARA ACUMULAR AMOSTRA (small-caps)

- Backtest 17,5 meses × 6 ativos (runs 81–86): positiva nos 6, com 26–34 trades/ativo.
- **Walk-forward OOS 6 janelas (runs 87–92):**
  - **IWM: 27t, WR 51.9%, PF 1.70, avgR 0.547, DD 2.31% — passa todos exceto amostra**
  - **IWN: 24t, WR 50.0%, PF 1.87, avgR 0.491, DD 1.14% — passa todos exceto amostra**
  - **IJR: 30t, WR 46.7%, PF 1.83, avgR 0.391, DD 1.19% — passa todos exceto amostra**
  - SPY (PF 0.75), QQQ (PF 1.18, avgR -0.08), MDY (PF 1.07, avgR -0.05): reprovam.
- Padrão: o edge vive em small-caps (como a pullback-trend-v1). OOS agregado IWM+IWN+IJR: 81 trades.
- **Expansão de ativos (2026-08-06, 8 novos testados):** aprovada em qualidade também em **VB (23t OOS, PF 1.75, avgR 0.425)** e **SLYV (21t, 1.62, 0.418)**. Reprovada em IJS, VBR, AVUV, IWO, IWV. Mapa final de qualidade OOS: **IWM, IWN, IJR, VB, SLYV** (amostra agregada 125 trades).
- **Decisão:** candidata forte. Para fechar o critério de amostra: mais histórico ou aceitar amostra agregada dos 5 ativos — decisão do dono. Não vai ao live antes disso (framework Fase 7).
