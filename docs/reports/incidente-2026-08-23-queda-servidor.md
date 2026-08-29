# Incidente — Queda do servidor da casa (2026-08-23 → 2026-08-27)

**Severidade:** alta — 4 pregões perdidos, operação parada por 4 dias
**Detectado em:** 2026-08-27 (o dono notou que o bot não operou no dia)
**Diagnóstico:** 2026-08-27/28, por SSH no servidor
**Status:** causa identificada; blindagem de software RESOLVIDA em 2026-08-28 (ADR-013 — migração para app do umbrelOS, reboot validado). Pendências de hardware (BIOS/nobreak) seguem com o dono.

---

## 1. Resumo executivo

O servidor da casa (umbrelOS, `<SERVIDOR_CASA>`) **morreu de forma abrupta em
2026-08-23 18:06:29 UTC** (14:06 ET, domingo) e **só voltou em 2026-08-27
21:59 UTC** — quando foi religado manualmente, já depois do fechamento do
pregão. Os pregões de **24, 25, 26 e 27/08 foram integralmente perdidos**.

Agravante estrutural descoberto na investigação: o reboot **apagou a
configuração de host** (usuário de serviço, timers, runner do CI) e **removeu
todos os containers do trader**. Ou seja, mesmo que a energia tivesse voltado em
minutos, **o bot não teria subido sozinho**.

Consequência para o gate B (ADR-010): a migração para a casa terminou em 08-23
03:07 UTC e a máquina caiu na mesma tarde — **o setup da casa nunca completou um
pregão**. O último dia real de operação foi **2026-08-21, ainda na VM Oracle**.

## 2. Linha do tempo (UTC)

| Quando | O quê |
|---|---|
| 08-22 22:39 | boot anterior inicia (fim de semana da migração) |
| 08-23 03:07 | último deploy bem-sucedido (`46e8159`), stack completa no ar |
| 08-23 04:15 | última atividade do IB Gateway registrada no log do IBC |
| 08-23 18:05:22 | dockerd começa a falhar DNS externo (`1.0.0.1`, `1.1.1.1` — i/o timeout) |
| 08-23 18:05:33 | umbreld falha ao atualizar o app-store (`Request timed out`) |
| **08-23 18:06:29** | **último registro do journal. Silêncio.** |
| 08-24 a 08-27 | máquina desligada — 4 pregões perdidos |
| 08-27 21:59:17 | boot atual (religada manualmente) |
| 08-27 21:59:48 | `trader-gateway` sobe pelo `restart: always` e escreve log do IBC |
| 08-27 ~22:00 | Gateway recebe SIGTERM (`IBC returned exit status 143`); containers do trader desaparecem |
| 08-27 22:00:04 | `umbreld v1.7.4` inicia e recria apenas os apps do umbrelOS |

## 3. Causa da queda: desligamento sujo (energia)

O log do boot anterior **termina no meio do funcionamento normal**, sem nenhuma
linha de `systemd[1]: Stopping ...`. Desligamento limpo deixa rastro; este não
deixou.

O minuto anterior é a evidência mais forte: **a rede morreu antes da máquina**.
O DNS externo parou de responder às 18:05:22 e o servidor apagou 67 segundos
depois. Roteador caindo primeiro e servidor logo em seguida é a assinatura de
**falta de energia na casa**, não de defeito interno.

### Não é evento isolado

Dos 7 boots com log preservado, **4 terminaram sujos**:

| Boot terminou em (UTC) | Como |
|---|---|
| 2026-07-07 12:24 | sujo |
| 2026-08-08 02:42 | limpo (`systemd: Stopping...`) |
| 2026-08-08 16:39 | limpo |
| 2026-08-16 11:03 | sujo |
| 2026-08-21 21:52 | sujo |
| **2026-08-23 18:06** | **sujo** |

E em nenhum caso a máquina voltou sozinha: ficou fora 12h, 25h e 4 dias, sempre
até alguém religar. **A BIOS não está configurada para retornar após falta de
energia** — corrigir isso sozinho teria salvado 3 dos 4 pregões.

### Descartado como causa

- **Memória/disco:** filesystem `clean`, sem erro de I/O, sem OOM, RAM folgada
  (5,5 GB de 11 GB em uso).
- **Térmico / MCE:** nenhum registro.
- **PCIe:** há erros **corrigíveis** contínuos em `pcieport 0000:00:1c.7` —
  device `02:00.0 Intel Wireless 3165`, a placa WiFi, que está **DOWN** (a
  máquina usa cabo, `enp1s0`). A taxa é estável em ~700 erros/dia em todos os
  boots, **não sobe antes das quedas**, e erro corrigível não derruba máquina.
  É defeito real a eliminar (desabilitar a placa na BIOS), mas **não é a causa**.

