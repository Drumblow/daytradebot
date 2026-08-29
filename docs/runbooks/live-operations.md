# Runbook — operação do paper live

Operação diária do bot no servidor da casa. Desde **2026-08-28** ele roda como um
**app do umbrelOS** (ADR-013), não mais como compose solto com timers systemd.

> ### O que mudou, e por que importa
>
> A raiz do umbrelOS é um overlay rugix **resetado a cada boot**. Até 08-28 isso
> significava que todo reboot apagava os timers `trader-*`, o usuário `trader` e o
> runner do CI — e o bot **não voltava sozinho**. Custou 4 pregões no incidente de
> 2026-08-23.
>
> Como app, o estado vive em `/home/umbrel/umbrel` (persistente) e o umbreld sobe
> tudo no boot. Validado com reboot real em 2026-08-28: containers de pé aos 91s,
> Gateway logado na IBKR aos 144s, **sem intervenção**.
>
> **O que ainda não volta sozinho:** o usuário `trader`, o `sudoers.d` e o acesso
> SSH administrativo. O app sobrevive; o acesso humano, não. Ver "Acesso" abaixo.

## Pré-requisitos

**Nenhum diário.** O container `daytradebot_scheduler_1` faz tudo, em horário de
Nova York (o horário de verão americano é resolvido pelo tzdata):

| Quando | O quê |
|---|---|
| seg–sex **9h25 ET** | sobe as 11 instâncias |
| seg–sex **16h10 ET** | para as 11 instâncias |
| diário **16h30 ET** | `pg_dump` + retenção de 7 dias |

Se o app subir *dentro* do pregão (servidor religou às 10h de uma terça), o
scheduler liga as instâncias na hora, sem esperar o próximo 9h25.

Cada instância também tem uma **guarda de janela** no entrypoint: fora do horário
ela sai com `exit 0` e fica parada. É o que impede 11 conexões na IBKR quando o
servidor religa de madrugada.

Gateway e Postgres ficam no ar 24/7.

## Acesso

> **Endereços são marcadores.** Este repositório é **público**, então IPs reais
> não ficam versionados. `<SERVIDOR_CASA>` e `<VM_ORACLE>` estão no `~/.ssh/config`
> do PC de dev. Segredos vivem **apenas** no servidor e nunca no git — ver
> `docs/SECURITY.md`.

```bash
ssh -i ~/.ssh/trader_home_deploy umbrel@<SERVIDOR_CASA>
```

**Depois de um reboot o `sudo` pede senha** — o `sudoers.d` some no boot. Duas
saídas:

1. **Sem senha:** `gh workflow run host-check.yml` — inspeciona containers,
   Gateway e banco pelo runner que vive dentro do app.
2. **Com senha, se precisar de sudo na sessão:**

```bash
echo 'umbrel ALL=(ALL) NOPASSWD:ALL' | sudo tee /etc/sudoers.d/umbrel-nopasswd
```

Isso também some no próximo boot. É consciente: a solução permanente é usar o
runner do app, que sobrevive.

Onde ficam as coisas (`APP` = `/home/umbrel/umbrel/app-data/daytradebot`):

| Item | Caminho |
|------|---------|
| Compose do app (fonte: repo da store) | `$APP/docker-compose.yml` |
| Dados do Postgres | `$APP/data/postgres` |
| Credenciais IBKR (`TWSUSERID`/`TWSPASSWORD`) | `$APP/secrets/ibkr.env` (600, uid 1001) |
| Instalação do IB Gateway 10.45 | `$APP/gateway/ibgateway/1045` |
| `jts.ini` (preferências do Gateway) | `$APP/gateway/jts.ini` |
| Logs do IBC | `$APP/logs/ibc/` |
| Backups | `$APP/backups/` |
| Config do runner do CI | `$APP/runner/` |

O binário e as configs de estratégia **não estão mais no host**: vão assados na
imagem `ghcr.io/drumblow/trader-bot`.

## Monitorar

```bash
sudo docker ps -a --filter name=daytradebot_
sudo docker logs daytradebot_iwm-pullback_1 --tail 50
sudo docker logs daytradebot_gateway_1 --tail 30
sudo docker logs daytradebot_scheduler_1 --tail 30
sudo docker exec daytradebot_postgres_1 psql -U trader -d trader_db -p 5433 -c "SELECT ..."
```

CLI de diagnóstico — client_id **99**, nunca 1–11. Herdar o ambiente de uma
instância evita ter que descobrir a senha do banco, que é derivada do `APP_SEED`
do umbrelOS e não existe em arquivo nenhum:

```bash
ENVS=$(sudo docker inspect -f '{{range .Config.Env}}-e {{.}} {{end}}' daytradebot_iwm-pullback_1)
```

```bash
sudo docker run --rm --network host --entrypoint /opt/trader/bin/trader-cli -w /opt/trader $ENVS -e TRADER__IBKR__CLIENT_ID=99 ghcr.io/drumblow/trader-bot:latest status
```

## Parar / retomar manualmente

```bash
sudo docker exec daytradebot_scheduler_1 /usr/local/bin/scheduler.sh stop-instances
```

```bash
sudo docker exec daytradebot_scheduler_1 /usr/local/bin/scheduler.sh start-instances
```

Parar o bot **não desprotege posições abertas** — stop e alvo ficam server-side na
IBKR. Para impedir a abertura do dia seguinte, pare o app inteiro:

```bash
sudo umbreld client apps.stop.mutate --appId daytradebot
```

## Restart no meio do pregão

Seguro — o bot reconstrói limites diários do banco e dedupra fills por
`broker_fill_id`.

```bash
sudo docker restart daytradebot_iwm-pullback_1
```

## Deploy

`push` em `main` → `images.yml` compila e publica em `ghcr.io/drumblow/*` → o job
`deploy` roda no runner do app e recria as 11 instâncias com `--no-deps`, deixando
Postgres e Gateway de pé. Fora do pregão elas sobem com a imagem nova e saem
sozinhas pela guarda, prontas para as 9h25.

O job só roda com a variável `APP_DEPLOY=enabled` no repositório.

## Circuit breaker

Se uma instância encerrar com `circuit breaker: ...`:

1. `sudo docker logs daytradebot_<instancia>_1 --tail 200`
2. Gateway: `sudo docker logs daytradebot_gateway_1 --tail 100` + log do IBC
3. `system_events` no banco para histórico
4. Corrigir a causa e `sudo docker restart daytradebot_<instancia>_1`

## Backup do banco

O scheduler roda diariamente às 16h30 ET. Ele **confere o tamanho do arquivo**: um
`pg_dump` que falha no meio ainda produz um `.gz` válido e pequeno, então um backup
de menos de 10 KB é registrado como erro em vez de passar por bom.

Manual:

```bash
sudo docker exec daytradebot_postgres_1 pg_dump -U trader -p 5433 -d trader_db | gzip > /home/umbrel/umbrel/app-data/daytradebot/backups/manual-$(date +%F).sql.gz
```

## ⚠️ Sessão única IBKR

A IBKR aceita **uma sessão por usuário**. **Não abrir TWS/Gateway local (PC de
trabalho ou outro host) com o usuário do bot** — derruba a sessão da casa e mata as
11 instâncias. Para consultar a conta, usar outro usuário ou o portal web.

## Depois de uma queda de energia

O app volta sozinho. O que ainda precisa de olho:

```bash
gh workflow run host-check.yml
```

Depois, conferir o que foi perdido enquanto a máquina esteve desligada:

```bash
sudo docker exec daytradebot_postgres_1 psql -U trader -d trader_db -p 5433 -c "SELECT max(timestamp) FROM candles; SELECT max(timestamp) FROM system_events;"
```

Cada dia sem candles é um pregão perdido e precisa de relatório em `docs/reports/`
registrando a lacuna — a amostra do gate B da ADR-010 não pode contar dias em que o
bot não rodou.

> **A máquina ainda não liga sozinha.** O app se recupera depois que o servidor
> liga; ele não liga o servidor. Enquanto a BIOS não estiver em *Restore on AC Power
> Loss = Power On* (e sem nobreak), uma queda de energia continua deixando tudo
> parado até alguém apertar o botão — foi o que custou 4 pregões em 08-23/08-27.

> **Não existe alerta.** Se algo cair, a descoberta continua sendo alguém olhando:
> o `webhook_url` segue sem configuração (`docs/HANDOFF.md` §5).

## Fallback Oracle (desligado)

A VM Oracle `<VM_ORACLE>` (chave `~/.ssh/humanbot.key`) está com serviços parados e
timers desabilitados desde 2026-08-22. Para reativar como fallback — só se a casa
ficar inoperante por dias — subir `ibgateway.service` + `trader-instances.target` +
enable dos timers, mas **nunca** com o Gateway da casa ativo ao mesmo tempo (sessão
única). Preferir consertar a casa: o banco de produção vive lá.

## Rollback do app

Enquanto `/data/trader` e o volume `trader_trader-postgres-data` existirem (até
**2026-09-04**), o caminho de volta está em
`docs/runbooks/cutover-app-umbrelos.md` → "Rollback".
