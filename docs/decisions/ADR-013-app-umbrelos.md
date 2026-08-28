# ADR-013 — Empacotar o bot como app do umbrelOS

**Status:** proposto (planejamento) — 2026-08-28
**Contexto anterior:** ADR-011 (VM Oracle, desativada), ADR-012 (servidor da casa)
**Incidente que motiva:** `docs/reports/incidente-2026-08-23-queda-servidor.md`

---

## 1. Problema

O ADR-012 trouxe o live para o servidor de casa, mas colocou a infraestrutura em
lugares que **o umbrelOS apaga a cada boot**. A raiz (`/`) é um overlay rugix
resetado no boot; só sobrevivem `/data`, `/home`, `/kopia`, `/var/lib/docker`,
`/var/lib/systemd/timesync` e `/var/log`.

Consequência medida no incidente de 2026-08-23: uma queda de energia levou junto
os timers `trader-*`, o usuário `trader`, o runner do CI e os próprios containers.
O bot ficou **4 pregões fora do ar sem nenhum alerta**, e a recuperação é um
procedimento manual de 6 passos que alguém precisa lembrar de executar.

O que existe hoje e some no boot:

| Some | Onde está |
|---|---|
| units/timers `trader-start`, `trader-stop`, `trader-backup` | `/etc/systemd/system/` |
| usuário `trader` (uid 1001) + `sudoers.d` + `authorized_keys` | `/etc/`, `/home/trader` |
| runner self-hosted do GitHub Actions | `/` |
| os 11 containers `trader-*` (removidos, não parados) | estado do Docker |

## 2. Decisão

Empacotar todo o serviço como um **app do umbrelOS**, publicado numa app store
comunitária própria (repositório público `Drumblow/umbrel-daytradebot-store`).

O motivo é direto: o estado do umbreld vive em `/home/umbrel/umbrel`, que está no
`/dev/sda4` **persistente**. Um app instalado é registrado em `umbrel.yaml`, tem
seus arquivos em `app-data/<id>/`, e o umbreld **sobe os apps sozinho no boot**
(`apps.ts`, com `autoStart` por app). Ou seja: a plataforma já resolve exatamente
o problema que o ADR-012 deixou em aberto, e resolve sem nenhum script de
`reprovision` nosso.

Isso substitui a "Blindagem contra o reset do rugix (PENDENTE)" descrita em
`docs/runbooks/live-operations.md` — que era um `reprovision.sh` disparado por um
mecanismo de boot que a gente ainda não tinha.

## 3. Como o umbreld 1.7.4 executa um app (verificado no host)

Tudo abaixo foi lido em `/opt/umbreld/source/modules/apps/` no servidor, não é
suposição. Referências de arquivo entre parênteses.

- **Project name = id do app.** `docker compose --project-name "${app}"`
  (`legacy-compat/app-script:358`). Os containers são forçados ao esquema antigo
  `${app}_${servico}_1` (`app.ts:114`), como se vê em `immich_server_1`.
- **Compose merge, nesta ordem:** `app_proxy` (só se o serviço `app_proxy` estiver
  declarado — `app-script:335`), Tor (se habilitado), `docker-compose.common.yml`
  e por último o compose do app, que **pode sobrescrever tudo**.
  `common.yml` só declara a rede externa `umbrel_main_network`.
- **`up --detach --build`** (`app-script:485`) — ou seja, `build:` no compose do
  app **funciona**, a imagem é construída no host.
- **Variáveis disponíveis** (`app-script:200-215`): `APP_ID`, `APP_DATA_DIR`,
  `APP_VERSION`, `APP_PROXY_HOSTNAME`, `APP_PROXY_PORT`, `APP_DOMAIN`,
  `DEVICE_DOMAIN_NAME` e — importante para segredos — `APP_SEED` e `APP_PASSWORD`,
  derivados deterministicamente da entropia da máquina (`derive_entropy`).
