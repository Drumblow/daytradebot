# Estudo pareado de políticas de saída (breakeven, trailing, stop além da barra, parcial) — 07/09/2026

**Origem:** rodada extra da pesquisa de lucratividade (lacuna "gestão de saída"). Estudo em Python sobre os JSONs de `trader-cli backtest -o` (2 bp) e candles do banco dev; o script e os dados por trade ficaram FORA do repositório por decisão do dono (nada de código nesta rodada). Nenhum arquivo Rust foi tocado.

**Resultado em uma linha:** nenhuma política de saída gerida (B breakeven em 1R; C trailing 1×ATR14 após 1R; D stop além da barra anterior após 1R; E 50% em 1R + trailing) supera o alvo fixo + flatten (A) com t > 2 em 2025 E 2026 nos pares vivos; C/D melhoram a balance-area no pool de 8 símbolos só no agregado (t 2,1–2,4) e nunca no lado short; na range-fade e na opening-reversal todas pioram. A linha "gestão de saída" fica fechada com número até haver motor de `modify_order` e uma hipótese nova com fonte.

---

# Pré-registro — políticas de saída em desenho pareado (lacuna "gestão de saída")

**Data/hora do pré-registro:** 2026-09-07, antes de qualquer resultado das políticas B–E
(o único número visto até aqui foi a reprodução do baseline do motor, necessária para
validar o simulador). **Status:** estudo de pesquisa em Python; nada de Rust muda.

## 1. Pergunta

As entradas das três v1 (`balance-area-breakout-v1`, `range-extreme-fade-v1`,
`opening-reversal-v1`) ganham avg R quando a saída deixa de ser "alvo fixo + stop fixo +
flatten" e passa a ser gerida (breakeven / trailing / stop além da barra / parcial)?
A hipótese nula é que NÃO: a continuação intraday depois de 1R não paga o custo de
abrir mão do alvo fixo (o intraday da balance com flatten tem avg R ≈ −0,007, ADR-018).

## 2. Dados (fixos)

- Entradas: JSON por trade de `trader-cli backtest -o` (binário `target/release` de
  06/09/2026, `main d5e7279`), 2 bp/lado, `--from 2025-02-24 --to 2026-09-03`, 15m.
  Cada trade traz `entry_price` (já com slippage), `quantity`, `entry_time` (= timestamp
  de abertura da barra do fill), `stop_price` (inicial), `target_price`.
- Universo primário (decisão): pares VIVOS — bab {IJS, VBR, AVUV}; ref {AVUV, SLYV, IWV};
  orv {IWM, IWN}.
- Universo secundário (robustez de sinal, não decide): pool dos 8 símbolos com candles
  até 02/09/2026 {AVUV, IJS, IWM, IWN, IWO, IWV, SLYV, VBR} para cada estratégia.
- Candles 15m do Postgres dev (porta 5434), RTH 09:30–15:45 ET, 26 barras/dia.
- Validação obrigatória antes de rodar B–E: o simulador reproduz o `net_pnl` do JSON
  ao centavo (Σ|diff| < 0,01) com a convenção do `SimulatedBroker` (stop primeiro no
  pior caso; stop enche em min/max(open, stop) com slippage; alvo enche em
  max/min(open, alvo) sem slippage; avaliação a partir da barra do fill inclusive;
  comissão US$ 0,35/perna). Se não reproduzir, o estudo não vale.

## 3. Modelo de custo comum a TODAS as políticas (inclusive A)

- Slippage 2 bp em toda execução a mercado: stop (inicial ou movido), flatten.
  Alvo e parcial em 1R são limites: sem slippage.
- Comissão IBKR Pro fixed: max(US$ 1,00; 0,005 × ações) por perna; perna de entrada +
  cada perna de saída (a parcial da política E paga duas pernas de saída).
- Flatten: fechamento a mercado no close da barra 15:45 ET (última barra RTH) com
  slippage — é a régua da ADR-018.
- Denominador de R = risk_amount do JSON = |entry − stop_inicial| × qty. O MESMO para
  todas as políticas (pareado por trade).

