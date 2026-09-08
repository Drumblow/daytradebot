# Operações — HumanStyle Trader Bot

**Versão:** 1.0  
**Status:** Aprovado para implementação  
**Última atualização:** 2026-09-07 (ADR-018: flatten de fim de pregão)  

---

## 1. Propósito

Este documento define como operar, monitorar, fazer deploy e recuperar o sistema *HumanStyle Trader Bot* em ambientes de desenvolvimento, teste e produção (paper trading).

---

## 2. Ambientes

| Ambiente | Propósito | Dados | Broker |
|----------|-----------|-------|--------|
| `local` | Desenvolvimento | PostgreSQL local/Docker | Simulado ou IBKR paper |
| `staging` | Testes de integração | PostgreSQL de staging | IBKR paper |
| `paper` | Validação em condições reais | PostgreSQL de produção | IBKR paper |
| `production` | **NÃO USAR NO MVP** | — | — |

> **Regra de ouro:** nenhum código opera dinheiro real sem cumprir o **gate composto de go-live** (ADR-010): validação estatística da estratégia com dados históricos + **4 semanas de paper live contínuo** com ≥ 20 trades dentro de ±30% do backtest + aprovação documentada.
> _(Redação anterior: "mínimo 3 meses de paper trading" — substituída pela ADR-010 em 2026-08-04, que torna explícitos os critérios que o calendário aproximava.)_
>
> _(O ADR-019 §7 **propõe** cinco critérios além dos seis do ADR-010: limite inferior do IC95 do PF por bootstrap em blocos ≥ 1,0; PF em unidades de risco ≥ 1,2; concentração dos 2 melhores meses ≤ 60%; holdout travado; e sensibilidade ao custo reportada a 4–5 bp. Destes, só **dois** são impressos hoje pelo `walkforward` (PF em R e concentração) — o IC em blocos depende do relatório em Python do §8 do mesmo ADR, ainda pendente. `t-stat` e `corr(risco, R)` aparecem no relatório, mas são **relatório obrigatório, não critério**. Nada disso vige: são **proposta**, e o gate que decide go-live continua sendo o do ADR-010. Não bloqueie nem libere nada por eles até o dono aprovar.)_

---

## 3. Requisitos de infraestrutura

### 3.1 Mínimo para desenvolvimento

- Rust 1.80+ (instalado via rustup).
- PostgreSQL 15+ ou Docker Desktop.
- Git.
- Acesso à internet para crates e API IBKR.

### 3.2 Mínimo para execução contínua (paper)

- VPS ou máquina dedicada (Oracle Free Tier, Hetzner, etc.).
- 2 vCPU, 4 GB RAM, 20 GB SSD.
- PostgreSQL na mesma máquina ou serviço gerenciado.
- Conexão estável (recomendado wired, não Wi-Fi para execução live).
- Backup automático do banco.

---

## 4. Configuração de ambiente

### 4.1 Variáveis de ambiente obrigatórias

```bash
# Banco de dados
export DATABASE_URL="postgres://user:pass@localhost:5432/trader_db"

# Modo de operação. O loader lê prefixo TRADER com separador "__", então os
# nomes que realmente chegam em [app] são estes — e é por eles (ou pelo arquivo
# apontado por TRADER_CONFIG) que se troca o modo, sem depender do images.yml.
export TRADER__APP__MODE="paper"            # nunca "production" no MVP
export TRADER__APP__PAPER_WARNING="true"    # exibe aviso visual de paper trading

# Broker (IBKR TWS API / IB Gateway)
export TRADER__IBKR__HOST="127.0.0.1"
export TRADER__IBKR__PORT="7497"    # 7496 para TWS real, 7497 para paper
export TRADER__IBKR__CLIENT_ID="1"
export TRADER__IBKR__PAPER="true"
export TRADER__IBKR__ACCOUNT_ID="DU1234567"

# Client Portal API (alternativa; não usada com TWS API)
export IBKR_ACCOUNT_ID="DU1234567"
export IBKR_PAPER="true"
export IBKR_API_URL="https://localhost:5000/v1/api"
export IBKR_CLIENT_ID="seu_client_id"

# Provedor padrão do CLI: "simulated" ou "ibkr"
export TRADER_PROVIDER="simulated"

# Estratégia da instância. A pullback-trend-v1 saiu de produção no ADR-016
# (04/09/2026): em produção rodam balance-area-breakout-v1, opening-reversal-v1
# e range-extreme-fade-v1.
export TRADER_STRATEGY_ID="range-extreme-fade-v1"
export TRADER_STRATEGY_CONFIG_PATH="./config/strategies/range-extreme-fade-v1.toml"

# Risco
export TRADER_RISK_PROFILE="conservative"

# Fim de pregão (ADR-018) — sobrescrevem o bloco [session] do config.
# Os três são horário de NOVA YORK, nunca UTC nem hora local da máquina.
export TRADER__SESSION__FLATTEN_START="15:55:00"
export TRADER__SESSION__FLATTEN_END="16:10:00"
export TRADER__SESSION__LAST_BAR="15:45:00"

# Logging
export RUST_LOG="info"
export RUST_LOG_FORMAT="json"       # ou "pretty"
```

