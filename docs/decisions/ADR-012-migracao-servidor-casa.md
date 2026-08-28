# ADR-012: Migração do live para o servidor da casa (umbrelOS, containers)

**Status:** Aprovado
**Data:** 2026-08-23
**Autor:** CTO (codeagent) + dono

---

## Contexto

O live rodava na VM Oracle Cloud Always Free (`instance-20260807-1139`, ADR-011) com
**954 MB RAM** (~310 MB livres) — o limite prático para o portfólio de 11 instâncias.
O dono decidiu migrar para o **servidor da casa** (umbrelOS em `<SERVIDOR_CASA>`,
Debian 13, 4 cores x86_64, 11 GB RAM, 211 GB livres), que oferece folga para
desenvolver e validar novas estratégias. A migração foi executada em 2026-08-22/23
(fim de semana — janela sem pregão).

## Decisão

**O live passa a rodar no servidor da casa, em containers Docker**, com dados
persistentes em `/data/trader` (área persistente do umbrelOS — o root `/` é overlay
imutável via rugix).

### Topologia (host umbrelOS)

| Componente | Forma | Detalhe |
|---|---|---|
| Postgres 15 | container `trader-postgres` | volume nomeado `trader_trader-postgres-data`, `127.0.0.1:5433`, tunagem p/ 11 GB de RAM |
| IB Gateway 10.45 | container `trader-gateway` (imagem própria) | Debian 13 + **ZuluFX JRE 17** (JavaFX obrigatório p/ login) + IBC 3.24.1 + Xvfb controlado por entry próprio; API `127.0.0.1:4002`; `network_mode: host`; `mem_limit 1g`, `shm_size 256m` |
| 11 instâncias do bot | containers `trader-<nome>` (imagem própria) | binário e configs por **volume bind** de `/data/trader/{bin,config}` (deploy não rebuilda imagem); envs por arquivo; `restart: unless-stopped` |
| Timers systemd (host) | `trader-start/stop/backup` | sobem/param containers via `trader-containers.sh` (start/stop), backup 21:30 UTC via `docker exec pg_dump` |

### Pontos que custaram investigação (registrados para não repetir)

1. **xvfb-run como PID 1 travava** sem executar o comando → entry próprio
   (`gateway-entry.sh`) que sobe o Xvfb e executa o gatewaystart explícito.
2. **Java 21 é rejeitado pelo IBC** ("No java executable found") e **Temurin puro
   falta JavaFX** (`NoClassDefFoundError: javafx/embed/swing/JFXPanel`) → **ZuluFX
   JRE 17** (`zulu17.68.17-ca-fx-jre17.0.20`), mesma família que a IBKR embute.
3. **Gateway não roda como root**: JxBrowser/Chromium falha silenciosamente sem
   login dialog → usuário `trader` no container, **uid 1001** (casa com o host;
   o `umbrel` é uid 1000).
4. **/dev/shm default 64MB** insuficiente para o renderer do Chromium → `shm_size: 256m`.
5. Libs de UI/GTK/browser necessárias no container (libnss3, gtk3, libasound2, etc.).

### Cutover

- **Ordem rígida pela sessão única IBKR**: Oracle parado (instâncias + `ibgateway`
  + timers desabilitados) **antes** de subir o Gateway na casa; validado com
  `trader-cli status` (client_id 99) contra a API 4002 da casa.
- Banco migrado por `pg_dump` do backup diário 21:30 UTC de 2026-08-22 (134.843
  candles, 604 trades, 201 backtest_runs, 1.117 system_events — histórico do gate B
  preservado integralmente).
- CI/CD: secrets `VM_HOST`/`VM_USER`/`VM_SSH_KEY` do GitHub apontam para a casa;
  `deploy.yml` instalou binário+configs em `/data/trader` e timers docker-based.

## Consequências

- **RAM deixa de ser gargalo**: ~2 GB usados hoje vs 954 MB físicos na Oracle com swap.
- **Atualização do umbrelOS**: containers e volumes sobrevivem (dados em `/data`);
  timers systemd do host são reinstalados pelo deploy (idempotente) se um update
  do OS os remover.
- **Acesso de administração**: dono autorizou a chave `trader_home_deploy` (ed25519)
  para `trader@<SERVIDOR_CASA>` com sudo NOPASSWD (necessário p/ docker + systemd do CI).
- **Sessão única IBKR segue valendo**: nunca abrir TWS/Gateway local com o usuário
  do bot; o Gateway agora vive no container da casa.
- **Oracle fica como fallback desligado**: serviços parados e timers desabilitados;
  VM mantida por ora (pode ser encerrada a qualquer momento — nada do live roda lá).
- `docs/runbooks/live-operations.md` e `docs/HANDOFF.md` atualizados para a nova topologia.
---

## Atualização 2026-08-27 — consequência subestimada

A consequência registrada como "*Atualização do umbrelOS: containers e volumes
sobrevivem (dados em `/data`); timers systemd do host são reinstalados pelo
deploy (idempotente) se um update do OS os remover*" **se mostrou insuficiente
na prática**.

Não é preciso um update do umbrelOS: a raiz é um overlay rugix
(`upperdir=/run/rugix/mounts/data/state/default/overlay/b`) e **qualquer reboot
pode resetá-la**. No incidente de 2026-08-23 sumiram de uma vez o usuário
`trader`, as units `trader-*` e o runner self-hosted — e os containers do trader
foram removidos (volume e imagens sobreviveram). O deploy que reinstalaria os
timers **depende do runner que também some**, então a recuperação automática
prevista aqui não acontece.

A decisão de rodar na casa continua válida (RAM, custo, folga para
desenvolvimento), mas ela **exige um mecanismo de reprovisionamento no boot que
viva em `/data`** — ver `docs/runbooks/live-operations.md` → "Blindagem contra o
reset do rugix" — além de nobreak e retorno automático na BIOS.

Incidente completo: `docs/reports/incidente-2026-08-23-queda-servidor.md`.
