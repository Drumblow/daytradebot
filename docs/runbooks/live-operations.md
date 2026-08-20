# Runbook — Operação do live paper (IBKR) na VM Oracle

Desde 2026-08-07 (ADR-011) o live roda **na VM Oracle Cloud**, não no PC Windows. A rotina manual diária acabou: timers systemd sobem e param as 8 instâncias automaticamente.

## Pré-requisitos

**Nenhum diário.** Os timers fazem tudo:

- `trader-start.timer`: seg–sex **9h25 America/New_York** → sobe `trader-instances.target` (8 instâncias).
- `trader-stop.timer`: seg–sex **16h10 ET** → para as instâncias.
- IB Gateway (`ibgateway.service`, headless via IBC + Xvfb, `Restart=always`) e Postgres (Docker, `restart unless-stopped`) ficam no ar 24/7.

O bot **falha no boot** se: banco indisponível, `ibkr.paper = false`, ou porta de conta real (7496/4001). Isso é intencional — falha fechada. Nesse caso o systemd registra a falha no journal e o restart é tentado conforme a policy da unit.

## Acesso à VM

```bash
ssh -i ~/.ssh/humanbot.key ubuntu@137.131.186.91
```

Onde ficam as coisas na VM:

| Item | Caminho |
|------|---------|
| Binário do bot | `/opt/trader/bin/trader-cli` |
| IBC / credenciais do Gateway | `/opt/trader/ibc/` (`ibc.ini`, permissão 600) |
| Env das 11 instâncias | `/etc/trader/instances/*.env` (client_ids 1–11 para market data; o broker usa automaticamente **id+100** — conexões separadas desde 2026-08-19, erradica o erro 326; id **99** reservado a diagnósticos manuais) |
| Env compartilhado | `/etc/trader/trader.env` |
| Backups do banco | `/opt/trader/backups` (timer systemd `trader-backup.timer`, diário 21:30 UTC — instalado e habilitado pelo deploy) |
| Compose do Postgres | `docker-compose.yml` do repo (deploy na VM) |
| Units systemd / timers / script de backup | versionados em `deploy/` no repo e instalados pelo CI — **não editar à mão na VM** (editar o repo e dar push) |

## Monitorar

A partir do PC (via SSH) ou direto na VM:

```bash
# estado das 8 instâncias
systemctl list-units 'trader@*'

# seguir o log de uma instância
sudo journalctl -u trader@iwm-pullback -f

# log do IB Gateway headless
sudo journalctl -u ibgateway -f
```

Os comandos de inspeção do CLI rodam **na própria VM**, como o usuário `trader` (que tem o env com o `DATABASE_URL`):

```bash
sudo -u trader env $(cat /etc/trader/trader.env | xargs) /opt/trader/bin/trader-cli status    # últimos sinais/trades
sudo -u trader env $(cat /etc/trader/trader.env | xargs) /opt/trader/bin/trader-cli journal   # trades do dia + P&L
sudo -u trader env $(cat /etc/trader/trader.env | xargs) /opt/trader/bin/trader-cli analyze   # live vs backtest + critérios
```

Banco: tabelas `signals`, `orders`, `fills`, `trades`, `system_events` (Postgres em `127.0.0.1:5433` na VM).

## Alertas

Configure `[alerts].webhook_url` (Slack/Discord/Teams) para receber:
- início/encerramento do live;
- trade fechado (com P&L);
- circuit breaker (10 falhas consecutivas de dados/reconciliação → o live encerra com erro).

**Na VM isso é o único canal de notificação** — sem webhook, um CB só é descoberto no próximo acesso SSH. Ver §5 do `docs/HANDOFF.md`.

## Parar / retomar manualmente

```bash
sudo systemctl stop trader-instances.target    # para as 8 instâncias (shutdown gracioso)
sudo systemctl start trader-instances.target   # sobe as 8 fora do timer
```

Parar o bot **não desprotege posições abertas**: stop e alvo ficam **server-side na IBKR** (bracket). Para impedir que o timer do dia seguinte suba o live (ex.: manutenção), mascarar o timer: `sudo systemctl stop trader-start.timer` (e `start` para religar).

## Restart no meio do pregão

Seguro. O bot reconstrói limites diários do banco e reconecta o rastreamento de fills. Fills já persistidos nunca são contados em dobro (dedupe por `broker_fill_id`). O deploy via CI/CD (`push` em `main`) já faz restart condicional — só reinicia instâncias ativas.

```bash
sudo systemctl restart trader-instances.target   # ou uma instância: sudo systemctl restart trader@iwm-pullback
```

## Circuit breaker

Se uma instância encerrar com `circuit breaker: ...`:

1. Veja o log: `sudo journalctl -u trader@<instancia> -n 200 --no-pager`.
2. Verifique o Gateway: `sudo systemctl status ibgateway` e `sudo journalctl -u ibgateway -n 100`.
3. Veja `system_events` no banco para o histórico de falhas.
4. Corrija a causa e suba de novo: `sudo systemctl restart trader@<instancia>` — o estado se recupera do banco.

## Backup do banco

Backup diário automático via timer systemd `trader-backup.timer`: `pg_dump` às **21:30 UTC**, retenção de **7 dias**, em `/opt/trader/backups`. (Até 2026-08-17 era um job de `/etc/cron.d`, mas a VM não tinha o pacote `cron` — nunca executou.) Para um backup manual ou restore:

```bash
docker exec trader-postgres pg_dump -U trader trader_db | gzip > /opt/trader/backups/manual-$(date +%F).sql.gz
```

## ⚠️ Sessão única IBKR

A IBKR permite **uma sessão por usuário**. Abrir TWS ou IB Gateway no PC local com o usuário do bot **derruba a sessão da VM** e mata as 8 instâncias (aconteceu em 2026-08-07 13h04 ET — interrupção de ~26 min). **Nunca abrir TWS/Gateway local com esse usuário.** Para consultar a conta, usar outro usuário ou o portal web da IBKR.
