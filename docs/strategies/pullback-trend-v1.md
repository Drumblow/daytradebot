# Estratégia: Pullback em Tendência de Alta v1

## 1. Fonte

* **Livro:** Trading Price Action Trends
* **Autor:** Al Brooks
* **Seções relevantes:**
  * Capítulo 4 — Bar Basics: Signal Bars, Entry Bars, Setups
  * Capítulo 5 — Signal Bars: Reversal Bars
  * Capítulo 10 — Second Entries
  * Capítulo 18 — Example of How to Trade a Trend
  * Capítulo 19 — Signs of Strength in a Trend
  * Capítulo 20 — Two Legs

---

## 2. Conceito em uma frase

> Em uma tendência de alta clara, o preço faz um pullback (pausa corretiva). Após dois movimentos para baixo no pullback (high 2), se surgir uma barra de sinal de reversão bullish, entramos na continuação da tendência com stop abaixo da barra de sinal e alvo múltiplo do risco.

---

## 3. Contexto de Mercado (filtro obrigatório)

O setup só é válido quando o mercado está em **tendência de alta clara**.

### Timeframes

```text
Timeframe operacional: 15min
Timeframe de contexto: 1h
Timeframe macro: diário (opcional para o MVP)
```

### Condições no timeframe de contexto (1h)

```text
[1] Preço acima da EMA 20 por pelo menos 10 candles consecutivos
[2] Máximas e mínimas ascendentes (higher highs e higher lows)
[3] Pelo menos 60% dos últimos 20 candles fecharam acima da EMA 20
```

### Condições no timeframe operacional (15min)

```text
[4] Preço acima da EMA 20
[5] Último movimento de alta criou nova máxima (higher high) nos últimos 20 candles
[6] Não houve candle de fechamento abaixo da EMA 20 nos últimos 10 candles
```

### Regras de rejeição de contexto

```text
REJEITAR se preço estiver abaixo da EMA 20 no 1h ou no 15min
REJEITAR se não houver sequência de higher highs/higher lows
REJEITAR se o mercado estiver em trading range apertado (muitos dojis sobrepostos)
REJEITAR se houver climaxe de compra exagerada (3+ barras bullish consecutivas grandes sem pullback)
```

---

## 4. Setup de Entrada (High 2 Pullback)

A estratégia usa o conceito de **high 2** de Al Brooks: pullback em bull trend com duas pernas para baixo, seguido de barra de sinal bullish.

### 4.1 Estrutura do pullback

```text
[1] Pullback ocorre após um impulso de alta que fez nova máxima
[2] Pullback tem 2 a 6 candles (ideal: 3 a 5)
[3] A primeira perna para baixo não quebra a última mínima de swing (mantém higher low)
[4] Entre as duas pernas, há uma pequena reação para cima (mini-rally B)
[5] A segunda perna para baixo forma um higher low em relação à primeira perna
[6] O pullback chega próximo à EMA 20 (toca ou fica até 0,3% acima/abaixo)
```

### 4.2 Barra de sinal (reversal bar bullish)

A barra de sinal deve ter **pelo menos 3 das 5 características** abaixo:

```text
[1] Corpo positivo (close > open)
[2] Sombra inferior >= 1,5x o tamanho do corpo
[3] Fechamento no terço superior da barra
[4] Fechamento acima do fechamento da barra anterior
[5] Mínima da barra de sinal não é a menor do pullback (sinal de rejeição)
```

### 4.3 Regras de rejeição do setup

```text
REJEITAR se pullback tiver apenas 1 perna (high 1) — exigimos high 2
REJEITAR se pullback exceder 6 candles sem sinal válido
REJEITAR se segunda perna fizer lower low (quebra estrutura)
REJEITAR se barra de sinal for doji grande ou tiver sombra superior grande
REJEITAR se barra de sinal se sobrepor excessivamente às barras anteriores (>50%)
REJEITAR se não houver confirmação no candle seguinte (rompimento da máxima)
```

---

## 5. Entrada

```text
Tipo de ordem: buy stop
Gatilho: máxima da barra de sinal + 1 tick
Condição: ordem é colocada assim que a barra de sinal fecha

Observação: a entrada só é executada se o próximo candle romper a máxima da barra de sinal.
Se o rompimento não ocorrer, cancelar a ordem e aguardar novo setup.
```

---

## 6. Stop e Alvo

```text
Stop inicial: mínima da barra de sinal - 1 tick
Stop alternativo (mais conservador): mínima da segunda perna do pullback - 1 tick

Alvo inicial: 2R (2x a distância entre entrada e stop)
Alvo estendido: 3R (parcial opcional, conforme configuração)

Saída por tempo:
  - Se após 10 candles o preço não atingir alvo nem stop, avaliar saída no próximo pullback
```

---

## 7. Gestão de Risco

```text
Risco por trade: 1% do capital
Perda máxima diária: 2% do capital
Máximo de trades por dia: 3
Risco-retorno mínimo: 1:2
Spread máximo permitido: 0,05%
Volatilidade máxima: ATR(14) percentual <= 1,5% no 15min
Máximo de perdas consecutivas antes de parar: 3
```

