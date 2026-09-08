# Estratégia: Balance-Area Breakout v2 — filtro de convicção do rompimento

**ID:** `balance-area-breakout-v2`
**Versão:** 2.0.0
**Status:** proposto / especificado — **NÃO implementado (07/09/2026)**
**Autor da especificação:** coding agent (pesquisa multiagente de 06–07/09/2026), a pedido do dono
**Origem:** `docs/cto-plano-lucratividade-2026-09.md` §6.2 (item 11 do ranking, Tier B), com o diagnóstico de §2.2/§2.3
**Depende de:** ADR-018 (flatten de fim de sessão no backtest, proposto) e ADR-019 (harness de validação e gate estatístico, proposto)
**Relação com a v1:** a `balance-area-breakout-v1` **não muda** (regra do repo: `docs/strategy-analysis-framework.md` §4). A v2 é uma estratégia nova, com gate A e gate B do zero, e roda em símbolos onde a v1 não roda (§14).

---

## 0. Regras de leitura

- Todos os números abaixo são **re-simulação dos críticos (06/09/2026)** sobre os JSONs do binário de 06/09 e os candles do banco dev, com flatten no close da barra 15:45 ET e 2 bp/lado, salvo indicação. Onde o designer (`edge-existente-2`) e os críticos divergem, vale o número do crítico. O motor com o ADR-018 é a fonte de verdade: o harness (ADR-019) precisa reproduzir as tabelas de §3 antes de qualquer veredito.
- Onde não existe número, está escrito **a medir**, com o como.
- Nada aqui autoriza mexer em `crates/trader-core/src/strategies/balance_area_breakout_v1/` nem em `config/strategies/balance-area-breakout-v1.toml`.

## 1. Fonte

- **Livro principal:** James Dalton — *Mind over Markets* (1990), Cap. 4 — "Special Situations — Balance-Area Break-outs". Análise: `docs/books/analysis/dalton-mind-over-markets.md` (Setup A; §2.7 "Iniciativa vs. Responsiva", Cap. 3).
- **Citações centrais (Setup A):** *"Balance area break-out strategy is straightforward — go with the break-out"*; *"a break-out is usually the start of a much bigger move, and trades placed with the initiator are ultimately early"*; a saída implícita no day timeframe é "retorno ao balanceamento (stop) ou fim do dia".
- **Livro de apoio (stop):** Adam Grimes — *The Art and Science of Technical Analysis*, Cap. 8. Análise: `docs/books/analysis/grimes-art-science-ta.md` §4: stop inicial mais próximo que o range médio de 1 barra *"significantly impaired whatever edge you might have had"*; guideline de stop raramente a menos de 2 ATR; stops por volatilidade (N × ATR) *"often are ideal initial stops for algorithmic trading systems"*. A própria análise já propõe a regra de sanidade "rejeitar sinal se `entrada − stop < 1×ATR14(15min)`" (tabela §5.4 daquela análise).
- **O que é interpretação nossa:** o valor numérico do limiar (1,5 ou 1,0 × ATR14), a leitura de "atividade iniciativa forte" como "distância entrada–stop em ATR" e a escolha de segurar a posição até o flatten do live. Nenhum dos dois livros dá esses números.

## 2. Conceito em uma frase

Mesmo setup da v1 (congestão multi-dia + fechamento fora da área), mas só entramos quando o rompimento tem convicção mensurável — a distância entre a entrada (extremo da barra de rompimento) e o stop (de volta dentro da área) é de pelo menos `min_stop_atr_mult` × ATR14 — e seguramos o trade até o alvo, o stop ou o sino (flatten 15h55 ET do live, replicado no backtest pelo ADR-018).

## 3. Por que existe uma v2 — evidência (re-simulação dos críticos, 06/09/2026)

### 3.1 A v1 medida com a régua do live

O backtest do repo não faz flatten; o live encerra tudo às 15h55 ET (`crates/trader-cli/src/commands/paper.rs:2189-2197`). Plano-mestre §2.3, pares vivos (IJS/VBR/AVUV), in-sample, 2 bp:

| v1, 96 trades | Sem flatten | Com flatten |
|---|---|---|
| PF | 1,92 | **1,55** |
| avg R | 0,214 | **−0,007** |
| P&L | +14.648 | +6.730 |
| Overnight | 20 trades = +9.577 (65% do P&L) | — |
| Por par (PF / avg R com flatten) | — | IJS 1,86 / 0,27 · VBR 1,47 / −0,04 · AVUV 1,43 / −0,17 |