## 4. Políticas (fixadas agora; nenhuma será ajustada depois)

Decisão só no fechamento de cada barra (paridade com C13: o bot age uma vez por candle
fechado); a modificação de stop vale a partir da barra SEGUINTE. "Tocou 1R" = máxima
(long) / mínima (short) da barra ≥ entry ± R, com R = |entry − stop_inicial|, detectado
intrabar, incluindo a barra do fill. Stop só se move a favor (ratchet).

- **A — baseline:** stop fixo + alvo fixo da v1 + flatten 15:45. (ADR-018.)
- **B — breakeven em 1R:** igual a A; após a barra em que 1R foi tocado, stop = preço de
  entrada. Alvo fixo mantido.
- **C — trailing 1×ATR14 após 1R, sem alvo:** sem alvo desde a entrada; após 1R,
  stop = max(stop, HH − 1×ATR14) (long; espelho no short), onde HH = máxima desde o fill
  (inclusive) e ATR14 = média simples do TR das 14 barras fechadas ANTES da barra do fill
  (mesma definição de `indicators::atr`), fixo por trade. Flatten 15:45.
- **D — stop além da barra anterior após 1R (Brooks), sem alvo:** sem alvo; após 1R, ao
  fechar cada barra, stop = max(stop, low da barra fechada − 1 tick) (long; espelho).
  Flatten 15:45.
- **E — 50% em 1R + resto trailing (Grimes):** limite de venda de floor(qty/2) em
  entry + 1R (fill em max(open, nível), sem slippage); no fechamento dessa barra o resto
  vai a breakeven e daí segue a regra C (HH − 1×ATR14). Sem alvo para o resto. Flatten.

Ordem de avaliação em cada barra: (1) stop vigente (pior caso, posição inteira);
(2) alvo/parcial limite; (3) no close: atualização de HH/LL, detecção de 1R e movimento
de stop; (4) se última barra do dia ET: flatten a mercado no close.

## 5. Métricas e cortes (todos pré-definidos)

Por política e universo: n, avg R, SE(avg R), PF_R (ΣR+ / Σ|R−|), WR, net US$,
distribuição de `exit_reason`, barras médias em posição.
Pareado vs A: d_i = R_i(P) − R_i(A); média, SE = sd(d)/√n, t = média/SE — pooled, por
estratégia, por bloco temporal (2025 = entrada < 2026-01-01; 2026 = resto), por direção.
Concentração: share do top-5 dias no net de cada política.
Por trade (independente de política): MFE e MAE em R no caminho "sem alvo" até o
primeiro de {stop inicial, flatten}; % de trades que tocaram 1R.

## 6. Critério de adoção (pré-registrado, por estratégia, universo primário)

Adotar a política P só se: média(d) > 2×SE em AMBOS os blocos (2025 e 2026) E
PF_R(P) ≥ PF_R(A). Caso contrário, a política é registrada como tentativa reprovada.
**N de tentativas** deste estudo: 4 (B, C, D, E) × 3 estratégias = 12 comparações;
somam-se às ~6 ablações de saída já feitas pelo designer sobre a v2 da balance
(`docs/strategies/balance-area-breakout-v2.md` §8, não pareadas, não re-simuladas).
Nenhuma variante além de B–E será testada nesta rodada, mesmo que os resultados sugiram.

## 7. O que NÃO decide este estudo

- Sizing (o R é por risco inicial; o cap de notional não muda entre políticas).
- Overnight/swing (todas as políticas fecham no sino; a hipótese multi-dia é o item #21
  do plano).
- Slippage do flatten real (a medir em produção, ADR-018).


---

# Estudo pareado de politicas de saida - resultados

Trades carregados: 654 em 8 simbolos x 3 estrategias. Candles do banco dev (15m, RTH).

**Reproducao do motor (modo REPRO, comissao 0,35/perna, sem flatten):** n=654, soma|diff net| = 0.000000, max |diff| = 0.000000 US$.


## PARES VIVOS (universo primario - decide)


### balance-area-breakout-v1 - n=96 (AVUV, IJS, VBR)

