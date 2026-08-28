# Runbook — cutover para o app do umbrelOS

Migração do stack de produção (compose solto em `/data/trader` + timers systemd)
para o app `daytradebot` do umbrelOS. Contexto e justificativa em
[ADR-013](../decisions/ADR-013-app-umbrelos.md).

> **Quando executar:** fora do pregão, de preferência sábado. O passo 8 é um
> reboot de verdade e derruba tudo por alguns minutos.
>
> **Endereços são marcadores.** Este repositório é público: `<SERVIDOR_CASA>`
> está no `~/.ssh/config` do PC de dev, não aqui.

---

## Antes de começar

| Pré-requisito | Como conferir |
|---|---|
| Pacotes do GHCR públicos | `docker pull ghcr.io/drumblow/trader-bot:latest` **de fora do host** |
| Pregão fechado e instâncias paradas | `sudo docker ps --filter name=trader- \| wc -l` → só gateway e postgres |
| Store adicionada no umbrelOS | app "Day Trade Bot" aparece na dashboard |
| Senha do usuário `umbrel` à mão | necessária **depois do reboot** — ver passo 9 |

> Os pacotes do GHCR nascem **privados** e o GitHub não expõe API para mudar
> isso. É na interface: github.com/users/Drumblow/packages → cada pacote →
> *Package settings* → *Change visibility* → Public. Sem isso o passo 4 falha
> com `denied`.

---

## 1. Backup, antes de tocar em qualquer coisa

```bash
sudo docker exec trader-postgres pg_dump -U trader -p 5433 -d trader_db \
  | gzip > /data/trader/backups/pre-cutover-$(date +%Y%m%d-%H%M).sql.gz
ls -lh /data/trader/backups/pre-cutover-*.sql.gz
```

O dump de produção tem ~6,6 MB. **Um arquivo muito menor que isso significa que
o `pg_dump` falhou no meio** — o gzip termina com sucesso mesmo assim. Não siga
adiante sem conferir o tamanho.

Confira também o que está sendo levado:

```bash
sudo docker exec trader-postgres psql -U trader -p 5433 -d trader_db -tAc \
  "select 'signals', count(*) from signals
   union all select 'orders', count(*) from orders
   union all select 'fills', count(*) from fills
   union all select 'trades', count(*) from trades
   union all select 'candles', count(*) from candles"
```

Anote os números. Eles são a prova no passo 6.

## 2. Parar o stack antigo

```bash
sudo systemctl disable --now trader-start.timer trader-stop.timer trader-backup.timer
sudo docker compose -f /data/trader/docker-compose.yml down
sudo docker ps --filter name=trader-        # deve sair vazio
```

O volume `trader_trader-postgres-data` **não** é removido por `down` sem `-v`.
Ele é a rede de segurança até o passo 10 — não apague.

Libera as portas 5433 e 4002, que o app vai querer.

## 3. Instalar o app

```bash
sudo umbreld client apps.install.mutate --appId daytradebot
```

O hook `pre-install` cria a estrutura em
`/home/umbrel/umbrel/app-data/daytradebot` e avisa o que falta. O gateway vai
entrar em ciclo de restart reclamando de credencial — **é o comportamento
esperado** até o passo 4.

## 4. Levar os dados que não vêm na imagem

```bash
APP=/home/umbrel/umbrel/app-data/daytradebot

# instalação do IB Gateway 10.45 (~240 MB) — a imagem pública não redistribui
sudo cp -a /data/trader/gateway/ibgateway/1045 "$APP/gateway/ibgateway/"

# settings do Gateway (jts.ini com TrustedIPs=127.0.0.1)
sudo cp -a /data/trader/gateway/gateway-settings/. "$APP/gateway/gateway-settings/"

sudo chown -R 1001:1001 "$APP/gateway"
```

Credenciais IBKR — extraídas do `ibc.ini` atual **sem passar por nenhuma tela**:

```bash
sudo bash -c 'umask 077; APP=/home/umbrel/umbrel/app-data/daytradebot; \
  { printf "TWSUSERID=%s\n" "$(sed -n "s/^IbLoginId=//p" /data/trader/gateway/ibc/ibc.ini)"; \
    printf "TWSPASSWORD=%s\n" "$(sed -n "s/^IbPassword=//p" /data/trader/gateway/ibc/ibc.ini)"; \
  } > "$APP/secrets/ibkr.env"'

# confere que preencheu, sem imprimir valor
sudo awk -F= '{printf "%s: %d chars\n", $1, length($2)}' \
  /home/umbrel/umbrel/app-data/daytradebot/secrets/ibkr.env
```

Duas linhas com mais de zero caracteres = ok.

## 5. Restaurar o banco

O app já criou um Postgres vazio com senha derivada do `APP_SEED`.

```bash
gunzip -c /data/trader/backups/pre-cutover-*.sql.gz \
  | sudo docker exec -i daytradebot_postgres_1 psql -U trader -p 5433 -d trader_db
```

Erros de `role already exists` são normais. Erros de `relation ... does not
exist` **não são** — pare e investigue.

## 6. Validar antes de confiar

```bash
sudo umbreld client apps.restart.mutate --appId daytradebot
```

**Contagens iguais às do passo 1:**