Com flatten a v1 **reprova o gate A pelo avg R** (> 0,15); só IJS passa sozinha. É desse ponto que a v2 parte — não do PF 1,96 do gate A de 04/09.

### 3.2 Stop estreito perde (v1 OOS, 92 trades, 3 pares — plano §2.2)

| Tercil de stop (bp do preço) | WR | PF |
|---|---|---|
| ≤ 17 bp | 29% | 0,53 |
| 17–28 bp | 53% | 1,99 |
| > 28 bp | 58% | 3,05 |

Ida e volta = 4 bp = 18% do R da v1 (stop mediano 22–23 bp). Grimes Cap. 8 diz o mesmo em prosa.

### 3.3 PF por bucket de stop/ATR14 (alvo 2R da v1, flatten, 2 bp)

`stop_distance_atr` já é gravado no `market_snapshot` da v1 (`crates/trader-core/src/strategies/balance_area_breakout_v1/entry.rs:120`). ATR14 = média simples dos TR das últimas 14 barras, **incluindo** a barra de rompimento (`context.rs:23-38`).

| Bucket stop/ATR | Pares vivos IJS/VBR/AVUV (PF) | Pool de 8 símbolos (PF, n) |
|---|---|---|
| [0 ; 0,7) | 0,38–0,61 | 0,29–0,60 (n a medir pelo harness) |
| [0,7 ; 1,0) | 0,97–1,42 | **1,02** (n=57, net +147) |
| [1,0 ; 1,5) | **0,51–0,68** (n=22, net −2.136 no crítico de código) | **1,01** (n=52, net +67) |
| ≥ 1,5 | 7,03–9,12 | **4,45** (n=55, net +21.681 de +21.748 filtrado) |

Faixas = os dois críticos que re-simularam (estatístico e de código), com definições de ATR ligeiramente diferentes; o crítico de operação usou a tabela do designer (0,60 / 0,96 / 0,74 / 8,49 nos pares vivos). A ordem e o sinal são os mesmos nas três. Leitura honesta: **os buckets não são monotônicos nos pares vivos** ([1,0;1,5) é pior que [0,7;1,0)); no pool, os dois buckets intermediários têm PF ≈ 1 e **100% do P&L filtrado está em ≥ 1,5** (55 trades / 8 símbolos / 18 meses ≈ 4,6 trades/ano/símbolo). Nos 5 símbolos "não usados para escolher o corte" (IWN, SLYV, IWO, IJR, SCHA): 0,29 / 0,85 / 1,01 / 3,60 — mesmo desenho, com a ressalva de §4 (c).

### 3.4 Efeito do limiar e do alvo (pool de 8, flatten, 2 bp)

| Limiar `min_stop_atr_mult` | 0,8 | 1,0 | 1,2 | 1,5 |
|---|---|---|---|---|
| PF | 2,03 | 2,42 | 3,32 | 4,45 |
| n | 166 | 125 | 91 | 55 |

(PF do crítico de código; n do designer para 0,8–1,2.) É monotônico no limiar, o que satisfaz "vizinhos passam" trivialmente, mas **não é platô**: é um filtro convergindo em poucos vencedores grandes.

| Alvo (filtro ≥ 1,0) | 2R | 2,5R | 3R | 3,5R |
|---|---|---|---|---|
| PF | 2,15 | 2,14 | 2,30 | 2,44 |

(Números do designer, citados pelo crítico de operação como "não monotônico, dentro do ruído".) Por isso o alvo fica em **2R como baseline e 3R como sensibilidade** — não como regra.

Resultado agregado do filtro ≥ 1,0 + 3R + flatten: pool 107–130 trades, PF 2,3–2,6; pares vivos 43–54 trades, PF 2,48–2,83, avg R 0,28–0,31, net ≈ +10,2k. Por símbolo (crítico de código; n por símbolo 13–20 no designer, IWO n=6): IJS 2,55 · VBR 2,82 · AVUV 3,15 · IWN 3,20 · SLYV 4,47 · IJR 1,57 · SCHA 1,40.

### 3.5 O que a v2 é na prática: "rompimento forte segurado até o sino"

Distribuição de `exit_reason` da v2 (filtro ≥ 1,0 + 3R + flatten, pool): **58% `EndOfDay`, 11% alvo, 31% stop** (62 / 12 / 33 de 107). O alvo 3R é decorativo; o P&L depende do fill do flatten. Stop mediano da v2 = **41 bp** (p10 24 bp) contra **23 bp** da v1 — sai da zona de custo da ADR-016 (4 bp ida e volta ≈ 10% de 41 bp, contra 18% de 22 bp na v1 — aritmética sobre os números do plano §2.2). MFE (designer, não re-simulado pelos críticos, sem flatten): 62% dos trades chegam a 1R, 29% a 2R, 9% a 3R.

