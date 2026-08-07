# Estratégia: Failure Test Long v1

> **Status:** especificação (Fases 1–3 do framework concluídas neste documento). Sem código implementado.
> **Framework:** `docs/strategy-analysis-framework.md`
> **Análise do livro:** `docs/books/analysis/grimes-art-science-ta.md`

---

## 1. Fonte

* **Livro:** The Art and Science of Technical Analysis: Market Structure, Price Action, and Trading Strategies
* **Autor:** Adam H. Grimes (Wiley, 2012)
* **Seções relevantes:**
  * **Capítulo 6 — Practical Trading Templates, seção "Failure Test"** (setup principal: conceito, gatilho, stop, alvo, padrões de falha)
  * Capítulo 2 — The Market Cycle and the Four Trades (classificação: suporte segurando / término de tendência)
  * Capítulo 3 — On Trends (indicadores do autor: Keltner EMA20 ± 2,25×ATR; MACD modificado)
  * Capítulo 5 — Interfaces between Trends and Ranges (springs/upthrusts; níveis significativos)
  * Capítulo 7 — Tools for Confirmation (MACD modificado; janela de 40 barras; divergência)
  * Capítulo 8 — Trade Management (stop inicial fora do ruído; alvos; parcial em 1R)
  * Capítulo 9 — Risk Management (sizing fracionário fixo; "smaller size" para este setup)
  * Apêndice B — definição exata do MACD modificado (linha rápida = SMA(3) − SMA(10); linha lenta = SMA(16) da rápida)
* **Origens históricas citadas pelo autor:** Wyckoff (spring/upthrust, ~100 anos); Victor Sperandeo, padrão "2B" (1993).

---

## 2. Conceito em uma frase

> O mercado sonda abaixo de um suporte claramente definido — disparando stops e prendendo vendedores — mas **não há convicção real** além do nível: o preço **fecha de volta acima do suporte** na mesma barra ou na seguinte. Entramos comprados contra essa falha, com stop logo abaixo do extremo da sonda e expectativa de movimento rápido a favor (1–3 barras).

Grimes (Cap. 6): *"Markets probe for stop orders and activity beyond significant price levels. Many times, there is no real conviction behind these moves, and the moves fail and reverse quickly once the stop orders are triggered. Entering after such a move allows for excellent reward/risk potential with a clearly defined risk point."*

**Distinção crítica (guardrail do próprio autor):** isto **não** é "comprar em suporte". Grimes explicitamente **não** opera "simple buying or selling at support levels" (Cap. 6, Summary) — ele só entra **depois** que o suporte foi violado e a violação **falhou**, com ponto de risco inequívoco.

---

## 3. Fase 1 — Extração do conceito (8 perguntas do framework)

1. **Nome do setup:** Failure Test (variante long = *spring* de Wyckoff / "2B" de compra).
2. **Contexto de mercado:** mercado **sobreestendido / "primed for reversal"**. Melhores exemplos: tendências maduras e estendidas, geralmente com **divergência de momentum** no timeframe operacional; ou consolidação estendida encostada no nível (traders presos amplificam a reversão). (Cap. 6)
3. **Timeframe:** o autor afirma que o padrão funciona em todos os timeframes (*"it works just as well today"*); exemplos do livro em diário e FX. **Nossa adaptação:** candles de 15min, day trade.
4. **Sinais de entrada:** para short — *"the market trades above a clearly defined resistance area, but immediately reverses on the same or the following bar and closes back under the resistance... Enter short on the close of the bar that closes back below resistance. The buy setup is exactly symmetrical."* (Cap. 6). Para long: penetração abaixo do suporte + fechamento de volta **acima** do suporte, na mesma barra ou na seguinte; entrada **no fechamento** da barra de recuperação.
5. **Stop:** *"a hard stop must be entered just beyond the extreme of the test beyond the level"* — logo abaixo da mínima da excursão além do suporte (se a barra anterior à entrada marcou o extremo, o stop vai abaixo **dela**). Respeito absoluto: *"Respect the stop level without question."* (Cap. 6)
6. **Alvo/saída:** parcial quando o lucro iguala o risco inicial (1R); restante discricionário, **sem** expectativa-base de reversão completa de tendência. Validação: *"the trade should be immediately profitable (within one to three bars on the trading time frame)"*; **consolidação perto do nível sem atingir o primeiro alvo é mau sinal** — reduzir/sair. (Cap. 6)
7. **Quando NÃO operar:** sem condição de sobreextensão (mercado não "primed"); **nunca adicionar a posição perdedora**; risco de gap overnight → operar com **tamanho menor** que outros setups (*"it may make sense to trade these on smaller size and risk compared to other setups"*). Reentrada na **segunda falha** após stop-out é *"virtually obligatory"* — o risco somado das duas tentativas não deve exceder o risco máximo por trade. (Cap. 6)
8. **Estatísticas citadas:** **nenhuma numérica por setup** (o autor defende validação pelo próprio trader). Afirmações qualitativas: *"This is the simplest and most clearly defined of all the patterns in this chapter"*; padrão centenário que segue funcionando. Regras transversais com números: stops iniciais raramente a menos de **2 ATRs** e **nunca dentro do range médio de 1 barra** (Cap. 8); parcial de **25–33% em 1R** (Cap. 8); risco por trade **< 1% conservador, ≥ 3% extremamente agressivo** (Cap. 9).

