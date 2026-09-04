# HANDOFF — Estado do projeto e do que já foi testado

**Atualizado:** 2026-08-29 (guarda de overshoot ADR-015; painel web ADR-014; **fix do `risk_amount`**: passa a gravar o risco real — distância do stop × quantidade — e não o orçamento de 1%; o cap de notional cortava a posição mas o denominador do `result_in_r` continuava o orçamento, comprimindo o avgR do gate ADR-010 e quebrando a paridade com o backtest, que sempre calculou R sobre o risco real. Trades ≤ id 12 carregam o R antigo; o painel mostra os dois) — 11 instâncias/4 estratégias
**Público:** próximo agente (ou humano) que assumir o projeto. Leia isto antes de qualquer outra coisa.

---

## 1. Onde o projeto está

Bot de day trade em Rust (workspace multi-crates), **hospedado no servidor da casa como um app do umbrelOS desde 2026-08-28** (ADR-013; o ADR-012 trouxe para a casa, o ADR-013 empacotou como app — dados em `/home/umbrel/umbrel/app-data/daytradebot`, a VM Oracle da ADR-011 virou fallback desligado), em **paper trading live na IBKR** (conta paper DUR507388). O PC Windows está fora da operação — serve só para dev e acesso via SSH.

> ### ✅ Estado em 2026-08-28: migrado para app do umbrelOS, reboot validado
>
> **O problema que motivou tudo:** uma queda de energia derrubou o servidor em
> 2026-08-23 18:06 UTC e ele só voltou em 08-27 21:59 UTC. **4 pregões perdidos.**
> O reboot apagou o usuário `trader`, os timers `trader-*` e o runner do CI — a
> raiz do umbrelOS é um overlay rugix resetado a cada boot. A recuperação era um
> procedimento manual de 6 passos que alguém precisava lembrar de executar.
>
> **Resolvido (ADR-013).** Todo o serviço virou um app do umbrelOS, cujo estado
> vive em `/home/umbrel/umbrel` (persistente) e que o umbreld sobe sozinho no
> boot. Cutover em 2026-08-28, com **reboot real de validação**: containers de pé
> aos **91s**, Gateway logado na IBKR aos **144s**, banco íntegro
> (signals 303 · orders 9 · fills 43 · trades 6 · candles 233.149, idêntico ao
> pré-cutover), **sem intervenção humana**.
>
> Os timers systemd viraram o container `scheduler`; o runner do CI virou
> container e é hoje o único acesso administrativo que sobrevive a um reboot.
> As imagens em `ghcr.io/drumblow/*` **não contêm credencial nem binário da
> IBKR** — verificado em teste.
>
> **Amostra do gate B tem lacuna:** o setup da casa ainda não completou um pregão
> inteiro; o último dia real de operação foi **2026-08-21, ainda na Oracle**. O
> primeiro pregão do app é **2026-08-31**.
>
> Ainda em aberto: **BIOS sem religamento automático após queda de energia** (o
> app se recupera depois que a máquina liga — ele não liga a máquina) e
> **nenhum alerta** se algo cair.
>
> Leia `docs/decisions/ADR-013-app-umbrelos.md` e
> `docs/runbooks/cutover-app-umbrelos.md`. O objetivo atual é cumprir o **gate
> composto de go-live (ADR-010)** para operar dinheiro real:

- **A. Estratégia (estatística):** backtest ≥ 6 meses com ≥ 50 trades, win rate ≥ 40%, PF ≥ 1.3, DD ≤ 10%, avg R > 0.15 + walk-forward OOS. **✅ FECHADA para pullback-trend-v1 em IWM, IWV e IWO** (walk-forward 6 janelas sobre 17,5 meses, 2026-08-06). Duas novas estratégias aprovadas em qualidade OOS (amostra < 50): `opening-reversal-v1` (IWM/IWN/IJR/VB/SLYV) e `balance-area-breakout-v1` (9 ativos) — ver §10/§11.
- **B. Operação:** 4 semanas de paper live contínuo (uptime ≥ 99%) + ≥ 20 trades reais dentro de ±30% do backtest + zero violações de risco + reconciliação semanal. **Em andamento desde 2026-08-04 (9/20 pregões, 3/20 trades — 1 stop, 2 alvos, P&L -$535.01 — portfólio de 8 instâncias/3 estratégias desde 2026-08-07; ver relatórios dias 4–9 em `docs/reports/`).**
- **C. Governança:** ADR de go-live + primeiro mês com risco reduzido. Pendente.

Documentos-chave: `docs/cto-validation-plan-2026-08.md` (plano completo S1–S7), `docs/decisions/ADR-009` (entrada stop), `docs/decisions/ADR-010` (gate), `docs/runbooks/` (operação diária, checklist go-live, troubleshooting).

## 2. O que foi implementado e TESTADO (com evidência)