## 4. Ressalvas honestas (o que a evidência NÃO sustenta)

(a) **Um dia carrega a v2.** 10/10/2025, 8 shorts simultâneos nos 8 ETFs = US$ 15.617 de US$ 24.282 (**64% do P&L do pool**); top-5 dias = **77%** do lucro bruto; 130 trades = 55 datas, WR por dia 34,5%. Com o teto de 3 posições da conta (ADR-017, `config/default.toml:36-38`) esse dia rende no máximo 3/8.
(b) **Instabilidade temporal.** Pool 2025: 80 trades, PF **3,93** (+28k); 2026: 50 trades, PF **0,60** (−3,8k); após 01/12/2025: 64 trades, PF **0,88**; blocos 6–7 de 7 do walk-forward = −586 e −5.228. A regra é "validada" num regime que pode ter acabado (agosto/2026 teve o menor range diário da amostra, plano §2.2).
(c) **O "holdout" de 5 símbolos não é holdout.** 27 das 46 datas coincidem com datas dos pares vivos — são os mesmos eventos em ETFs a ~0,97 de correlação. N efetivo ≈ número de datas, não de trades.
(d) **~20 tentativas nesta família** (limiares × alvos × políticas de saída × escolha de buckets, sobre os mesmos 96 trades). Bootstrap por trade da v2 viva já dá PF 90% em 1,31–4,42 e superestima a precisão por ignorar o clustering por data.
(e) **Amostra.** Filtro ≥ 1,0 nos 3 pares vivos ≈ 53 in-sample → ~45 OOS (< 50): o gate A só fecha com ≥ 5 pares agregados. Com ≥ 1,5 a amostra é ~55 trades em 8 símbolos.
(f) **Execução.** 55–58% dos trades terminam no flatten a mercado às 15h55. Em IJS e SLYV o notional por posição (~US$ 238k, cap 1× equity em `crates/trader-core/src/risk/mod.rs:253`) é 64% / 82% da barra **mediana** de meio-dia (US$ 370k / 290k); o backtest fecha a 2 bp e a conta paper enche a NBBO sem impacto. O live vai pagar mais — quanto, **a medir** nos fills (§14, passo 6).
(g) **Risco por trade sobe** de ~0,2% para ~0,3–0,45% da equity (stop maior sob o mesmo cap de notional: mesma quantidade, R maior em US$). Três posições correlacionadas num dia ruim ≈ 1,3% — cabe nos 4%/dia, mas o gate B (±30% do backtest) fica mais sensível a 2–3 stops.
(h) **O walk-forward do repo não é OOS** para esta escolha: não há re-fit, o "OOS" é a mesma rodada determinística após o 1º bloco (v2 pool "OOS" 119 trades PF 2,33 ≈ in-sample 2,27). O critério de falsificação original do designer não podia falhar. O único OOS verdadeiro é o holdout travado + paper forward.
(i) v1 e v2 no mesmo símbolo se bloqueiam (uma posição por ativo, `AGENTS.md` §3.1); trocar os 3 vivos zeraria a amostra do gate B da v1 acumulada desde 18/08.

## 5. Fase 1 — Extração do conceito (delta em relação à v1)

| # | Pergunta | Resposta (com fonte) |
|---|---|---|
| 1 | Nome | Balance-Area Break-out com convicção (Dalton Cap. 4 + Grimes Cap. 8) |
| 2 | Contexto | Idêntico à v1: área de balanceamento multi-dia (Cap. 4) |
| 3 | Timeframe | 15min sobre ~3 dias (v1) |
| 4 | Entrada | "go with the break-out" **quando há atividade iniciativa** — Dalton Cap. 3/4: rompimento sem follow-through é rejeição; proxy nosso: extremo da barra de rompimento longe o bastante da borda para que o stop fique ≥ N × ATR14 (interpretação) |
| 5 | Stop | Literal da v1: de volta dentro da área (0,3×ATR); **piso** de N × ATR14 entre entrada e stop (Grimes Cap. 8: "nunca dentro do ruído", raramente < 2 ATR — nossa janela útil é [N; 3,0] ATR porque `max_stop_atr_mult = 3.0`, v1 `config.rs:46`) |
| 6 | Alvo/saída | "much bigger move" sem alvo fixo (Cap. 4); saída implícita "fim do dia" (day timeframe). Nossa adaptação: bracket 2R obrigatório (alvo fixo é exigência do motor, `risk/mod.rs:146-155`) **e** flatten do live/ADR-018 como saída de fato |
| 7 | Quando NÃO operar | v1 + rompimento **sem convicção** (`BreakoutWithoutConviction`) |
| 8 | Estatísticas do autor | Nenhuma; as nossas estão em §3 e valem o que §4 diz |