Motor sem flatten (referencia ADR-018): avg R 0.214, PF_R 1.32, overnight 20 trades.  
Caminho sem alvo ate stop/flatten: MFE mediana 1.05R (media 1.60R), MAE mediana -1.02R, tocaram 1R: 53.1%, ATR14/R mediano 0.93, stop mediano 22 bp.

| Politica | n | avg R | SE | PF_R | WR% | net US$ | top-5 dias % | barras | saidas |
|---|---|---|---|---|---|---|---|---|---|
| A | 96 | -0.037 | 0.129 | 0.94 | 41.7 | 6062 | 187 | 6.9 | stop 47, target 29, end_of_day 20 |
| B | 96 | 0.021 | 0.122 | 1.04 | 37.5 | 7676 | 147 | 6.3 | stop 37, target 27, breakeven 18, end_of_day 14 |
| C | 96 | 0.214 | 0.214 | 1.42 | 47.9 | 10638 | 140 | 6.4 | trail_stop 45, stop 37, end_of_day 14 |
| D | 96 | 0.260 | 0.214 | 1.53 | 52.1 | 12010 | 127 | 6.0 | trail_stop 46, stop 37, end_of_day 13 |
| E | 96 | 0.138 | 0.143 | 1.30 | 55.2 | 8737 | 126 | 6.3 | partial_1r+trail_stop 39, stop 37, end_of_day 10, partial_1r+breakeven 7, partial_1r+end_of_day 3 |

Pareado vs A (d = R_P - R_A): media +- SE (t), pooled e por bloco / direcao. Criterio: t > 2 em 2025 E 2026, e PF_R(P) >= PF_R(A).

| Politica | pooled | 2025 | 2026 | long | short | PF_R >= A? | ADOTA? |
|---|---|---|---|---|---|---|---|
| B | 0.058 +- 0.040 (t 1.4, n 96) | 0.029 +- 0.043 (t 0.7, n 56) | 0.099 +- 0.076 (t 1.3, n 40) | 0.067 +- 0.050 (t 1.3, n 58) | 0.045 +- 0.068 (t 0.7, n 38) | sim (1.04 vs 0.94) | **nao** |
| C | 0.251 +- 0.169 (t 1.5, n 96) | 0.327 +- 0.277 (t 1.2, n 56) | 0.145 +- 0.118 (t 1.2, n 40) | 0.485 +- 0.265 (t 1.8, n 58) | -0.106 +- 0.117 (t -0.9, n 38) | sim (1.42 vs 0.94) | **nao** |
| D | 0.297 +- 0.171 (t 1.7, n 96) | 0.428 +- 0.277 (t 1.5, n 56) | 0.114 +- 0.131 (t 0.9, n 40) | 0.537 +- 0.268 (t 2.0, n 58) | -0.069 +- 0.120 (t -0.6, n 38) | sim (1.53 vs 0.94) | **nao** |
| E | 0.176 +- 0.101 (t 1.7, n 96) | 0.168 +- 0.151 (t 1.1, n 56) | 0.186 +- 0.123 (t 1.5, n 40) | 0.287 +- 0.148 (t 1.9, n 58) | 0.006 +- 0.118 (t 0.0, n 38) | sim (1.30 vs 0.94) | **nao** |

### range-extreme-fade-v1 - n=69 (AVUV, IWV, SLYV)

Motor sem flatten (referencia ADR-018): avg R 0.297, PF_R 1.67, overnight 9 trades.  
Caminho sem alvo ate stop/flatten: MFE mediana 1.16R (media 1.73R), MAE mediana -0.96R, tocaram 1R: 55.1%, ATR14/R mediano 0.98, stop mediano 21 bp.