```bash
sudo docker exec daytradebot_postgres_1 psql -U trader -p 5433 -d trader_db -tAc \
  "select 'signals', count(*) from signals
   union all select 'orders', count(*) from orders
   union all select 'fills', count(*) from fills
   union all select 'trades', count(*) from trades
   union all select 'candles', count(*) from candles"
```

**Gateway conectado na IBKR:**

```bash
sudo docker logs daytradebot_gateway_1 --tail 40
sudo ss -ltn | grep 4002        # deve estar escutando
```

**Conectividade ponta a ponta** (client_id 99 = diagnostico; **nunca** 1-11):

A senha do banco e o `APP_SEED`, que o umbreld deriva e nao guarda em arquivo
nenhum. Em vez de tentar descobri-la, herde o ambiente de uma instancia ja
criada — assim a senha nunca aparece na tela:

```bash
# herda o ambiente (inclusive DATABASE_URL) de uma instancia ja criada
ENVS=$(sudo docker inspect -f '{{range .Config.Env}}-e {{.}} {{end}}' daytradebot_iwm-pullback_1)

sudo docker run --rm --network host --entrypoint /opt/trader/bin/trader-cli -w /opt/trader $ENVS -e TRADER__IBKR__CLIENT_ID=99 ghcr.io/drumblow/trader-bot:latest status
```

O `-e TRADER__IBKR__CLIENT_ID=99` vem **depois** de proposito: sobrescreve o
client_id 1 herdado da instancia. Usar 1-11 aqui brigaria com um bot real.

**Scheduler com a agenda certa:**

```bash
sudo docker logs daytradebot_scheduler_1 --tail 20
```

Deve listar três entradas de cron em horário de Nova York e o número de
instâncias sob controle (11).

## 7. Instâncias sobem no horário?

Se estiver fora do pregão, force uma vez:

```bash
sudo docker exec daytradebot_scheduler_1 /usr/local/bin/scheduler.sh start-instances
```

Vai subir e as instâncias vão **sair sozinhas** pela guarda de janela — o log do
scheduler mostra `0/11 rodando`. Isso é o resultado correto fora do pregão.
Para ver as 11 realmente operando, só no pregão seguinte.

## 8. O reboot — a validação que dá sentido à migração

```bash
sudo reboot
```

Espere ~3 minutos. Depois:

```bash
ssh umbrel@<SERVIDOR_CASA>
sudo docker ps --filter name=daytradebot --format '{{.Names}}\t{{.Status}}'
```

**Critério de sucesso:** `daytradebot_postgres_1`, `daytradebot_gateway_1` e
`daytradebot_scheduler_1` de pé, **sem ninguém ter feito nada**. É exatamente o
que não acontecia antes: no incidente de 2026-08-23 o servidor voltou e o bot
ficou 4 pregões parado.

Se as instâncias estiverem paradas e for fora do pregão, isso é o esperado.

## 9. Acesso administrativo depois do reboot

O usuário `trader` e o `sudoers.d` dele são apagados no boot. Depois do reboot
você entra como `umbrel`, mas **o sudo pede senha**. Para devolver acesso sem
senha ao agente/deploy, rode você mesmo, uma vez, na sessão:

```bash
echo 'umbrel ALL=(ALL) NOPASSWD:ALL' | sudo tee /etc/sudoers.d/umbrel-nopasswd
sudo chmod 440 /etc/sudoers.d/umbrel-nopasswd
```

> Isso **também some no próximo boot**. É consciente: a solução permanente é o
> container `runner` da fase 3, que vive dentro do app e sobrevive junto com ele.
> Até lá, este passo se repete a cada reboot.

## 10. Limpeza — só depois do passo 8 ter passado

Espere **7 dias** de operação normal antes disto.

```bash
sudo rm -f /etc/systemd/system/trader-{start,stop,backup}.{timer,service}
sudo systemctl daemon-reload
sudo docker volume rm trader_trader-postgres-data
sudo rm -rf /data/trader
sudo umbreld client apps.uninstall.mutate --appId daytradebot-probe
```

Atualizar depois: `HANDOFF.md` §1 e §4, `live-operations.md` (remover a seção
"Blindagem contra o reset do rugix", que deixa de existir como pendência) e
`deploy/README.md`.

---

## Rollback

Enquanto o passo 10 não rodar, o caminho de volta é curto:

```bash
sudo umbreld client apps.stop.mutate --appId daytradebot
sudo systemctl enable --now trader-start.timer trader-stop.timer trader-backup.timer
sudo docker compose -f /data/trader/docker-compose.yml up -d --no-recreate
```

O volume antigo e `/data/trader` estão intactos, então o stack antigo volta com
o banco no ponto em que parou no passo 2 — perdendo apenas o que o app tiver
gravado depois. Se o cutover tiver rodado por um pregão inteiro, prefira
restaurar o dump do app no banco antigo em vez de perder o dia.

## O que este runbook não resolve

Nenhum alerta automático continua existindo. Se o app não voltar de um reboot, a
descoberta segue sendo alguém olhando — o `webhook_url` continua sem
configuração (`HANDOFF.md` §5). A migração remove a causa mais provável de queda
silenciosa; ela não cria vigilância.