## 6. Contexto de mercado (herdado da v1, sem mudança)

Área de balanceamento: 78 candles anteriores à barra atual, largura ≤ 2% do preço médio e ≤ 10×ATR14, cobertura ≥ 2 dias ET (`context.rs:73-135`); janela 09:45–15:30 ET; fechamento fora da área = aceitação (`setup.rs:18-47`). Uma mudança por variante (plano §3, princípio 6): a v2 **não** toca nesses limiares.

## 7. Setup de entrada (herdado)

- Long: último candle fecha acima da máxima da área → buy stop 1 tick acima da máxima desse candle. Short: espelho.
- Validade da entrada: 2 candles (ADR-009). Guarda de overshoot pré-envio (ADR-015) inalterada.

## 8. Entrada, stop e alvo — o que muda

- **Entrada e stop:** iguais à v1 (`entry.rs:37-42`): `entry = extremo ± 1 tick`; `stop = borda_da_área ∓ 0,3×ATR`.
- **Filtro de convicção (novo):** `risk = |entry − stop|`; rejeitar com `RejectionReason::BreakoutWithoutConviction` se `risk < min_stop_atr_mult × ATR14`. Avaliado **depois** de `let risk` (`entry.rs:44`) e **antes** de `StopTooWide` (`entry.rs:51-61`), de modo que a ordem de rejeições é: risco zero → sem convicção → stop largo demais → RR ruim.
- **Aritmética do limiar:** como `risk = (extremo − borda) + 0,3×ATR (+ 1 tick)`, `risk ≥ 1,5×ATR` equivale ao **extremo** da barra de rompimento ≥ 1,2×ATR além da borda (e `risk ≥ 1,0×ATR` a ≥ 0,7×ATR). O plano-mestre escreve "fechando ≥ 1,2 ATR"; na v1 a entrada é no extremo da barra, não no fechamento — a regra objetiva é sobre o extremo (interpretação nossa, coerente com o código).
- **Alvo:** `target_r_multiple = 2.0` (baseline, igual à v1). 3R entra **só** como sensibilidade via override do harness (`--set target_r_multiple=3.0`, ADR-019), nunca como regra da v2 sem novo pré-registro.
- **Saída de fato:** alvo, stop ou flatten (`ExitReason::EndOfDay`, ADR-018). Sem `time_exit`, sem trailing, sem breakeven, sem parcial — todas ablações já feitas pelo designer (breakeven em 1R PF 1,68; parcial 50% em 1R + runner 3R 2,02; saída por tempo 1,47–1,66; trailing 1,0/1,5 ATR 1,83/1,78; swing GTC 2,72 — não re-simuladas pelos críticos), nenhuma supera a versão simples e todas contam no N.
- **RR mínimo:** 1,5 (estrutural 2 com alvo 2R). Direções: long e short.

## 9. Gestão de risco

- Risco orçado 1,0%/trade (global); limites de instância inalterados (2%/dia, 3 trades/dia, 3 perdas seguidas — `config/default.toml:29-32`) e de conta inalterados (ADR-017: 4%/dia, 3 posições, 200% de notional — `config/default.toml:36-38`).
- **Efeito colateral declarado:** com o cap de notional 1× (`risk/mod.rs:253`) o risco **real** por trade sobe para ~0,3–0,45% da equity (§4 g). Não é aumento de risco após perda (sem martingale); é consequência do stop mais largo com a mesma quantidade. O cap por liquidez do ADR-020 (`max_notional_usd` ≈ US$ 50–90k em SLYV, 85–120k em IJS) **reduz** o tamanho e deve estar ligado antes do gate B em SLYV.
- Sempre stop server-side (bracket); paper only.

## 10. Fase 2 — Tabela subjetivo → objetivo