## 4. O que o reboot destruiu (e por quê)

A raiz do umbrelOS é um **overlay rugix**:

```
/  overlay  lowerdir=/run/rugix/mounts/system
            upperdir=/run/rugix/mounts/data/state/default/overlay/b
```

Esse estado foi resetado no boot. Tudo que a migração instalou em `/` sumiu:

| Perdido | Sobreviveu |
|---|---|
| usuário `trader` (uid 1001) e `authorized_keys` | `/data/trader` completo (binário, configs, 11 envs, backups) |
| units/timers `trader-*` em `/etc/systemd/system/` | volume `trader_trader-postgres-data` — **113 MB, banco íntegro** |
| runner self-hosted do GitHub Actions (`casa-umbrel`) | imagens `trader-bot:latest` e `trader-gateway:10.45` |
| todos os containers `trader-*` (removidos, não parados) | `/var/log` e journal (ficam em `/dev/sda4`) |

**Nenhum dado foi perdido** — todo o histórico do gate B está preservado no
volume do Postgres.

Sobre os containers: o Gateway chegou a subir no boot pelo `restart: always`,
levou SIGTERM um minuto depois e sumiu junto com os outros. Volume e imagens
preservados é assinatura de `compose down`, não de faxina geral; o início do
`umbreld` às 22:00:04 coincide com o SIGTERM. **Hipótese principal, não
comprovada** — não há linha no journal atribuindo a remoção a um processo.

A ADR-012 previa que "containers e volumes sobrevivem e os timers são
reinstalados pelo deploy". Isso se mostrou **insuficiente**: os timers somem, e
o deploy que os reinstalaria depende de um runner que também some.

## 5. Impacto

- **4 pregões perdidos:** 24, 25, 26 e 27/08.
- **Gate B parado** desde 08-21; o setup da casa ainda não validou um dia sequer.
- **Sem backup do banco desde 08-23 02:52** — o timer diário nunca mais rodou.
- **CI quebrado:** sem runner, `push` em `main` não entrega nada no servidor.

## 6. Ações

Recuperação executada em 2026-08-28 (madrugada, mercado fechado):

| # | Ação | Estado |
|---|---|---|
| 1 | Recriar usuário `trader` (uid 1001) + chave + sudo NOPASSWD | ✅ feito |
| 2 | Subir Postgres e conferir integridade | ✅ feito — banco íntegro, sem perda (ver §6.1) |
| 3 | Subir `trader-gateway`; login IBKR na conta DUR507388, API em 4002 | ✅ feito |
| 4 | Backup manual do banco | ✅ feito — `/data/trader/backups/manual-2026-08-28.sql.gz` |
| 5 | Reinstalar timers `trader-*` | ✅ feito — start 13:25 UTC, stop 20:10 UTC, backup 21:30 UTC |
| 6 | Criar os 11 containers das instâncias (ficam parados até o timer) | ✅ feito |
| 7 | Corrigir `trader-containers.sh` (bug encontrado na recuperação, §6.2) | ✅ feito |
| 8 | Reinstalar runner `casa-umbrel` | ✅ feito — v2.337.0 em `/data/trader/actions-runner` (persistente), online |
| 9 | Backfill dos candles de 08-22 a 08-27 | ✅ feito em 08-28 — e fechou também a lacuna de 08-06 a 08-20 de SPY/QQQ/VB/SCHA/MDY/IJR |
| 10 | **BIOS: *Restore on AC Power Loss = Power On*** | ⬜ pendente (dono) |
| 11 | **Nobreak para o servidor** | ⬜ pendente (dono) |
| 12 | Blindagem contra o reset do rugix | ✅ **2026-08-28** — resolvida de forma diferente da proposta: em vez de um `reprovision.sh` disparado no boot, o serviço virou um **app do umbrelOS** (ADR-013), cujo estado é persistente e que o umbreld sobe sozinho. Reboot real validado: containers aos 91s, IBKR aos 144s, sem intervenção. |
| 13 | Desabilitar a placa WiFi na BIOS (ruído de PCIe) | ⬜ pendente (dono) |
| 14 | Configurar `webhook_url` de alertas | ⬜ pendente |

### 6.1 Integridade do banco — confirmada

O Postgres subiu limpo. Contagens após a recuperação:

| Tabela | Linhas | Faixa de ids | Última atividade |
|---|---|---|---|
| candles | 134.843 | — | 2026-08-21 19:45 UTC |
| signals | 302 | 1–302 (sem buracos) | 2026-08-21 14:42 |
| trades | 6 | 7–12 | entrada 2026-08-20 |
| orders | 8 | — | 2026-08-20 |
| system_events | 527 | 1–577 | 2026-08-23 02:52 |
| backtest_runs | 201 | — | — |

`candles` e `backtest_runs` batem **exatamente** com os números da migração
(ADR-012), e os ids de `signals` são contínuos — não houve truncamento.

⚠️ **Divergência de documentação, não de dados:** a ADR-012 registrou "604
trades, 1.117 system_events" migrados; o banco tem 6 e 527.

Verificado contando as linhas direto no dump do dia da migração
(`/data/trader/backups/trader_db-20260823-025253.sql.gz`):

| Tabela | No dump de 08-23 | No banco hoje |
|---|---|---|
| trades | **6** | 6 |
| system_events | **527** | 527 (541 após os testes de recuperação) |
| candles | 134.843 | 134.843 |
| signals | 302 | 302 |
| backtest_runs | 201 | 201 |

**Nada se perdeu na migração nem na queda** — o dump de origem já tinha esses
números. Os valores da ADR-012 estavam simplesmente errados quando foram
escritos.

Também não são trades de backtest: o backtest **não persiste trade a trade**
(só o agregado em `backtest_runs.metrics`), e a soma de `total_trades` dos 201
runs dá **6.090**, com o maior run individual em 146 — nenhum caminho leva a
604. Os 6 trades (ids 7–12) são exatamente os descritos nos relatórios diários,
e o `system_events` id 322 citado no relatório do dia 10 é coerente com ids na
casa das centenas, não dos milhares.

### 6.2 Bug encontrado: o timer teria falhado em silêncio

Ao testar o caminho exato que o timer usa, `trader-containers.sh start`
imprimiu `trader containers started` **sem subir nenhum container**.

Causa: o script tentava `docker compose start <svc>` e só caía para
`up -d --no-recreate` se o primeiro falhasse — mas **`compose start` retorna 0
mesmo quando o container não existe** (verificado: exit code 0). Depois do
reboot que removeu os containers, o `||` nunca disparava. Some-se o
`>/dev/null 2>&1 || true` em cada linha e o resultado é um timer que termina
"com sucesso" às 9h25 sem nenhum bot rodando, sem log e sem alerta.

Correção (`deploy/home/trader-containers.sh`): usar `up -d --no-recreate`
direto (idempotente) e **verificar** ao final que as 11 instâncias estão
rodando, saindo com erro se não estiverem — assim o timer aparece como
`failed` no systemd em vez de mentir. Testado: 11/11.

Sem esse teste, o pregão de 28/08 teria sido o quinto perdido.

### 6.3 Lacuna de candles 08-22 a 08-27

O último candle no banco é de **2026-08-21 19:45 UTC**. Os pregões de 24 a 27/08
não têm dados — o bot estava desligado. Isso afeta o warmup das estratégias
multi-dia (a `range-extreme-fade-v1` precisa de ~14 dias de ATR diário) e
qualquer backtest que cruze o período.

Fazer o backfill com `trader-cli ingest` **fora da janela de manutenção noturna
da IBKR** (23h45–00h45 ET), conferindo depois que as barras têm `high > low`
(ver `docs/runbooks/troubleshooting.md` e a armadilha de barras degeneradas).

### 6.4 Observações menores

- As instâncias saíram com **exit 137 (SIGKILL)** no `compose stop`: não
  encerram dentro do prazo de graça do SIGTERM. Não desprotege posição (stop e
  alvo são server-side na IBKR), mas vale investigar o shutdown gracioso.
- Um `circuit_breaker` disparou às 04:04 UTC durante o teste ("falhas
  consecutivas ao buscar candles na IBKR"), com as instâncias rodando às 00h04
  ET — dentro da manutenção noturna da IBKR e fora da janela em que elas
  deveriam estar de pé. Artefato do teste, não defeito.

## 7. Lições

1. **Servidor em casa sem nobreak e sem retorno automático na BIOS custa dias de
   operação**, não minutos. A Oracle nunca teve esse modo de falha.
2. **Em host de raiz imutável, infra instalada em `/` é volátil.** Não basta
   versionar as units no repo e instalá-las pelo CI: é preciso um mecanismo de
   reprovisionamento que sobreviva ao reset e rode no boot.
3. **A falta do canal de alerta transformou 1 pregão perdido em 4.** O incidente
   só foi notado no quarto dia. O item 3 das pendências do HANDOFF deixou de ser
   melhoria e virou requisito.