---

## 4. Contexto de Mercado (filtro obrigatório)

O setup só é válido quando o mercado está **"primed for reversal"** — sobreestendido contra um suporte significativo.

### Timeframes

```text
Timeframe operacional: 15min
Timeframe de contexto: 1h (opcional na v1 — ver seção 14)
Timeframe macro: não usado na v1
```

### Condições objetivas no timeframe operacional (15min)

> Todas as regras numéricas desta seção são **interpretação nossa** sobre os conceitos do Cap. 6, exceto os parâmetros dos indicadores, que são literais do autor (Cap. 3 e Apêndice B).

```text
[1] SOBREEXTENSÃO (pelo menos UMA das condições):
    a) preço tocou/fechou abaixo do canal de Keltner inferior
       (EMA20 − 2,25 × ATR14) em algum dos últimos `climax_lookback_candles` candles
       — parâmetros do canal são literais do autor (Cap. 3); o uso como
       proxy de "overextended" é interpretação nossa;
    b) queda acumulada ≥ `overextension_atr_mult` × ATR14 desde a última
       máxima de swing (interpretação nossa);
    c) divergência de momentum: nova mínima de preço SEM nova mínima na linha
       rápida do MACD modificado (SMA3 − SMA10) vs. janela de 40 barras
       — indicador literal (Apêndice B); janela de 40 barras sugerida pelo
       próprio autor para o MACD (Cap. 7); o uso como filtro de contexto
       segue "usually be accompanied by momentum divergence" (Cap. 6).

[2] NÍVEL SIGNIFICATIVO: existe suporte S conforme seção 5.1.
    Fonte: "clearly defined support area" / níveis "tested cleanly multiple
    times" (Caps. 5–6).

[3] SEM TENDÊNCIA DE ALTA ESTABELECIDA CONTRA O TRADE:
    este setup é contra o movimento de curto prazo mas NÃO deve ser operado
    contra um impulso de baixa fresco e fortíssimo (clímax em andamento):
    rejeitar se o MACD rápido estiver em nova mínima extrema E a última
    barra tiver range > `climax_bar_atr_mult` × ATR14 (interpretação nossa,
    derivada de Cap. 7: "in these extreme situations... the indicator should
    be disregarded" e da cautela geral com climaxes, Caps. 3/6/10).
```

### Regras de rejeição de contexto

```text
REJEITAR se nenhuma condição de sobreextensão [1] estiver presente
REJEITAR se não existir suporte válido (seção 5.1)
REJEITAR se houver clímax de venda em andamento [3]
REJEITAR se ATR(14) percentual > max_atr_pct (volatilidade anormal — padrão do projeto)
```

---

## 5. Setup de Entrada (Failure Test Long)

### 5.1 Definição do suporte (nível S)

> Interpretação nossa sobre "clearly defined support area" e "levels that have been tested cleanly multiple times" (Caps. 5–6).

```text
[1] S = preço de um pivô de mínima (swing low) identificado nos últimos
    `level_lookback_candles` candles (default: 60 candles de 15min ≈ 1,5 dia)
[2] O nível deve ter sido TOCADO pelo menos `level_min_touches` vezes
    (default: 2), contando toques em que |low − S| ≤ `level_touch_tolerance_pct`
    (default: 0,10%) sem fechamento decisivo abaixo
[3] Nenhum fechamento abaixo de S − tolerance nos últimos `level_lookback_candles`
    candles ANTES da sonda atual (se já fechou abaixo com convicção, o nível
    já falhou — não é mais "support holding")
[4] O nível deve ter pelo menos `level_min_age_candles` candles de idade
    (default: 8) — nível recém-formado não é "visible to all market participants"
```

### 5.2 A sonda (penetração)

> Literal do livro: *"the market trades above a clearly defined resistance area, but immediately reverses..."* — simétrico para long.