| Item | Estado | Evidência |
|------|--------|-----------|
| Live persiste ordem→fill→trade (polling `executions` 15s + `FillTracker`) | ✅ implementado, testado em simulação; validação live em curso | S1, `trade_tracker.rs` (7 testes) |
| Estado de risco durável (rebuild do banco no boot/restart) | ✅ **verificado em live 2026-08-04** | log boot: "estado de risco reconstruído do banco" |
| Risco via config (`[risk]` no TOML) + paridade backtest/live | ✅ | `risk_config.rs` |
| Backtest: stops/alvos intrabar, Sharpe correto, sem fallback sintético silencioso, runs persistidos | ✅ | S3; backtest real SPY executado (run id=5) |
| Walk-forward OOS (`trader-cli walkforward`) | ✅ | executado 2026-08-03 (runs id=2,3) |
| `trader-cli analyze` (live vs backtest + veredito) | ✅ | executado 2026-08-03 |
| Hardening: guards de modo/porta real, migrações no boot, **live falha fechado sem banco** (verificado 2026-08-04 com Postgres parado), circuit breaker, alertas webhook | ✅ parcialmente verificado | S6 |
| `system_events` live_started/live_stopped | ✅ **verificado em live 2026-08-04** | tabela `system_events` |
| **Entrada buy stop (ADR-009)** | ✅ **validado contra a conta paper real em 2026-08-04** | `ibkr_stop_entry_smoke`: bracket STP aceito (parent STP + TP LMT + SL STP), cadeia de 3 ordens visível, cancelamento em cascata OK |
| Cancelamento de entrada stop expirada no live | implementado | ainda não exercitado em sessão real |
| Regra `min_candles_above_ema20` (S2) | ✅ **exercida em live 2026-08-04** | rejeição real: `NoContext { streak: 7, min_required: 10 }` |
| Cursor de candles por horário nominal de fechamento (bug de processar candle de ontem no startup) | ✅ **corrigido e verificado 2026-08-04** | cursor sincroniza no último candle realmente fechado |
| Janela de horário ajustada para DST (13:30–20:00 UTC = 9h30–16h ET) | ✅ 2026-08-04 | `config/strategies/pullback-trend-v1.toml` (revisar na virada do DST) |
| `classify_market_phase` alinhado à janela DST (13:30–21:00 UTC) + save de `market_phase` com mapeamento explícito (constraint `pre_market`) | ✅ **corrigido e verificado 2026-08-04** | bug encontrado em live: rejeição `OutsideTradingHours` indevida 13:30–14:30 UTC + falha de constraint ao persistir contexto |
| Latência dos candles medida em live: candle processado ~30s após o fechamento (dados efetivamente em tempo real; erro 10168 afeta só `get_quote`, que o bot não usa) | ✅ medido 2026-08-04 | log: candle 13:45 processado 14:00:32 |

Testes automatizados: **145 passando, 0 falhando; `cargo clippy --workspace --all-targets -- -D warnings` limpo** (atualizado 2026-08-06).

## 3. O que NÃO foi testado ainda (riscos abertos)

- ~~Fill real de uma entrada stop no live~~ ✅ aconteceu em 2026-08-04 (2 trades completos: sinal → STP → fill → trade persistido → alerta → risco atualizado).
- ~~Cancelamento de entrada expirada em sessão real~~ ✅ **verificado 2026-08-04 19:00:32 UTC**: ordem 16 (STP 772.45) cancelada após 1 candle sem rompimento; confirmação [202] do gateway e status `cancelled` no banco.

## 3.2 Operação multi-ativo (desde 2026-08-07 — topologia VM Oracle)

**8 instâncias do live rodando na VM Oracle** (uma por ativo, um processo systemd `trader@<nome>` cada — não é refactor multi-símbolo), cobrindo as 3 estratégias aprovadas:

| client_id | Ativo | Estratégia |
|-----------|-------|------------|
| 1 | IWM | pullback-trend-v1 |
| 2 | IWV | pullback-trend-v1 |
| 3 | IWO | pullback-trend-v1 |
| 4 | IJS | balance-area-breakout-v1 |
| 5 | VBR | balance-area-breakout-v1 |
| 6 | AVUV | balance-area-breakout-v1 |
| 7 | IWM | opening-reversal-v1 |
| 8 | IWN | opening-reversal-v1 |

**Justificativa (decisão do dono, 2026-08-07):** o gate B precisava de ritmo — 1/20 trades após 3 pregões. SPY/QQQ foram cortados (reprovados estatisticamente em tudo). Novas estratégias vão ao paper live para **acumular amostra forward** — resolve a pendência de amostra do §5 item 1 pela opção "seguir acumulando".

