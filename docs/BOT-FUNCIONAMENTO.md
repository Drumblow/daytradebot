# HumanStyle Trader Bot — Como Funciona Hoje

**Versão do documento:** 2026-09-07 (revisão após o ADR-018, o hotfix ET da `range-extreme-fade-v1` e o ADR-019; a contagem do gate B em §7.3 continua sendo a de 2026-08-06)
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
│ trader-cli (1 processo por estratégia×ativo) │
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
- **`trader-cli`**: o executável que roda tudo (`paper`, `backtest`, `walkforward`, `ingest`, `analyze`, `journal`, `status`, `flatten`...).

O mesmo binário roda o live e os backtests: o que o backtest mede é o que o live faz.

---

## 3. Um dia de operação (o que acontece, passo a passo)

1. **9h15 ET** — sobe-se o Postgres e faz login no IB Gateway (conta paper).
2. **9h30** — um processo do bot por par estratégia×ativo é lançado. No boot, cada processo:
   - roda as migrações do banco e **falha fechado** se o banco estiver fora (nunca opera sem persistência);
   - reconstrói os limites de risco do dia a partir do banco (P&L, nº de trades, perdas seguidas — restart no meio do pregão é seguro);
   - recupera qualquer ordem aberta de uma sessão anterior;
   - sincroniza o cursor de candles — **nunca opera setup velho**.
3. **Durante o pregão** — a cada 30 segundos o bot:
   - busca os candles recentes na IBKR (janela de ~200 candles ≈ 8 dias de contexto);
   - para cada candle recém-fechado: persiste o candle, atualiza o contexto de mercado e pergunta à estratégia se há setup;
   - se há setup, o RiskManager valida (limites diários, risco/retorno, volatilidade, horário) e só então a ordem vai ao broker;
   - a ordem é um **bracket**: entrada (stop) + stop loss + alvo ficam **server-side na IBKR** — mesmo se o bot ou o computador morrerem, a posição continua protegida **dentro do pregão**. As três pernas vão com TIF `Day` e expiram no sino: por isso existe o flatten de fim de sessão (ADR-018), e por isso nunca se deixa posição virar a noite confiando no stop do broker;
   - a cada 15s, um rastreador lê as execuções do dia e transforma fills em trades no banco (com comissão real desde 2026-08-06).
4. **15h55–16h10 ET — flatten de fim de pregão (ADR-018).** A cada tick dentro dessa janela — configurável em `[session]` de `config/default.toml`, ver §5 — o bot cancela a entrada stop ainda pendente, encerra **a mercado** a posição que ele mesmo abriu e grava `exit_reason = end_of_day` no trade, mais um `system_events` do tipo `session_flatten`. Roda uma única vez por dia de calendário de Nova York. Posição no broker que **esta** instância não rastreia **não** é fechada, de propósito (duas instâncias no mesmo símbolo se inverteriam): vira alerta CRITICAL `untracked_position` e exige intervenção manual com `trader-cli flatten`.
5. **16h ET** — encerramento gracioso. Tudo o que aconteceu já está no banco; o relatório do dia sai do `journal`.

> **O flatten ainda não está em produção.** O ADR-018, o hotfix ET da `range-extreme-fade-v1` e o harness do ADR-019 estão implementados e commitados na `main` local, **sem push** — as instâncias em produção seguem rodando sem flatten. O push não é neutro: `.github/workflows/images.yml` dispara em push para `main` com paths `crates/**` ou `config/**`, publica as imagens e, com `APP_DEPLOY=enabled` e fora do pregão, **recria as instâncias de produção**. Isso muda o `config_hash` da `range-extreme-fade-v1` no meio do gate B e reinicia as 4 semanas de contagem (§3.8 do plano) — quando dar o push é decisão do dono.

---

## 4. As estratégias (o "cérebro" do bot)

