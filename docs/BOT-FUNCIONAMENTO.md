# HumanStyle Trader Bot — Como Funciona Hoje

**Versão do documento:** 2026-08-06 (fim do dia 3 de validação live)
**Público:** dono do projeto e qualquer pessoa que queira entender o sistema sem ler código.

---

## 1. O que é este bot

Um robô de **day trade** escrito em Rust que opera ETFs de índice americanos (SPY, QQQ, IWM e outros) na Interactive Brokers, hoje em **conta paper** (simulação com dinheiro de mentira). Ele observa candles de 15 minutos, detecta padrões de price action descritos em livros clássicos de trading, e envia ordens com stop e alvo automaticamente.

O objetivo atual **não é lucrar** — é provar, com dados, que as estratégias e a operação são confiáveis o suficiente para um dia operar dinheiro real. Essa prova segue um critério formal chamado **gate de go-live** (ADR-010), explicado na seção 8.

**Regras invioláveis (garantidas por código):**
- Nunca opera sem stop loss. Nunca.
- Nunca opera dinheiro real no estágio atual — o modo real é bloqueado por código e por guarda de porta do gateway.
- Nunca aumenta risco após perda (sem martingale).
- Nunca abre posição duplicada no mesmo ativo.
- Toda decisão fica registrada no banco de dados com o motivo — inclusive cada rejeição.

---

## 2. Arquitetura em uma imagem

```text
        IB Gateway (TWS API, porta 7497 = paper)
                  │  candles 15min / execuções
                  ▼
┌──────────────────────────────────────────────┐
│ trader-cli (1 processo por ativo)            │
│  loop de 30s: busca candles → estratégia →   │
│  risco → ordem bracket → rastreia fills      │
└──────┬───────────────────────┬───────────────┘
       │                       │
       ▼                       ▼
 trader-core              PostgreSQL
 (domínio puro:           (candles, sinais, ordens,
  estratégias, risco,      fills, trades, eventos,
  backtest engine)         runs de backtest)
```

- **`trader-domain`**: entidades e contratos (Signal, Order, Fill, Trade, traits).
- **`trader-core`**: toda a inteligência — estratégias, contexto de mercado, gestão de risco, engine de execução. Não conhece IBKR nem banco: é puro e 100% testável.
- **`trader-adapters`**: fala com a IBKR (dados e ordens) e simula um broker para backtest.
- **`trader-infra`**: PostgreSQL, configuração, logging.
- **`trader-backtest`**: reexecuta a história candle a candle usando **exatamente o mesmo código do live** (paridade é regra do projeto).
- **`trader-cli`**: o executável que roda tudo (`paper`, `backtest`, `walkforward`, `ingest`, `analyze`, `journal`, `status`...).

O mesmo binário roda o live e os backtests: o que o backtest mede é o que o live faz.

---

## 3. Um dia de operação (o que acontece, passo a passo)

1. **9h15 ET** — sobe-se o Postgres e faz login no IB Gateway (conta paper).
2. **9h30** — um processo do bot por ativo é lançado (hoje: SPY, QQQ, IWM). No boot, cada processo:
   - roda as migrações do banco e **falha fechado** se o banco estiver fora (nunca opera sem persistência);
   - reconstrói os limites de risco do dia a partir do banco (P&L, nº de trades, perdas seguidas — restart no meio do pregão é seguro);
   - recupera qualquer ordem aberta de uma sessão anterior;
   - sincroniza o cursor de candles — **nunca opera setup velho**.
3. **Durante o pregão** — a cada 30 segundos o bot:
   - busca os candles recentes na IBKR (janela de ~200 candles ≈ 8 dias de contexto);
   - para cada candle recém-fechado: persiste o candle, atualiza o contexto de mercado e pergunta à estratégia se há setup;
   - se há setup, o RiskManager valida (limites diários, risco/retorno, volatilidade, horário) e só então a ordem vai ao broker;
   - a ordem é um **bracket**: entrada (stop) + stop loss + alvo ficam **server-side na IBKR** — mesmo se o bot ou o computador morrerem, a posição continua protegida;
   - a cada 15s, um rastreador lê as execuções do dia e transforma fills em trades no banco (com comissão real desde 2026-08-06).
4. **16h ET** — encerramento gracioso. Tudo o que aconteceu já está no banco; o relatório do dia sai do `journal`.

---

## 4. As estratégias (o "cérebro" do bot)

Cada estratégia é um plugin que implementa a mesma trait `Strategy`, nasce de um livro específico (regra de ouro: *nenhuma regra sem fonte*) e tem documento próprio em `docs/strategies/` com as regras objetivas, as citações e o veredito de validação.