- **Hooks:** `pre-install`, `post-install`, `pre-start`, `post-start`, `pre-stop`,
  `post-stop`, `pre-update`, `post-update` (`app-script:482-758`).
- **Templates:** arquivos `*.template` em `app-data` são renderizados a cada start
  (`template_app`), antes do `pre-start`.
- **App store comunitária:** o repositório precisa de um `umbrel-app-store.yml`
  com `id` e `name`, e **todo app precisa ter `id` começando com o `id` da store**
  — apps que não obedecem são silenciosamente filtrados
  (`app-repository.ts:175`). Store `daytradebot` + app `daytradebot` satisfaz.
- **Manifesto** (`schema.ts`): obrigatórios `manifestVersion`, `id`, `name`,
  `tagline`, `category`, `version`, `port` (int), `description`, `website`,
  `support`, `gallery`. Opcionais úteis: `backupIgnore`, `dependencies`,
  `releaseNotes`, `permissions`.

## 4. Arquitetura do app

App id: **`daytradebot`** · store id: **`daytradebot`** ·
`APP_DATA_DIR` = `/home/umbrel/umbrel/app-data/daytradebot`

Serviços no compose do app (14 containers):

| Serviço | Container | Papel |
|---|---|---|
| `postgres` | `daytradebot_postgres_1` | banco, 24/7 |
| `gateway` | `daytradebot_gateway_1` | IB Gateway 10.45 + IBC, 24/7 |
| `scheduler` | `daytradebot_scheduler_1` | janela de pregão (substitui os timers systemd) |
| `runner` | `daytradebot_runner_1` | runner self-hosted do GitHub Actions |
| 11 × instância | `daytradebot_<simbolo>-<estrategia>_1` | os bots |

### 4.1 Rede — manter `network_mode: host` na fase 1

O compose do app é mesclado por último e sobrescreve a rede, então
`network_mode: host` continua funcionando dentro de um app.

**Por que não migrar para a bridge agora:** o `jts.ini` do Gateway tem
`TrustedIPs=127.0.0.1` (verificado no host). Na `umbrel_main_network` os bots
chegariam de `10.21.x.x` e o Gateway recusaria a conexão da API. Migrar a rede
exige mexer na config do Gateway e revalidar as 11 conexões — é uma segunda
mudança de risco empilhada em cima da primeira.

A fase 1 mantém a rede **idêntica ao que roda hoje**: Postgres em
`127.0.0.1:5433`, Gateway em `127.0.0.1:4002`. A migração para bridge (que fecha
essas portas do host e é mais correta) fica como ADR posterior.

### 4.2 Janela de pregão sem systemd

Os timers `trader-start`/`trader-stop` somem no boot; a lógica precisa vir para
dentro do app. Duas peças:

1. **Guard no entrypoint do bot.** O entrypoint passa a sair com `exit 0` se
   estiver fora da janela 9h25–16h10 ET. Com `restart: on-failure`, um bot que
   quebra reinicia, mas um bot que sai limpo (fora do horário) fica parado.
   Isso resolve o caso "servidor religou às 3h da manhã": o `up` do umbreld cria
   os 11 containers, eles saem em segundos e ninguém conecta na IBKR fora de hora.
2. **Container `scheduler`.** `crond` com `docker.sock` montado: `docker start`
   nas 11 instâncias às 9h25 ET, `docker stop` às 16h10 ET, `pg_dump` às 16h30 ET.
   Substitui os três timers de uma vez. Toda a agenda em horário de Nova York —
   o backup saiu de 21h30 UTC para 16h30 ET justamente para que uma única
   timezone governe tudo e o horário de verão seja resolvido pelo tzdata.
   Se o app subir *dentro* da janela (servidor religou às 10h de uma terça), o
   scheduler liga as instâncias na hora, sem esperar o próximo 9h25.

### 4.3 Entrega do binário