```text
[1] PENetração: low[i] < S (a barra i opera abaixo do suporte)
[2] Profundidade da sonda limitada: (S − low[i]) ≤ `probe_max_atr_mult` × ATR14
    (default: 1,0) — interpretação nossa: sondas profundas demais sugerem
    rompimento real, não "lack of conviction beyond the level"
[3] A sonda ocorre em no máximo `probe_max_bars` barras consecutivas
    (default: 2) — "There can be significant volatility, volume, and activity
    on the breakout, but there should be no real conviction beyond the level"
```

### 5.3 A falha (recuperação = barra de sinal)

> Literal do livro: *"immediately reverses on the same or the following bar and closes back under the resistance"* (simétrico: fecha de volta **acima** do suporte).

```text
[1] close[i] > S  (recuperação na MESMA barra da sonda — spring clássico)
    OU
    close[i] ≤ S e close[i+1] > S  (recuperação na barra SEGUINTE)
[2] A barra de recuperação (= barra de sinal) deve fechar no `signal_close_min_position`
    superior do próprio range (default: 50%) — interpretação nossa para
    operacionalizar "reverses"; o livro não quantifica a força do fechamento
[3] Extremo da excursão E = min(low das barras de sonda, incluindo a barra
    anterior à entrada se ela marcou a mínima) — literal: "if the previous day
    set the high-water mark for the excursion... place the stop above the high
    of the bar preceding the entry bar" (Cap. 6, exemplo EURUSD)
```

### 5.4 Regras de rejeição do setup

```text
REJEITAR se não houver penetração do suporte (sem sonda, não é failure test)
REJEITAR se a sonda exceder `probe_max_bars` barras sem recuperação
REJEITAR se a sonda for profunda demais (> `probe_max_atr_mult` × ATR14)
REJEITAR se a barra de recuperação fechar abaixo de `signal_close_min_position` do range
REJEITAR se houver fechamento prévio abaixo do nível (nível já rompido)
REJEITAR se o suporte tiver menos de `level_min_touches` toques
```

---

## 6. Entrada

**Regra literal do livro (Cap. 6):** *"Enter short on the close of the bar that closes back below resistance."* — entrada **no fechamento** da barra de recuperação.

**Adaptação obrigatória à nossa infra (interpretação nossa, explícita):** nossos candles de 15min chegam **~30s após o fechamento**, então "entrar no fechamento" é impossível. Duas alternativas, configuráveis:

```text
Tipo de ordem (default): buy stop
Gatilho: máxima da barra de recuperação + 1 tick
Condição: ordem enviada assim que a barra de recuperação é processada;
          expira após `entry_validity_candles` candles sem rompimento
          (mesma semântica da ADR-009 / pullback-trend-v1)
Justificativa: o gatilho stop exige confirmação adicional de momentum a favor,
          o que compensa parcialmente a entrada ~30s atrasada e é coerente com
          o espírito de "no real conviction beyond the level" — se o preço nem
          supera a máxima da barra de recuperação, a "falha" não se confirmou.

Alternativa configurável: entry_order_type = "market_next_open"
          (market na abertura do candle seguinte) — mais fiel ao livro,
          sem confirmação extra. Default NÃO recomendado na v1.
```

```text
Validação pós-entrada (literal, Cap. 6): "the trade should be immediately
profitable (within one to three bars on the trading time frame)".
Implementação (interpretação nossa): SAÍDA POR TEMPO — se após
`validation_bars` candles (default: 3) o lucro flutuante < `validation_min_r`
(default: 0,5R), encerrar a mercado no fechamento desse candle.
Regra associada (literal): "consolidation near the level... is a bad sign...
Reduce exposure or close out these trades if they do not work quickly".
```

> **Dependência de infra:** a saída por tempo exige gestão ativa de posição
> (hoje o bracket é "fire and forget" com TP/SL server-side). Se a infra não
> suportar na v1, registrar como limitação conhecida e avaliar no backtest
> o impacto de NÃO ter a saída por tempo. **Pendente de decisão do dono.**

---

## 7. Stop e Alvo

