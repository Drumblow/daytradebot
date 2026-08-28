# deploy/ — infraestrutura do servidor de produção (versionada)

Desde 2026-08-23 (ADR-012) o live roda no **servidor da casa** (umbrelOS,
`<SERVIDOR_CASA>`) em containers Docker com dados em `/data/trader`. Estes arquivos
são a **fonte da verdade** da infraestrutura — o deploy do GitHub Actions instala
tudo daqui (idempotente, a cada push que toque `deploy/**`).

## `deploy/home/` — host umbrelOS da casa

| Arquivo | Destino no host | Função |
|---------|-----------------|--------|
| `docker-compose.yml` | `/data/trader/docker-compose.yml` | postgres + gateway + 11 instâncias (network host; volumes bind de bin/config) |
| `gateway/Dockerfile` | contexto de build `/data/trader/gateway/` | imagem do IB Gateway 10.45: Debian 13 + ZuluFX JRE 17 + IBC 3.24.1 + Xvfb |
| `gateway/gateway-entry.sh` | idem (COPY na imagem) | entry que controla Xvfb (xvfb-run como PID 1 travaria) |
| `bot/Dockerfile` | contexto de build `/data/trader/bot/` | imagem do trader-cli (binário montado por volume, não embedido) |
| `bot/entrypoint.sh` | idem (COPY na imagem) | lê SYMBOL/STRATEGY do env e executa o trader-cli |
| `systemd/trader-start.{timer,service}` | `/etc/systemd/system/` | sobe as instâncias seg–sex 9h25 ET (via `trader-containers.sh start`) |
| `systemd/trader-stop.{timer,service}` | `/etc/systemd/system/` | para as instâncias seg–sex 16h10 ET |
| `systemd/trader-backup.{timer,service}` | `/etc/systemd/system/` | backup diário 21:30 UTC (`backup.sh`) |
| `trader-containers.sh` | `/data/trader/bin/` | start/stop dos 11 containers |
| `backup.sh` | `/data/trader/bin/` | pg_dump diário + retenção 7 dias |

**Arquitetura do container do bot:** a imagem `trader-bot` é estável (Debian slim +
entrypoint); o **binário e as configs são montados por volume** de
`/data/trader/{bin,config}` — o deploy troca os arquivos e reinicia os containers
RUNNING, sem rebuild de imagem.

**Não** estão no repo (seguem só no host, sensíveis): `/data/trader/env/*.env`
(credenciais/database_url), `ibc.ini` (login IBKR), `.env` do Postgres.

## Histórico: `deploy/systemd` e `deploy/scripts` (VM Oracle — desativada)

Unidades `trader@.service`/`target`/timers e `backup.sh` da VM Oracle (ADR-011),
desativada no cutover de 2026-08-22. Mantidas por referência; o pipeline atual
instala apenas `deploy/home/`.