| Politica | n | avg R | SE | PF_R | WR% | net US$ | top-5 dias % | barras | saidas |
|---|---|---|---|---|---|---|---|---|---|
| A | 69 | 0.182 | 0.130 | 1.42 | 58.0 | 4050 | 80 | 5.7 | target 36, stop 24, end_of_day 9 |
| B | 69 | 0.157 | 0.127 | 1.37 | 55.1 | 3720 | 87 | 5.6 | target 34, stop 23, end_of_day 9, breakeven 3 |
| C | 69 | 0.017 | 0.148 | 1.04 | 46.4 | 981 | 359 | 6.7 | trail_stop 31, stop 24, end_of_day 14 |
| D | 69 | 0.122 | 0.147 | 1.28 | 52.2 | 2475 | 147 | 6.4 | trail_stop 33, stop 23, end_of_day 13 |
| E | 69 | 0.101 | 0.125 | 1.24 | 60.9 | 2174 | 130 | 6.7 | partial_1r+trail_stop 26, stop 23, end_of_day 8, partial_1r+breakeven 6, partial_1r+end_of_day 6 |

Pareado vs A (d = R_P - R_A): media +- SE (t), pooled e por bloco / direcao. Criterio: t > 2 em 2025 E 2026, e PF_R(P) >= PF_R(A).

| Politica | pooled | 2025 | 2026 | long | short | PF_R >= A? | ADOTA? |
|---|---|---|---|---|---|---|---|
| B | -0.024 +- 0.031 (t -0.8, n 69) | -0.039 +- 0.050 (t -0.8, n 43) | 0.000 +- 0.000 (t -, n 26) | 0.000 +- 0.000 (t -, n 33) | -0.047 +- 0.060 (t -0.8, n 36) | nao (1.37 vs 1.42) | **nao** |
| C | -0.164 +- 0.102 (t -1.6, n 69) | -0.163 +- 0.144 (t -1.1, n 43) | -0.165 +- 0.131 (t -1.3, n 26) | -0.255 +- 0.110 (t -2.3, n 33) | -0.081 +- 0.168 (t -0.5, n 36) | nao (1.04 vs 1.42) | **nao** |
| D | -0.060 +- 0.089 (t -0.7, n 69) | -0.021 +- 0.127 (t -0.2, n 43) | -0.124 +- 0.111 (t -1.1, n 26) | -0.187 +- 0.093 (t -2.0, n 33) | 0.057 +- 0.147 (t 0.4, n 36) | nao (1.28 vs 1.42) | **nao** |
| E | -0.081 +- 0.055 (t -1.5, n 69) | -0.071 +- 0.078 (t -0.9, n 43) | -0.098 +- 0.073 (t -1.3, n 26) | -0.160 +- 0.064 (t -2.5, n 33) | -0.008 +- 0.087 (t -0.1, n 36) | nao (1.24 vs 1.42) | **nao** |

### opening-reversal-v1 - n=67 (IWM, IWN)

Motor sem flatten (referencia ADR-018): avg R 0.186, PF_R 1.29, overnight 8 trades.  
Caminho sem alvo ate stop/flatten: MFE mediana 1.44R (media 2.00R), MAE mediana -1.00R, tocaram 1R: 64.2%, ATR14/R mediano 0.85, stop mediano 31 bp.

| Politica | n | avg R | SE | PF_R | WR% | net US$ | top-5 dias % | barras | saidas |
|---|---|---|---|---|---|---|---|---|---|
| A | 67 | 0.342 | 0.166 | 1.70 | 55.2 | 7790 | 93 | 7.1 | stop 30, target 29, end_of_day 8 |
| B | 67 | 0.360 | 0.153 | 1.95 | 46.3 | 7804 | 93 | 5.4 | target 26, stop 22, breakeven 14, end_of_day 5 |
| C | 67 | 0.217 | 0.142 | 1.60 | 64.2 | 4070 | 122 | 5.1 | trail_stop 42, stop 22, end_of_day 3 |
| D | 67 | 0.247 | 0.144 | 1.65 | 59.7 | 5194 | 102 | 5.3 | trail_stop 42, stop 22, end_of_day 3 |
| E | 67 | 0.250 | 0.124 | 1.70 | 67.2 | 5013 | 87 | 5.1 | partial_1r+trail_stop 42, stop 22, end_of_day 2, partial_1r+end_of_day 1 |

Pareado vs A (d = R_P - R_A): media +- SE (t), pooled e por bloco / direcao. Criterio: t > 2 em 2025 E 2026, e PF_R(P) >= PF_R(A).