### 4.2 Arquivos de configuração

```text
config/
├── default.toml              # configuração base
├── local.toml                # sobrescrição local (não versionado)
├── paper.toml                # configuração de paper trading
└── strategies/               # um .toml por estratégia implementada (são 9)
    ├── balance-area-breakout-v1.toml   # em produção
    ├── opening-reversal-v1.toml        # em produção
    ├── range-extreme-fade-v1.toml      # em produção
    └── ...                             # as outras 6 continuam versionadas:
                                        # 5 arquivadas + pullback-trend-v1,
                                        # desligada pelo ADR-016
```

### 4.3 Janela de fim de pregão (`[session]`)

O bloco `[session]` do `config/default.toml` define o fim de pregão do ADR-018:

```toml
[session]
flatten_start = "15:55:00"   # início da janela de encerramento a mercado (ET)
flatten_end   = "16:10:00"   # fim da janela, exclusivo (ET)
last_bar      = "15:45:00"   # última barra de 15 min do RTH (ET)
```

O que o operador precisa saber:

- **Os três horários são de Nova York.** O live converte o relógio para
  `America/New_York` antes de comparar; mudar o fuso da máquina não move a janela.
- **Live e backtest leem a MESMA seção** (`SessionSettings` em `trader-infra::config`).
  A paridade entre o que roda em produção e o que o gate A mede é por
  configuração, não por coincidência: quem editar isto muda os dois.
- `last_bar` é **checagem de sanidade**, não gatilho. O backtest identifica a
  última barra do pregão por mudança de data ET (em meio expediente o dia acaba
  antes); se o pregão terminar depois de `last_bar`, os candles não são RTH-only
  e o log avisa.
- **O processo aborta no boot** se `flatten_start >= flatten_end` — com a janela
  vazia nenhuma posição seria encerrada no sino, e falhar alto é melhor do que
  operar com o flatten desligado em silêncio.
- Para reproduzir a régua antiga (sem flatten) em backtest/walkforward existe a
  flag `--no-flatten`. Ela é ferramenta de comparação: **não** tem equivalente
  no live.

---

## 5. IB Gateway / TWS

### 5.1 Instalação

1. Baixe o IB Gateway em https://www.interactivebrokers.com/en/index.php?f=16457
2. Instale e faça login com sua conta paper.
3. Em **Editar > Configuração Global > API > Configurações**, configure:
   - **Socket port:** `7497` (paper) ou `7496` (real)
   - **Permitir conexões de localhost:** ativado
   - **Criar API mensagem de log:** ativado (para debug)
   - **Trusted IP Addresses:** adicione `127.0.0.1`

### 5.2 Verificação de conexão

```bash
# Com provider simulado (não requer IB Gateway)
cargo run --bin trader-cli -- test-connection --provider simulated

# Com IB Gateway aberto (requer conta liberada)
cargo run --bin trader-cli -- test-connection --provider ibkr
```

> **Nota:** Até a conta estar liberada, use `--provider simulated` para todos os comandos.

---

## 6. Deploy

### 6.1 Deploy local (desenvolvimento)

```bash
# 1. Subir banco
docker-compose up -d postgres

# 2. Rodar migrations
sqlx migrate run

# 3. Compilar
cargo build --release

# 4. Executar worker em paper
cargo run --bin trader-cli -- paper --symbol SPY
```

### 6.2 Deploy em VPS (paper)

```bash
# 1. Build em máquina de CI ou local para Linux
cargo build --release --target x86_64-unknown-linux-gnu

# 2. Copiar binário, configurações e migrations
rsync -avz target/release/trader-cli user@vps:/opt/trader/
rsync -avz config/ user@vps:/opt/trader/config/
rsync -avz migrations/ user@vps:/opt/trader/migrations/

# 3. Na VPS
sudo systemctl restart trader-paper
```

