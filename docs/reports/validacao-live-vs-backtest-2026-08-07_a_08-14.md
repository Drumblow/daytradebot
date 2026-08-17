# Validação cruzada live × backtest — semana dos dias 4–9 (2026-08-07 → 2026-08-14)

**Data da análise:** 2026-08-17
**Método:** dump do banco da VM (133.849 candles, 298 sinais, 5 trades) restaurado localmente; backtests das 8 combinações estratégia×ativo rodados sobre **exatamente os mesmos candles** que o live processou, janela 2026-07-27 → 2026-08-14 (warmup incluído, mesma lógica live/backtest — paridade é regra do projeto).
**Pergunta do dono:** os resultados do servidor eram os esperados? As entradas eram as corretas? Houve sinais que deveriam ter acontecido e não aconteceram?

---

## 1. Resumo executivo

**A validação revelou um problema mais grave que qualquer divergência de estratégia: desde a migração para a VM (08-07), o bot opera com dados de mercado degradados.** Os candles que o live busca da IBKR em tempo real na VM chegam como barras de 1 print (open=high=low=close, volume 0) na maioria das barras do dia — confirmado contra fonte externa (Yahoo) em 2026-08-17. Consequências:

1. **O backtest não reproduz o live no período** — e a causa é o dado, não o código da estratégia.
2. **A amostra do gate B dos dias 4–9 está contaminada**: os 2 trades foram execuções reais, mas os sinais foram calculados sobre barras degeneradas.
3. **O bot está operando com esses dados AGORA** — decisão do dono necessária (ver §6).

## 2. O que o live fez (dias 4–9, fonte: banco da VM)

| Dia | Sinais | Ordens | Trades | P&L líquido |
|-----|--------|--------|--------|-------------|
| 08-07 | 3 (IWM pb) | 0 | 0 | — |
| 08-10 | 3 (IWV pb) | 3 (expiradas/canceladas) | 0 | — |
| 08-11 | 0 | 0 | 0 | — |
| 08-12 | 1 (IWV pb) | 1 | 1 (alvo) | **+$69.42** |
| 08-13 | 1 (IWO pb) | 1 | 1 (alvo após gap) | **-$83.39** |
| 08-14 | 0 | 0 | 0 | — |
| **Total** | **8** | **5** | **2** | **-$13.97** |

## 3. O que o backtest produziu no mesmo período (mesmos candles do banco)

| Combinação | Trades no período 08-07→08-14 | Divergência vs live |
|------------|-------------------------------|---------------------|
| IWM pullback | 0 | live teve 3 sinais (08-07) — **não reproduzidos** |
| IWV pullback | 0 | live teve 3 sinais (08-10) + trade (08-12) — **não reproduzidos** |
| IWO pullback | 0 | live teve trade (08-13) — **não reproduzido** |
| IWM opening-reversal | 1 (08-12 10h00 ET, short, +$117.46) | **trade fantasma — live não sinalizou** |
| IWN opening-reversal | 0 | — |
| IJS/VBR/AVUV balance | 0 | — |

Duas divergências em direções opostas (sinais do live ausentes no backtest; um trade do backtest ausente no live) — a assinatura clássica de **divergência de dados**, não de lógica: a estratégia é determinística e o código é o mesmo nos dois caminhos.

## 4. Causa-raiz: dados degenerados desde a migração para a VM

### 4.1 Evidência no banco

Candles de 15min por dia (IWV) — quantidade com range real (high > low):

| Período | Candles/dia com range | Range médio | Volume/dia |
|---------|----------------------|-------------|------------|
| 08-03 → 08-06 (PC/TWS) | **26/26** | 0.37–0.57 | 110k–330k |
| 08-07 → 08-14 (VM/Gateway) | **2–8/26** | 0.01–0.02 | 2k–22k |
| 08-17 (VM, hoje) | 2/8 (até 11h15) | 0.017 | ~3k |

A inflexão é exatamente o dia da migração (08-07).