| Politica | pooled | 2025 | 2026 | long | short | PF_R >= A? | ADOTA? |
|---|---|---|---|---|---|---|---|
| B | 0.018 +- 0.066 (t 0.3, n 67) | 0.064 +- 0.092 (t 0.7, n 42) | -0.059 +- 0.087 (t -0.7, n 25) | 0.051 +- 0.101 (t 0.5, n 26) | -0.003 +- 0.088 (t -0.0, n 41) | sim (1.95 vs 1.70) | **nao** |
| C | -0.125 +- 0.108 (t -1.1, n 67) | -0.018 +- 0.147 (t -0.1, n 42) | -0.304 +- 0.148 (t -2.0, n 25) | -0.197 +- 0.165 (t -1.2, n 26) | -0.079 +- 0.144 (t -0.5, n 41) | nao (1.60 vs 1.70) | **nao** |
| D | -0.095 +- 0.099 (t -1.0, n 67) | -0.011 +- 0.135 (t -0.1, n 42) | -0.236 +- 0.139 (t -1.7, n 25) | -0.111 +- 0.134 (t -0.8, n 26) | -0.085 +- 0.140 (t -0.6, n 41) | nao (1.65 vs 1.70) | **nao** |
| E | -0.092 +- 0.101 (t -0.9, n 67) | -0.004 +- 0.140 (t -0.0, n 42) | -0.239 +- 0.130 (t -1.8, n 25) | -0.128 +- 0.164 (t -0.8, n 26) | -0.068 +- 0.129 (t -0.5, n 41) | sim (1.70 vs 1.70) | **nao** |

## POOL 8 SIMBOLOS (universo secundario - robustez)


### balance-area-breakout-v1 - n=241 (AVUV, IJS, IWM, IWN, IWO, IWV, SLYV, VBR)

Motor sem flatten (referencia ADR-018): avg R 0.066, PF_R 1.09, overnight 53 trades.  
Caminho sem alvo ate stop/flatten: MFE mediana 1.00R (media 1.63R), MAE mediana -1.00R, tocaram 1R: 49.8%, ATR14/R mediano 0.96, stop mediano 22 bp.

| Politica | n | avg R | SE | PF_R | WR% | net US$ | top-5 dias % | barras | saidas |
|---|---|---|---|---|---|---|---|---|---|
| A | 241 | -0.036 | 0.081 | 0.94 | 41.9 | 11542 | 184 | 7.1 | stop 114, target 74, end_of_day 53 |
| B | 241 | 0.018 | 0.077 | 1.03 | 39.0 | 16032 | 134 | 6.6 | stop 96, target 71, end_of_day 41, breakeven 33 |
| C | 241 | 0.158 | 0.124 | 1.30 | 46.9 | 20174 | 136 | 6.8 | trail_stop 98, stop 96, end_of_day 47 |
| D | 241 | 0.189 | 0.124 | 1.37 | 49.8 | 21937 | 125 | 6.4 | trail_stop 108, stop 96, end_of_day 37 |
| E | 241 | 0.080 | 0.086 | 1.16 | 52.7 | 15813 | 131 | 6.7 | stop 96, partial_1r+trail_stop 89, end_of_day 28, partial_1r+end_of_day 17, partial_1r+breakeven 11 |

Pareado vs A (d = R_P - R_A): media +- SE (t), pooled e por bloco / direcao. Criterio: t > 2 em 2025 E 2026, e PF_R(P) >= PF_R(A).

