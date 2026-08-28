# Runbook — Operação do live paper (IBKR) no servidor da casa (umbrelOS)

Desde 2026-08-23 (ADR-012) o live roda **no servidor da casa**: umbrelOS em
`<SERVIDOR_CASA>`, containers Docker, dados em `/data/trader`. A VM Oracle ficou
como fallback desligado (serviços parados, timers desabilitados).

> ### ⚠️ Leia isto primeiro: a hospedagem NÃO sobrevive a um reboot
>
> A raiz (`/`) do umbrelOS é um **overlay rugix**
> (`lowerdir=/run/rugix/mounts/system`, `upperdir=/run/rugix/mounts/data/state/default/overlay/b`)
> que é **resetado no boot**. Tudo que foi instalado em `/` desaparece:
>
> | Some no reboot | Sobrevive |
> |---|---|
> | usuário de serviço `trader` (uid 1001) e seu `authorized_keys` | `/data/trader` (binário, configs, envs, backups) |
> | units e timers `trader-*` em `/etc/systemd/system/` | volume `trader_trader-postgres-data` (banco) |
> | runner self-hosted do GitHub Actions | imagens `trader-bot:latest` e `trader-gateway:10.45` |
> | os containers `trader-*` (removidos, não apenas parados) | `/var/log` e o journal (ficam em `/dev/sda4`) |
>
> Consequência prática: **depois de cada reboot é preciso rodar a recuperação abaixo**,
> senão o bot simplesmente não opera e nada avisa. Verificado no incidente de
> 2026-08-23 (`docs/reports/incidente-2026-08-23-queda-servidor.md`), que custou 4 pregões.

## Pré-requisitos

**Nenhum diário.** Timers systemd do host fazem tudo:

- `trader-start.timer`: seg–sex **9h25 America/New_York** → `trader-containers.sh start` (sobe 11 containers).
- `trader-stop.timer`: seg–sex **16h10 ET** → `trader-containers.sh stop`.
- `trader-backup.timer`: diário **21:30 UTC** → `backup.sh` (pg_dump, retenção 7d).

Gateway (`trader-gateway` container, `restart: always`) e Postgres
(`trader-postgres`, volume nomeado) ficam no ar 24/7.

## Acesso

> **Endereços são marcadores.** Este repositório é **público**, então IPs reais
> não ficam versionados. `<SERVIDOR_CASA>` e `<VM_ORACLE>` estão na memória do
> agente e no `~/.ssh/config` do PC de dev. Segredos (credenciais IBKR do IBC,
> senha do Postgres, envs das instâncias) vivem **apenas** em `/data/trader` no
> servidor e nunca no git — ver `docs/SECURITY.md`.


```bash
ssh -i ~/.ssh/trader_home_deploy trader@<SERVIDOR_CASA>
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

A VM Oracle `<VM_ORACLE>` (chave `~/.ssh/humanbot.key`) está com serviços
parados e timers desabilitados desde 2026-08-22 (cutover). Para reativar como
fallback (só se a casa ficar inoperante por dias): subir `ibgateway.service` +
`trader-instances.target` + enable dos timers — mas **nunca** com o Gateway da
casa ativo ao mesmo tempo (sessão única). Preferir consertar a casa: o banco de
produção vive lá desde o cutover.

## Recuperação pós-reboot

Rodar **toda vez** que o servidor reiniciar. Confirmar antes que o Gateway da
Oracle continua parado (sessão única IBKR).

### 1. O host voltou? Quando e como caiu?

```bash
uptime -s                                    # quando este boot começou
sudo journalctl --list-boots --no-pager      # histórico
sudo journalctl -b -1 -n 25 --no-pager       # como o boot anterior terminou
```

Se o último registro do boot anterior for uma linha comum de serviço (sem
`systemd[1]: Stopping ...`), foi **desligamento sujo** — queda de energia ou
travamento, não reboot planejado.

### 2. Recriar o usuário de serviço

Como o usuário `trader` não existe mais, este passo sai do console do umbrelOS
(ou por SSH como `umbrel`, que sobrevive ao reset por ser usuário da imagem).
A chave pública é a `trader_home_deploy.pub` do PC de dev:

```bash
sudo useradd -m -u 1001 -U -s /bin/bash trader
sudo install -d -m 700 -o trader -g trader /home/trader/.ssh
echo '<conteúdo de ~/.ssh/trader_home_deploy.pub>' | sudo tee /home/trader/.ssh/authorized_keys
sudo chown trader:trader /home/trader/.ssh/authorized_keys
sudo chmod 600 /home/trader/.ssh/authorized_keys
echo 'trader ALL=(ALL) NOPASSWD:ALL' | sudo tee /etc/sudoers.d/trader
sudo chmod 440 /etc/sudoers.d/trader
```

O uid **1001 é obrigatório**: todo o `/data/trader` pertence a ele e os
containers rodam com esse uid. `useradd` avisa que `/home/trader` já existe —
é esperado, o diretório fica em `/dev/sda4` e sobrevive ao reset.

### 3. Subir a stack

```bash
cd /data/trader
sudo docker compose up -d postgres          # primeiro só o banco
sudo docker logs trader-postgres --tail 20  # conferir recovery do WAL
sudo docker compose up -d                   # gateway + 11 instâncias
```

Depois de um corte sujo, o Postgres faz recovery automático no boot
(`database system was not properly shut down; automatic recovery in progress`).
Isso é esperado; o que **não** pode aparecer é erro de página corrompida.

### 4. Reinstalar timers e runner

A fonte da verdade das units é **`deploy/home/systemd/` no repo**. Como o
runner do CI também some no reset, na primeira recuperação elas vão do PC de dev
para o servidor por `scp`:

```bash
# no PC de dev, dentro do repo
scp -i ~/.ssh/trader_home_deploy deploy/home/systemd/trader-*.{service,timer}     trader@<SERVIDOR_CASA>:/tmp/