### 6.3 Systemd service (exemplo)

```ini
# /etc/systemd/system/trader-paper.service
[Unit]
Description=HumanStyle Trader Bot - Paper Trading
After=network.target postgresql.service

[Service]
Type=simple
User=trader
WorkingDirectory=/opt/trader
EnvironmentFile=/opt/trader/.env
ExecStart=/opt/trader/trader-cli paper --symbol SPY
Restart=always
RestartSec=10
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=multi-user.target
```

### 6.4 Push para `main` é deploy (app umbrelOS)

O caminho real de produção não é o rsync do §6.2: é o workflow
`.github/workflows/images.yml`, que **dispara sozinho em push para `main`**
quando o commit toca `crates/**`, `config/**`, `deploy/images/**`, `Cargo.toml`,
`Cargo.lock` ou o próprio workflow.

- Ele publica as imagens e, com a variável `APP_DEPLOY=enabled` **e** fora do
  pregão, **recria as 8 instâncias de produção** (`--no-deps`; postgres e
  gateway ficam de pé).
- Dentro do pregão (seg–sex, 09h25–16h10 ET) a recriação é bloqueada pela
  guarda de janela; as imagens continuam sendo publicadas. Forçar exige
  `gh workflow run images.yml -f forcar_em_pregao=true`, que derruba posição
  aberta.
- Consequência prática: **`git push` de um commit que toca `crates/` ou
  `config/` é uma mudança de produção**, não um backup do trabalho. Trate o
  push como o passo 5 do §6.7, com o mesmo checklist.

> **Estado em 2026-09-07:** os commits do ADR-018 (flatten de fim de pregão), do
> hotfix ET da `range-extreme-fade-v1` e do ADR-019 (harness) estão na `main`
> **local, sem push**. Dar push muda o `config_hash` da `range-extreme-fade-v1`
> em produção (`49ee6f045b4c35a7` → `818b53394244ca62`) no meio do gate B — e
> config_hash novo **reinicia as 4 semanas** de paper live (§3.8 do plano de
> lucratividade). Só publicar com decisão explícita do dono.

---

## 7. Runbooks

### 7.1 Como iniciar o bot

```bash
# Verificar conexão com broker (simulado por padrão)
trader-cli test-connection

# Verificar conexão com IB Gateway (quando conta estiver liberada)
trader-cli test-connection --provider ibkr

# Verificar conta
trader-cli account

# Ingere candles históricos de SPY (15m, 30 dias)
trader-cli ingest --symbol SPY --timeframe 15m --days 30

# Iniciar paper trading simulado
# (--strategy precisa ser uma estratégia viva; a pullback-trend-v1 saiu no ADR-016)
trader-cli paper --symbol SPY --strategy range-extreme-fade-v1
```

### 7.2 Como parar o bot de forma segura

```bash
# 1. Enviar SIGTERM
sudo systemctl stop trader-paper

# 2. Verificar se há ordens abertas
trader-cli orders --status open

# 3. Se houver posição aberta, ZERE — NÃO deixe "stop/alvo no broker".
#    As três pernas do bracket são enviadas com TIF Day e expiram no sino:
#    posição que atravessa o fechamento fica SEM STOP (achado C1 da auditoria,
#    razão de existir o flatten do ADR-018). Parar o bot desliga o flatten
#    automático, então parar com posição aberta e não zerar é a pior
#    combinação possível.
trader-cli flatten --symbol SPY --provider ibkr --confirm
trader-cli cancel-orders --symbol SPY --provider ibkr --confirm
#    (sem --confirm os dois comandos apenas mostram o que fariam)

# 4. Exceção: dá para parar com posição aberta se o processo voltar ANTES de
#    15h55 ET. Ao subir, ele recupera a ordem em aberto do banco, religa o
#    tracker de fills e volta a fazer o flatten na janela [session].
```

### 7.3 Queda de conexão com broker

```text
1. O bot tenta reconectar automaticamente (backoff exponencial).
2. Se reconectar em < 60s, retoma operação normal.
3. Se > 60s, o RiskManager suspende novas entradas até reconexão.
4. Se > 5 min, enviar alerta e aguardar intervenção manual.
5. Ao reconectar, reconciliar ordens abertas e posições.
```

### 7.4 Ordem rejeitada pelo broker

```text
1. Registrar erro em system_events e logs.
2. Não tentar reenviar automaticamente sem intervenção humana.
3. Notificar operador se taxa de rejeição > 5% em 1h.
```