Cada estratégia é um plugin que implementa a mesma trait `Strategy`, nasce de um livro específico (regra de ouro: *nenhuma regra sem fonte*) e tem documento próprio em `docs/strategies/` com as regras objetivas, as citações e o veredito de validação.

### 4.1 `pullback-trend-v1` — a pioneira (Al Brooks, *Trading Price Action Trends*)
Em tendência de alta estabelecida (preço acima da EMA20 por ≥10 candles), espera um recuo de duas pernas e uma barra de sinal forte; entra com buy stop 1 tick acima da barra, stop na mínima, alvo 2R. Foi a primeira a fechar o gate A e a primeira a ir ao live — e ficou lá até **04/09/2026**, quando o desligamento pedido pelo ADR-016 entrou no host (app v1.2.0): as três instâncias dela saíram do compose e da lista do deploy, que hoje recria 8 instâncias das outras três estratégias. O ADR-016 ainda tem status **proposto** no cabeçalho do arquivo, mas o desligamento já está executado — **a `pullback-trend-v1` não está em produção**.

### 4.2 `opening-reversal-v1` — a da primeira hora (Brooks, *Bar by Bar*, Cap. 11)
Entre 9h30 e 10h30 ET, quando o mercado testa a máxima ou mínima **do dia anterior** e falha (barra de reversão forte), entra contra o teste — long e short. Afastamento: o livro diz que essas reversões frequentemente formam o extremo do dia. Primeira estratégia do bot a operar vendida.

### 4.3 `balance-area-breakout-v1` — a do rompimento de congestão (Dalton, *Mind over Markets*, Cap. 4)
Após ~3 dias de congestão apertada (largura ≤ 2%), quando o preço fecha fora da área, entra na direção do rompimento — "go with the break-out" — com stop de volta dentro da área. Era a estratégia com maior cobertura de ativos do projeto **até o flatten de fim de pregão do ADR-018**: com a régua do live ela reprova o gate A pelo avg R (VBR −0,013; AVUV −0,173 com WR 31,4%) e só IJS segue passando em qualidade. Os 20 trades que carregavam 65% do P&L in-sample eram posições seguradas pela noite — que o live nunca teve.

### 4.4 `range-extreme-fade-v1` — a do rompimento que falha (Brooks, *Reading Price Charts Bar by Bar*, Cap. 9 + Cap. 5)
Em dia de trading range (a maioria dos dias), o mercado rompe **a máxima ou a mínima do dia** formada antes da barra de sinal — não um swing point de vários dias — sem momentum, atraído por stops, e falha; o bot entra **contra** o rompimento com barra de sinal forte, stop 1 tick além do extremo e **alvo fixo de 1,5R** (`target_r_multiple`; o alvo estrutural de volta ao interior do range é candidato de v2). É a estratégia do contexto que o resto do portfólio rejeita. Vetos **ativos**, herdados do livro: Barb Wire e meio do dia + meio do range. O terceiro veto do livro — lado errado da EMA — está implementado mas **desligado por configuração** (`use_ema_side_rule = false`, por calibração de frequência). O veto de meio do dia foi corrigido no **hotfix ET de 07/09/2026**, que trocou a janela de UTC fixo para horário de Nova York: "v1.0.1" é o rótulo da nota de correção, não um campo — o TOML segue com `version = "1.0.0"` e o mesmo `strategy_id`, o que mudou foram os valores de `midday_start_time`/`midday_end_time`. Por isso o `config_hash` muda (`49ee6f045b4c35a7` → `818b53394244ca62`) — e os números **pioram**: in-sample a fade cai de PF 1,74 / +4.560 para PF 1,57 / avg R 0,182 / +3.618 (−21% no net), tudo em AVUV. O bug estava ajudando; o edge é mais fino do que o gate A de 04/09 mostrava.

