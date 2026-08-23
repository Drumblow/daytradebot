# Runbook — Operação do live paper (IBKR) no servidor da casa (umbrelOS)

Desde 2026-08-23 (ADR-012) o live roda **no servidor da casa**: umbrelOS em
`192.168.50.68`, containers Docker, dados em `/data/trader`. A VM Oracle ficou
como fallback desligado (serviços parados, timers desabilitados).

## Pré-requisitos

**Nenhum diário.** Timers systemd do host fazem tudo:

- `trader-start.timer`: seg–sex **9h25 America/New_York** → `trader-containers.sh start` (sobe 11 containers).
- `trader-stop.timer`: seg–sex **16h10 ET** → `trader-containers.sh stop`.
- `trader-backup.timer`: diário **21:30 UTC** → `backup.sh` (pg_dump, retenção 7d).

Gateway (`trader-gateway` container, `restart: always`) e Postgres
(`trader-postgres`, volume nomeado) ficam no ar 24/7.

## Acesso

```bash
ssh -i ~/.ssh/trader_home_deploy trader@192.168.50.68
# sudo sem senha (precisa p/ docker e systemctl)
sudo docker ps
```

Onde ficam as coisas no host:

| Item | Caminho |
|------|---------|
| Compose (fonte da verdade: `deploy/home/docker-compose.yml`) | `/data/trader/docker-compose.yml` |
| Binário do bot | `/data/trader/bin/trader-cli` (volume bind dos containers) |
| Configs de estratégia | `/data/trader/config/strategies/` |
| Envs das 11 instâncias | `/data/trader/env/instances/*.env` (client_ids 1–11; broker id+100; poll id+200) |
| Env compartilhado | `/data/trader/env/trader.env` |
| IBC / credenciais do Gateway | `/data/trader/gateway/ibc/` (ibc.ini, permissão 600, dentro da imagem e no contexto de build) |
| Logs do IBC | `/data/trader/logs/ibc/` |
| Backups do banco | `/data/trader/backups` |
| Scripts | `/data/trader/bin/{backup.sh,trader-containers.sh}` |
| Units systemd / timers | versionados em `deploy/home/systemd/`, instalados pelo CI |

Credenciais do Postgres: `.env` de deploy em `/data/trader/docker/.env`
(mesmo `POSTGRES_PASSWORD` da migração — o `DATABASE_URL` do bot não mudou).

## Monitorar

```bash
# estado dos containers
sudo docker ps --filter name=trader-

# log de uma instância
sudo docker logs trader-iwm-pullback --tail 50

# log do Gateway (IBC)
sudo docker logs trader-gateway --tail 30
sudo bash -c "tail -50 /data/trader/logs/ibc/ibc-3.24.1_GATEWAY-1045_*.txt"

# banco (tabelas: signals, orders, fills, trades, system_events)
sudo docker exec trader-postgres psql -U trader -d trader_db -p 5433 -c "SELECT ..."

# CLI — status/conectividade (client_id 99 = diagnóstico; NUNCA 1–11)
sudo docker run --rm --network host \
  --entrypoint /opt/trader/bin/trader-cli -w /opt/trader \
  -e DATABASE_URL="postgres://trader:$(grep -oP '(?<=POSTGRES_PASSWORD=).*' /data/trader/docker/.env)@127.0.0.1:5433/trader_db" \
  -e TRADER__IBKR__HOST=127.0.0.1 -e TRADER__IBKR__PORT=4002 -e TRADER__IBKR__CLIENT_ID=99 \
  -v /data/trader/bin:/opt/trader/bin:ro -v /data/trader/config:/opt/trader/config:ro \
  trader-bot:latest status
```

## Parar / retomar manualmente

```bash
sudo /data/trader/bin/trader-containers.sh stop   # 11 instâncias (gateway/postgres continuam)
sudo /data/trader/bin/trader-containers.sh start  # sobe as 11
```

Parar o bot **não desprotege posições abertas** (stop e alvo ficam server-side na
IBKR). Para impedir o timer do dia seguinte: `sudo systemctl stop trader-start.timer`
(e `start` para religar).

## Restart no meio do pregão

Seguro — o bot reconstrói limites diários do banco e dedupra fills por `broker_fill_id`.
O deploy via CI (`push` em `main`) já faz restart condicional: só containers RUNNING.

```bash
sudo docker restart trader-iwm-pullback
```

## Circuit breaker

Se uma instância encerrar com `circuit breaker: ...`:
1. `sudo docker logs trader-<instancia> --tail 200`.
2. Gateway: `sudo docker logs trader-gateway --tail 100` + log IBC.
3. `system_events` no banco para histórico.
4. Corrigir a causa e `sudo docker restart trader-<instancia>`.

## Backup do banco

Timer `trader-backup.timer` (21:30 UTC) roda `/data/trader/bin/backup.sh`
(`docker exec trader-postgres pg_dump` → gzip → retenção 7d em `/data/trader/backups`).
Manual:

```bash
sudo docker exec trader-postgres pg_dump -U trader -p 5433 -d trader_db | gzip > /data/trader/backups/manual-$(date +%F).sql.gz
```

## ⚠️ Sessão única IBKR

A IBKR aceita **uma sessão por usuário**. O Gateway agora vive no container da casa;
**não abrir TWS/Gateway local (PC de trabalho ou outro host) com o usuário do bot**
— derruba a sessão da casa e mata as 11 instâncias. Para consultar a conta, usar
outro usuário ou o portal web da IBKR.

## Fallback Oracle (desligado)

A VM Oracle `137.131.186.91` (chave `~/.ssh/humanbot.key`) está com serviços
parados e timers desabilitados desde 2026-08-22 (cutover). Para reativar como
fallback (só se a casa ficar inoperante por dias): subir `ibgateway.service` +
`trader-instances.target` + enable dos timers — mas **nunca** com o Gateway da
casa ativo ao mesmo tempo (sessão única). Preferir consertar a casa: o banco de
produção vive lá desde o cutover.