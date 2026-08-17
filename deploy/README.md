# deploy/ — infraestrutura da VM Oracle (versionada)

Estes arquivos são a **fonte da verdade** da infraestrutura da VM de produção
paper (`instance-20260807-1139`). Até 2026-08-16 eles existiam só na VM (criados
à mão na migração do ADR-011) — o que permitiu que dois bugs ficassem invisíveis
por 10 dias (timer de parada sem efeito; backup agendado com o serviço `cron`
inativo). Desde 2026-08-17 o deploy do GitHub Actions instala tudo daqui
(idempotente, a cada push que toque `deploy/**` — ver `.github/workflows/deploy.yml`).

| Arquivo | Destino na VM | Função |
|---------|---------------|--------|
| `systemd/trader@.service` | `/etc/systemd/system/` | template das 8 instâncias (`PartOf=` faz o timer de parada funcionar) |
| `systemd/trader-instances.target` | `/etc/systemd/system/` | agregador das instâncias |
| `systemd/trader-start.{timer,service}` | `/etc/systemd/system/` | sobe as instâncias seg–sex 9h25 ET |
| `systemd/trader-stop.{timer,service}` | `/etc/systemd/system/` | para as instâncias seg–sex 16h10 ET |
| `systemd/journald-retention.conf` | `/etc/systemd/journald.conf.d/retention.conf` | retenção explícita de logs (21 dias / 1 GB) |
| `scripts/backup.sh` | `/opt/trader/bin/backup.sh` | pg_dump diário + retenção 7 dias |
| `systemd/trader-backup.{timer,service}` | `/etc/systemd/system/` | agenda o backup (21:30 UTC) — substitui o `/etc/cron.d/trader-backup` original: a VM nem tinha o pacote `cron` instalado (por isso o backup nunca rodou, 08-07→08-17) |

**Não** estão aqui (seguem manuais na VM): `/etc/trader/trader.env` e
`/etc/trader/instances/*.env` (contêm config sensível/local), `ibgateway.service`
e a config do IBC (credenciais), `docker-compose.yml` do Postgres.