### 4.5 Arquivadas (o funil funcionando como deve)
- **`failure-test-long-v1`** (Grimes, spring de Wyckoff): reversão em suporte. Implementada e testada, mas só 6–12 trades em 17,5 meses — amostra impossível. Arquivada.
- **`breakout-first-pullback-v1`** (Grimes, 1º pullback após rompimento): idem — 1–2 trades/ativo no período. Arquivada.
- **`low2-m2s-short-v1`**, **`value-area-reentry-v1`** e **`trendline-break-test-v1`**: implementadas, backtestadas e reprovadas; arquivadas com veredito em `docs/strategies/`. São **cinco** arquivadas no total — as duas de cima são só as que este documento detalha.

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
| Fim de pregão (ADR-018) | flatten a mercado entre 15h55 e 16h10 ET; última barra de 15min às 15h45 ET |

Os três horários de fim de pregão vivem na seção `[session]` de `config/default.toml` (`flatten_start = 15:55:00`, `flatten_end = 16:10:00`, `last_bar = 15:45:00`) e são **horário de Nova York**. O live e o motor de backtest leem a mesma seção — a paridade de fim de sessão é por configuração, não por coincidência. No backtest o gatilho é a **mudança de data ET**, não o relógio (em pregão de meio expediente o dia acaba mais cedo), e o fim da série de dados não conta como sino; `last_bar` é só checagem de sanidade.

O estado de risco é **durável**: reconstruído do banco a cada boot e resetado a cada dia — tanto no live quanto no backtest (paridade corrigida em 2026-08-05, com teste de regressão).

> **Pendente para dinheiro real:** hoje o risco é por processo. Com N processos (multi-ativo/multi-estratégia), é preciso um **limite global de portfólio** — os ativos aprovados são small-caps correlacionados e as perdas chegariam juntas num dia ruim.

---

## 6. Dados e qualidade

- Fonte única: IBKR (candles de 15min, chegam ~30s após o fechamento — medido em live).
- Base histórica atual: **2025-02-21 → hoje (~17,5 meses), 9.400+ candles por ativo, zero gaps intraday**, cobrindo 14 ETFs (SPY, QQQ, IWM, IWN, IJR, MDY, IJS, VBR, AVUV, SCHA, VB, IWO, SLYV, IWV).
- Toda ingestão registra quantidade e gaps na tabela `ingestions`.
- Candles são imutáveis; o live persiste os candles que processa (desde 2026-08-06).

> **Ressalva aberta:** o banco dev tem feed degradado do Gateway **a partir de 07/08/2026** (barras com 3–10% do volume e 15–25% do range reais). As barras desse trecho saem menores do que foram, e os stops derivados delas também — o que contamina o **nível** absoluto do último bloco do walk-forward. Comparações com/sem flatten não são afetadas (os dois lados leem os mesmos candles). Detalhe em `docs/reports/gate-a-com-flatten-2026-09-07.md` §6.

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

Critérios do gate A **em vigor** (ADR-010, walk-forward OOS): ≥ 50 trades, win rate ≥ 40%, profit factor ≥ 1.3, drawdown ≤ 10%, avg R > 0.15, expectativa positiva. O ADR-019 §7 propõe **cinco** critérios a mais: limite inferior do IC95 do PF por bootstrap em blocos ≥ 1,0; PF em unidades de risco (PF_R) ≥ 1,2; os 2 melhores meses respondendo por ≤ 60% do lucro; holdout travado passando nos seis atuais, rodado uma vez; e sensibilidade ao custo (reportar o PF a 4–5 bp). Nada disso é o gate — é **proposta**. Dos cinco, **dois** já saem impressos pelo `walkforward` (PF_R e concentração dos 2 melhores meses), sob a linha marcada como tal; o IC em blocos depende do relatório em Python do §8 do ADR, ainda pendente. O t-stat do avg R e a corr(risco, R) são **relatório obrigatório**, não critério.

### 7.2 Resultados atuais (walk-forward OOS, 6 janelas, **com flatten** — ADR-018)