```text
Stop inicial (literal, Cap. 6):
  stop = E − 1 tick − jitter
  onde E = extremo da excursão abaixo do suporte (seção 5.3[3])
  e jitter ∈ [0, `stop_jitter_atr_mult` × ATR14] (default: 0,10)
  — o "jitter" além dos níveis óbvios é conceito literal do autor:
    "I... introduce a small random 'jitter' element to stop placement...
    markets tend to seek out those stop levels" (Cap. 6, stops de pullback,
    aplicado pelo autor como princípio geral; para o failure test o livro
    diz apenas "just beyond the extreme").

Sanidade do stop (literal, Cap. 8):
  REJEITAR se (entrada − stop) < 1 × range médio de barra(20)
  — "If you try to place your initial stops closer than one average bar's
    range, you are probably working within the noise level and have
    significantly impaired whatever edge you might have had."
  REJEITAR se (entrada − stop) > `max_stop_atr_mult` × ATR14 (default: 3,0)
  — interpretação nossa: sonda funda demais gera stop largo e RR ruim
    para alvo intraday (o livro sugere stops de 2–4 ATRs como guideline
    geral, Cap. 8).

Alvo (adaptação — ver nota):
  Alvo único: `target_r_multiple` × risco (default: 1,5R)
  — O livro prescreve PARCIAL em 1R ("take profit on the first part when
    the profit equals the initial risk", Cap. 6; "usually exit between 25 and
    33 percent of the position... at a profit equal to my initial risk",
    Cap. 8) + restante discricionário. Nossa infra tem TP ÚNICO no bracket,
    então a v1 usa alvo único de 1,5R como meio-termo. INTERPRETAÇÃO NOSSA.
    Candidato de evolução: bracket com 2 TPs (parcial 33% em 1R + resto em
    estrutura/MMO), alinhando à gestão literal do autor.

Saída por tempo: ver seção 6 (validação em `validation_bars` barras).
Saída de fim de dia: flat obrigatório até 15:30 ET (padrão do projeto — e
  elimina o risco de gap overnight que o autor aponta como principal
  cautela deste setup, Cap. 6: "there is significant gap risk with trades
  held overnight").
```

---

## 8. Gestão de Risco

```text
Risco por trade: 0,5% do capital
  — LITERAL na direção, interpretação no número: "it may make sense to trade
    these on smaller size and risk compared to other setups" (Cap. 6). Nosso
    padrão é 1%; metade disso operacionaliza "smaller size and risk".
Perda máxima diária: 2% do capital (padrão do projeto)
Máximo de trades por dia: 3 (padrão do projeto)
Risco-retorno mínimo: 1:1,2
  — interpretação nossa: com alvo de 1,5R, rejeitar sinais cujo RR efetivo
    (considerando entrada stop no gatilho) fique abaixo de 1,2
Spread máximo permitido: 0,05% (padrão do projeto)
Volatilidade máxima: ATR(14) percentual <= 1,5% no 15min (padrão do projeto)
Máximo de perdas consecutivas antes de parar: 3 (padrão do projeto)
NUNCA adicionar a posição perdedora (literal, Cap. 6: "it is important to
  not add to losing trades. Respect the stop level without question.")
Nunca abrir nova posição com posição aberta no mesmo ativo (regra de ouro do projeto)
```

### Reentrada após stop-out (segunda falha)

```text
Literal (Cap. 6): a segunda falha, logo após um stop-out, "is also an
excellent entry... (and is virtually obligatory)... the sum of the risk on
both trades should not be significantly larger than the maximum risk taken
on other types of trades."

Decisão para v1 (interpretação nossa / pendente do dono):
  allow_reentry = false (default) — a v1 prioriza simplicidade e auditabilidade.
  Parâmetros já previstos no config para a v2: allow_reentry, reentry_window_candles
  (default: 3), com risco da reentrada descontado do risco do dia.
```

### Cálculo do tamanho da posição

```text
risco_monetario = capital * risco_por_trade        # 0,5%
distancia_stop  = |preco_entrada - stop|
quantidade      = floor(risco_monetario / distancia_stop)
```

Idêntico ao padrão do projeto e ao fracionário fixo do Cap. 9: *"Trade size = Desired dollar risk ÷ Per-unit risk."*

---

## 9. Fase 2 — Tabela Subjetivo → Objetivo (consolidada)