### 4.1 `pullback-trend-v1` — a pioneira (Al Brooks, *Trading Price Action Trends*)
Em tendência de alta estabelecida (preço acima da EMA20 por ≥10 candles), espera um recuo de duas pernas e uma barra de sinal forte; entra com buy stop 1 tick acima da barra, stop na mínima, alvo 2R. **É a única no live hoje.**

### 4.2 `opening-reversal-v1` — a da primeira hora (Brooks, *Bar by Bar*, Cap. 11)
Entre 9h30 e 10h30 ET, quando o mercado testa a máxima ou mínima **do dia anterior** e falha (barra de reversão forte), entra contra o teste — long e short. Afastamento: o livro diz que essas reversões frequentemente formam o extremo do dia. Primeira estratégia do bot a operar vendida.

### 4.3 `balance-area-breakout-v1` — a do rompimento de congestão (Dalton, *Mind over Markets*, Cap. 4)
Após ~3 dias de congestão apertada (largura ≤ 2%), quando o preço fecha fora da área, entra na direção do rompimento — "go with the break-out" — com stop de volta dentro da área. É a mais robusta do projeto em cobertura de ativos.

### 4.4 Arquivadas (o funil funcionando como deve)
- **`failure-test-long-v1`** (Grimes, spring de Wyckoff): reversão em suporte. Implementada e testada, mas só 6–12 trades em 17,5 meses — amostra impossível. Arquivada.
- **`breakout-first-pullback-v1`** (Grimes, 1º pullback após rompimento): idem — 1–2 trades/ativo no período. Arquivada.

Matar uma estratégia fraca em 1 dia, no papel, sem custo, é exatamente para isso que o pipeline existe.

---

## 5. Gestão de risco (o "cinto de segurança")

Limites atuais (por processo/ativo), de `config/default.toml`:

| Regra | Limite |
|---|---|
| Risco por trade | 1% do capital (0,5% em estratégias que pedem menor) |
| Perda máxima diária | 2% — atingiu, bloqueia entradas até o próximo dia |
| Trades por dia | 3 |
| Perdas consecutivas | 3 — atingiu, pausa até o próximo dia |
| Risco/retorno mínimo | 1,5 |
| Janela operacional | 9h45–15h30 ET (cada estratégia tem a sua) |

O estado de risco é **durável**: reconstruído do banco a cada boot e resetado a cada dia — tanto no live quanto no backtest (paridade corrigida em 2026-08-05, com teste de regressão).

> **Pendente para dinheiro real:** hoje o risco é por processo. Com N processos (multi-ativo/multi-estratégia), é preciso um **limite global de portfólio** — os ativos aprovados são small-caps correlacionados e as perdas chegariam juntas num dia ruim.

---

## 6. Dados e qualidade

- Fonte única: IBKR (candles de 15min, chegam ~30s após o fechamento — medido em live).
- Base histórica atual: **2025-02-21 → hoje (~17,5 meses), 9.400+ candles por ativo, zero gaps intraday**, cobrindo 14 ETFs (SPY, QQQ, IWM, IWN, IJR, MDY, IJS, VBR, AVUV, SCHA, VB, IWO, SLYV, IWV).
- Toda ingestão registra quantidade e gaps na tabela `ingestions`.
- Candles são imutáveis; o live persiste os candles que processa (desde 2026-08-06).

---

## 7. Validação — o que está provado (e o que não)

### 7.1 O pipeline (cada estratégia passa por aqui)

```text
livro → doc de especificação (regras objetivas + citações)
      → código + testes unitários com candles sintéticos
      → backtest (17,5 meses, dados reais)
      → walk-forward out-of-sample (6 janelas)
      → veredito com critérios fixos
      → (só se aprovada) paper live → gate B
```

Critérios do gate A (walk-forward OOS): ≥ 50 trades, win rate ≥ 40%, profit factor ≥ 1.3, drawdown ≤ 10%, avg R > 0.15, expectativa positiva.

### 7.2 Resultados atuais (walk-forward OOS, 17,5 meses)

| Estratégia | Ativos aprovados | Números (melhores pares) |
|---|---|---|
| **pullback-trend-v1** | **IWM, IWV, IWO — gate A FECHADO** (amostra ≥ 50 incluída) | IWV: 88t, WR 46.6%, PF 1.59, avgR 0.35 |
| **opening-reversal-v1** | IWM, IWN, IJR, VB, SLYV (falta só amostra) | IWM: PF 1.70, avgR 0.55 · VB: PF 1.75, avgR 0.43 |
| **balance-area-breakout-v1** | 9 ativos: IWN, IWM, QQQ, IJS, VBR, AVUV, SLYV, SCHA, IWO | IJS: PF 3.48, avgR 0.74 (melhor par do projeto) |

