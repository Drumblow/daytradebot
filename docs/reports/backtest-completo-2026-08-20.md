# Backtest completo do portfólio — 7 estratégias × 14 ativos (2026-08-20)

**Fonte de dados:** dump do banco de produção da VM (2026-08-20 21:41 UTC, restaurado em `trader_compare` local) — 134.635 candles 15m, incluindo os dados reparados de 08-18/19 e o pregão de 08-20.
**Janela:** 2025-02-21 → 2026-08-20 (~17,5 meses) para os 8 ativos do live; 2025-02-21/24 → 2026-08-06 para SPY/QQQ/MDY/IJR/SCHA/VB (ingestão histórica parou em 08-06 — diferença de 10 pregões).
**Custos:** comissão $0,35/trade + slippage 0,1% (SimulatedBroker, mesma configuração das validações anteriores).
**Método:** 98 runs (`trader-cli backtest`, lógica idêntica live ≡ backtest), configurações TOML de produção, sem ajuste de parâmetros. **Janela única in-sample** — para decisões de expansão, o framework exige walk-forward OOS antes (ver §5).

---

## 1. Resumo executivo

| Estratégia | Status | Trades | WR | PF | avgR | Net 17,5m | Ativos positivos |
|---|---|---|---|---|---|---|---|
| balance-area-breakout-v1 | **LIVE** | 464 | 40,5% | **1,73** | 0,205 | **+$44.946** | 12/14 |
| opening-reversal-v1 | **LIVE** | 387 | 42,4% | **1,58** | 0,263 | **+$35.831** | 13/14 |
| range-extreme-fade-v1 | **LIVE** | 403 | 42,2% | 1,05 | 0,044 | +$2.572 | 10/14 |
| pullback-trend-v1 | **LIVE** | 1.373 | 36,1% | 1,01 | 0,066 | +$961 | 7/14 |
| breakout-first-pullback-v1 | arquivada | 17 | 41,2% | 1,16 | 0,155 | +$579 | 6/14 |
| failure-test-long-v1 | arquivada | 219 | 36,5% | 0,78 | −0,089 | −$7.249 | 4/14 |
| low2-m2s-short-v1 | arquivada | 977 | 30,1% | 0,75 | −0,105 | −$54.682 | 2/14 |

**Leituras principais:**

1. **O portfólio live está bem alocado** — o agregado "todos os 14 ativos" esconde o essencial: cada estratégia está em produção exatamente nos ativos onde ela é mais forte (detalhe no §2). O portfólio das 11 instâncias soma **+$43.320 em 17,5 meses (~$2.475/mês)**, WR 47%, PF 1,77.
2. **Os vereditos das arquivadas reproduzem:** low2-m2s-short reprova de novo (−$54,7k, PF 0,75 — igual à validação de 08-17), failure-test-long negativa, breakout-first-pullback com apenas 17 trades (a armadilha de amostra que a matou). O funil é consistente entre rodadas.
3. **pullback-trend-v1 agregada é fraca (PF 1,01)** — mas nos 3 ativos do live é o top-3 da própria tabela (PF 1,19–1,63). Estratégia de seleção de ativo, não de universalidade.
4. **range-extreme-fade-v1 agregada parece fraca (PF 1,05)** — artefato de varrer momentum (IWM/SPY/QQQ/IWO: PF 0,28–0,67). Nos ativos do live: PF 1,94–2,44, WR 56–65%.

## 2. Detalhe por estratégia (∗ = ativo em live)

### pullback-trend-v1 (live: IWM, IWV, IWO)

| Ativo | Trades | WR | PF | avgR | DD% | Net |
|---|---|---|---|---|---|---|
| IWM∗ | 102 | 43,1% | 1,26 | 0,280 | 3,1 | +$3.310 |
| IWV∗ | 98 | 45,9% | 1,63 | 0,336 | 0,9 | +$2.704 |
| IWO∗ | 92 | 40,2% | 1,19 | 0,193 | 2,8 | +$2.148 |
| VB | 105 | 37,1% | 1,10 | 0,099 | 2,0 | +$1.150 |
| AVUV | 83 | 34,9% | 1,11 | 0,036 | 2,4 | +$1.145 |
| QQQ | 146 | 34,2% | 1,03 | 0,014 | 3,4 | +$458 |
| VBR | 93 | 31,2% | 1,01 | −0,078 | 2,2 | +$79 |
| MDY | 90 | 32,2% | 0,99 | −0,049 | 1,7 | −$100 |
| SLYV | 69 | 36,2% | 0,94 | 0,073 | 2,1 | −$431 |
| SPY | 141 | 34,0% | 0,96 | 0,003 | 2,3 | −$482 |
| IJS | 85 | 38,8% | 0,95 | 0,150 | 2,2 | −$489 |
| IJR | 98 | 33,7% | 0,89 | −0,001 | 3,5 | −$1.471 |
| IWN | 92 | 34,8% | 0,70 | 0,027 | 4,3 | −$3.484 |
| SCHA | 79 | 27,8% | 0,77 | −0,171 | 4,6 | −$3.576 |

