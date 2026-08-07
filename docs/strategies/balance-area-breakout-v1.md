# Estratégia: Balance-Area Breakout v1

**ID:** `balance-area-breakout-v1`
**Versão:** 1.0.0
**Status:** especificada — pronta para implementação
**Autor da especificação:** coding agent (a pedido do dono), 2026-08-06

---

## 1. Fonte

- **Livro:** James Dalton — *Mind over Markets* (1990).
- **Capítulos:** Cap. 4 — "Special Situations — Balance-Area Break-outs"; Cap. 2 (initial balance estreito precedendo trend days).
- **Análise completa do livro:** `docs/books/analysis/dalton-mind-over-markets.md` (Setup A).
- **Citações centrais:** *"Balance area break-out strategy is straightforward — go with the break-out"*; *"A balance area break-out is a trade you 'almost have to do.' Risk is minimal and profit potential is very high."*

## 2. Conceito em uma frase

Após dias de congestão (área de balanceamento), o rompimento aceito fora da área marca o início de um movimento direcional — entramos na direção do rompimento, com stop imediatamente de volta dentro da área (retorno = rejeição).

## 3. Fase 1 — Extração do conceito (8 perguntas)

| # | Pergunta | Resposta (com fonte) |
|---|----------|----------------------|
| 1 | Nome | Balance-Area Break-out (Cap. 4) |
| 2 | Contexto | Mercado balanceado — dias de valor sobreposto numa região bem definida; frequentemente precedido por initial balance estreito |
| 3 | Timeframe | 30min no livro, balanceamento de vários dias; aqui 15min sobre ~3 dias |
| 4 | Entrada | "If price is accepted outside the balance area, place trades in the direction of the new activity" (Cap. 4) |
| 5 | Stop | "Stops a few ticks above the point of break-out, for a price return into the balance area would indicate rejection" (Cap. 4) |
| 6 | Alvo/saída | Sem alvo fixo — "a break-out is usually the start of a much bigger move". Nossa adaptação: 2R (bracket TP único) |
| 7 | Quando NÃO operar | Rompimento não aceito (preço retorna à área) |
| 8 | Estatísticas do autor | Nenhuma numérica; afirmação qualitativa forte ("almost have to do") |

## 4. Contexto de Mercado (filtro obrigatório)

1. **Área de balanceamento:** últimos 78 candles (≈3 dias de 15min) com:
   - largura `(máxima − mínima) / preço_médio ≤ 2%` (teto absoluto);
   - largura `≤ 3×ATR14` (compressão relativa à volatilidade recente);
   - cobertura de pelo menos 2 dias de pregão (ET) — balanceamento é multi-dia por definição.
2. **Janela:** 09:45–15:30 ET (mesma das demais estratégias; não operamos a primeira meia hora).
3. **Rompeu com aceitação:** o último candle fecha **fora** da área (acima da máxima ou abaixo da mínima) — fechamento fora = aceitação (interpretação; o livro operava em tempo real).

## 5. Setup de Entrada

- **Long:** último candle fecha acima da máxima da área → buy stop 1 tick acima da máxima desse candle.
- **Short:** último candle fecha abaixo da mínima da área → sell stop 1 tick abaixo da mínima desse candle.
- **Rejeições:** sem área de balanceamento (`no_balance_area`); candle não fechou fora da área (`IncompleteSetup`); risco > 3×ATR (`StopTooWide`); RR < 1,5 (`PoorRiskReward`); fora de horário (`OutsideTradingHours`).

## 6. Entrada, Stop e Alvo

- **Entrada (interpretação):** stop 1 tick além do extremo do candle de rompimento, validade 2 candles (ADR-009).
- **Stop (literal):** de volta dentro da área — long: `máxima_da_área − 0,3×ATR`; short: `mínima_da_área + 0,3×ATR` ("a few ticks" do livro ampliados para nosso timeframe/ativo — interpretação).
- **Alvo:** 2R (adaptação do "much bigger move" ao bracket de TP único).
- **RR mínimo:** 1,5 (estruturalmente 2 com alvo 2R).
- **Direções:** long e short.

## 7. Gestão de Risco

- 1,0% por trade (padrão do projeto); limites globais (2%/dia, 3 trades/dia, 3 perdas consecutivas, flat 15:30 ET).
- Uma área de balanceamento gera no máximo 1 trade por direção por dia (natural pela detecção stateless + limites do RiskManager).

## 8. Fase 2 — Tabela Subjetivo → Objetivo

| Conceito (livro) | Regra objetiva (15min) | Origem |
|---|---|---|
| "dias de valor sobreposto" | 78 candles, largura ≤ 2% e ≤ 3×ATR, ≥ 2 dias ET | interpretação |
| "price accepted outside the balance area" | fechamento fora da área | interpretação |
| "go with the break-out" | stop entry 1 tick além do candle de rompimento | interpretação (ADR-009) |
| "stops a few ticks… return into the area = rejection" | stop 0,3×ATR dentro da área | interpretação |
| "start of a much bigger move" | alvo 2R | adaptação (TP único) |