| Conceito subjetivo (livro) | Regra objetiva (candles 15min OHLCV) | Origem |
|---|---|---|
| "clearly defined support area" | Pivô de mínima com ≥ 2 toques em 60 candles, tolerância 0,10%, sem fechamento prévio abaixo, idade ≥ 8 candles | Caps. 5–6 (conceito) / números: **interpretação** |
| "overextended or primed for reversal" | Toque no Keltner inferior (EMA20 − 2,25×ATR14) OU queda ≥ 2,0×ATR14 do último swing OU divergência no MACD modificado | Canal: literal (Cap. 3) / gatilhos: **interpretação** |
| "momentum divergence on the trading time frame" | Nova mínima de preço sem nova mínima no MACD rápido (SMA3−SMA10) vs. janela de 40 barras | Indicador literal (Ap. B); janela 40: autor (Cap. 7) |
| "trades below support but reverses and closes back above" | `low < S` e `close > S` (mesma barra) ou `close > S` na barra seguinte | Literal (Cap. 6) |
| "no real conviction beyond the level" | Sonda ≤ 2 barras e profundidade ≤ 1,0×ATR14 | Conceito literal / números: **interpretação** |
| "enter on the close of the bar" | Buy stop na máxima da barra de recuperação + 1 tick (atraso de ~30s inviabiliza "no fechamento") | **Interpretação/adaptação de infra** |
| "stop just beyond the extreme of the test" | `min(low da sonda) − 1 tick − jitter(≤0,1×ATR14)` | Literal + jitter literal (Cap. 6) |
| "stops must not be within the noise" | Rejeitar se distância do stop < 1×range médio(20) | Literal (Cap. 8) |
| "partial profit at 1R" | Alvo único 1,5R (bracket de TP único) | **Interpretação/adaptação de infra** |
| "profitable within one to three bars" | Saída a mercado se lucro < 0,5R após 3 candles | Literal (Cap. 6) / limiar 0,5R: **interpretação** |
| "consolidation near the level is a bad sign" | Coberta pela saída por tempo acima | Literal (Cap. 6) |
| "smaller size and risk" | Risco 0,5% por trade (metade do padrão 1%) | Literal na direção / número: **interpretação** |
| "second failure entry virtually obligatory" | `allow_reentry=false` na v1; parâmetros previstos | Literal (Cap. 6) / adiamento: **decisão de MVP** |

---

## 10. Fase 3 — Especificação Técnica

```text
Inputs:
  - candles 15min (OHLCV) do ativo, incluindo histórico ≥ level_lookback_candles
  - indicadores calculados no 15min:
      EMA(20), ATR(14), canal de Keltner (EMA20 ± 2,25×ATR14)
      MACD modificado: linha rápida = SMA(3) − SMA(10); lenta = SMA(16) da rápida
      range médio de barra (20), volume médio (20)
  - pivôs de mínima (swing lows) com índice e preço
  - configuração de risco (seção 8) e parâmetros (seção 14)
  - calendário/horário ET (janela operacional, seção 12)

Outputs:
  - Signal (buy) com:
      entry_price       = máxima da barra de recuperação + 1 tick
      stop_price        = extremo da sonda − 1 tick − jitter
      target_price      = entry + target_r_multiple × (entry − stop)
      direction         = Long
      motivo da entrada = "failure_test_long: sonda abaixo de S=<valor> falhou,
                           recuperação em <n> barra(s), contexto=<climax|queda|divergencia>"
      metadados auditáveis: S, toques do nível, low da sonda, profundidade em ATR,
                           valores de EMA20/ATR14/Keltner/MACD no candle do sinal,
                           RR efetivo, distância do stop em ATR e em ranges de barra
  - Rejected { reason, details } — ver seção 11

Estado interno:
  - posição atual (para regra posicao_ja_aberta)
  - trades do dia, perda acumulada do dia, perdas consecutivas
  - último sinal emitido e candle de emissão
  - registro do último stop-out do setup (para reentrada — v2; v1 só registra)
  - candles desde a entrada (para saída por tempo / validação em 3 barras)

Eventos que disparam análise:
  - fechamento de candle 15min (principal)
  - candle seguinte à entrada (gestão: validação em 1–3 barras)
  - NÃO há evento de preço em tempo real (sem quotes) — decisões só no
    fechamento de candle
```

---

## 11. Rejeições Registradas pelo Bot

> Nomenclatura em snake_case inglês. Coluna "Domínio" indica se já existe variante correspondente no enum `RejectionReason` de `trader-domain` (a ser mapeada na implementação) ou se é **nova** (exige extensão do domínio — fora do escopo deste documento).

```text
# Contexto
not_overextended                  # nova — nenhuma condição da seção 4[1]
climax_in_progress                # nova — clímax de venda em andamento (4[3])
high_volatility                   # existe (HighVolatility)

# Nível
support_level_not_found           # nova — sem pivô que atenda 5.1
support_not_tested_enough         # nova — < level_min_touches toques
support_already_broken            # nova — fechamento prévio abaixo do nível
level_too_recent                  # nova — < level_min_age_candles

# Sonda / sinal
no_probe                          # nova — sem penetração do suporte
probe_too_deep                    # nova — profundidade > probe_max_atr_mult × ATR14
probe_too_long                    # nova — > probe_max_bars sem recuperação
no_recovery_close                 # nova — sem fechamento de volta acima de S
weak_recovery_bar                 # nova — fechamento < signal_close_min_position do range
setup_invalidated                 # existe — preço já passou do gatilho antes do envio
entry_expired                     # nova — ordem stop expirou sem rompimento (entry_validity_candles)

# Risco / estruturais
stop_within_noise                 # nova — distância < 1×range médio(20) (Cap. 8)
stop_too_wide                     # nova — distância > max_stop_atr_mult × ATR14
poor_risk_reward                  # existe (PoorRiskReward)
high_spread                       # existe (HighSpread)
outside_trading_hours             # existe (OutsideTradingHours)
daily_loss_limit_reached          # existe (DailyLossLimitReached)
max_trades_reached                # existe (MaxTradesReached)
consecutive_losses                # existe (ConsecutiveLosses)
position_already_open             # existe (PositionAlreadyOpen)
reentry_disabled                  # nova — segunda falha detectada com allow_reentry=false (v1: apenas registra)
```