| Conceito (livro) | Regra objetiva (15min) | Origem |
|---|---|---|
| "dias de valor sobreposto" (Dalton Cap. 4) | 78 candles, largura ≤ 2% e ≤ 10×ATR, ≥ 2 dias ET | v1 (interpretação) |
| "price accepted outside the balance area" | fechamento fora da área | v1 (interpretação) |
| "go with the break-out" | stop entry 1 tick além do extremo da barra de rompimento | v1 (ADR-009) |
| "stops a few ticks… return into the area = rejection" | stop 0,3×ATR dentro da área | v1 (interpretação) |
| "atividade iniciativa"; "no follow-through = rejection" (Dalton Cap. 3/4) | `|entry − stop| ≥ min_stop_atr_mult × ATR14` — extremo da barra de rompimento ≥ (N − 0,3)×ATR além da borda | **v2 — interpretação nossa** (N de §3) |
| "stop nunca menor que 1 range médio de barra; raramente < 2 ATR" (Grimes Cap. 8) | mesmo piso acima; janela útil [N; 3,0] ATR | **v2 — interpretação nossa** (Grimes fala de 2–4 ATR; 3,0 é o teto herdado da v1) |
| "start of a much bigger move" / "fim do dia" | alvo 2R (bracket) + flatten 15h55 ET como saída real | v1 + ADR-018 (adaptação) |

## 11. Fase 3 — Especificação técnica

```text
Inputs:
  - candles 15min (≥ 80: área + ATR14)
  - ATR14 simples (context.rs::atr) — inclui a barra de rompimento
  - config: config/strategies/balance-area-breakout-v2.toml
      min_stop_atr_mult (novo)        — 1.5 (§13; 1.0 só com a declaração)
      demais parâmetros = v1 (78 / 0.02 / 10.0 / 0.3 / 3.0 / 2.0 / 1.5 / 09:45–15:30)

Outputs:
  - Signal (long|short) | Rejected(RejectionReason, detalhes)
  - entrada (stop), stop (dentro da área), alvo 2R
  - market_snapshot: tudo da v1 + min_stop_atr_mult + stop_bp
    (stop_distance_atr já existe; stop_bp = risk / entry × 10.000)
  - detalhes da rejeição BreakoutWithoutConviction: { risk, atr, min_stop_atr_mult, stop_distance_atr }

Estado interno: nenhum (stateless, como a v1)
Eventos: fechamento de candle 15min dentro da janela
Saídas pós-entrada: bracket (alvo/stop) + flatten de sessão (ADR-018 / live) — nada da estratégia
```

Esboço do ponto único de mudança (v2 `entry.rs`, após `let risk`):

```rust
if risk < params.min_stop_atr_mult * atr_value {
    return Err((
        RejectionReason::BreakoutWithoutConviction,
        json!({
            "reason": "rompimento sem convicção (stop mais perto que o mínimo em ATR)",
            "risk": risk,
            "atr": atr_value,
            "min_stop_atr_mult": params.min_stop_atr_mult,
            "stop_distance_atr": risk / atr_value,
        }),
    ));
}
```

Decimal em tudo (`rust_decimal`); nenhum f64.

## 12. Rejeições registradas pelo bot

Reuso: `OutsideTradingHours`, `IncompleteSetup`, `NoBalanceArea`, `StopWithinNoise`, `StopTooWide`, `PoorRiskReward`, `MaxTradesReached`, `DailyLossLimitReached`, `ConsecutiveLosses`, `HighVolatility`, `PositionAlreadyOpen`.

Nova (a adicionar ao domínio, `crates/trader-domain/src/signals.rs:105` em diante, serde `breakout_without_conviction`, com o caso no teste `rejection_reason_serde_round_trip_snake_case` de `signals.rs:275-329`):

- `breakout_without_conviction` — `|entry − stop| < min_stop_atr_mult × ATR14`.

`signals.rejection_reason` é `TEXT` sem CHECK (`crates/trader-infra/src/db/migrations/0001_initial_schema.sql:111`): sem migração. A contagem desta rejeição no paper é, ela mesma, a medida da seletividade do filtro (com limiar 1,0 o designer mede queda de frequência de ~45% em trades; com 1,5, no pool re-simulado, sobrevivem 55 dos 237 trades da v1 — a contagem de **sinais** rejeitados no paper é **a medir**, porque sinal ≠ trade).

## 13. Regra pré-registrada e critérios de avaliação

**Hipótese (falsificável):** rompimentos de área de balanceamento com `|entry − stop| ≥ 1,5 × ATR14`, segurados até alvo 2R / stop / flatten, têm PF ≥ 1,3 e avg R > 0,15 no **holdout travado** e no **paper forward**, com o resultado **ex-10/10/2025** e o **segmento 2026** ambos com PF por dia ≥ 1,3. Se falhar, a família fica arquivada com número (não reabrir sem fato novo).