| Politica | pooled | 2025 | 2026 | long | short | PF_R >= A? | ADOTA? |
|---|---|---|---|---|---|---|---|
| B | 0.054 +- 0.022 (t 2.5, n 241) | 0.036 +- 0.021 (t 1.8, n 145) | 0.080 +- 0.045 (t 1.8, n 96) | 0.066 +- 0.027 (t 2.4, n 142) | 0.037 +- 0.036 (t 1.0, n 99) | sim (1.03 vs 0.94) | **nao** |
| C | 0.194 +- 0.094 (t 2.1, n 241) | 0.224 +- 0.146 (t 1.5, n 145) | 0.150 +- 0.086 (t 1.7, n 96) | 0.392 +- 0.147 (t 2.7, n 142) | -0.089 +- 0.081 (t -1.1, n 99) | sim (1.30 vs 0.94) | **nao** |
| D | 0.225 +- 0.094 (t 2.4, n 241) | 0.306 +- 0.143 (t 2.1, n 145) | 0.103 +- 0.091 (t 1.1, n 96) | 0.403 +- 0.148 (t 2.7, n 142) | -0.030 +- 0.078 (t -0.4, n 99) | sim (1.37 vs 0.94) | **nao** |
| E | 0.116 +- 0.057 (t 2.0, n 241) | 0.093 +- 0.080 (t 1.2, n 145) | 0.150 +- 0.077 (t 2.0, n 96) | 0.230 +- 0.082 (t 2.8, n 142) | -0.048 +- 0.069 (t -0.7, n 99) | sim (1.16 vs 0.94) | **nao** |

### range-extreme-fade-v1 - n=189 (AVUV, IJS, IWM, IWN, IWO, IWV, SLYV, VBR)

Motor sem flatten (referencia ADR-018): avg R 0.010, PF_R 1.02, overnight 16 trades.  
Caminho sem alvo ate stop/flatten: MFE mediana 0.80R (media 1.34R), MAE mediana -1.09R, tocaram 1R: 45.0%, ATR14/R mediano 1.04, stop mediano 22 bp.

| Politica | n | avg R | SE | PF_R | WR% | net US$ | top-5 dias % | barras | saidas |
|---|---|---|---|---|---|---|---|---|---|
| A | 189 | -0.175 | 0.080 | 0.72 | 41.3 | -5661 | - | 5.3 | stop 101, target 72, end_of_day 16 |
| B | 189 | -0.144 | 0.077 | 0.75 | 39.7 | -3868 | - | 4.9 | stop 92, target 69, breakeven 14, end_of_day 14 |
| C | 189 | -0.221 | 0.090 | 0.64 | 36.5 | -6753 | - | 5.8 | stop 94, trail_stop 71, end_of_day 24 |
| D | 189 | -0.180 | 0.086 | 0.70 | 40.7 | -4825 | - | 5.4 | stop 92, trail_stop 74, end_of_day 23 |
| E | 189 | -0.154 | 0.078 | 0.73 | 47.6 | -4211 | - | 5.7 | stop 92, partial_1r+trail_stop 57, partial_1r+breakeven 16, end_of_day 13, partial_1r+end_of_day 11 |

Pareado vs A (d = R_P - R_A): media +- SE (t), pooled e por bloco / direcao. Criterio: t > 2 em 2025 E 2026, e PF_R(P) >= PF_R(A).

| Politica | pooled | 2025 | 2026 | long | short | PF_R >= A? | ADOTA? |
|---|---|---|---|---|---|---|---|
| B | 0.030 +- 0.020 (t 1.5, n 189) | 0.033 +- 0.031 (t 1.1, n 117) | 0.025 +- 0.017 (t 1.5, n 72) | 0.040 +- 0.030 (t 1.3, n 85) | 0.022 +- 0.028 (t 0.8, n 104) | sim (0.75 vs 0.72) | **nao** |
| C | -0.046 +- 0.058 (t -0.8, n 189) | -0.019 +- 0.084 (t -0.2, n 117) | -0.090 +- 0.071 (t -1.3, n 72) | -0.069 +- 0.092 (t -0.7, n 85) | -0.028 +- 0.075 (t -0.4, n 104) | nao (0.64 vs 0.72) | **nao** |
| D | -0.005 +- 0.051 (t -0.1, n 189) | 0.014 +- 0.072 (t 0.2, n 117) | -0.036 +- 0.066 (t -0.5, n 72) | -0.081 +- 0.070 (t -1.2, n 85) | 0.057 +- 0.073 (t 0.8, n 104) | nao (0.70 vs 0.72) | **nao** |
| E | 0.021 +- 0.039 (t 0.5, n 189) | 0.050 +- 0.054 (t 0.9, n 117) | -0.027 +- 0.049 (t -0.5, n 72) | 0.011 +- 0.062 (t 0.2, n 85) | 0.029 +- 0.049 (t 0.6, n 104) | sim (0.73 vs 0.72) | **nao** |