Cuidados:
- **Risco é por processo**: 8 instâncias ⇒ limites diários somados proporcionalmente maiores. OK para paper; **antes de dinheiro real é obrigatório um limite global de portfólio** (PortfolioManager).
- A API exige um `client_id` por conexão — nunca subir duas instâncias com o mesmo id (mapeamento nos envs `/etc/trader/instances/*.env`).
- O filtro de fills por símbolo já separa as execuções das instâncias na mesma conta.
- **Sessão única IBKR**: nunca abrir TWS/Gateway local com o usuário do bot — derruba a sessão da VM (aconteceu em 2026-08-07 13h04 ET; ver ADR-011 e §7).
- **Alertas via webhook** (`[alerts].webhook_url` vazio — configurar e testar; com a operação na VM, é o único canal se o CB disparar).
- **Comissões reais**: parse de `CommissionReport` implementado no dia 3 — fills novos têm comissão real; fills antigos (antes de 2026-08-06) gravaram comissão 0.
- `get_order_status` assume `Filled` para ordem que some das abertas (sem caller em produção hoje).
- **Market data**: `get_quote` falha com erro 10168 (conta paper sem inscrição de dados em tempo real). **Não afeta o bot hoje**: as estratégias usam candles históricos (chegam ~30s após o fechamento — medido) e nunca chamam `get_quote` (o filtro de spread fica inativo enquanto `quote=None` nas chamadas).
- `exit_reason` dos 2 primeiros trades ficou `manual` mas era alvo (TP abaixo do mercado encheu a preço melhor) — classificação por proximidade de preço não cobre esse caso.

## 3.1 Incidentes de 2026-08-04 (resolvidos)

1. **Entradas com latência**: 2 sinais chegaram à corretora com o preço já muito além do gatilho (poll de 30s + processamento). O buy stop virou marketable: entradas a 767.20/771.34 com gatilho 766.42/770.46, e o TP limit ficou abaixo do mercado → trade abriu e fechou em ~1s com perda pequena (-6.52 e -4.08). **Correção:** guard `SetupInvalidated` na estratégia — se o último fechamento já passou do gatilho, o sinal é rejeitado (teste unitário incluso).
2. **Fills 4h no passado**: a TWS reporta execuções em horário local (ET), não UTC. **Correção:** parse com `chrono-tz` (assume America/New_York, honra sufixo de timezone). Os 11 fills e 2 trades de 2026-08-04 foram **corrigidos retroativamente** (+4h) no banco.
3. **Efeito colateral:** os 2 trades-artefato consumiram 2 dos 3 trades/dia e 2 das 3 perdas consecutivas do dia. **Decisão (2026-08-04, aprovada pelo dono):** os 2 trades (id 7 e 8) foram marcados no `journal` com `latency_artifact: true`. O `analyze` exclui artefatos da amostra de validação e o `rebuild_risk_state` não os conta em P&L/perdas consecutivas. O registro permanece no banco para auditoria.

**Aviso benigno conhecido:** no restart, o replay de execuções do dia loga `fill sem ordem rastreada` para trades já fechados e persistidos — esperado, não é erro.

## 4. Como operar (rotina diária — usuário no Canadá, fuso ET)

**A rotina manual acabou, e desde 2026-08-28 ela também sobrevive a reboot.**
Tudo roda como um app do umbrelOS (ADR-013), instalado da app store
`Drumblow/umbrel-daytradebot-store`.

- O **umbreld sobe o app sozinho no boot**. Validado com reboot real em
  2026-08-28: containers de pé aos 91s, Gateway logado na IBKR aos 144s, sem
  ninguém tocar em nada.
- O container `daytradebot_scheduler_1` substitui os três timers systemd: sobe
  as instâncias seg–sex **9h25 ET**, para **16h10 ET**, backup **16h30 ET**
  (retenção 7d). Se o app subir dentro do pregão, ele liga as instâncias na hora.
- Cada instância tem uma **guarda de janela** no entrypoint: fora do horário ela
  sai com `exit 0` e fica parada. É o que impede 11 conexões na IBKR quando o
  servidor religa de madrugada.
- Postgres e Gateway ficam no ar 24/7.

Containers agora se chamam `daytradebot_<serviço>_1` (não mais `trader-*`).

```bash
# acesso ao servidor
ssh -i ~/.ssh/trader_home_deploy umbrel@<SERVIDOR_CASA>

# estado dos containers
sudo docker ps -a --filter name=daytradebot_

# log de uma instância
sudo docker logs daytradebot_iwm-pullback_1 --tail 50

# parar / subir as 11 instâncias
sudo docker exec daytradebot_scheduler_1 /usr/local/bin/scheduler.sh stop-instances
sudo docker exec daytradebot_scheduler_1 /usr/local/bin/scheduler.sh start-instances

# app inteiro
sudo umbreld client apps.restart.mutate --appId daytradebot
```

> **Sem sudo depois de um reboot?** É esperado: o `sudoers.d` some no boot.
> Opere pelo runner que vive dentro do app, sem senha:
> `gh workflow run host-check.yml` (só lê) e
> `gh workflow run ops.yml -f acao=start-instances|stop-instances|backup|restart-app|logs-gateway`.