**Limiar — decisão do dono, registrada antes do primeiro run do harness:**
- **1,5 (limiar honesto, o que a evidência mede).** Amostra ~55 trades / 8 símbolos / 18 meses; o gate A por estratégia (≥ 50 OOS) só fecha com ≥ 5 pares e mais histórico — **a medir** quantos pares/meses.
- **1,0 (para caber no gate).** Só com a declaração explícita neste doc e no relatório: *o bucket [1,0;1,5) tem PF esperado ≈ 1 (1,01 no pool; < 1 nos pares vivos) e a v2 será julgada também pelo bucket ≥ 1,5 isolado*. Critério adicional do crítico de operação: aceitar 1,0 só se [1,0;1,5) tiver PF > 1 em ≥ 4 das 6 janelas OOS com flatten; senão o corte real é 1,5.
- Esta spec fixa o TOML em **1,5**; a variante 1,0 roda como override rotulado (`--set min_stop_atr_mult=1.0 --label …`, ADR-019) **sobre o módulo v2** — na v1 a chave não existe e o `--set` falha por construção (`deny_unknown_fields`, ADR-019 §2) —, contada no N. O que conta é o pré-registro, não o número: mudar o default depois de ver os runs é nova tentativa.

**Alvo:** 2R baseline; 3R sensibilidade (uma tentativa, rotulada). Sem 2,5R/3,5R (já medidos: ruído).

**Gate A (ADR-019, com flatten):** ≥ 50 OOS agregados em ≥ 5 pares, WR ≥ 40%, PF ≥ 1,3, DD ≤ 10%, avg R > 0,15; **e** limite inferior do IC95 em blocos do PF ≥ 1,0; PF_R ≥ 1,2; share dos 2 melhores meses ≤ 60%; pass no holdout travado. Reportar obrigatoriamente: PF por bucket stop/ATR por janela, share do melhor dia e dos top-5 dias, P&L por ano, relatório ex-10/10/2025, `corr(risk, R)`, PF por `exit_reason`, n_trials da família.

**Gate B:** 4 semanas de paper, ≥ 20 trades por par, métricas dentro de ±30% do backtest **com flatten**, lidas contra o replay de portfólio com teto de 3 posições (§6.4 do plano) — não contra 8 backtests isolados.

**Kill pré-registrado:** falha da hipótese acima (PF por dia < 1,3 ex-10/10/2025 no holdout **ou** no segmento 2026 — limiar do crítico estatístico), ou slippage real do flatten em SLYV/IJS acima dos 4 bp da sensibilidade de §14 passo 6 (limiar nosso, igual ao da sensibilidade) sem cap de liquidez (ADR-020) que o traga de volta.

## 14. Protocolo de validação (ordem obrigatória)