Os números abaixo vêm de `docs/reports/gate-a-com-flatten-2026-09-07.md` (runs OOS 725–732, janela 2025-02-24 → 2026-09-03, 2 bp de slippage), já com o flatten de fim de pregão e o hotfix ET da fade. **Nenhum run anterior ao ADR-018 é comparável com estes.**

| Estratégia | Ativos aprovados | Números (melhores pares) |
|---|---|---|
| **pullback-trend-v1** | não re-rodada com o flatten — números de 2026-08-06, **não comparáveis com as linhas abaixo** | IWV: 88t, WR 46.6%, PF 1.59, avgR 0.35 |
| **opening-reversal-v1** | IWM, IWN — passam em qualidade (falta só amostra) | IWM: 32t, WR 59,3%, PF 1,72, avgR 0,450 · IWN: 26t, WR 53,8%, PF 1,56, avgR 0,283 |
| **balance-area-breakout-v1** | **nenhum — reprova o gate A com a régua do live**; só IJS passa em qualidade | IJS: 23t, WR 60,8%, PF 2,54, avgR 0,383 · VBR: 34t, PF 1,51, **avgR −0,013** · AVUV: 35t, **WR 31,4%**, PF 1,43, **avgR −0,173** |
| **range-extreme-fade-v1** | AVUV, SLYV — passam em qualidade (falta só amostra); **reprova em IWV** | SLYV: 20t, WR 70,0%, PF 2,64, avgR 0,424 · AVUV: 26t, WR 57,6% (só publicada antes do hotfix), PF 1,47, avgR 0,159 · IWV: 18t, WR 55,5%, PF 1,31, **avgR 0,043** |

Padrão encontrado: **o edge vive em small-caps** (IWM/IWN/IJR e primos). SPY e MDY reprovaram em tudo.

Duas leituras que só a régua do live tornou visíveis: a `balance-area-breakout-v1` **cai** com o flatten (in-sample PF 1,92 → 1,55, avg R 0,214 → −0,007; 20 dos 96 trades eram overnight), e a `opening-reversal-v1` **sobe** (PF 1,23 → 1,74) porque perde 8 trades overnight, todos perdedores — aritmética, não reabilitação.

**Honestidade estatística:** testamos 42 pares estratégia×ativo — alguns passes podem ser sorte, e a re-rodada com o flatten mostrou isso na prática: o que parecia "bem além do acaso" na balance-area (9/14 com PF majoritariamente > 2) era o efeito de carregar posição pela noite. O harness do ADR-019 acrescentou o PF medido **em unidades de risco**: a balance-area fica com PF_R 0,74 em AVUV e 0,97 em VBR — sem edge em R. O PF em dólares acima de 1 vem da correlação entre tamanho e resultado (0,57 em AVUV, a mais alta do conjunto): o cap de notional põe posição maior justamente nos trades de stop largo. Nenhuma das oito combinações re-rodadas tem t-stat de avg R ≥ 2 (o de AVUV é −0,80), e só a fade em SLYV fica abaixo do teto de concentração proposto (59% do lucro nos 2 melhores meses). **Sob o gate proposto no ADR-019 §7 — que ainda não vigora — nenhuma das oito passaria.** O Deflated Sharpe deixou de ser pendência em 08/09 (`trader-research/`): no portfólio dos oito pares ele fica em **0,45** com N=42 tentativas, contra o 0,95 convencional. E o mesmo relatório mostrou que **a unidade em que o gate é lido decide o veredito** — por par reprova tudo, por estratégia reprova tudo, no portfólio passa tudo. Ver `docs/reports/estatistica-gate-a-2026-09-08.md`.

### 7.3 O gate de go-live (ADR-010) — onde estamos

