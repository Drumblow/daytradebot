# ADR-014 — Painel web de status (`trader-web`)

**Status:** ACEITO — implementado em 2026-08-28
**Contexto anterior:** ADR-013 (app umbrelOS), que deixou registrado em aberto:
*"App sem UI web nesta fase; `port` no manifesto é declarado mas não há
`app_proxy`. Um painel de status seria o próximo passo natural."*

---

## 1. Problema

Depois do cutover para o app do umbrelOS, a única forma de saber o que o bot
está fazendo é SSH (que não sobrevive ao reboot) ou os workflows `host-check`/
`ops` do GitHub Actions. Não existe resposta rápida para as perguntas do dia a
dia: *o bot está vivo? operou hoje? por que não entrou? quanto está o P&L?*

O manifesto do app declara `port: 4002` — a API do Gateway — então clicar no
app na dashboard do umbrel não abre nada útil.

## 2. Decisão

Um serviço novo, **`trader-web`**, no workspace do bot:

- **Crate Rust (axum)** que serve uma API JSON read-only sobre o mesmo
  PostgreSQL e um dashboard estático (HTML/CSS/JS puro) **embutido no binário**
  via `include_str!`. Um único artefato, sem toolchain Node, sem assets soltos.
- **Read-only por construção:** a conexão abre com
  `default_transaction_read_only=on`; qualquer escrita acidental é recusada
  pelo Postgres. O painel não conhece o broker — o único contato com o Gateway
  é um probe TCP de liveness na 4002 (configurável, `TRADER_WEB_GATEWAY_ADDR`).
- **Imagem própria** `ghcr.io/drumblow/trader-web`, publicada pelo job `web` de
  `images.yml`, empacotada como os demais serviços (binário do CI + Debian slim).
- **Serviço `web` no compose do app** (store), `network_mode: host` como o
  resto da fase 1 — é o único jeito de alcançar o Postgres, que escuta só em
  `127.0.0.1:5433`. Bind em `0.0.0.0:8551`; o manifesto passa a declarar
  `port: 8551`, então clicar no app abre o painel.
- Roda **24/7** (como postgres/gateway, fora do controle do scheduler): fora do
  pregão o painel continua útil — histórico, eventos, backtests.

## 3. O que o painel mostra

Tudo vem das tabelas que o bot **de fato escreve** (constatação da análise:
`account_snapshots` e `positions` existem no schema mas nada as escreve hoje —
não há repository para elas):

| Seção | Fonte |
|---|---|
| KPIs (P&L hoje/total, win rate, R, sinais do dia, ordens em aberto) | `trades`, `signals`, `orders` |
| Curva de P&L acumulado | `trades` (soma cumulativa de `net_pnl` — **não** é equity da conta, que exigiria `account_snapshots`) |
| P&L por dia (barras) | `trades` agrupados por dia **em horário de NY** |
| Cards das 11 instâncias (heartbeat de candle, último sinal/trade, sparkline) | `candles`, `signals`, `trades` + config das instâncias |
| Tabelas: trades, sinais (com motivo de rejeição), ordens & fills, eventos, backtests | respectivas tabelas |
| Fase do pregão e hora ET | calculado no servidor (chrono-tz), janela 09:25–16:10 ET |

O pareamento símbolo×estratégia×client_id das instâncias não existe no banco —
vive no compose. O default do binário espelha as 11 instâncias de produção e
`TRADER_WEB_INSTANCES` (JSON) sobrescreve sem rebuild.

## 4. Decisões de implementação

- **Queries em runtime (`sqlx::query_as`), não os macros `query!`** usados em
  `trader-infra`: o job de CI do painel compila **sem** serviço de Postgres, e
  o crate não depende de nenhum outro crate do workspace — leitura e agregação
  não precisam dos invariantes de escrita do domínio. Enums do Postgres são
  lidos com `::text` (o cast é inócuo para colunas TEXT).
- **Zero dependência de frontend:** gráficos são SVG gerado à mão (curva de
  P&L em degraus, barras diárias, sparklines). Nada de CDN — o painel funciona
  com a LAN sem internet. Todo dado do banco entra no DOM via `textContent`
  (mensagens de evento são texto arbitrário).
- **f64 nunca aparece no caminho do dinheiro:** valores saem do banco como
  `NUMERIC` → `Decimal` → JSON string; conversão para número só no JS, para
  exibição e escala de gráfico.
- **"Ordens em aberto" é escopado a hoje (ET):** toda ordem do bot é TIF `day`,
  então status aberto de dias anteriores é sempre falha de reconciliação (ordem
  que o bot nunca confirmou/cancelou no banco), não ordem viva no broker. O
  KPI conta as de hoje e mostra as antigas como aviso explícito ("N sem
  reconciliar") — encontrado na auditoria: uma ordem `submitted` de 07/08
  ficaria no contador para sempre.
- **O rodapé mostra host:porta/banco da conexão** (sem credencial). Motivo
  concreto: na validação o painel apontado para o snapshot local de dev (3
  trades, dados até 07/08) foi confundido com produção (6 trades). O alvo da
  conexão visível elimina essa classe de confusão.
- **`host-check.yml` ganhou um passo "auditoria do painel"** com as mesmas
  agregações SQL do `trader-web`, rodando no Postgres de produção — conferência
  de um clique entre o que a UI mostra e o que o banco real contém.

## 5. Segurança

- **Sem autenticação na fase 1.** O painel é read-only e fica exposto na LAN —
  o mesmo nível de exposição que a API do Gateway (4002) e o Postgres (5433)
  já têm hoje com `network_mode: host`. Quando a migração para a
  `umbrel_main_network` acontecer (pendência do ADR-013), o caminho certo é
  `app_proxy`, que dá a autenticação do próprio umbrel de graça — o painel já
  funciona atrás de proxy sem mudança.
- Nenhum segredo no repositório da store: `DATABASE_URL` deriva de
  `${APP_SEED}`, como os demais serviços.
- A API não expõe credencial nenhuma (o banco não guarda credenciais; o
  `market_snapshot`/`journal` são dados de mercado).

## 6. Deploy e sequência de ativação

1. Repo do bot: merge → `images.yml` publica `trader-web` e o job `deploy`
   sobe o serviço **somente se** o compose instalado no host já declarar `web`
   (guarda com `docker compose config --services`), para não quebrar quando a
   store estiver uma versão atrás.
2. Repo da store: compose + manifesto `1.1.0` (`port: 8551`) → atualizar o app
   no umbrel (a dashboard oferece a atualização; o `up` do umbreld cria o
   container novo).

## 7. Riscos e limitações

| Risco | Mitigação |
|---|---|
| Painel na LAN sem auth | read-only; mesmo perímetro das portas 4002/5433 já expostas; app_proxy fica para a migração de rede |
| Query pesada no Postgres de produção durante o pregão | pool de 5 conexões, agregações indexadas (índices existentes por `exit_time`/`timestamp`), refresh de 30 s |
| Porta 8551 colidir com outro app do umbrel | porta incomum; se colidir, trocar em um único lugar (compose + manifesto) |
| "P&L acumulado" ser lido como equity da conta | rotulado no painel como "trades fechados"; equity real exigiria escrever `account_snapshots` (trabalho futuro) |

## 8. Futuro

- Escrever `account_snapshots` no live e trocar a curva por equity real.
- `app_proxy` + `umbrel_main_network` junto com a migração de rede do ADR-013.
- Filtros por estratégia/símbolo e drill-down de sinal (market_snapshot).
