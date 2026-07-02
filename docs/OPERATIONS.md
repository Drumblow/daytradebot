# Operações — HumanStyle Trader Bot

**Versão:** 1.0  
**Status:** Aprovado para implementação  
**Última atualização:** 2026-07-02  

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

> **Regra de ouro:** nenhum código opera dinheiro real sem passagem explícita por paper trading por no mínimo 3 meses e aprovação documentada.

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

# Modo de operação
export TRADER_MODE="paper"          # nunca "production" no MVP
export TRADER_PAPER_WARNING="true"  # exibe aviso visual de paper trading

# Broker (IBKR)
export IBKR_ACCOUNT_ID="DU1234567"
export IBKR_PAPER="true"
export IBKR_API_URL="https://localhost:5000/v1/api"  # Client Portal API
export IBKR_CLIENT_ID="seu_client_id"

# Se usar TWS API
export IBKR_TWS_HOST="127.0.0.1"
export IBKR_TWS_PORT="7497"         # 7496 para TWS real, 7497 para paper
export IBKR_TWS_CLIENT_ID="1"

# Estratégia ativa
export TRADER_STRATEGY_ID="pullback-trend-v1"
export TRADER_STRATEGY_CONFIG_PATH="./config/strategies/pullback-trend-v1.toml"

# Risco
export TRADER_RISK_PROFILE="conservative"

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
└── strategies/
    └── pullback-trend-v1.toml
```

---

## 5. Deploy

### 5.1 Deploy local (desenvolvimento)

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

### 5.2 Deploy em VPS (paper)

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

### 5.3 Systemd service (exemplo)

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

---

## 6. Runbooks

### 6.1 Como iniciar o bot

```bash
# Verificar conexão com broker
trader-cli test-connection

# Verificar conta
trader-cli account

# Iniciar paper trading
trader-cli paper --symbol SPY --strategy pullback-trend-v1
```

### 6.2 Como parar o bot de forma segura

```bash
# 1. Enviar SIGTERM
sudo systemctl stop trader-paper

# 2. Verificar se há ordens abertas
trader-cli orders --status open

# 3. Se houver posição aberta, decidir:
#    - manter stop/alvo no broker (recomendado);
#    - fechar manualmente via CLI.
```

### 6.3 Queda de conexão com broker

```text
1. O bot tenta reconectar automaticamente (backoff exponencial).
2. Se reconectar em < 60s, retoma operação normal.
3. Se > 60s, o RiskManager suspende novas entradas até reconexão.
4. Se > 5 min, enviar alerta e aguardar intervenção manual.
5. Ao reconectar, reconciliar ordens abertas e posições.
```

### 6.4 Ordem rejeitada pelo broker

```text
1. Registrar erro em system_events e logs.
2. Não tentar reenviar automaticamente sem intervenção humana.
3. Notificar operador se taxa de rejeição > 5% em 1h.
```

### 6.5 Perda máxima diária atingida

```text
1. RiskManager bloqueia novas entradas imediatamente.
2. Posições abertas mantêm stops/alvos.
3. Enviar alerta CRITICAL.
4. Só liberar no próximo dia útil, após reset automático às 00:00 UTC.
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
3. Parar o bot (SIGTERM).
4. Aguardar ordens pendentes serem resolvidas ou cancelá-las.
5. Deploy do novo binário.
6. Rodar migrations.
7. Iniciar bot em modo "dry-run" por 15 minutos.
8. Se tudo OK, ativar trading.
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
```

---

## 11. Referências

- `docs/ARCHITECTURE.md`
- `docs/SECURITY.md`
- `docs/runbooks/`