Hoje o binário é um arquivo em `/data/trader/bin` trocado pelo CI e montado por
volume. Para um app de store isso não funciona numa instalação limpa.

**Decisão:** o binário e as configs passam a ser **assados na imagem**,
publicada em `ghcr.io/drumblow/trader-bot`. O compose fixa a tag por digest. O
deploy vira: CI compila → publica imagem → runner no host puxa e recria os
containers. Some o truque do "swap atômico de inode", que é frágil.

Custo: o deploy fica alguns minutos mais lento. Ganho: uma instalação a partir da
store funciona sem CI, e a versão que roda é rastreável por digest.

## 5. Segredos — o app é público

Restrição do usuário: *"precisamos nos certificar que nenhum dado sensível vai
estar presente pois usamos login pessoal na conta IBKR"*.

Hoje o `ibc.ini` **com usuário e senha da IBKR entra na imagem por `COPY`**
(`deploy/home/gateway/Dockerfile`). Numa imagem publicada no GHCR isso vazaria as
credenciais para qualquer pessoa que desse `docker pull`. É o ponto mais crítico
da migração.

| Segredo | Hoje | No app |
|---|---|---|
| Login IBKR | `COPY` na imagem | `TWSUSERID`/`TWSPASSWORD` lidos em runtime de `${APP_DATA_DIR}/secrets/ibkr.env`; o `ibc.ini` da imagem vai com os campos vazios e **nunca** é escrito com credencial |
| Senha do Postgres | `.env` no host | derivada de `${APP_SEED}` — nunca versionada, nunca digitada |
| `DATABASE_URL` das 11 instâncias | `.env` por instância | montado do env do compose, derivado do `APP_SEED` |
| IP interno do servidor | — | não aparece; a store não cita endereço |
| Instalador do IB Gateway 10.45 | `COPY` de `ibgateway/` | baixado no `build` a partir da IBKR (não redistribuímos binário deles) |

O `post-install` cria `${APP_DATA_DIR}/secrets/ibkr.env` com placeholders vazios e
permissão `600`. O usuário preenche uma vez (via SSH ou o app Files) e reinicia o
app. O `pre-start` falha com mensagem clara se o arquivo ainda estiver vazio —
melhor que subir e falhar no login.

Checklist de publicação, a rodar **antes** de tornar o repositório público:
`git log -p` do repo da store não pode conter usuário IBKR, senha, `DUR507388`,
`192.168.*`, nem qualquer `.env` preenchido.

## 6. Migração dos dados

O banco atual está no volume `trader_trader-postgres-data`; `/data/trader` tem
3,4 GB (a maior parte é o instalador do Gateway e logs do IBC).

Não usar `external: true` apontando para o volume antigo: prende o app a um
volume que não existe numa instalação limpa. O caminho é `pg_dump` do volume
antigo → `psql` no volume novo do app. O dump de produção tem ~6,6 MB, então é
questão de segundos.

O que migra para `${APP_DATA_DIR}`: `secrets/`, `gateway-settings/`, `backups/`,
`logs/`. O que **não** migra: `/data/trader/bin` (vira imagem) e
`/data/trader/config` (vira imagem).

`backupIgnore` no manifesto deve excluir `logs/` e `data/postgres/` dos backups
do umbrel — o backup do banco já é feito pelo `pg_dump` do scheduler.

## 7. Plano de execução

Cada fase termina num ponto em que dá para parar sem quebrar o que roda hoje.
**Nada é removido do host antes da fase 5.**

### Fase 0 — validar as premissas ✅ (executada 2026-08-28, 18h47 ET)

Store pública `Drumblow/umbrel-daytradebot-store` criada com um app descartável
`daytradebot-probe` (Postgres na 5434 — deliberadamente **não** a 5433, que é a
de produção) e instalada no host com `umbreld client apps.install.mutate`.