**Deploy:** `push` em `main` → `images.yml` compila e publica as imagens em
`ghcr.io/drumblow/*` → o job `deploy` roda no runner do app e recria as 11
instâncias. Fora do pregão elas sobem com a imagem nova e saem sozinhas pela
guarda, ficando prontas para as 9h25.

⚠️ **NUNCA abrir TWS/Gateway local com o mesmo usuário do bot** — a IBKR permite
uma única sessão por usuário e o login local derruba a sessão do servidor
(derrubou as 8 instâncias em 2026-08-07 13h04 ET). Ver ADR-011 e ADR-012.

⚠️ **O que ainda NÃO se recupera sozinho:** o usuário `trader`, o `sudoers.d` e
o acesso SSH administrativo continuam sendo apagados a cada boot pelo rugix — o
app sobrevive, o acesso humano não. E **não existe alerta** se algo cair: o
`webhook_url` segue sem configuração (§5).


## 5. Pendências priorizadas

0. ~~**⚠️ CRÍTICO — feed de dados degradado na VM**~~ ✅ **RESOLVIDO em 2026-08-17** (ver `docs/reports/validacao-live-vs-backtest-2026-08-07_a_08-14.md`, incl. §8 revalidação). Resumo: a barra recém-fechada chega como 1 print e consolida ~3–4 min depois; o bot agora **espera a barra estabilizar** (guarda v3, verificada ao vivo), o repositório **auto-repara** linhas flat via upsert, e o histórico 08-07+ foi re-ingerido (26/26 barras com range/dia). **Decisão do dono (2026-08-17): pregões 4–9 EXCLUÍDOS da amostra do gate B** (trades 10 e 11 marcados `data_quality_suspect`) — a semana conta como teste de infraestrutura; a amostra limpa recomeça em 08-18. ~~**Anomalia aberta:** o trade 11 (IWO, 08-13) tem `exit_reason='target'` inconsistente com os preços do bracket~~ ✅ **RESOLVIDA em 2026-08-29:** não era bug de casamento de pernas — a perna de alvo executou de verdade (TP limit abaixo do mercado após o gap de 0.67 na entrada enche a preço melhor; o classificador direction-aware de 08-17 classificou certo). A causa-raiz (entrar com o preço já além do gatilho) foi eliminada pela **guarda de overshoot do ADR-015**, que também corrigiu o fill otimista do backtest (gap agora enche na abertura e overshoot além de 25% da distância do stop invalida a entrada — vale para live, replay e backtest; ver `docs/decisions/ADR-015-guarda-overshoot-entrada-stop.md`). ⚠️ Backtests anteriores ao ADR-015 não são comparáveis com os novos; re-rodar o backtest do portfólio antes de comparar gate B. **Nova estratégia aprovada em 08-17:** `range-extreme-fade-v1` (fade de extremos em dias de range) validada em AVUV/SLYV/IWV — **instalada na VM em 08-17 ~22h45 ET** (instâncias `avuv-rangefade`, `slyv-rangefade`, `iwv-rangefade`, client_ids 9–11, habilitadas; sobem com o timer de 09h25 ET; portfólio vai a 11 instâncias); marcador de exclusão do gate B registrado em `system_events` (id 322). Detalhes em `docs/strategies/range-extreme-fade-v1.md` §16 e `docs/reports/day10-2026-08-17.md`.

1. ~~**Decisão do dono — gate A e portfólio de estratégias**~~ ✅ **decidida em 2026-08-07:** opção "seguir acumulando" — as 3 estratégias aprovadas em qualidade OOS foram ao paper live (8 instâncias, §3.2) para acumular amostra forward. Amostras OOS < 50 por ativo (ver §10) continuam abertas para fechamento formal do gate A dessas estratégias.
2. **failure-test-long-v1 arquivada** (amostra 10–12 trades em 17,5 meses; negativa fora de IWM). Revisar seletividade só com decisão explícita (v1.1 = nova versão).
3. ~~**Webhook de alertas sem URL**~~ ✅ **CONFIGURADO — URL ativa e funcional no servidor (confirmado pelo dono em 2026-08-31)**, em `${APP_DATA_DIR}/secrets/alerts.env`, montado read-only nas 11 instâncias pelo entrypoint. ⚠️ Se a URL for um webhook **nativo do Discord**, o binário em produção (`main`) ainda envia `{"text": ...}`, que o Discord recusa com 400 — e a versão em `main` **engole** a recusa. O corpo correto por destino (Discord `content` / Slack `text`) e a checagem de status estão na branch `fix/auditoria-2026-08-30` (C4 da auditoria), ainda não mergeada. Eventos que **continuam sem alerta** mesmo com a URL: ordem rejeitada, limite diário atingido, timeout de confirmação, falha de backup, contagem de instâncias errada no open (A9 da auditoria).
4. ~~Fase 7 (dashboard)~~ ✅ **feita em 2026-08-28 (ADR-014):** painel web `trader-web` servido pelo app (porta 8551), read-only, com R real vs R orçado e badges nos trades fora da amostra de validação.