### 4.2 Confirmação contra fonte externa (2026-08-17, IWV)

| Barra | Mercado real (Yahoo) | Persistido pela VM |
|-------|---------------------|--------------------|
| 09:30 | H 442.43 / L 441.77 / C 441.99 / V 12.733 | L 442.30 / C 442.43 / V 2.604 (barra parcial) |
| 09:45 | H 441.99 / L 441.67 / C 441.70 | **441.98 flat / V 0** |
| 10:00 | H 441.98 / L 441.61 / C 441.96 | **441.70 flat / V 0** |
| 10:45 | H 441.88 / L 441.59 / C 441.59 | **441.71 flat / V 0** |

Os "candles" da VM são **um único print no início da barra**: a conta paper não tem subscrição de dados em tempo real (erro 10168, já documentado no HANDOFF para `get_quote`) e o Gateway headless na VM recebe barras recentes degeneradas. No PC, o TWS da mesma conta entregava barras completas — por isso os dias 1–3 têm dados ricos.

### 4.3 Por que o live mesmo assim gerou sinais "razoáveis"

A estratégia rodou sobre essa série degenerada. Como as barras carregam um print real do início do período, os fechamentos acompanham grosseiramente o mercado (por isso EMAs e tendência ainda saem), mas **highs/lows — a matéria-prima de price action — não existem**. Os triggers das ordens bateram com o mercado real por coincidência estrutural: o stop-entry só dispara se o mercado real romper o gatilho, e os fills foram reais (ex.: trade 10 entrou 440.53–440.59 com gatilho 440.41; o mercado realmente passou por ali).

### 4.4 O trade fantasma do backtest (IWM opening-reversal, 08-12)

O backtest "achou" um short às 10h00 que o live não viu: artefato da série degenerada (teste de máxima/mínima do dia anterior medido contra barras sem range). **Não foi um sinal perdido pelo live — foi um sinal que não existiu no mercado real.** A análise de "setups perdidos" só será possível com dados reparados.

## 5. Consequência para o gate B (ADR-010)

- Os pregões 4–9 **contam para uptime** (o pipeline operou, reconciliou, protegeu posições).
- Os 2 trades **são execuções reais**, mas os sinais que os originaram foram calculados sobre barras degradadas. Recomendação honesta: **marcar os trades 10 e 11 como `data_quality_suspect: true` no journal** e tratá-los como fora da amostra de validação estatística (como os artefatos do dia 1), OU mantê-los com ressalva documentada. **Decisão do dono.**
- Sem o conserto do dado, os próximos pregões continuarão acumulando amostra inválida.

## 6. Ações recomendadas (ordem de prioridade)

1. **Decisão operacional imediata:** pausar o live (mascarar `trader-start.timer`) até o dado ser consertado — o bot está entrando em trades com base em barras sem high/low. *(Brackets server-side protegem posições abertas; hoje não há nenhuma.)*
2. **Consertar o feed na VM:** habilitar compartilhamento de market data para a conta paper no portal IBKR (a conta paper herda as subscrições da conta principal quando habilitado em *Settings → Paper Trading Account → share market data*), ou assinar dados para a paper. Validar com barra recém-fechada retornando OHLCV completo antes de religar o live.
3. **Reparar o histórico:** apagar os candles degenerados de 08-07 em diante e re-ingerir da IBKR (barras passadas já vêm restated/completas) — manter registro do reparo. Depois disso, **refazer esta validação cruzada**.
4. **Guarda de código (anti-regressão):** ✅ **implementada em 2026-08-17** — `Candle::is_degenerate()` no domínio. **v2 (mesmo dia, após medir o feed):** o live agora **espera a consolidação da barra** em vez de pular — o feed sem dados em tempo real entrega a barra recém-fechada como 1 print e consolida ~3–4 min depois (medido); o cursor não avança até consolidar (máx ~15 min, depois desiste e alerta). Com feed em tempo real a consolidação é imediata e o custo é zero. O `save` de candles virou **upsert que só sobrescreve linhas degeneradas** — o banco se auto-repara quando a versão consolidada chega, e um `ingest` posterior repara o histórico de 08-07+. O `ingest` também conta e alerta barras degeneradas.
5. ~~Adicionar checagem de qualidade de dados ao `trader-cli status`~~ → parcialmente coberto pelo novo `trader-cli debug-candles` (diagnóstico cru do feed, com `--realtime` para testar o switch de market data type). O item de `status` segue pendente (baixa prioridade).
6. **Reparar o histórico:** apagar os candles degenerados de 08-07 em diante e re-ingerir da IBKR (barras passadas vêm restated/completas) — manter registro do reparo. Depois disso, **refazer esta validação cruzada**.