### 7.5 Perda máxima diária atingida

```text
1. RiskManager bloqueia novas entradas imediatamente.
2. Posições abertas mantêm stops/alvos ATÉ O SINO — as pernas do bracket são
   TIF Day. O bloqueio de entradas NÃO desliga o flatten de fim de pregão: na
   janela [session] (15h55–16h10 ET) a posição é encerrada a mercado e o trade
   sai com exit_reason = end_of_day (ADR-018; antes ele saía como `manual` com
   journal.forced_exit = "session_flatten").
3. Enviar alerta CRITICAL.
4. Só liberar no próximo dia útil, após reset automático às 00:00 UTC.
```

### 7.6 Posição órfã no broker (flatten automático não executou)

```text
Sintoma: system_events com event_type = "untracked_position" + alerta CRITICAL,
e a posição continua aberta no broker depois da janela de flatten.

Por que o bot não fechou (de propósito): o flatten de fim de sessão só encerra
o que a PRÓPRIA instância rastreia. Hoje o AVUV é o único símbolo com duas
instâncias (`avuv-balance` e `avuv-rangefade`) — antes do ADR-016 eram três
símbolos duplicados, IWM e IWV incluídos, porque cada um também tinha instância
da pullback. Se as duas zerassem a mesma posição, a primeira fecharia e a
segunda abriria uma posição INVERTIDA do mesmo tamanho, a mercado, cinco
minutos antes do sino. A regra não vale só para o símbolo duplicado: instância
nenhuma fecha o que não abriu. Posição que ninguém rastreia é problema de
operação, não de automação: vira alerta e espera decisão humana.

Procedimento (humano, ANTES das 16h ET — depois do sino não há mais stop):
1. Conferir no broker de quem é a posição (qual instância/estratégia abriu).
2. Se nenhuma instância a rastreia, zerar à mão:
       trader-cli flatten --symbol IWM --provider ibkr --confirm
       trader-cli cancel-orders --symbol IWM --provider ibkr --confirm
   (sem --confirm os comandos só mostram o que fariam)
3. Registrar o ocorrido: a posição órfã não vira trade no banco, então o P&L
   dela não aparece em nenhum relatório automático.
4. Se a órfã reaparecer todo dia, é resíduo antigo, não um bug do flatten —
   ele avisa e não fecha, e vai avisar de novo amanhã.

Caso já ocorrido, ENCERRADO: o IWM carregou 827 ações órfãs de 07/08/2026 até
04/09/2026. Enquanto elas existiam, as duas instâncias de IWM ficaram cegas —
não avaliaram nada. A venda executou na abertura de 04/09/2026 e a conta ficou
sem posições e sem ordens. A causa-raiz já foi corrigida no código:
`confirm_order` tratava fim de stream sem status como falha, então o chamador
achava que a ordem não tinha saído, não a rastreava, e ela ficava órfã no
broker; hoje ausência de confirmação é assumida como aceita, com aviso.

**Não há posição órfã em aberto hoje** (07/09/2026). O comentário do
`paper.rs` que ainda fala do IWM "que carrega 827 ações órfãs" é histórico,
não estado do sistema.
```

### 6.6 Divergência de posição

```text
1. Reconciliador compara posição local com posição do broker.
2. Se divergir, marcar no system_events e alertar.
3. Se posição real > posição esperada, reduzir ao esperado.
4. Se posição real < posição esperada, reabrir ordem se justificado.
5. Nunca aumentar exposição sem passar pelo RiskManager.
```

### 6.7 Atualização de código em produção (paper)

```text
1. Criar tag de release.
2. Fazer backup do banco.
3. Parar o bot (SIGTERM) — seguindo o §7.2, inclusive o passo de zerar posição.
4. Aguardar ordens pendentes serem resolvidas ou cancelá-las.
5. Deploy do novo binário.
6. Rodar migrations. As duas do ADR-018/ADR-019 são obrigatórias:
   - 0004_exit_reason_end_of_day.sql — amplia o CHECK de trades.exit_reason
     para aceitar 'end_of_day'. SEM ELA, todo insert de trade fechado pelo
     flatten falha (SQLSTATE 23514). O DROP do CHECK antigo é por busca, não
     por nome, justamente para não falhar em silêncio se algum ambiente
     divergir. Ela NÃO reescreve linha nenhuma.
   - 0005_backtest_runs_lookup.sql — cria idx_backtest_runs_baseline, usado
     pela busca de baseline (latest_for) do `analyze`.
7. Iniciar bot em modo "dry-run" por 15 minutos.
8. Se tudo OK, ativar trading.

OPCIONAL, e decisão do dono (ADR-018, decisão 2):
   sql/maintenance/0004-reclassificar-flatten.sql reclassifica os flattens
   antigos gravados como `manual` (com journal.forced_exit = "session_flatten")
   para `end_of_day`. Enquanto ele não rodar nada fica errado — o código
   reconhece o flatten histórico pelo journal, via
   Trade::effective_exit_reason(). É higiene, não correção; exige dump da
   tabela antes.
```