## 6. Decisões recentes (não reverter sem discutir)

- **Entrada é buy stop** (ADR-009), não limit — fidelidade ao livro. Configurável via `entry_order_type` no TOML da estratégia.
- **Gate de go-live é o composto da ADR-010** (4 semanas + métricas), que substituiu o "3 meses" de calendário.
- **Live não sobe sem banco** e recusa porta real (7496/4001) — falha fechada intencional.
- Backtests anteriores a 2026-08-04 usavam entrada limit: **não comparáveis** com os novos.


## 7. Diário de validação

- **Dia 1 — 2026-08-04:** `docs/reports/day1-2026-08-04.md`. 5 bugs encontrados e corrigidos; 1 trade válido (IWM, stop -521.01); 2 trades-artefato excluídos; 1 entrada expirada/cancelada corretamente; amostra do gate: 1/20 trades.
- **Dia 2 — 2026-08-05:** `docs/reports/day2-2026-08-05.md`. 3 instâncias (SPY/QQQ/IWM) 9h39–16h20 ET; 1 interrupção de ~2 min às 10h08 (processos morreram ao fechar a sessão do CLI; relançados detached — nova rotina padrão). **Zero sinais e zero trades** — mercado sem setup H2. Amostra do gate segue 1/20 trades. `failure-test-long-v1` implementada e smoke-testada no mesmo dia (fora do pregão).
- **Dia 3 — 2026-08-06:** `docs/reports/day3-2026-08-06.md`. Live estável o pregão todo; **zero sinais/trades** (2º dia seguido sem setup H2 — amostra do gate segue 1/20; ritmo dos 20 trades em risco, depende da expansão). Dev: bug do alerta do CB corrigido (verificado com webhook local), comissões reais implementadas, 3 novas estratégias validadas (2 aprovadas em qualidade OOS), expansão de 8 ativos — **pullback fecha o gate A em IWM/IWV/IWO**.
- **Dia 4 — 2026-08-07:** **portfólio expandido para 8 instâncias / 3 estratégias** (§3.2) por decisão do dono — novas estratégias ao paper live para acumular amostra forward. Manhã **sem sinais** (pullback sem streak EMA20; balance sem área válida — exceção VBR com área ativa 248,05–251,67 sem rompimento; opening-reversal sem setup na janela). **Migração para a VM Oracle executada à tarde** (ADR-011): IB Gateway headless via IBC + Xvfb, Postgres migrado por pg_dump, 8 instâncias systemd com timers 9h25/16h10 ET, CI/CD de deploy (GitHub Actions → scp → restart condicional). **Interrupção 13h04–13h30 ET documentada:** o login do Gateway na VM derrubou o TWS local (sessão única IBKR) e as 8 instâncias locais morreram — cutover antecipado para 13h30 ET por causa disso. Relatório completo: `docs/reports/day4-2026-08-07.md` (3 sinais IWM na manhã sem ordens persistidas por causa do kill da sessão; 5 CBs com auto-recuperação na tarde de estreia da VM; 1 candle perdido no cutover).
- **Dia 5 — 2026-08-10:** `docs/reports/day5-2026-08-10.md`. 1º pregão 100% VM. 3 sinais IWV pullback → 3 brackets → stop 440.97 nunca tocado → entradas expiradas/canceladas corretamente. **0 trades.** Gate: 5/20 pregões, 1/20 trades.
- **Dia 6 — 2026-08-11:** `docs/reports/day6-2026-08-11.md`. **0 sinais/0 trades** (pregão processado integralmente — 26/26 candles/ativo). 1 CB breve em pregão (13h29 ET, ~30s de recuperação). Gate: 6/20 pregões.
- **Dia 7 — 2026-08-12:** `docs/reports/day7-2026-08-12.md`. **1º win do portfólio expandido:** IWV pullback +$69.42 líquido (alvo 440.71, ~25 min; comissões reais $12.69 descontadas). 1 CB breve em pregão (14h28 ET). Gate: 7/20 pregões, **2/20 trades**.
- **Dia 8 — 2026-08-13:** `docs/reports/day8-2026-08-13.md`. **1 trade — loss -$83.39 em IWO por gap de abertura de candle** (abriu 393.95 > gatilho 393.55 → stop marketable → TP marketable 2s depois). NÃO é artefato de latência (diferente do dia 1): trade válido pro gate. `exit_reason` gravou `manual` mas era a perna de alvo — bug de classificação corrigido em 2026-08-17 (classificador direction-aware) e registro ajustado para `target`. Gate: 8/20 pregões, **3/20 trades**.
- **Dia 9 — 2026-08-14:** `docs/reports/day9-2026-08-14.md`. **0 sinais/0 trades.** CB em pregão 14h56 ET (3 instâncias, ~30s). **Revisão de fim de semana (08-15/16) encontrou 4 problemas de infra, todos corrigidos em 2026-08-17:** (1) **timer de parada nunca funcionou** — target sem `PartOf=` nas instâncias; bot rodou 24/7 desde o dia 4, gerando os loops de CB noturnos/de fim de semana; (2) **backups nunca rodaram** — a VM nem tinha o pacote `cron` instalado (agendamento migrado para timer systemd `trader-backup.timer`); (3) journal perdeu logs anteriores a 08-16 — retenção explícita configurada; (4) classificação de `exit_reason` (item 4 acima). Unidades systemd, timer, script de backup e cron passaram a ser **versionados no repo (`deploy/`) e instalados pelo CI**. Gate: 9/20 pregões, 3/20 trades, P&L acumulado -$535.01.