### opening-reversal-v1 - n=224 (AVUV, IJS, IWM, IWN, IWO, IWV, SLYV, VBR)

Motor sem flatten (referencia ADR-018): avg R 0.026, PF_R 1.04, overnight 22 trades.  
Caminho sem alvo ate stop/flatten: MFE mediana 1.21R (media 1.68R), MAE mediana -1.06R, tocaram 1R: 54.0%, ATR14/R mediano 0.83, stop mediano 30 bp.

| Politica | n | avg R | SE | PF_R | WR% | net US$ | top-5 dias % | barras | saidas |
|---|---|---|---|---|---|---|---|---|---|
| A | 224 | 0.051 | 0.089 | 1.08 | 43.8 | 6633 | 205 | 7.1 | stop 121, target 81, end_of_day 22 |
| B | 224 | 0.020 | 0.081 | 1.04 | 33.9 | 3601 | 322 | 5.3 | stop 99, target 68, breakeven 49, end_of_day 8 |
| C | 224 | -0.018 | 0.076 | 0.96 | 52.2 | 1145 | 1021 | 5.3 | trail_stop 119, stop 99, end_of_day 6 |
| D | 224 | 0.018 | 0.079 | 1.04 | 50.0 | 4855 | 267 | 5.3 | trail_stop 119, stop 99, end_of_day 6 |
| E | 224 | 0.013 | 0.070 | 1.03 | 55.8 | 3260 | 301 | 5.3 | partial_1r+trail_stop 117, stop 99, end_of_day 4, partial_1r+end_of_day 2, partial_1r+breakeven 2 |

Pareado vs A (d = R_P - R_A): media +- SE (t), pooled e por bloco / direcao. Criterio: t > 2 em 2025 E 2026, e PF_R(P) >= PF_R(A).

| Politica | pooled | 2025 | 2026 | long | short | PF_R >= A? | ADOTA? |
|---|---|---|---|---|---|---|---|
| B | -0.030 +- 0.038 (t -0.8, n 224) | -0.030 +- 0.054 (t -0.5, n 132) | -0.031 +- 0.050 (t -0.6, n 92) | -0.032 +- 0.057 (t -0.6, n 89) | -0.029 +- 0.050 (t -0.6, n 135) | nao (1.04 vs 1.08) | **nao** |
| C | -0.068 +- 0.050 (t -1.4, n 224) | -0.022 +- 0.071 (t -0.3, n 132) | -0.135 +- 0.070 (t -1.9, n 92) | -0.093 +- 0.074 (t -1.3, n 89) | -0.052 +- 0.068 (t -0.8, n 135) | nao (0.96 vs 1.08) | **nao** |
| D | -0.032 +- 0.048 (t -0.7, n 224) | -0.013 +- 0.068 (t -0.2, n 132) | -0.060 +- 0.065 (t -0.9, n 92) | -0.034 +- 0.066 (t -0.5, n 89) | -0.031 +- 0.068 (t -0.5, n 135) | nao (1.04 vs 1.08) | **nao** |
| E | -0.038 +- 0.049 (t -0.8, n 224) | -0.016 +- 0.068 (t -0.2, n 132) | -0.069 +- 0.069 (t -1.0, n 92) | -0.044 +- 0.075 (t -0.6, n 89) | -0.034 +- 0.065 (t -0.5, n 135) | nao (1.03 vs 1.08) | **nao** |

## Descritivo (pool): modificacoes de stop por trade e barras ate 1R