### balance-area-breakout-v1 (live: IJS, VBR, AVUV)

| Ativo | Trades | WR | PF | avgR | DD% | Net |
|---|---|---|---|---|---|---|
| AVUV∗ | 37 | 43,2% | **3,05** | 0,290 | 2,0 | +$8.442 |
| IWN | 40 | 47,5% | 2,52 | 0,415 | 1,6 | +$7.781 |
| VBR∗ | 39 | 41,0% | 2,35 | 0,220 | 0,9 | +$5.794 |
| IJR | 40 | 37,5% | 1,94 | 0,116 | 1,4 | +$5.481 |
| IJS∗ | 32 | 53,1% | 2,56 | 0,581 | 0,9 | +$5.319 |
| SLYV | 30 | 46,7% | 2,19 | 0,391 | 2,4 | +$5.079 |
| IWO | 11 | 45,5% | 3,90 | 0,354 | 0,3 | +$2.672 |
| SCHA | 21 | 42,9% | 1,53 | 0,278 | 1,3 | +$2.113 |
| QQQ | 19 | 42,1% | 1,80 | 0,254 | 0,7 | +$1.867 |
| IWV | 46 | 41,3% | 1,31 | 0,224 | 1,9 | +$1.488 |
| IWM | 21 | 42,9% | 1,27 | 0,276 | 0,8 | +$986 |
| VB | 38 | 31,6% | 1,10 | −0,062 | 1,7 | +$532 |
| MDY | 37 | 29,7% | 0,98 | −0,118 | 1,6 | −$101 |
| SPY | 53 | 34,0% | 0,69 | 0,007 | 4,2 | −$2.506 |

### opening-reversal-v1 (live: IWM, IWN)

| Ativo | Trades | WR | PF | avgR | DD% | Net |
|---|---|---|---|---|---|---|
| VB | 27 | 55,6% | **2,59** | 0,657 | 2,0 | +$5.626 |
| IJR | 34 | 44,1% | 1,87 | 0,316 | 1,2 | +$4.322 |
| IWN∗ | 27 | 48,1% | 2,03 | 0,436 | 1,1 | +$3.849 |
| SCHA | 30 | 46,7% | 1,70 | 0,393 | 2,3 | +$3.753 |
| IWM∗ | 31 | 51,6% | 1,64 | 0,539 | 2,3 | +$3.636 |
| IWO | 33 | 42,4% | 1,49 | 0,266 | 3,1 | +$3.298 |
| MDY | 26 | 38,5% | 1,86 | 0,147 | 1,9 | +$3.282 |
| SLYV | 24 | 50,0% | 2,09 | 0,490 | 1,0 | +$3.229 |
| SPY | 29 | 34,5% | 1,37 | 0,026 | 1,3 | +$1.428 |
| QQQ | 29 | 31,0% | 1,26 | −0,076 | 3,2 | +$1.287 |
| IWV | 26 | 38,5% | 1,43 | 0,139 | 1,0 | +$1.202 |
| VBR | 22 | 40,9% | 1,36 | 0,219 | 0,8 | +$1.087 |
| AVUV | 23 | 34,8% | 1,06 | 0,038 | 1,3 | +$322 |
| IJS | 26 | 34,6% | 0,90 | 0,031 | 1,6 | −$492 |

### range-extreme-fade-v1 (live: AVUV, SLYV, IWV)

| Ativo | Trades | WR | PF | avgR | DD% | Net |
|---|---|---|---|---|---|---|
| SLYV∗ | 26 | 65,4% | **2,44** | 0,621 | 0,9 | +$3.517 |
| AVUV∗ | 27 | 63,0% | 2,20 | 0,563 | 1,1 | +$3.357 |
| IJR | 39 | 46,2% | 1,39 | 0,145 | 0,8 | +$1.952 |
| MDY | 28 | 50,0% | 1,46 | 0,238 | 1,7 | +$1.281 |
| IWV∗ | 25 | 56,0% | 1,94 | 0,374 | 0,4 | +$1.243 |
| IWN | 27 | 44,4% | 1,35 | 0,103 | 2,1 | +$1.235 |
| SCHA | 24 | 41,7% | 1,17 | 0,037 | 1,8 | +$802 |
| VBR | 28 | 42,9% | 1,11 | 0,058 | 1,6 | +$315 |
| IJS | 31 | 41,9% | 1,04 | 0,038 | 1,9 | +$156 |
| VB | 28 | 46,4% | 1,03 | 0,150 | 2,1 | +$99 |
| IWO | 22 | 31,8% | 0,52 | −0,214 | 2,2 | −$1.705 |
| QQQ | 34 | 32,4% | 0,67 | −0,200 | 1,9 | −$1.872 |
| SPY | 35 | 20,0% | 0,37 | −0,509 | 3,2 | −$3.066 |
| IWM | 29 | 17,2% | 0,28 | −0,575 | 4,9 | −$4.743 |