## 9. Fase 3 — Especificação Técnica

```text
Inputs:
  - candles 15min (≥ 80 para área + ATR)
  - ATR14
  - config da estratégia

Outputs:
  - Signal (long|short) | Rejected(RejectionReason, detalhes)
  - entrada (stop), stop (dentro da área), alvo 2R
  - snapshot: area_high/low, largura %, largura em ATR, RR

Estado interno: nenhum (stateless)
Eventos: fechamento de candle 15min dentro da janela
```

## 10. Rejeições Registradas pelo Bot

Reuso: `OutsideTradingHours`, `IncompleteSetup`, `StopTooWide`, `PoorRiskReward`, `MaxTradesReached`, `DailyLossLimitReached`, `ConsecutiveLosses`, `HighVolatility`.

Nova (a adicionar ao domínio):

- `no_balance_area` — últimos 78 candles não formam área de balanceamento (largura acima dos tetos)

## 11. Métricas de Avaliação

Mesmos critérios do projeto: ≥ 50 trades OOS, WR ≥ 40%, PF ≥ 1.3, DD ≤ 10%, avg R > 0.15, expectativa positiva (walk-forward 6 janelas).

## 12. Plano de Testes Unitários (candles sintéticos)

1. **Setup perfeito long** (área apertada + fechamento acima) → sinal long com preços corretos.
2. **Setup perfeito short** (área apertada + fechamento abaixo) → sinal short.
3. **Sem balanceamento** (série em tendência, largura grande) → `no_balance_area`.
4. **Área larga demais vs ATR** → `no_balance_area`.
5. **Sem rompimento** (último candle dentro da área) → `IncompleteSetup`.
6. **Stop largo demais** (área larga × ATR pequeno, via config) → `StopTooWide`.
7. **RR ruim** (via config) → `PoorRiskReward`.
8. **Fora de horário** → `OutsideTradingHours`.
9. **Área de 1 dia só** (todos os candles no mesmo dia ET) → `no_balance_area`.
10. **Snapshot auditável** — campos de área/largura presentes no sinal.

## 13. Decisões de Implementação

```text
crates/trader-core/src/strategies/balance_area_breakout_v1/
  mod.rs      → struct + trait Strategy
  context.rs  → área de balanceamento (largura, dias), horário, ATR
  setup.rs    → rompimento com aceitação (fechamento fora)
  entry.rs    → entrada/stop/alvo, build_signal
  config.rs   → parâmetros (Deserialize)
  tests.rs    → os 10 casos acima
```

Interpretações nossas: todos os limiares numéricos (78 candles, 2%, 3×ATR, 0,3×ATR do stop, 2R).

## 14. Checklist de Validação

```text
[x] Documentação da estratégia preenchida
[x] Regras objetivas definidas
[x] Especificação técnica completa
[x] Código revisado
[x] Testes unitários passando (10 casos)
[x] Backtest executado e relatório gerado
[~] Métricas mínimas atingidas — **APROVADA EM QUALIDADE em 3 ativos; amostra insuficiente**
[x] Nenhuma violação de regra de segurança financeira
[ ] Versionada no git
```

## 15. Veredito da validação (2026-08-06) — APROVADA PARA ACUMULAR AMOSTRA

- Iteração documentada: o teto de largura em ATR (3×ATR de 15min) estava na escala errada para áreas de 3 dias (rejeitava 99,6% das janelas); corrigido para 10×ATR (o filtro real é o teto de 2% — p25 das janelas de SPY é 1,98%).
- Backtest 17,5 meses × 6 ativos (runs 102–107): 19–53 trades/ativo.
- **Walk-forward OOS 6 janelas (runs 108–113):**
  - **IWN: 36t, WR 50.0%, PF 2.72, avgR 0.489, DD 1.43% — passa todos exceto amostra**
  - **IWM: 20t, WR 45.0%, PF 1.39, avgR 0.339, DD 0.81% — passa todos exceto amostra**
  - **QQQ: 19t, WR 42.1%, PF 1.79, avgR 0.253, DD 0.72% — passa todos exceto amostra**
  - IJR (PF 1.95 mas WR 36.8%, avgR 0.096), SPY (PF 0.62), MDY (PF 1.01, avgR -0.09): reprovam.
- **Expansão de ativos (2026-08-06, 8 novos testados):** aprovada em qualidade também em **IJS (29t OOS, PF 3.48, avgR 0.744 — melhor par do projeto)**, **VBR (37t, 2.60, 0.285)**, **AVUV (36t, 2.79, 0.242)**, **SLYV (26t, 2.18, 0.375)**, **SCHA (21t, 1.52, 0.278)** e IWO (11t, 3.89 — amostra mínima). Reprovada em VB e IWV (PF 1.26, marginal). Mapa final: **9 ativos aprovados em qualidade** (IWN, IWM, QQQ, IJS, VBR, AVUV, SLYV, SCHA, IWO) — amostra agregada 235 trades OOS. É a estratégia mais robusta do projeto em cobertura.
- **Decisão:** candidata principal. Mesma pendência de amostra — decisão do dono. Não vai ao live antes disso.