| # | Premissa | Resultado |
|---|---|---|
| a | `network_mode: host` sobrevive ao merge de compose do umbreld | ✅ `daytradebot-probe_postgres_1` com `NetworkMode=host`, escutando `127.0.0.1:5434`. E o serviço sem `network_mode` caiu na `umbrel_main_network` (`10.21.0.11`) — **os dois modos convivem no mesmo app** |
| b | O app volta sozinho depois de um reboot | ⚠️ **não demonstrado** — ver abaixo |
| c | `${APP_SEED}` chega no compose | ✅ `APP_SEED` e `APP_PASSWORD` com 64 chars; o Postgres subiu `healthy` com a senha derivada (se viesse vazia, o initdb teria recusado) |
| d | Esquema de nome dos containers | ✅ `${app}_${servico}_1`, confirmando `app.ts:114` |

Variáveis efetivamente injetadas: `APP_ID`, `APP_DATA_DIR`
(`/home/umbrel/umbrel/app-data/<id>`), `APP_VERSION`, `DEVICE_DOMAIN_NAME`,
`APP_SEED`, `APP_PASSWORD`.

**Sobre (b).** O reboot não foi executado porque o `NOPASSWD` do usuário `trader`
vive em `/etc/sudoers.d` e é apagado no boot: depois de reiniciar, o acesso
administrativo remoto some junto e a recuperação da produção passaria a depender
da senha do usuário `umbrel`. A evidência hoje é indireta, mas de duas fontes que
se sustentam:

1. O app está registrado em `/home/umbrel/umbrel/umbrel.yaml`, e `/home` está no
   `/dev/sda4` persistente (verificado com `findmnt`/`df`).
2. `apps.ts:176-188`: no `start()` do umbreld, **todo** app instalado recebe
   `app.start()`, exceto os que têm `autoStart` desligado. O `settings.yml` do
   probe não desliga.

O teste real fica para a fase 4, quando o cutover já exige presença. Enquanto ele
não roda, (b) é a única premissa do ADR ainda apoiada em leitura de código em vez
de observação — e é a premissa que justifica o ADR inteiro.

> Se (a) tivesse falhado, o plano inteiro mudava — por isso foi a primeira fase.
> O probe fica instalado até o teste de reboot; depois é desinstalado.

### Fase 1 — repositório da store ✅ (2026-08-28)

`Drumblow/umbrel-daytradebot-store`, **público** — corrigindo o que este ADR
dizia antes: o umbreld clona a store por git anônimo, então um repositório
privado ele não consegue ler. A consequência é que o checklist da §5 deixa de
ser um portão único antes de publicar e passa a valer **commit a commit**; o
`README` e o `.gitignore` da store registram a regra.

Conteúdo: `umbrel-app-store.yml` (`id: daytradebot`), `daytradebot/` com
manifesto, compose de 14 serviços e o hook `pre-install`.

Validado no host com `docker compose config`: compose resolve, 14 serviços,
`app_proxy` ausente (o umbreld então não injeta o proxy), client_ids 1–11 sem
colisão com o 99 de diagnóstico, manifesto sem campo obrigatório faltando.

### Fase 2 — imagens sem segredo ✅ (2026-08-28)

Três imagens em `deploy/images/`, publicadas por `.github/workflows/images.yml`
em `ghcr.io/drumblow/{trader-bot,trader-gateway,trader-scheduler}`.

O que mudou de verdade no gateway, e é o motivo desta fase existir: o
`ibc.ini` da imagem agora vai com `IbLoginId=` e `IbPassword=` **vazios**.
Descobrimos lendo o `gatewaystart-vm.sh` que o IBC aceita `TWSUSERID`/
`TWSPASSWORD` por variável de ambiente quando esses campos estão em branco —
então a credencial nunca é escrita em disco dentro do container, nem passa pelo
`envsubst` do umbreld. O instalador do Gateway também saiu da imagem e vem por
volume.

O CI tem duas guardas que **falham o build**: uma recusa `ibc.ini` com
credencial preenchida, outra procura credencial no contexto de build.