# no servidor
sudo install -m 644 /tmp/trader-*.{service,timer} /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable --now trader-start.timer trader-stop.timer trader-backup.timer
systemctl list-timers 'trader-*' --no-pager
```

O runner self-hosted do GitHub Actions (`casa-umbrel`) fica em
**`/data/trader/actions-runner`** — o diretório sobrevive ao reset, só o unit
systemd some. Depois de um reboot basta reinstalar o serviço:

```bash
cd /data/trader/actions-runner
sudo ./svc.sh install trader
sudo ./svc.sh start
```

Conferir se ficou `online` em *Settings → Actions → Runners* do repositório
(ou `gh api repos/Drumblow/daytradebot/actions/runners`). Sem runner, o job de
deploy do `push` em `main` fica na fila sem executar — os jobs de teste e build
rodam normalmente na nuvem, então o CI "passa" sem entregar nada no servidor.

Se o registro tiver expirado, gerar novo token e reconfigurar:

```bash
# no PC de dev
gh api -X POST repos/Drumblow/daytradebot/actions/runners/registration-token --jq .token

# no servidor
cd /data/trader/actions-runner
./config.sh --unattended --url https://github.com/Drumblow/daytradebot   --token <TOKEN> --name casa-umbrel --labels home --replace --work _work
```

### 5. Backup imediato

Um corte sujo pode ter interrompido o backup diário. Depois de subir o banco:

```bash
sudo docker exec trader-postgres pg_dump -U trader -p 5433 -d trader_db   | gzip > /data/trader/backups/manual-$(date +%F).sql.gz
```

### 6. Conferir o que foi perdido

```bash
sudo docker exec trader-postgres psql -U trader -d trader_db -p 5433 -c   "SELECT max(timestamp) FROM candles; SELECT max(timestamp) FROM system_events;"
```

Comparar com os pregões do período: cada dia sem candles é um pregão perdido e
precisa de relatório em `docs/reports/` registrando a lacuna (a amostra do
gate B da ADR-010 não pode contar dias em que o bot não rodou).

## Blindagem contra o reset do rugix (PENDENTE)

O procedimento acima é manual e precisa ser repetido a cada reboot — é frágil
justamente no momento em que ninguém está olhando. A proposta ainda **não
implementada**:

1. Guardar em `/data/trader/recovery/` (persistente) o `authorized_keys`, as
   units `trader-*`, o `sudoers.d/trader` e o tarball do runner.
2. Um único script `/data/trader/bin/reprovision.sh` que recria usuário, units,
   timers e runner a partir desse diretório — idempotente.
3. Disparar esse script no boot por um mecanismo que sobreviva ao reset. Um unit
   systemd em `/etc/systemd/system/` **não serve** (é exatamente o que some).
   Alternativas a avaliar: app do umbrelOS que rode o script, `@reboot` de um
   crontab persistido, ou hook do próprio umbrelOS.
4. Enquanto isso não existir, **conferir o servidor manualmente após qualquer
   queda de energia** — não há alerta automático (o webhook de alertas segue
   sem URL configurada, ver `docs/HANDOFF.md` §5).

Recomendação de hardware que sai deste incidente: habilitar na BIOS o retorno
automático após falta de energia (*Restore on AC Power Loss = Power On*) e
colocar o servidor em nobreak. Sem isso a máquina fica desligada até alguém
notar — foi o que custou 4 pregões em 08-23/08-27.