1. **ADR-018 no motor** e re-run da v1 com flatten nos pares vivos (baseline do gate B da v1 — decisão do dono, plano §10, item 1).
2. **ADR-019 no harness**: reproduzir a tabela de §3.3 e §3.4 com `--output --slippage-bps 2 --label` (critério: Σ|diff| ≈ 0 contra a re-simulação dos críticos). Enquanto isso não bater, nenhum número deste doc é do motor.
3. **Varredura pré-registrada, antes do módulo — offline.** O harness só aceita `--set` em chave que existe (`deny_unknown_fields`, ADR-019 §2) e a v1 não tem `min_stop_atr_mult`; logo o limiar **não** pode ser varrido por override na v1. O que dá para fazer sem módulo: rodar a v1 com flatten e `--output` e filtrar os trades por `stop_distance_atr` (já no snapshot) em `trader-research/` (plano §5.3) — o mesmo procedimento dos críticos. O alvo (`target_r_multiple`) pode ir por `--set` na v1 (runs `experimental = true`, rotulados). Ressalva declarada: o pós-filtro é aproximado — um trade removido libera vaga para outro que o motor teria bloqueado por `max_trades_per_day` ou posição aberta; a diferença contra o módulo é reportada no passo 7. Grade: limiar ∈ {1,0; 1,5} × alvo ∈ {2R; 3R} = 4 combinações por par, em IJS/VBR/AVUV/IWN/SLYV (+ IJR/SCHA/IWO só para o pool). Registrar N = 4 + ~20 anteriores.
4. **Holdout travado**: os últimos 4–6 meses do histórico, fixados por data no ADR-019 **antes** de rodar. O holdout aborta com `--set` (ADR-019 §1), então roda **uma vez**, sobre o módulo v2 com o TOML pré-registrado (isto é, dentro do passo 7), nunca sobre override nem sobre pós-filtro. Esses meses **são negativos** na re-simulação (após 01/12/2025 PF 0,88; blocos 6–7 negativos) e precisam ser **explicados** (range diário, número de datas, ex-10/10 não se aplica aqui), não ignorados.
5. **Replay de portfólio** (plano §6.4) com teto de 3 posições, `capital_fraction`, flatten: P&L por dia, pior dia, share do 10/10/2025 após o teto.
6. **Custo realista**: `--slippage-bps 4` em IJS e SLYV; cap de notional por liquidez (ADR-020) ligado; comparar. Medir o slippage real do flatten nos fills de produção da v1 (`fills` × `orders` do dump do servidor) antes do gate B — a medir.
7. **Só então** implementar o módulo v2 (§16). Com o módulo: repetir a grade do passo 3 por `--set` + `--label` (agora exata, no motor) e reportar a diferença contra o pós-filtro; rodar o holdout do passo 4 uma vez; walk-forward 6 janelas com flatten nos 5 pares; gate A.
8. **Gate B em IWN e SLYV** (onde a v1 não roda; IWN tem `openrev` 09:30–10:30 — medir o bloqueio mútuo por símbolo antes). **v1 continua em IJS/VBR/AVUV como controle.** Não trocar os três vivos.
9. Relatório em `docs/reports/` com todas as tabelas de §13, n_trials e o veredito por critério.

## 15. Plano de testes unitários (candles sintéticos, reaproveitando a série canônica da v1)

Série canônica: 3 dias de área 100,00–100,80 (78 candles, ATR = 0,80) + candle de rompimento no 4º dia (`balance_area_breakout_v1/tests.rs:33-60`).

1. **Rompimento sem convicção** — barra de rompimento com extremo a 0,5×ATR além da borda (risk ≈ 0,8×ATR) → `BreakoutWithoutConviction`, detalhes com `risk`, `atr`, `min_stop_atr_mult`.
2. **Rompimento com convicção (limiar 1,5)** — extremo a 1,3×ATR além da borda (risk ≈ 1,6×ATR) → sinal long com entrada/stop/alvo corretos e alvo = entrada + 2R.
3. **Limiar exato** — risk = min_stop_atr_mult × ATR (igualdade) → aceito (regra é `<`, não `≤`).
4. **Ordem das rejeições** — rompimento gigante (risk > 3×ATR) → `StopTooWide`, não `BreakoutWithoutConviction` (o filtro de piso vem antes, mas o teto ainda vale).
5. **Limiar via config** — `min_stop_atr_mult = 0.0` reproduz o comportamento da v1 na série canônica (mesmo sinal, mesmos preços); `= 10.0` rejeita tudo.
6. **Short espelhado** — caso 2 para o lado short.
7. **Snapshot auditável** — `stop_distance_atr`, `min_stop_atr_mult`, `stop_bp` presentes; `strategy_id == "balance-area-breakout-v2"`; `config_hash` muda ao mudar `min_stop_atr_mult`.
8. **TOML do projeto faz parse** e `deny_unknown_fields` rejeita chave desconhecida (ADR-019).
9. **Serde da nova rejeição** — round-trip `breakout_without_conviction` (em `trader-domain`).
10. Os 10 casos da v1 (área, dias, horário, RR) copiados e passando na v2 sem alteração de resultado, exceto onde o filtro atua.

## 16. Decisões de implementação (proposto — nada disto existe)

```text
crates/trader-core/src/strategies/balance_area_breakout_v2/   (cópia da v1; v1 intocada)
  mod.rs      → BalanceAreaBreakoutV2, name "Balance-Area Breakout v2", version "2.0.0"
  context.rs  → cópia literal (área, horário, ATR)
  setup.rs    → cópia literal (fechamento fora da área)
  entry.rs    → evaluate_prices: filtro após `let risk` (v1 entry.rs:44), antes de StopTooWide;
                build_signal: snapshot + min_stop_atr_mult + stop_bp
  config.rs   → StrategyParameters + `min_stop_atr_mult: Decimal`; `#[serde(deny_unknown_fields)]`;
                default id "balance-area-breakout-v2", version "2.0.0",
                source "Dalton Cap. 4 + Grimes Cap. 8"
  tests.rs    → §15