---

## 7. Monitoramento

### 7.1 Logs

- Logs estruturados via `tracing`.
- Em produção, usar formato JSON para ingestão em ferramentas como Loki, Datadog ou CloudWatch.
- Campos obrigatórios em logs importantes:
  - `correlation_id`
  - `symbol`
  - `strategy_id`
  - `event_type`
  - `timestamp`

### 7.2 Métricas

Expor métricas básicas via endpoint HTTP (futuro) ou logs:

```text
bot_uptime_seconds
candles_received_total
candles_duplicated_total
signals_generated_total
signals_rejected_total
orders_submitted_total
orders_filled_total
orders_rejected_total
trades_closed_total
daily_pnl
max_drawdown_percent
```

### 7.3 Alertas

| Condição | Severidade | Canal |
|----------|------------|-------|
| Perda máxima diária atingida | CRITICAL | Email + SMS/Discord |
| Falha de conexão com broker > 5 min | CRITICAL | Email + SMS/Discord |
| Divergência de posição | CRITICAL | Email + SMS/Discord |
| Ordem rejeitada pelo broker | WARNING | Email |
| Taxa de rejeição > 5% em 1h | WARNING | Email |
| Candles perdidos > 3 em 1h | WARNING | Email |
| Latência candle→decisão > 30s | WARNING | Email |

---

## 8. Backup e recuperação

### 8.1 Backup do banco

```bash
# Backup diário
pg_dump -Fc -U trader trader_db > /backups/trader_db_$(date +%Y%m%d).dump

# Retenção de 30 dias
find /backups -name "trader_db_*.dump" -mtime +30 -delete
```

### 8.2 Recuperação

```bash
# Restaurar banco
pg_restore -U trader -d trader_db /backups/trader_db_20260702.dump
```

### 8.3 Disaster recovery

| Cenário | Ação |
|---------|------|
| Perda do banco | Restaurar do backup mais recente; reingestar dados de mercado se necessário. |
| Perda do servidor | Provisionar novo servidor, restaurar banco, redeploy binário, verificar conexão. |
| Falha do broker | Suspender trading; manter posições com stops no broker; aguardar normalização. |
| Bug crítico no código | Rollback para versão anterior; analisar trades afetados; corrigir e revalidar. |

---

## 9. Segurança operacional

- Nunca executar o bot como root.
- Arquivo `.env` com permissão `600`.
- Chaves e credenciais nunca no Git.
- Acesso SSH apenas por chave.
- Firewall bloqueando portas desnecessárias.
- PostgreSQL acessível apenas localmente ou por VPN.

---

## 10. Checklist diário de operação (paper)

```text
[ ] Bot está conectado ao broker.
[ ] Nenhuma divergência de posição.
[ ] Perda diária dentro do limite.
[ ] Número de trades dentro do limite.
[ ] Nenhuma ordem pendente sem status há mais de 15 min.
[ ] Logs não mostram erros críticos.
[ ] Backup do banco foi executado.

FIM DE PREGÃO — conferir depois das 16h10 ET (ADR-018):
[ ] Nenhuma posição aberta no broker, em nenhum símbolo.
[ ] Nenhum system_events do tipo "untracked_position" no dia. Se houver, o
    flatten automático NÃO fechou aquela posição (§7.6): zerar à mão com
    `trader-cli flatten --symbol X --provider ibkr --confirm` — e, se já passou
    do sino, tratar como posição overnight SEM STOP.
[ ] Os trades encerrados pelo flatten aparecem com exit_reason = end_of_day
    (não `manual`).
```

---

## 11. Referências

- `docs/ARCHITECTURE.md`
- `docs/SECURITY.md`
- `docs/runbooks/`
- `docs/decisions/ADR-018-paridade-fim-de-sessao-backtest.md` — flatten de fim
  de pregão, a janela `[session]` e o `exit_reason = end_of_day`.