- **Pregões 31/08, 01/09 e 02/09 (app umbrelOS)** — `docs/reports/pregoes-2026-08-31_a_09-02.md`. **Zero trades nos três dias, e o backtest sobre os mesmos pregões também dá zero** (11 pares): a seca é regime de mercado, não falha de estratégia — 0 barras de uptrend em IWV e IWO, e o pullback exige uptrend. Estatística do portfólio: 0,74 trade/pregão, 56% dos pregões zerados, 21% das janelas de 3 pregões secas. **Quatro problemas encontrados:** (1) as 2 instâncias de IWM estão cegas desde 07/08 — exposição órfã na conta paper faz `find_exposure` interromper o ciclo antes de gravar contexto; (2) a ordem 17 (VBR, 28/08) não executou com o mercado 35c além do gatilho — provável rejeição da IBKR assumida como aceita no timeout de confirmação; (3) `live_stopped` nunca aconteceu em toda a vida do projeto (só SIGINT era tratado, `docker stop` manda SIGTERM); (4) 3 barras abandonadas sem registro. Itens 1, 3 e 4 corrigidos; o 2 agora vira alerta (`order_missing`).

- **03/09/2026 — noite de correcoes, incidente e uma decisao de portfolio.** Relatorios: `docs/reports/incidente-2026-09-03-ordens-duplicadas.md` e `docs/decisions/ADR-016-desligar-pullback-trend-v1.md`.
  - **Branch de auditoria mergeada e no ar** (C1, C3, C5, A1, A2, A3, A5, A6, alertas Discord) + A7 (suite verde antes de publicar imagem, guarda de janela no deploy, pin por commit).
  - **A posicao orfa de IWM foi confirmada e encerrada:** 827 acoes long desde 07/08, sem stop, ~-5.8k nao realizados que nunca apareceram no painel. Ordem de venda enfileirada para a abertura de 04/09. Enquanto ela existia, as DUAS instancias de IWM ficaram 18 pregoes sem avaliar nada.
  - **Causa raiz das ordens orfas encontrada:** `confirm_order` tratava fim de stream sem status como FALHA, embora a ordem ja estivesse transmitida. O chamador achava que falhou, nao rastreava, e a ordem ficava orfa. Corrigido.
  - **Incidente:** o flatten manual repetiu a ordem 3x por causa disso e deixou 3 vendas de 827 enfileiradas (viraria posicao vendida de 1.654). Detectado em 3 minutos pela acao nova `exposicao`, cancelado, causa corrigida.
  - **Ferramentas novas de operacao** (`ops.yml`): `exposicao` (le posicoes e ordens reais no broker — o painel so le o banco), `flatten`, `cancelar-ordens`, `logs-instancia`.
  - **Queda de energia** derrubou o servidor as 21h30 UTC; voltou por acao manual. A BIOS continua sem religamento automatico.