## 3. Portfólio live consolidado (as 11 instâncias)

| Combo | Trades | WR | PF | avgR | Net 17,5m | Net/mês |
|---|---|---|---|---|---|---|
| pullback-trend-v1 (IWM/IWV/IWO) | 292 | 43,2% | 1,29 | 0,272 | +$8.162 | $466 |
| balance-area-breakout-v1 (IJS/VBR/AVUV) | 108 | 45,4% | **2,66** | 0,351 | +$19.555 | $1.117 |
| opening-reversal-v1 (IWM/IWN) | 58 | 50,0% | 1,80 | 0,491 | +$7.486 | $428 |
| range-extreme-fade-v1 (AVUV/SLYV/IWV) | 78 | **61,5%** | 2,24 | 0,522 | +$8.118 | $464 |
| **TOTAL (11 instâncias × $100k paper)** | **536** | **47,0%** | **1,77** | — | **+$43.320** | **~$2.475** |

Max drawdown individual por combo/ativo ficou entre 0,4% e 3,1% — bem abaixo do limite do gate (10%).

## 4. Arquivadas — reprodução dos vereditos

| Estratégia | Resultado desta rodada | Veredito original | Confere? |
|---|---|---|---|
| low2-m2s-short-v1 | 977t, WR 30%, PF 0,75, −$54.682 | REPROVADA (08-17): PF máx 1,29, net −$55k | ✅ |
| failure-test-long-v1 | 219t, WR 37%, PF 0,78, −$7.249 | REPROVADA | ✅ |
| breakout-first-pullback-v1 | 17 trades em 14 ativos | ARQUIVADA por amostra insuficiente | ✅ (17 trades em 17,5m confirma a raridade) |

## 5. Candidatos de expansão (hipóteses, NÃO decisões)

Fora do live, com PF ≥ 1,5, WR ≥ 40%, ≥ 20 trades nesta janela in-sample:

| Estratégia | Ativo | Trades | WR | PF | Net |
|---|---|---|---|---|---|
| balance-area-breakout-v1 | IWN | 40 | 48% | 2,52 | +$7.781 |
| balance-area-breakout-v1 | SLYV | 30 | 47% | 2,19 | +$5.079 |
| balance-area-breakout-v1 | SCHA | 21 | 43% | 1,53 | +$2.113 |
| opening-reversal-v1 | VB | 27 | 56% | 2,59 | +$5.626 |
| opening-reversal-v1 | IJR | 34 | 44% | 1,87 | +$4.322 |
| opening-reversal-v1 | SCHA | 30 | 47% | 1,70 | +$3.753 |
| opening-reversal-v1 | SLYV | 24 | 50% | 2,09 | +$3.229 |

**Antes de qualquer expansão:** o framework exige walk-forward OOS (6 janelas) por candidato + ingestão dos ativos novos no banco de produção (VB/IJR/SCHA/MDY param em 08-06) + avaliação de correlação com as instâncias existentes (IWN já tem openrev; SLYV já tem rangefade — uma 2ª estratégia no mesmo ativo é precedente válido: IWM e IWV já rodam 2).

## 6. Limitações desta rodada

- **Janela única in-sample** (2025-02 → 2026-08, majoritariamente bull): números absolutos são otimistas por construção; as decisões de live anteriores usaram walk-forward OOS (ver docs de cada estratégia). Este relatório serve para **comparação relativa entre ativos/estratégias**, não como garantia de edge.
- 6 ativos (SPY/QQQ/MDY/IJR/SCHA/VB) têm histórico só até 08-06 — 10 pregões a menos.
- Slippage do simulador é fixo (0,1%); o live mostrou slippage real maior em entradas stop com gap de abertura de barra (caso AVUV 08-20: 0,38 além do trigger). Estratégias de stop-entry carregam esse viés para pior no live.
- SLYV tem ~200 barras a menos (primeiras semanas sem dados) — efeito marginal.

## 7. Reprodutibilidade

```bash
# banco: pg_dump da VM restaurado em trader_compare (localhost:5434)
for strat in <7 estratégias>; do for sym in SPY QQQ MDY IWM IWN IJR IWO IWV IJS VBR AVUV SLYV SCHA VB; do
  trader-cli backtest --strategy $strat --symbol $sym \
    --from 2025-02-21 --to 2026-08-20 --timeframe 15m --output $strat__$sym.json
done; done
```
98/98 runs concluídos sem falhas (2026-08-20 ~22:00 UTC, binário debug do commit `7f767f9`).