---

## 12. Filtros de Horário e Ativo

```text
Ativos: SPY, QQQ, IWM (mesmos do projeto; iniciar por SPY, como na pullback-trend-v1)
Horário de operação: 09:45 – 15:30 ET
  — padrão do projeto; coerente com o autor, que destaca o padrão como
    "especially powerful for intraday traders when combined with time-of-day
    influences" (Cap. 6, sobre o Anti, princípio aplicável aqui) — INTERPRETAÇÃO.
Flat obrigatório: nenhuma posição após 15:30 ET
  — elimina o risco de gap overnight, principal cautela do autor para este
    setup (Cap. 6: "there is significant gap risk with trades held overnight...
    gap opens beyond the stop").
Não operar em dias de relatórios macro agendados (ex.: FOMC, payroll) antes do anúncio
  — padrão do projeto.
```

---

## 13. Métricas de Avaliação

> Grimes **não cita estatísticas por setup** — os critérios abaixo derivam dos padrões do projeto (`pullback-trend-v1.md`, seção 10), ajustados à natureza contra-tendência do setup: win rate esperado **menor** que estratégias com tendência (o próprio autor: *"Trend termination plays are not usually high-probability plays, but the compensation is that winning trades tend to offer potential rewards much larger than the initial risk"*, Cap. 2).

### Métricas mínimas para aprovação em backtest

```text
número mínimo de trades: 50
win rate mínimo: 35%            # (projeto: 40%; reduzido p/ setup contra-tendência — interpretação nossa)
profit factor mínimo: 1,3
drawdown máximo: 10%
média de R por trade: > 0,15
expectativa matemática positiva
% de trades validados em ≤ 3 barras: registrar (métrica diagnóstica do setup,
  derivada de "immediately profitable (within one to three bars)", Cap. 6)
taxa de reentrada que seria acionada (segunda falha): registrar para decisão da v2
```

### Métricas para aprovação em paper trading

```text
mínimo 20 trades em paper
resultado próximo ao backtest (±30% nas métricas principais)
nenhuma violação de regra de risco
latência entrada×fechamento: medir o slippage médio do gatilho stop vs.
  "close da barra de recuperação" (quantifica o custo da nossa adaptação de entrada)
uptime do bot sem falhas críticas
```

---

## 14. Decisões de Implementação (proposta — NÃO implementar a partir deste doc sem aprovação)

### Onde viveria no código

```text
trader-core/src/strategies/failure_test_long_v1/
  mod.rs      → estrutura pública e trait Strategy
  context.rs  → sobreextensão, clímax, filtros de contexto
  levels.rs   → detecção de pivôs e do nível S (NOVO módulo vs. pullback-trend-v1)
  setup.rs    → detecção de sonda + recuperação (failure test)
  entry.rs    → gatilho, stop (com jitter), alvo, validação em 3 barras
  config.rs   → parâmetros da estratégia (Deserialize)
```

### Configurações parametrizáveis (valores default propostos)