- **04/09/2026 — pendencias medias fechadas.** Pool do Postgres (10 conexoes por processo x 11 instancias = 110 contra `max_connections=100`; passa a 3 + statement_timeout de 30s), validacao de LADO do stop (o RiskManager usava distancias absolutas: long com stop acima da entrada passava e virava bracket que estopa no instante seguinte), `build_risk_config` FALHA FECHADO (NaN ou typo no horario viravam o padrao em silencio), guarda de FILL PARCIAL (as pernas do bracket saem com quantidade cheia; se a entrada enche em parte, o stop ao disparar vende o que existe e ABRE POSICAO INVERTIDA no resto — o watchdog agora compara as quantidades e grita), `.sqlx` commitado (workspace compila com SQLX_OFFLINE, sem banco vivo), scheduler alerta por webhook (backup falhado, pg_dump minusculo, contagem errada no open) e painel deixa de rodar como root. **Imagem do bot segue como root de proposito**: le `secrets/alerts.env`, um diretorio 0700 do UID 1001; trocar o usuario sem alinhar o UID mataria o webhook em silencio.
- **04/09/2026 — A8 (backup off-site) RETIRADO DO ESCOPO** por decisao do dono. O backup diario local continua; nao ha copia fora da maquina. Falha do disco perde banco e backups.
- **04/09/2026 — PENDENTE: atualizar o app para a v1.2.0** (`umbrel-daytradebot-store`, commit `a093a5e`, **nao publicado**). Leva duas coisas que so o compose resolve: desligar as 3 instancias da pullback (ADR-016) e montar `secrets/` no scheduler para os alertas dele funcionarem. O compose que roda no host e a versao EXPANDIDA que o umbreld gera — editar a mao e mexer em artefato gerado, e se o umbreld regenerar a pullback volta em silencio. Depois que a v1.2.0 entrar, tirar as mesmas tres da lista `INSTANCIAS` do job `deploy` em `images.yml` (nao pode ir antes: o deploy pararia de recriar containers que o scheduler ainda liga).
- **04/09/2026 — C2 FECHADO (ADR-017): limite de risco da conta inteira.** Era o bloqueador de go-live. Todo o risco era por processo: cada instancia so somava trades do proprio simbolo, entao `max_daily_loss_pct = 2%` virava ate 22% com 11 instancias, e o cap de notional (~100% do capital por posicao) valia por processo. Agora toda instancia checa a CONTA antes de abrir posicao: perda diaria agregada (4%), posicoes simultaneas (3) e notional agregado (200%). Sem tabela nova — o broker e autoritativo sobre exposicao, o banco sobre P&L do dia. Falha fechado sem banco. Padroes calibrados para conta paper com margem; **apertar antes de dinheiro real**. Ainda falta kill-switch comum.
- **04/09/2026 — A4 corrigido: o backtest tinha custo de execucao 100x menor do que a config dizia.** `slippage_pct` valia 0,001 (documentado 0,1%) e era dividido por 100 de novo na aplicacao: efetivo 0,001%. As SAIDAS nao tinham custo nenhum e nao modelavam gap. Agora: slippage e fracao do preco, aplicado sempre contra o trader; stop acionado vira execucao a mercado (enche na abertura em caso de gap, e escorrega); alvo e limite (enche melhor num gap, sem slippage). Default **calibrado em 2 bp** — os ETFs operados custam de US$ 120 a 435, onde 1 centavo vale 0,23 a 0,83 bp. `backtest --slippage-bps N` permite varrer. **TODO backtest anterior a esta correcao esta invalidado.** O gate A precisa ser refeito.

## 8. Nova estratégia implementada: `failure-test-long-v1` (2026-08-05)

- Análise de 4 livros novos (`docs/books/analysis/`): Brooks Bar-by-Bar, Grimes, Dalton, López de Prado. Tabela de fontes atualizada em `docs/strategy-analysis-framework.md` §2. Chan ficou pendente (PDF escaneado, precisa OCR).
- Escolhida pelo dono: **Failure Test / spring de Wyckoff (Grimes)** — reversão long em suporte, complementar à pullback-trend-v1. Doc completo: `docs/strategies/failure-test-long-v1.md`.
- Implementação: `crates/trader-core/src/strategies/failure_test_long_v1/` + dispatch por strategy id (`trader-cli/src/dispatch.rs`) + override de risco 0,5%/trade (`risk_config.rs`) + **saída ativa por tempo** (0,5R em 3 candles, `trader-core/src/execution/time_exit.rs`, com paridade live/backtest; no live cancela as pernas do bracket antes de fechar a mercado).
- **105 testes passando, clippy limpo.** Validação executada na mesma noite (resultados em `docs/reports/day2-2026-08-05.md` §5): amostra insuficiente (6–9 trades/ano/símbolo) — **inconclusiva, não vai pro live**. Próximo passo: mais histórico (2 anos) ou revisão de seletividade — decisão do dono.
- **Bug crítico de paridade corrigido em 2026-08-05:** o backtest não resetava o estado de risco por dia (limites diários valiam para o período inteiro). Todos os backtests anteriores a essa correção subestimam o número de trades — runs 11–16 inválidos.

## 9. Dia 3 (2026-08-06, manhã) — observabilidade, símbolos e comissões

- **Bug encontrado e corrigido no teste do circuit breaker:** o alerta crítico do CB se perdia — `Alerter` era fire-and-forget (`tokio::spawn`) e o processo encerrava antes do envio. Agora `critical_await`/`info_await` (timeout 5s) nos caminhos de encerramento (CB e live_stopped). **Verificado:** webhook local recebeu "Live iniciado" + "🚨 circuit breaker"; CB dispara aos 10 falhas e o processo sai com erro. Método de teste documentado: instância com símbolo inválido + receptor local `target/tmp/webhook_receiver.py`.
- **Comissões reais:** `CommissionReport` da IBKR agora é parseado e casa com o fill por `execution_id` (`broker.rs`); o `FillTracker` já descontava comissão do net_pnl — só faltava o parse. Fills novos do live passam a ter comissão real.
- **Novos símbolos validados e reprovados:** IWN/IJR/MDY ingeridos (9.430 candles cada, 0 gaps) e testados no pipeline completo — nenhum passa (ver §5).
- **Rotina de dev durante pregão:** live roda da cópia `target/tmp/live-bin/trader-cli.exe` (reiniciar a partir dela após mudanças); builds ficam livres em `target/debug`. Reinício do live em pregão é seguro (estado reconstruído do banco).
- **Melhorias de paridade live/backtest (fim do dia 3):** (a) janela de contexto do live subiu de 80 para **200 candles** (~8 dias; fetch de 10 dias) — divergência encontrada na investigação de "zero trades" (QQQ 08-05 tinha setup com histórico completo, não com 80 candles); (b) o live passa a **persistir no banco os candles que processa** (antes só entravam via ingest manual). Ambas verificadas: 145 testes passando, clippy limpo; live-bin rebuildado.
- **Doc novo:** `docs/BOT-FUNCIONAMENTO.md` — visão geral do sistema para não-técnicos (arquitetura, estratégias, risco, validação, gate).