crates/trader-core/src/strategies/mod.rs:3,13        → registrar módulo e re-export
crates/trader-domain/src/signals.rs:105 / :275-329  → BreakoutWithoutConviction + teste serde
crates/trader-cli/src/dispatch.rs:24-34, :55-57, :76, :89, :104, :119, :135
                                                    → variante BalanceAreaBreakoutV2 em LoadedStrategy,
                                                      ramo "balance-area-breakout-v2", mensagem de erro,
                                                      entry_validity_candles / config_hash / risk_params /
                                                      time_exit (None)
crates/trader-cli/src/risk_config.rs:64-75           → impl From<&BalanceAreaV2Params> for StrategyRiskParams (espelho do impl da v1)
config/strategies/balance-area-breakout-v2.toml       → v1 + min_stop_atr_mult = 1.5 (comentário: 1.0 só com a
                                                      declaração de §13), target_r_multiple = 2.0
docs/strategies/balance-area-breakout-v2.md           → este arquivo
```

Sem migração de banco; sem mudança em `MarketContext`, no `RiskManager` ou no simulador. Compose/scheduler/painel entram só no gate B (client_ids conforme plano §6.5).

Interpretações nossas (além das herdadas da v1): o limiar em ATR, a leitura de "atividade iniciativa" como distância entrada–stop, a escolha de 2R como baseline com 3R sensibilidade, e segurar até o flatten.

## 17. Dependências e sequenciamento

| Depende de | Por quê | Semana (plano §9) |
|---|---|---|
| ADR-018 (flatten no backtest, `ExitReason::EndOfDay`) | 58% das saídas da v2 são EOD; sem isso o backtest mede outra estratégia | 1 |
| ADR-019 (harness: `--output/--slippage-bps/--label/--holdout-from/--set`, PF_R, métricas por dia, n_trials) | `--output` para a varredura offline (§14 passo 3), `--set` sobre o módulo, holdout travado, reprodução das tabelas | 1–2 |
| §6.4 replay de portfólio | teto de 3 posições sobre o 10/10/2025 | 3–4 |
| ADR-020 (cap por liquidez) | SLYV/IJS antes do gate B | 2–3 |
| Decisão do dono sobre o gate B da v1 (plano §10, item 1) | v1 é o controle desta v2 | 2 |
| Módulo v2 + gate A/B | só após 1–6 de §14 | 4–6 (+4 semanas de paper) |

## 18. Checklist de validação

```text
[x] Documentação da estratégia preenchida (Fases 1–3)
[x] Regras objetivas definidas (§10)
[x] Especificação técnica completa (§11)
[ ] Decisão do dono: limiar 1,5 ou 1,0 com declaração (§13) — ANTES do primeiro run
[ ] ADR-018 implementado e v1 re-rodada com flatten
[ ] ADR-019 implementado; tabelas de §3 reproduzidas pelo harness (Σ|diff| ≈ 0)
[ ] Varredura pré-registrada 2×2 nos 5 pares (offline por stop_distance_atr antes do módulo; --set sobre o módulo depois), rotulada, N registrado
[ ] Holdout travado rodado uma vez sobre o módulo v2 (sem --set) e explicado
[ ] Replay de portfólio com teto de 3 posições; relatório ex-10/10/2025
[ ] --slippage-bps 4 em IJS/SLYV; slippage real do flatten medido nos fills
[ ] Código revisado (v1 intocada — diff em balance_area_breakout_v1/ deve ser vazio)
[ ] Testes unitários passando (10 casos de §15 + 10 herdados)
[ ] Walk-forward 6 janelas com flatten em ≥ 5 pares; gate A (ADR-019)
[ ] Gate B em IWN/SLYV com v1 como controle em IJS/VBR/AVUV
[ ] Nenhuma violação de regra de segurança financeira (stop server-side, paper, sem martingale)
[ ] Versionada no git
```

## 19. Contador de tentativas da família (para o DSR do ADR-019)

Já gastas na pesquisa de 06/09 (designer + críticos, sobre os mesmos 96/237 trades): limiares {0,8; 1,0; 1,2; 1,5} × alvos {2; 2,5; 3; 3,5} + 6 políticas de saída + escolha de buckets ≈ **20**. Previstas por este doc: 4 (§14 passo 3) + 1 (3R como sensibilidade já incluída) — total ≈ 24 antes do primeiro trade em paper. Qualquer ablação além destas (largura da área, buffer do stop, horário, direção) é regra nova sem fonte e exige novo pré-registro.