```rust
pub struct FailureTestLongV1Config {
    // --- nível (seção 5.1) — interpretação nossa ---
    pub level_lookback_candles: usize,      // 60
    pub level_min_touches: usize,           // 2
    pub level_touch_tolerance_pct: Decimal, // 0.001 (0,10%)
    pub level_min_age_candles: usize,       // 8

    // --- contexto (seção 4) ---
    pub keltner_ema_period: usize,          // 20  (literal, Cap. 3)
    pub keltner_atr_mult: f64,              // 2.25 (literal, Cap. 3)
    pub atr_period: usize,                  // 14
    pub climax_lookback_candles: usize,     // 10 (interpretação)
    pub overextension_atr_mult: f64,        // 2.0 (interpretação)
    pub macd_fast_sma: usize,               // 3  (literal, Ap. B)
    pub macd_slow_sma: usize,               // 10 (literal, Ap. B)
    pub macd_signal_sma: usize,             // 16 (literal, Ap. B)
    pub macd_lookback_candles: usize,       // 40 (autor, Cap. 7)
    pub climax_bar_atr_mult: f64,           // 2.5 (interpretação)

    // --- sonda e sinal (seções 5.2–5.3) ---
    pub probe_max_bars: usize,              // 2 (interpretação)
    pub probe_max_atr_mult: f64,            // 1.0 (interpretação)
    pub signal_close_min_position: f64,     // 0.50 (interpretação)

    // --- entrada / stop / alvo (seções 6–7) ---
    pub entry_order_type: String,           // "stop" (default) | "market_next_open"
    pub entry_validity_candles: usize,      // 2 (interpretação, análogo à ADR-009)
    pub stop_jitter_atr_mult: f64,          // 0.10 (jitter: conceito literal, Cap. 6)
    pub min_stop_bar_ranges: f64,           // 1.0 (literal, Cap. 8)
    pub max_stop_atr_mult: f64,             // 3.0 (interpretação)
    pub target_r_multiple: f64,             // 1.5 (interpretação — TP único)
    pub min_risk_reward: f64,               // 1.2 (interpretação)
    pub validation_bars: usize,             // 3 (literal: "one to three bars", Cap. 6)
    pub validation_min_r: f64,              // 0.5 (interpretação)

    // --- reentrada (seção 8) ---
    pub allow_reentry: bool,                // false (v1; livro: "virtually obligatory" → v2)
    pub reentry_window_candles: usize,      // 3 (interpretação)

    // --- risco (seção 8) ---
    pub risk_per_trade: Decimal,            // 0.005 (0,5% — "smaller size", Cap. 6)
    pub max_daily_loss: Decimal,            // 0.02
    pub max_trades_per_day: usize,          // 3
    pub max_spread_pct: Decimal,            // 0.0005
    pub max_atr_pct: Decimal,               // 0.015
}
```

**Nenhuma regra hardcoded** — todos os limiares acima vêm do config (regra de ouro do framework e do AGENTS.md).

---

## 15. Plano de Testes Unitários (candles sintéticos)

> Convenção igual à da `pullback-trend-v1`: séries artificiais de candles 15min construídas para cada caso. Valores numéricos ilustrativos; ATR médio ≈ 1,00 ponto nos fixtures para facilitar asserções.

```text
CASO 1 — setup perfeito (DEVE gerar sinal):
  - Downtrend estendido: 15 candles de baixa, preço cruza o Keltner inferior
  - Suporte S = 100,00 com 2 toques anteriores (pivôs em ~100,02 e 99,98)
  - Barra de sonda: low 99,40 (0,6×ATR abaixo), close 100,30 (> S, terço superior)
  - Esperado: Signal Long; entry = máxima da barra de recuperação + tick;
    stop = 99,40 − tick − jitter; target = entry + 1,5R

CASO 2 — falha SEM recuperação (DEVE rejeitar):
  - Igual ao caso 1, mas a sonda fecha ABAIXO de S e a barra seguinte
    também fecha abaixo (rompimento real)
  - Esperado: Rejected(no_recovery_close) e, após probe_max_bars,
    Rejected(probe_too_long)

CASO 3 — suporte fraco (DEVE rejeitar):
  - Sonda e recuperação perfeitas, mas nível com apenas 1 toque prévio
  - Esperado: Rejected(support_not_tested_enough)
  - Variante: nível com fechamento prévio abaixo → Rejected(support_already_broken)

CASO 4 — sem sobreextensão (DEVE rejeitar):
  - Sonda e recuperação em suporte, mas preço dentro do canal de Keltner,
    sem queda ≥ 2×ATR e sem divergência de MACD
  - Esperado: Rejected(not_overextended)

CASO 5 — sonda profunda demais (DEVE rejeitar):
  - low da sonda 1,8×ATR abaixo de S
  - Esperado: Rejected(probe_too_deep)

CASO 6 — risco-retorno ruim / stop no ruído (DEVE rejeitar):
  - Sonda rasa demais: stop distante < 1×range médio de barra → Rejected(stop_within_noise)
  - Variante: gatilho longe da recuperação fazendo RR < 1,2 → Rejected(poor_risk_reward)

CASO 7 — fora de horário (DEVE rejeitar):
  - Setup perfeito com timestamp 09:30 ET (antes da janela) e 15:45 ET (depois)
  - Esperado: Rejected(outside_trading_hours)

CASO 8 — clímax em andamento (DEVE rejeitar):
  - Sonda em barra gigante (range 3×ATR) com MACD em nova mínima extrema
  - Esperado: Rejected(climax_in_progress)

CASO 9 — barra anterior marcou o extremo (DEVE posicionar o stop corretamente):
  - Sonda na barra i (sem recuperação), extremo em i; recuperação só em i+1
    com mínima mais alta — stop deve ficar abaixo da mínima de i, não de i+1
  - Literal do livro (exemplo EURUSD, Cap. 6)

CASO 10 — validação em 3 barras (gestão):
  - Após entrada, 3 candles laterais com lucro < 0,5R → saída a mercado no
    fechamento do 3º candle
  - Variante: lucro ≥ 0,5R no 2º candle → posição mantida até TP/SL

CASO 11 — limites de risco do processo (DEVE rejeitar):
  - daily_loss_limit_reached / max_trades_reached / consecutive_losses /
    position_already_open (integração com as regras de ouro do projeto)
```