## 7. Notas de método (para auditoria)
- Backtests rodados com o binário local, mesmo commit do deploy, contra restore do banco da VM (`trader_compare`), janela 2026-07-27 → 2026-08-14, timeframe 15m, configs de `config/strategies/` — runs persistidos (ids 201–208 no banco de comparação).
- O EMA do snapshot do sinal 297 (440.1834) bate com o EMA recomputado sobre os candles persistidos (440.1924, algoritmo do projeto — seed SMA dos últimos 20 + 19 iterações), confirmando que o live avaliou a série degenerada persistida.
- Divergência de preenchimento conhecida e separada: o simulador do backtest preenche stop-entry em `trigger × (1+0.1%)`, enquanto o live preenche a mercado real — P&L de trades individuais nunca bate centavo a centavo; a comparação válida é de **sinais**, e é ela que falha por causa do dado.

---

## 8. Revalidação com dados reparados (2026-08-17, fim do dia)

Após o deploy do upsert de auto-reparo, o histórico de 08-07→08-17 foi re-ingerido da IBKR (barras consolidadas). IWV: 26/26 barras com range por dia (antes: 2–8), range médio 0.17–0.38 (antes: 0.01). O buraco do cutover de 08-07 também foi preenchido. **Os 8 backtests foram refeitos sobre os dados bons.** Resultado na janela:

| Combinação | Backtest (dados bons) | Live real | Leitura |
|------------|----------------------|-----------|---------|
| IWM pullback | **08-07 13h30 ET long +$71.56 (alvo)** | 3 sinais, 0 ordens (sessão morta no cutover) | **winner perdido pela migração** |
| IWV pullback | **08-10 11h45 ET long -$63.27 (stop)** | 3 sinais, entradas expiradas sem toque | setup diferente do que o live viu (dado degradado em tempo real) |
| IWM opening-reversal | **08-12 10h00 ET short +$117.46 (alvo)** | sem sinal | **winner perdido pelo dado degradado** (não era artefato de barra flat — persiste com dados bons) |
| IWV pullback 08-12 | sem setup | trade real +$69.42 | o win do live **não existiria** com dados bons |
| IWO pullback 08-13 | sem setup | trade real -$83.39 | o loss do live **não existiria** com dados bons |
| IWO/IWN/IJS/VBR/AVUV | 0 trades | 0 trades | consistente |

**Conclusão final:** com dados íntegros, live e backtest da semana são **descorrelacionados** — as decisões do live foram tomadas sobre barras degradadas em tempo real, ponto. Os dois trades reais da semana (o win e o loss) não teriam acontecido; dois outros trades (um win, um loss) teriam. **Recomendação formal: excluir os pregões 4–9 da amostra estatística do gate B** (trades 10 e 11 marcados `data_quality_suspect`), tratando a semana como teste de infraestrutura. A contagem válida recomeça com o feed estabilizado (guarda v3, deploy de 2026-08-17 ~15h ET). O dia 10 (08-17) também é de transição (dado degradado até ~15h ET) — avaliar ao fechar.

**Aprendizado para o processo:** a validação cruzada live × backtest, rodada semanalmente, teria pego a degradação no dia 8-10. Sugestão: incluir `debug-candles` (ou contagem de barras flat no banco) na reconciliação semanal do gate.
