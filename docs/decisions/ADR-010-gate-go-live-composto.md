# ADR-010: Gate de go-live composto (estratégia + operação)

**Status:** Aprovado  
**Data:** 2026-08-04  
**Autor:** CTO

---

## Contexto

`docs/OPERATIONS.md` definia: "nenhum código opera dinheiro real sem no mínimo 3 meses de paper trading e aprovação documentada". Os 3 meses fundiam duas validações diferentes:

1. **Validação estatística da estratégia** — edge existe? (trades suficientes, PF, drawdown, expectativa)
2. **Validação operacional** — o sistema aguenta o mundo real? (fills, slippage, latência, reconexão, gaps, disciplina em dias ruins)

A primeira pode ser feita com dados históricos (backtest, walk-forward, replay). A segunda não — exige tempo real de mercado. Tempo de calendário era um proxy indireto dos dois.

## Decisão

Substituir o gate único de "3 meses" por um **gate composto**, todos os itens obrigatórios:

**A. Estratégia (pode ser concluída imediatamente, com histórico):**
- Backtest com dados reais ≥ 6 meses: ≥ 50 trades, win rate ≥ 40%, PF ≥ 1.3, DD ≤ 10%, avg R > 0.15.
- Walk-forward OOS com métricas sustentadas fora da amostra.

**B. Operação (tempo real, não atalhável):**
- **4 semanas** de paper live contínuo (uptime ≥ 99%, sem circuit breaker).
- ≥ 20 trades reais em paper, com métricas dentro de ±30% do backtest.
- Zero violações de risco (todo trade com stop, limites diários respeitados).
- Reconciliação bot vs corretora sem divergências ao fim de cada semana.

**C. Governança:**
- Aprovação documentada (ADR de go-live com os resultados de A e B anexados).
- Primeiro mês em real com risco reduzido (0.25–0.5% por trade).

## Motivos

- Torna explícito o que os 3 meses mediam por proxy: amostra estatística (A) e estabilidade operacional (B).
- Não afrouxa nenhum critério que protege o capital — troca calendário por métricas verificáveis.
- 4 semanas é o mínimo para atravessar regimes curtos (dias de tendência, laterais, gap, rollover de gateway) sem se estender além do necessário quando A já está validada.

## Consequências

- `docs/OPERATIONS.md` e `docs/runbooks/go-live-checklist.md` atualizados para o gate composto.
- `trader-cli analyze` verifica B (trades, ±30%); A é verificada por `backtest`/`walkforward`.
- Se a operação paper revelar instabilidade (circuit breakers recorrentes, divergências de reconciliação), o relógio de 4 semanas reinicia após a correção.