---

## 16. Notas de Leitura

### Conceitos extraídos do livro

* **Failure test = spring de Wyckoff / 2B de Sperandeo:** sonda além do nível que falha e reverte rápido. *"The failure test pattern is the classic Wyckoff spring and upthrust; the only adaptation is the addition of a firm stop level and a concrete trading plan."* (Cap. 6)
* **Setup mais bem definido do livro:** *"There is no subjectivity in stop location and little subjectivity in managing losing trades--if the market makes a new extreme, then you are wrong and must exit the trade."* (Cap. 6)
* **Validação rápida é parte do setup:** *"If the failure test trade is successful, price should move sharply away from the level, and the trade should be immediately profitable (within one to three bars on the trading time frame)."* (Cap. 6)
* **Consolidação perto do nível = perigo:** *"Consolidation near the level is more consistent with an impending breakout... Reduce exposure or close out these trades if they do not work quickly in your favor."* (Cap. 6)
* **Não é reversão de tendência garantida:** *"Some of these trades will turn into dramatic trend reversals, but this should not be your baseline expectation."* (Cap. 6)
* **Tamanho menor:** *"there is significant gap risk with trades held overnight... it may make sense to trade these on smaller size and risk compared to other setups."* (Cap. 6) — para nós, o gap overnight é eliminado pelo flat EOD; mantemos o tamanho menor pela natureza contra-tendência.
* **Stop fora do ruído:** *"If you try to place your initial stops closer than one average bar's range, you are probably working within the noise level."* (Cap. 8)
* **Sizing fracionário fixo:** *"Trade size = Desired dollar risk ÷ Per-unit risk."* (Cap. 9)

### Guardrails do autor respeitados por esta especificação

* **Não** é "comprar em suporte" (proibido pelo autor sem falha confirmada) — exigimos sonda + recuperação.
* **Não** adicionar a perdedores; stop respeitado sem exceção.
* **Não** operar sem condição de reversão (sobreextensão/divergência).

### Desvios conscientes do livro (todos marcados no corpo)

1. Entrada buy stop no rompimento da máxima da barra de recuperação (em vez de "no fechamento") — limitação de infra (candles ~30s atrasados).
2. Alvo único 1,5R (em vez de parcial 25–33% em 1R + resto discricionário) — bracket de TP único.
3. Reentrada desabilitada na v1 (livro: "virtually obligatory") — simplicidade de MVP; parâmetros previstos para v2.
4. Numericalização de nível, sonda, recuperação e sobreextensão — o livro é deliberadamente qualitativo; todos os limiares são interpretação nossa e devem ser calibrados em backtest.

---

## 17. Checklist de Validação

```text
[x] Documentação da estratégia preenchida (Fases 1–3 do framework)
[x] Regras objetivas definidas (todas parametrizadas; interpretações marcadas)
[x] Especificação técnica completa (inputs/outputs/estado/eventos)
[ ] Código revisado (não iniciado — este documento não autoriza implementação)
[ ] Testes unitários com candles sintéticos passando (plano na seção 15)
[ ] Backtest executado e relatório gerado
[ ] Métricas mínimas atingidas (seção 13)
[ ] Nenhuma violação de regra de segurança financeira (stop obrigatório, sem martingale, sem posição dupla, modo paper)
[ ] Versionada no git como failure-test-long-v1
```

### Pendências de decisão do dono

1. **Saída por tempo (validação em 3 barras):** exige gestão ativa de posição além do bracket atual. Implementar na v1 ou registrar como limitação e avaliar o impacto no backtest?
2. **Tipo de entrada default:** `stop` na máxima da recuperação (proposto) vs. `market_next_open` (mais fiel ao livro).
3. **Reentrada (segunda falha):** confirmar adiamento para v2.
4. **Timeframe de contexto 1h:** a v1 opera só com 15min; adicionar filtro de 1h (ex.: não comprar failure test contra downtrend forte de 1h) ficou fora — avaliar após primeiro backtest.
5. **Extensão do enum `RejectionReason`** no `trader-domain` com as ~12 novas variantes da seção 11.