## 10. Dia 3 (tarde) — pipeline de novas estratégias: 3 candidatas desenvolvidas e validadas

Metodologia: doc pelo framework → código + testes sintéticos → backtest + walk-forward 6 janelas × 6 ativos (17,5 meses). Tudo feito sem subagentes. **145 testes passando, clippy limpo.** Docs completos em `docs/strategies/` com vereditos.

| Estratégia | Fonte | Veredito |
|---|---|---|
| `breakout-first-pullback-v1` | Grimes | **ARQUIVADA** — 1–2 trades/ativo em 17,5 meses; rara demais para validar (calibração documentada no doc §16) |
| `opening-reversal-v1` | Brooks (Cap. 11) | **Qualidade OOS aprovada em IWM (PF 1.70, avgR 0.547), IWN (1.87, 0.491), IJR (1.83, 0.391)** — falta só amostra (24–30 < 50). Primeira estratégia com short |
| `balance-area-breakout-v1` | Dalton (Cap. 4) | **Qualidade OOS aprovada em IWN (PF 2.72, avgR 0.489), IWM (1.39, 0.339), QQQ (1.79, 0.253)** — falta só amostra (19–36 < 50) |

Aprendizados:
- **O edge do projeto vive em small-caps**: IWM/IWN/IJR concentram todas as aprovações (exceto balance em QQQ também). SPY e MDY reprovaram em tudo.
- **Short não exigiu mudança de infra**: simulador, RiskManager (RR em valor absoluto) e brackets do domínio já eram simétricos.
- **Escala importa nas conversões livro→código**: filtros calibrados por intuição falharam 2× (expansão 1,5× range; largura 3×ATR) — a correção veio de medir a distribuição real antes de escolher limiares.
- Candidatas restantes nas análises: `range-extreme-fade`, `trendline-break-test`, `anti-long`, `value-area-reentry`, `gap-continuation`; técnicas AFML (purged CV, DSR, meta-labeling).

## 11. Expansão de ativos (2026-08-06, fim da tarde) — 8 novos ativos validados

Pergunta do dono: "se a estratégia dá ~34 trades/ano em 1 mercado, 10 mercados dão 340?". Resposta executada: ingest de **IJS, VBR, AVUV, SCHA, VB, IWO, SLYV, IWV** (9.454 candles cada, 0 gaps) + backtest + walk-forward 6w × 3 estratégias aprovadas (24 runs OOS persistidos na tabela `backtest_runs`).

**Resultados OOS (trades/WR/PF/avgR) — pares aprovados em qualidade:**

| Estratégia | Aprovados (novos) | Já aprovados |
|---|---|---|
| pullback-trend-v1 | **IWV (88t ✅ fecha TODOS os critérios), IWO (80t ✅ idem)**, VB (96t, WR 39.6% marginal) | IWM (91t ✅) |
| opening-reversal-v1 | VB (23t, PF 1.75, avgR 0.425), SLYV (21t, 1.62, 0.418) | IWM, IWN, IJR |
| balance-area-breakout-v1 | **IJS (29t, PF 3.48, avgR 0.744 — melhor par do projeto)**, VBR (37t, 2.60), AVUV (36t, 2.79), SLYV (26t, 2.18), SCHA (21t, 1.52), IWO (11t, 3.89) | IWN, IWM, QQQ |

**Leitura consolidada:**
- **pullback agora tem 3 ativos que fecham TODOS os critérios do gate A**: IWM, IWV, IWO (≥50 OOS + WR + PF + avgR + DD).
- **balance-area-breakout é a mais robusta**: 9 de 14 ativos aprovados em qualidade (amostra agregada 235 OOS).
- **opening-reversal: 5 ativos** (amostra agregada 125 OOS).
- **Caveat honesto (múltiplos testes):** 42 pares testados → alguns passes são sorte estatística. Mas 9/14 com PF majoritariamente > 2 (balance) está além do acaso; a formalização é o Deflated Sharpe do AFML (pendente).
- **Correlação:** os aprovados são quase todos small-caps — trades chegam em clusters nos mesmos dias. Risco global de portfólio é pré-requisito antes de escalar N processos com dinheiro real.