Padrão encontrado: **o edge vive em small-caps** (IWM/IWN/IJR e primos). SPY e MDY reprovaram em tudo.

**Honestidade estatística:** testamos 42 pares estratégia×ativo — alguns passes podem ser sorte (a força do balance-area, 9/14 com PF majoritariamente > 2, está bem além do acaso). A formalização disso é o Deflated Sharpe Ratio (López de Prado), pendente.

### 7.3 O gate de go-live (ADR-010) — onde estamos

- **A. Estratégia:** ✅ fechada para pullback em 3 ativos; 2 novas estratégias aprovadas em qualidade aguardando amostra.
- **B. Operação:** em contagem — 3/20 pregões, 1/20 trades válidos, zero violações de risco. *(Duas seguidas sem sinais = ritmo em risco; a expansão multi-ativo/estrategia é a resposta.)*
- **C. Governança:** pendente (ADR de go-live + 1º mês com risco reduzido).

---

## 8. Observabilidade (como sabemos o que o bot está fazendo)

- **Logs estruturados** de cada decisão (sinal, rejeição com motivo, ordem, fill, trade).
- **Banco de dados**: `signals`, `orders`, `fills`, `trades`, `system_events`, `backtest_runs` — tudo auditável depois.
- **Alertas via webhook** (Slack/Discord/Teams — falta só configurar a URL): início/fim do live, trade fechado, circuit breaker. *(Bug crítico corrigido em 2026-08-06: o alerta mais importante — o do circuit breaker — se perdia no encerramento do processo; agora é entregue com confirmação.)*
- **Circuit breaker**: 10 falhas consecutivas de dados/infra → alerta crítico + encerramento com erro (testado de propósito em 2026-08-06).
- **Comandos de acompanhamento**: `status` (últimos sinais/trades), `journal` (trades do dia + P&L), `analyze` (live vs backtest, critérios do gate).

---

## 9. Comandos do dia a dia

```bash
trader-cli paper --mode live --symbol SPY          # live (1 processo por ativo)
trader-cli status                                   # últimos sinais/trades
trader-cli journal                                  # trades do dia + P&L
trader-cli analyze                                  # live vs backtest + critérios
trader-cli backtest --symbol IWM --strategy opening-reversal-v1
trader-cli walkforward --symbol IJS --strategy balance-area-breakout-v1 --windows 6
trader-cli ingest --symbol VBR --days 365 --provider ibkr
```

Runbooks completos em `docs/runbooks/` (operação, troubleshooting, checklist de go-live) e a rotina detalhada em `docs/HANDOFF.md` §4.

---

## 10. Estrutura atual em números

- **145 testes automatizados passando; clippy limpo** (`-D warnings`).
- **5 estratégias implementadas** (1 no live, 2 aprovadas aguardando amostra, 2 arquivadas com veredito documentado).
- **14 ativos** com 17,5 meses de histórico validado (0 gaps).
- **7 livros** analisados e registrados como fontes (Brooks ×3, Grimes, Dalton, López de Prado, + Chan pendente de OCR).
- **3 dias de live paper** com bugs reais encontrados e corrigidos (8 documentados nos relatórios diários).

---

## 11. O que falta para o dinheiro real

1. **Gate B**: ~17 pregões restantes + 19 trades válidos (a expansão de ativos/estratégias no paper vai acelerar — decisão do dono sobre o portfólio).
2. **Amostra das 2 novas estratégias** (agregar ativos ou mais histórico).
3. **Risco global de portfólio** antes de escalar processos (small-caps correlacionados).
4. **Governança**: ADR de go-live + primeiro mês com risco reduzido.
5. **Melhorias opcionais mapeadas**: webhook URL real, Deflated Sharpe (validação anti-autoengano), meta-labeling (AFML), dashboard (Fase 7), OCR do Chan.

---

## 12. Referências

- `docs/HANDOFF.md` — estado operacional completo, pendências e rotina.
- `docs/reports/` — relatórios dos dias 1, 2 e 3 de validação live.
- `docs/strategies/` — especificação e veredito de cada estratégia.
- `docs/books/analysis/` — análise dos livros com as candidatas rankeadas.
- `docs/decisions/` — ADRs (arquitetura, entrada stop, gate de go-live).
- `docs/OPERATIONS.md`, `docs/runbooks/` — operação e emergências.