### Cálculo do tamanho da posição

```text
risco_monetario = capital * risco_por_trade
distancia_stop  = |preco_entrada - stop|
quantidade      = floor(risco_monetario / distancia_stop)
```

---

## 8. Rejeições Registradas pelo Bot

```text
contexto_nao_e_tendencia_alta
preco_abaixo_da_media_1h
preco_abaixo_da_media_15min
sem_higher_highs_higher_lows
mercado_em_trading_range
climax_de_compra_exagerado

pullback_apenas_uma_perna
pullback_muito_longo
segunda_perna_quebra_estrutura
barra_de_sinal_doji_grande
barra_de_sinal_sombra_superior_grande
barra_de_sinal_sem_corpo_positivo
barra_de_sinal_sem_sombra_inferior
barra_de_sinal_sobreposicao_excessiva

risco_retorno_ruim
spread_alto
volatilidade_alta
fora_do_horario
perda_maxima_diaria_atingida
posicao_ja_aberta
```

---

## 9. Filtros de Horário e Ativo

```text
Ativo: SPY (inicialmente)
Horário de operação: 09:45 – 15:30 ET (evitar abertura e fechamento)
Não operar em dias de relatórios macro agendados (ex.: FOMC, payroll) antes do anúncio
```

---

## 10. Métricas de Avaliação

### Métricas mínimas para aprovação em backtest

```text
número mínimo de trades: 50
win rate mínimo: 40%
profit factor mínimo: 1,3
drawdown máximo: 10%
média de R por trade: > 0,15
expectativa matemática positiva
```

### Métricas para aprovação em paper trading

```text
mínimo 20 trades em paper
resultado próximo ao backtest (±30% nas métricas principais)
nenhuma violação de regra de risco
uptime do bot sem falhas críticas
```

---

## 11. Decisões de Implementação

### Onde vive no código

```text
trader-core/src/strategies/pullback_trend_v1/
  mod.rs      → estrutura pública e trait Strategy
  context.rs  → regras de contexto de mercado
  setup.rs    → detecção de high 2 e barra de sinal
  entry.rs    → regras de entrada, stop e alvo
  config.rs   → parâmetros da estratégia
```

### Configurações parametrizáveis

```rust
pub struct PullbackTrendV1Config {
    pub ema_period: usize,              // 20
    pub context_ema_bars: usize,        // 10
    pub pullback_max_bars: usize,       // 6
    pub pullback_min_bars: usize,       // 2
    pub signal_body_min_ratio: f64,     // 1.5 (sombra inferior / corpo)
    pub signal_close_min_position: f64, // 0.66 (terço superior)
    pub risk_per_trade: Decimal,        // 0.01
    pub max_daily_loss: Decimal,        // 0.02
    pub max_trades_per_day: usize,      // 3
    pub min_risk_reward: f64,           // 2.0
    pub max_spread_pct: Decimal,        // 0.0005
    pub max_atr_pct: Decimal,           // 0.015
}
```

---

## 12. Notas de Leitura

### Conceitos extraídos do livro

* **High 2 pullback**: pullback em bull trend com duas pernas para baixo. Mais confiável que high 1 porque a segunda perna testa a força dos compradores.
* **Barra de reversão bullish**: corpo positivo, sombra inferior significativa, fechamento forte. Não precisa ser perfeita, mas precisa demonstrar que os touros assumiram o controle da barra.
* **Contexto é tudo**: em tendência forte, até sinais fracos funcionam. Em mercado sem tendência, até sinais fortes falham.
* **Entrada no stop**: comprar um tick acima da máxima da barra de sinal exige confirmação de momentum.
* **Stop na barra de sinal**: protege o caso em que o sinal falha. Se o preço voltar abaixo da barra de sinal, a premissa está errada.
* **Gestão de risco**: Al Brooks enfatiza que scalping exige win rate muito alto. Para traders iniciantes, swing trades com recompensa >= risco são mais viáveis.

### Citações relevantes

> "Buying pullbacks before the breakout generally offers more reward, smaller risk, and a higher probability of success." (Cap. 18)

> "The stronger the trend, the less important it is to have a strong signal bar for a with-trend trade." (Cap. 5)

> "A second entry is almost always more likely to result in a profitable trade than a first entry." (Cap. 10)

> "Once you realize that the market is in a strong trend, you don't need a setup to enter. You can enter anytime all day long at the market if you wish with a relatively small stop. The only purpose of a setup is to minimize the risk." (Cap. 19)

---

## 13. Checklist de Validação

```text
[ ] Documentação da estratégia preenchida
[ ] Regras objetivas definidas
[ ] Especificação técnica completa
[ ] Código revisado
[ ] Testes unitários com candles sintéticos passando
[ ] Backtest executado e relatório gerado
[ ] Métricas mínimas atingidas
[ ] Nenhuma violação de regra de segurança financeira
[ ] Versionada no git como pullback-trend-v1
```