- **A. Estratégia:** ⚠️ **reaberta pelo ADR-018.** Com o flatten — a régua do live — nenhuma combinação fecha o gate A: `balance-area-breakout-v1` reprova por avg R (VBR −0,013; AVUV −0,173 com WR 31,4%) e só IJS passa em qualidade; `range-extreme-fade-v1` passa em AVUV e SLYV e reprova em IWV (avg R 0,043); `opening-reversal-v1` passa em qualidade em IWM e IWN. **Nenhuma chega aos 50 trades OOS** — a amostra segue sendo o gargalo, e o paper forward é o único OOS verdadeiro. Os números da `pullback-trend-v1` são anteriores ao flatten e não foram re-rodados. Fonte: `docs/reports/gate-a-com-flatten-2026-09-07.md`.
- **B. Operação:** em contagem — 3/20 pregões, 1/20 trades válidos, zero violações de risco *(contagem de 2026-08-06, a reconferir)*. Publicar os commits do ADR-018/ADR-019 recria as instâncias de produção e muda o `config_hash` da `range-extreme-fade-v1`: isso reinicia as 4 semanas de contagem (§3.8 do plano).
- **C. Governança:** pendente (ADR de go-live + 1º mês com risco reduzido).

---

## 8. Observabilidade (como sabemos o que o bot está fazendo)

- **Logs estruturados** de cada decisão (sinal, rejeição com motivo, ordem, fill, trade).
- **Banco de dados**: `signals`, `orders`, `fills`, `trades`, `system_events`, `backtest_runs` — tudo auditável depois.
- **Alertas via webhook** (Slack/Discord/Teams — falta só configurar a URL): início/fim do live, trade fechado, circuit breaker. *(Bug crítico corrigido em 2026-08-06: o alerta mais importante — o do circuit breaker — se perdia no encerramento do processo; agora é entregue com confirmação.)*
- **Circuit breaker**: 10 falhas consecutivas de dados/infra → alerta crítico + encerramento com erro (testado de propósito em 2026-08-06).
- **Motivo de saída de cada trade** (`trades.exit_reason`): `target`, `stop`, `time`, `manual`, `risk_manager` e, desde o ADR-018, `end_of_day`. A tabela canônica é única (`ExitReason::as_str()`), e a leitura no repositório **falha fechado** em valor desconhecido — antes um valor não reconhecido virava `target` em silêncio. Cuidado ao agrupar: flatten anterior ao ADR-018 está gravado como `manual` + `journal.forced_exit = "session_flatten"` e só é reconhecido por `Trade::effective_exit_reason()`; quem agrupar por `exit_reason` cru mistura flatten com saída discricionária.
- **Comandos de acompanhamento**: `status` (últimos sinais/trades), `journal` (trades do dia + P&L), `analyze` (live vs backtest do **mesmo** `config_hash`, critérios do gate). Desde o ADR-019 o `analyze` escolhe o baseline pelo trio (estratégia, par, `config_hash`), ignora runs experimentais e filtra os trades do live pelo mesmo par estratégia+hash; sem run com aquele hash ele **avisa e não compara**, em vez de usar o run mais recente de outra config.

---

## 9. Comandos do dia a dia

```bash
trader-cli paper --mode live --symbol IWM --strategy opening-reversal-v1   # live (1 processo por par estratégia×ativo)
trader-cli status                                   # últimos sinais/trades
trader-cli journal                                  # trades do dia + P&L
trader-cli analyze --symbol IWM --strategy opening-reversal-v1   # live vs backtest do MESMO config_hash
trader-cli backtest --symbol IWM --strategy opening-reversal-v1
trader-cli backtest --symbol IWM --strategy opening-reversal-v1 --no-flatten   # só p/ reproduzir run antigo
trader-cli walkforward --symbol IJS --strategy balance-area-breakout-v1 --windows 6 --slippage-bps 2
trader-cli ingest --symbol VBR --days 365 --provider ibkr
trader-cli flatten --symbol IWM --provider ibkr --confirm   # zera posição órfã no broker (manual)
```