O `entrypoint.sh` do bot ganhou a guarda de janela de pregão descrita em §4.2, e
o `deploy/images/**` ficou fora do gatilho do `deploy.yml` para não reiniciar
produção.

### Fase 3 — runner (pendente)

9. ~~Container `scheduler`~~ — feito junto da fase 2 (`deploy/images/scheduler/`).
   Cron em horário de Nova York, então o horário de verão americano é resolvido
   pelo tzdata. Confere container por container em vez de confiar no código de
   saída — a lição que o `trader-containers.sh` custou.
10. Container `runner` com o runner do GitHub Actions em `${APP_DATA_DIR}/runner`.
11. Adaptar `.github/workflows/deploy.yml` para o novo alvo (pull de imagem em
    vez de troca de binário).

### Fase 4 — cutover (fora do pregão, sábado)
12. `pg_dump` do banco atual + parar os containers antigos.
13. Instalar o app; copiar para `${APP_DATA_DIR}`: instalação do Gateway
    (`/data/trader/gateway/ibgateway`), `gateway-settings/` e as credenciais
    IBKR extraídas do `ibc.ini` atual para `secrets/ibkr.env` — tudo no próprio
    servidor, sem a credencial passar por nenhum outro lugar. Restaurar o dump.
    Tornar públicos os pacotes no GHCR (nascem privados).
14. Subir e validar: Gateway conectado, 11 instâncias de pé, `status` do CLI ok.
15. **`reboot` de verdade** e confirmar que tudo volta sozinho. Esta é a
    validação que dá sentido ao ADR inteiro — sem ela, não houve migração.

### Fase 5 — limpeza do host (só depois do reboot validado)
16. Remover units/timers `trader-*`, o compose antigo e o volume antigo.
17. Manter `/data/trader` congelado por 7 dias como rollback, depois apagar.
18. Tornar o repositório da store público, após o checklist da §5.
19. Atualizar `HANDOFF.md`, `live-operations.md` (remover a seção de blindagem
    pendente) e `deploy/README.md`.

## 8. Riscos

| Risco | Mitigação |
|---|---|
| `network_mode: host` não sobreviver ao merge do umbreld | fase 0 testa isso antes de qualquer outra coisa |
| Credencial IBKR vazar na imagem pública | imagem sem `COPY` de segredo + auditoria do histórico git antes de publicar |
| Instalador do Gateway não ser baixável em build | fallback: manter o build local no host (`build:` funciona) e não publicar a imagem do gateway |
| Cutover estourar o fim de semana | fases 0–3 não tocam em nada do que roda; o cutover é só a fase 4 |
| `docker.sock` no scheduler/runner = acesso root ao host | é o mesmo nível de acesso que os timers com `sudo` têm hoje; dispositivo pessoal |
| Perder o banco no cutover | dump antes de parar qualquer coisa + volume antigo intacto por 7 dias |

## 9. Consequências

**Positivas:** reboot deixa de derrubar o bot (o problema do ADR-012); a
recuperação manual de 6 passos deixa de existir; o serviço vira instalável e
versionado; some o binário solto trocado por `install`/`mv`.

**Negativas:** o deploy fica mais lento (build e push de imagem em vez de copiar
um binário); passa a existir uma superfície pública (a store) que precisa ser
auditada a cada mudança; ficamos dependentes do formato de app do umbreld, que é
explicitamente marcado como `legacy-compat` no código e vai mudar num refactor
futuro deles.

## 10. Em aberto

- Migrar para a `umbrel_main_network` e fechar 5433/4002 do host (ADR futuro).
- App sem UI web nesta fase; `port` no manifesto é declarado mas não há
  `app_proxy`. Um painel de status seria o próximo passo natural.
- O `webhook_url` de alertas continua sem configuração — nenhum aviso automático
  se o bot cair, com ou sem app (ver `HANDOFF.md` §5).
