# ADR-011: Operação na VM Oracle (bot + Gateway + banco fora do PC)

**Status:** Aprovado  
**Data:** 2026-08-07  
**Autor:** CTO

---

## Contexto

A operação diária era manual e dependia do PC Windows do dono: abrir Docker Desktop, abrir o IB Gateway/TWS, lançar 8 processos `trader-cli` e manter o terminal aberto das 9h30 às 16h ET. A fragilidade se concretizou no dia 2 (2026-08-05): os processos morreram ao fechar a sessão do terminal, gerando uma interrupção de ~2 min e exigindo relançamento manual. Além disso, PC ligado, sono, viagem ou queda de energia/rede local interrompiam a validação do gate B (ADR-010), que exige uptime ≥ 99%.

O dono pediu operação autônoma, sem dependência do PC.

## Decisão

Bot, IB Gateway e PostgreSQL passam a rodar em uma **VM Oracle Cloud Always Free** (`instance-20260807-1139`, Ubuntu 24.04, x86_64, 954 MB RAM + 2 GB swap, 45 GB de disco). O PC Windows fica **fora do caminho operacional** — serve apenas para desenvolvimento e acesso administrativo via SSH.

### Topologia

- **IB Gateway 10.45 headless** na VM, via IBC 3.24.1 + Xvfb, sob systemd (`ibgateway.service`, `Restart=always`, heap JVM 384 MB, `OOMScoreAdjust=-500`). Login paper automático pelo IBC; credenciais em `/opt/trader/ibc/ibc.ini` (permissão 600). `AcceptNonBrokerageAccountWarning=yes` configurado — sem isso o disclaimer da conta paper bloqueia a API com erro 10141. `UseSSL=true` no Gateway↔IBKR (afeta só o link com os servidores; o socket API local segue plaintext, exigência da crate ibapi 3.1, que não fala SSL). API na porta **4002 (paper), bind localhost apenas**; UFW nega todo incoming exceto SSH.
- **PostgreSQL 15 em Docker** na VM (`docker-compose.yml` do repo: `restart unless-stopped`, binding `127.0.0.1:5433`, tunagem para 1 GB de RAM). Backup diário via `pg_dump` em cron (21:30 UTC, retenção de 7 dias, `/opt/trader/backups`). Banco local migrado por `pg_dump` (132.811 candles, 200 backtest_runs, histórico do gate preservado integralmente).
- **Bot: 8 instâncias systemd template** `trader@.service` (envs em `/etc/trader/instances/*.env`, client_ids 1–8), agrupadas em `trader-instances.target`. **Timers** `trader-start.timer` (seg–sex 9h25 America/New_York) e `trader-stop.timer` (16h10 ET) — o bot **não roda 24/7** deliberadamente, para evitar pacing da API da IBKR e loop de circuit breaker na madrugada. Binário em `/opt/trader/bin/trader-cli`.
- **Deploy:** `.github/workflows/deploy.yml` — push em `main` tocando `crates/**` ou `config/**` → testes → build release em GitHub-hosted runner → `scp` para a VM → restart condicional (só reinicia instâncias ativas; restart em pregão é seguro, pois o estado vem do banco). Secrets: `VM_HOST`/`VM_USER`/`VM_SSH_KEY` (chave dedicada `trader_deploy_ci`, não a chave pessoal). Environment `vm-production` no workflow.

## Motivos

- Remove o PC Windows do caminho crítico: o gate B (uptime ≥ 99%) deixa de depender de hábitos manuais.
- Always Free: custo zero recorrente, suficiente para o workload atual (8 processos Rust pequenos + Gateway + Postgres contidos).
- Timers em vez de 24/7: alinha o consumo de API à janela de operação e elimina classes inteiras de falha fora do pregão.

## Consequências

- **Sessão única IBKR:** a corretora permite UMA sessão por usuário. Abrir TWS/Gateway local com o mesmo usuário **derruba a sessão da VM**. Isso já aconteceu: em 2026-08-07 13h04 ET, o login do Gateway na VM derrubou o TWS local e as 8 instâncias locais morreram — o cutover foi antecipado para 13h30 ET por causa disso. **Regra operacional permanente: nunca abrir TWS/Gateway local com o usuário do bot.**
- **RAM apertada:** ~800 MB usados com tudo no ar, ~450 MB de swap em uso. `earlyoom` + `OOMScoreAdjust` protegem Postgres e Gateway. Fallback documentado: migrar para VM Ampere ARM (24 GB Always Free) se houver pressão de memória.
- **2FA:** login completo na IBKR pode exigir 2FA manual ~1x/semana (conta paper normalmente não pede; observar nos primeiros ciclos).
- Deploy passa a exigir push em `main` com testes verdes — não há mais "rebuildar o live-bin local" como caminho operacional.
- `docs/HANDOFF.md` e `docs/runbooks/live-operations.md` atualizados para a nova topologia.