| estrategia | n | mod/trade C | mod/trade D | mod/trade E | max mod D | tocou 1R % | barra do 1R mediana | 1R na barra do fill % | 1R ate 2 barras % | 1R ate 4 barras % |
|---|---|---|---|---|---|---|---|---|---|---|
| bab | 241 | 1.37 | 1.33 | 1.30 | 9 | 50 | 1.0 | 21 | 50 | 68 |
| ref | 189 | 0.97 | 0.99 | 0.93 | 7 | 45 | 2.0 | 13 | 44 | 69 |
| orv | 224 | 1.17 | 1.49 | 1.17 | 7 | 54 | 1.0 | 29 | 65 | 88 |

Desfecho em A dos trades que NAO tocaram 1R nas 4 primeiras barras (fill + 3), pool:

| estrategia | n | stop | target | end_of_day | avg R (A) |
|---|---|---|---|---|---|
| bab | 161 | 101 | 15 | 45 | -0.565 |
| ref | 131 | 94 | 23 | 14 | -0.620 |
| orv | 117 | 99 | 7 | 11 | -0.812 |

Nota: qualquer regra derivada desta tabela (ex.: barreira vertical em k barras) e uma tentativa nova, ja contaminada por esta leitura; so vale com holdout (paper forward ou dados apos 02/09/2026).


## Por simbolo (pool): avg R de A e de cada politica

| estrategia | simbolo | n | A | B | C | D | E | tocou 1R % |
|---|---|---|---|---|---|---|---|---|
| bab | AVUV | 35 | -0.212 | -0.034 | 0.008 | 0.074 | 0.015 | 54 |
| bab | IJS | 25 | 0.233 | 0.165 | 0.743 | 0.709 | 0.402 | 52 |
| bab | IWM | 23 | -0.049 | 0.096 | 0.438 | 0.316 | 0.275 | 57 |
| bab | IWN | 41 | 0.163 | 0.169 | 0.249 | 0.311 | 0.146 | 54 |
| bab | IWO | 13 | -0.319 | -0.184 | -0.314 | -0.398 | -0.217 | 38 |
| bab | IWV | 39 | -0.095 | -0.131 | -0.275 | -0.200 | -0.241 | 41 |
| bab | SLYV | 29 | -0.096 | 0.023 | 0.422 | 0.470 | 0.202 | 45 |
| bab | VBR | 36 | -0.056 | -0.026 | 0.045 | 0.128 | 0.075 | 53 |
| orv | AVUV | 28 | -0.249 | -0.308 | -0.272 | -0.246 | -0.279 | 39 |
| orv | IJS | 24 | 0.162 | 0.158 | 0.150 | 0.160 | 0.194 | 62 |
| orv | IWM | 36 | 0.435 | 0.403 | 0.282 | 0.240 | 0.310 | 67 |
| orv | IWN | 31 | 0.233 | 0.309 | 0.141 | 0.255 | 0.180 | 61 |
| orv | IWO | 39 | 0.122 | -0.088 | -0.142 | -0.062 | -0.106 | 49 |
| orv | IWV | 20 | -0.529 | -0.458 | -0.442 | -0.441 | -0.380 | 40 |
| orv | SLYV | 22 | 0.027 | 0.123 | 0.314 | 0.267 | 0.306 | 68 |
| orv | VBR | 24 | -0.135 | -0.203 | -0.293 | -0.167 | -0.238 | 42 |
| ref | AVUV | 29 | 0.158 | 0.065 | -0.136 | -0.001 | -0.017 | 52 |
| ref | IJS | 20 | -0.427 | -0.413 | -0.619 | -0.576 | -0.431 | 40 |
| ref | IWM | 27 | -0.659 | -0.542 | -0.648 | -0.457 | -0.496 | 33 |
| ref | IWN | 26 | -0.053 | 0.024 | -0.046 | -0.157 | -0.004 | 50 |
| ref | IWO | 22 | -0.370 | -0.279 | -0.265 | -0.238 | -0.232 | 41 |
| ref | IWV | 19 | -0.034 | 0.019 | 0.003 | 0.115 | 0.043 | 53 |
| ref | SLYV | 21 | 0.410 | 0.410 | 0.243 | 0.297 | 0.315 | 62 |
| ref | VBR | 25 | -0.389 | -0.389 | -0.243 | -0.368 | -0.355 | 32 |