O **flatten de fim de pregão (ADR-018) está ligado por padrão** no `backtest` e no `walkforward`: qualquer número medido hoje difere dos runs antigos. O `--no-flatten` existe só para reproduzir esses runs e medir o delta — o resultado com a flag carrega posição pela noite, o que o live nunca faz, e **não vale como gate A**.

O `walkforward` aceita ainda `--output`, `--label`, `--holdout-from`, `--strategy-config` e `--set chave=valor` (ADR-019). `--label` é obrigatório junto de `--set`, e esses runs entram como experimentais — nunca viram baseline do `analyze`. O comando **aborta** (não avisa e segue) em chave inexistente, em `--set` que não muda o `config_hash`, em `--set` combinado com `--holdout-from`, em TOML de `--strategy-config` cujo id não bate com `--strategy` e em `--holdout-from` inválido.

Runbooks completos em `docs/runbooks/` (operação, troubleshooting, checklist de go-live) e a rotina detalhada em `docs/HANDOFF.md` §4.

---

## 10. Estrutura atual em números

- **268 testes automatizados passando; clippy limpo** (`--all-targets -D warnings`).
- **9 estratégias implementadas** (3 em produção — balance-area, opening-reversal e range-extreme-fade — em 8 instâncias; 5 arquivadas com veredito documentado; e a `pullback-trend-v1`, desligada da produção em 04/09/2026 pelo ADR-016). A §4 acima detalha 6 das 9.
- **14 ativos** com 17,5 meses de histórico validado (0 gaps).
- **7 livros** analisados e registrados como fontes (Brooks ×3, Grimes, Dalton, López de Prado, + Chan pendente de OCR).
- **Live paper em contagem para o gate B** com bugs reais encontrados e corrigidos — pregões e trades válidos: a reconferir (a contagem de §7.3 é a de 2026-08-06).

---

## 11. O que falta para o dinheiro real

1. **Gate B**: contagem em curso (os números de §7.3 são de 2026-08-06 — a reconferir). Dar push nos commits do ADR-018/ADR-019 recria as instâncias de produção e muda o `config_hash` da `range-extreme-fade-v1`, o que **reinicia as 4 semanas** de contagem — quando publicar é decisão do dono.
2. **Amostra**: nenhuma das combinações re-rodadas com o flatten chega aos 50 trades OOS do ADR-010; só o paper forward produz amostra nova.
3. **Risco global de portfólio** antes de escalar processos (small-caps correlacionados).
4. **Governança**: ADR de go-live + primeiro mês com risco reduzido.
5. **Sizing por liquidez e fração de capital (ADR-020)**: **proposto, não implementado** — é a resposta à correlação de 0,57 entre tamanho e resultado da balance-area em AVUV.
6. **Melhorias opcionais mapeadas**: webhook URL real, Deflated Sharpe (validação anti-autoengano), meta-labeling (AFML), dashboard (Fase 7), OCR do Chan.

---

## 12. Referências

- `docs/HANDOFF.md` — estado operacional completo, pendências e rotina.
- `docs/reports/` — relatórios dos dias de validação live e os vereditos de gate A. O que vale hoje é `gate-a-com-flatten-2026-09-07.md`, que substitui `gate-a-revalidacao-2026-09-04.md`.
- `docs/strategies/` — especificação e veredito de cada estratégia (a nota de correção v1.0.1 do hotfix ET está em `range-extreme-fade-v1.md` §17).
- `docs/books/analysis/` — análise dos livros com as candidatas rankeadas.
- `docs/decisions/` — ADRs (arquitetura, entrada stop, gate de go-live). As duas que mais mudam operação e leitura de resultado hoje: **ADR-018** (paridade de fim de sessão — implementado) e **ADR-019** (harness de validação — implementado, mas o gate estatístico do §7 é **proposta**, não vigora). **ADR-020** (sizing por liquidez) é **proposto, não implementado**.
- `docs/OPERATIONS.md`, `docs/runbooks/` — operação e emergências.
