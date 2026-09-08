# HumanStyle Trader Bot

Robô trader automatizado/semi-automatizado baseado em **Price Action**, análise técnica e gestão de risco rigorosa. Opera como um trader humano disciplinado: espera contexto, evita mercado ruim, respeita stop e registra tudo.

> **Aviso:** Este projeto está em fase inicial (MVP) e opera exclusivamente em **paper trading**. Nenhum dinheiro real é negociado.

---

## Visão

Juntar a leitura contextual de um trader humano com a disciplina, repetibilidade e auditoria de uma máquina.

```text
operar pouco,
operar com contexto,
respeitar risco,
registrar tudo,
aprender com os dados,
e evitar os erros emocionais do humano.
```

---

## Características principais

- **Estratégias baseadas em livros** de Price Action (Al Brooks e outros).
- **Paper trading** na Interactive Brokers.
- **Arquitetura multi-broker** desde o início.
- **Gestão de risco rigorosa** com limites automáticos.
- **Backtest determinístico** usando a mesma lógica do live.
- **Diário automático** de trades com métricas.
- **Auditabilidade total** de todas as decisões.

---

## Stack tecnológica

| Camada | Tecnologia |
|--------|------------|
| Backend | Rust |
| Banco de dados | PostgreSQL |
| Async runtime | Tokio |
| Broker inicial | Interactive Brokers (IBKR) |
| Frontend | `trader-web` (axum + dashboard estático embutido, sem toolchain JS) |
| Deploy | App do umbrelOS no servidor da casa (ADR-013) |

---

## Arquitetura

O projeto segue o padrão **Ports & Adapters** com workspace Cargo:

```text
crates/
├── trader-domain/      # Entidades, enums, traits
├── trader-core/        # Lógica de estratégia, contexto, risco, execução
├── trader-adapters/    # Integrações com broker e data provider
├── trader-infra/       # Banco, config, logging, repositories
├── trader-backtest/    # Engine de backtest
├── trader-cli/         # Interface de linha de comando (entrypoint)
└── trader-web/         # Painel web de status (read-only, ADR-014)
```

Para detalhes, veja [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md).

---

## Documentação

| Documento | Descrição |
|-----------|-----------|
| [`docs/PRD.md`](docs/PRD.md) | Product Requirements Document |
| [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) | Arquitetura de software |
| [`docs/TECHNICAL-ROADMAP.md`](docs/TECHNICAL-ROADMAP.md) | Roadmap técnico e fases |
| [`docs/DATA-MODEL.md`](docs/DATA-MODEL.md) | Modelo de dados PostgreSQL |
| [`docs/OPERATIONS.md`](docs/OPERATIONS.md) | Operação, deploy e runbooks |
| [`docs/SECURITY.md`](docs/SECURITY.md) | Segurança e controles financeiros |
| [`docs/strategy-analysis-framework.md`](docs/strategy-analysis-framework.md) | Processo de criação de estratégias |
| [`docs/cto-plano-lucratividade-2026-09.md`](docs/cto-plano-lucratividade-2026-09.md) | Plano de lucratividade: novas técnicas, estratégias e ativos (pesquisa de 09/2026) |
| [`docs/reports/auditoria-2026-09-07.md`](docs/reports/auditoria-2026-09-07.md) | Auditoria do plano: 174 afirmações verificadas contra código, banco e fontes; 52 correções aplicadas |
| [`docs/reports/roadmap-decisao-2026-09-07.md`](docs/reports/roadmap-decisao-2026-09-07.md) | Baseline de US$ na conta real, marcos de go/no-go e critérios de encerramento |
| [`docs/reports/estudo-politicas-de-saida-2026-09-07.md`](docs/reports/estudo-politicas-de-saida-2026-09-07.md) | Estudo pareado de políticas de saída (breakeven, trailing, parcial) contra alvo fixo + flatten |
| [`docs/reports/gate-a-com-flatten-2026-09-07.md`](docs/reports/gate-a-com-flatten-2026-09-07.md) | **Fonte oficial dos números do gate A:** re-rodada pelo motor com o flatten (ADR-018), o hotfix ET e as métricas do harness (ADR-019). Substitui `gate-a-revalidacao-2026-09-04.md` — runs OOS 725–732 |
| [`docs/strategies/pullback-trend-v1.md`](docs/strategies/pullback-trend-v1.md) | Primeira estratégia do MVP |
| [`docs/decisions/ADR-*.md`](docs/decisions/) | Registro de decisões arquiteturais |
| [`AGENTS.md`](AGENTS.md) | Regras para agentes de IA |

---

## Primeira estratégia

A estratégia inicial é **Pullback em Tendência de Alta (High 2)** baseada em *Trading Price Action Trends*, de Al Brooks.

- **Ativo:** SPY
- **Timeframe operacional:** 15 minutos
- **Timeframe de contexto:** 1 hora
- **Entrada:** buy stop acima da barra de sinal
- **Stop:** abaixo da barra de sinal
- **Alvo:** 2R

Detalhes completos em [`docs/strategies/pullback-trend-v1.md`](docs/strategies/pullback-trend-v1.md).

---

## Como começar

### Pré-requisitos

- Rust 1.80+
- PostgreSQL 15+ ou Docker
- Conta na Interactive Brokers com paper trading ativo

### Passos

```bash
# 1. Clone o repositório
git clone <repo-url>
cd botdaytrade

# 2. Configure o ambiente
cp .env.example .env
# Edite .env com suas credenciais

# 3. Suba o banco
docker-compose up -d postgres

# 4. Rode as migrations
sqlx migrate run

# 5. Compile
cargo build --release

# 6. Teste a conexão com a corretora
cargo run --bin trader-cli -- test-connection

# 7. Inicie o paper trading
cargo run --bin trader-cli -- paper --symbol SPY
```

---

## Comandos previstos

```bash
# Backtest com dados do banco (falha sem dados reais; --allow-synthetic para smoke test)
# Desde o ADR-018 o motor encerra a posição no fim do pregão, como o live: nenhum
# relatório anterior a 07/09/2026 é comparável com os gerados hoje
trader-cli backtest --strategy pullback-trend-v1 --symbol SPY --from 2025-01-01 --to 2025-12-31 --timeframe 15m --slippage-bps 2 --output relatorio.json

# Régua antiga (a posição atravessa a noite): só para medir o delta do flatten
trader-cli backtest --strategy pullback-trend-v1 --symbol SPY --timeframe 15m --no-flatten

# Validação walk-forward out-of-sample (só dados reais)
# O flatten vem ligado por padrão, e --strategy tem default pullback-trend-v1 (desligada
# pelo ADR-016): passe sempre a estratégia e o par que quer medir
trader-cli walkforward --symbol IJS --strategy balance-area-breakout-v1 --from 2025-02-24 --to 2026-09-03 --windows 6

# Harness do ADR-019, ablação: exporta o run, fixa o custo e rotula.
# --label é obrigatório com --set/--strategy-config — sem ele a ablação entra no
# histórico e pode virar baseline do gate B por ordem de chegada
trader-cli walkforward --symbol IJS --strategy balance-area-breakout-v1 -w 6 \
  --slippage-bps 2.5 --output out/ijs-ablacao.json --label ablacao-stop-buffer \
  --set stop_buffer_atr_mult=0.4

# O bloco final travado é OUTRO comando: --holdout-from não convive com
# --set/--strategy-config, porque o holdout roda uma vez por família de hipótese
# e não participa de seleção (ADR-019 §1)
trader-cli walkforward --symbol IJS --strategy balance-area-breakout-v1 -w 6 \
  --slippage-bps 2.5 --output out/ijs.json --holdout-from 2026-06-15

# Abortam de propósito: --set sem --label, chave que não existe na estratégia,
# --set que não muda o config_hash, --set junto de --holdout-from, --holdout-from
# inválido e TOML de --strategy-config cujo id não é o de --strategy
trader-cli walkforward --symbol IJS --strategy balance-area-breakout-v1 -w 6 --strategy-config <toml alternativo> --label variante

# Análise do live/paper vs backtest, com veredito dos critérios de aceitação
# Desde o ADR-019 o baseline é escolhido por (estratégia, par, config_hash) e os trades
# do live são filtrados pelo mesmo par: mudar um parâmetro muda o hash e o run antigo
# deixa de servir — o comando avisa em vez de comparar com outro
trader-cli analyze --symbol IJS --strategy balance-area-breakout-v1

# Paper trading simulado (loop contínuo com candles sintéticos)
trader-cli paper --symbol SPY --strategy pullback-trend-v1 --mode simulated --timeframe 15m

# Paper trading em replay (candles do banco)
trader-cli paper --symbol SPY --strategy pullback-trend-v1 --mode replay --timeframe 15m

# Paper trading live (dados e ordens na conta paper da IBKR)
trader-cli paper --symbol SPY --strategy pullback-trend-v1 --mode live --timeframe 5m

trader-cli ingest --symbol SPY --timeframe 15m
trader-cli status
trader-cli journal --date 2026-07-01
```

> **Régua de fim de pregão:** a janela mora na seção `[session]` de
> `config/default.toml` (`flatten_start = 15:55:00`, `flatten_end = 16:10:00`,
> `last_bar = 15:45:00`, horário de **Nova York**) e é lida pelo live **e** pelo motor de
> backtest — paridade por configuração, não por coincidência (ADR-018). No backtest o
> gatilho é a mudança de data ET, não o relógio: fim da série de candles não é sino, e em
> pregão de meio expediente o dia acaba mais cedo sem ninguém mexer em nada.

---

## Status do projeto

**Estado atual:** MVP de paper trading simulado funcional e auditável.

- [x] Planejamento arquitetural
- [x] Fundação do workspace Rust
- [x] Correção arquitetural: ports em `trader-domain`, crate `trader-core` criado
- [x] Domínio e infraestrutura base (PostgreSQL, repositories)
- [x] Motor de contexto de mercado (`MarketContextAnalyzer`)
- [x] Primeira estratégia (`pullback-trend-v1`) com configuração e `market_snapshot`
- [x] Gestão de risco (`RiskManager`)
- [x] Paper trading simulado e replay de candles do banco
- [x] Backtest com slippage, Sharpe e carregamento do banco
- [x] Paridade de fim de pregão: o motor encerra no sino, como o live (`ExitReason::EndOfDay`, seção `[session]`, flag `--no-flatten` — ADR-018)
- [x] Harness de validação: `--output`, `--slippage-bps`, `--label`, `--holdout-from`, `--strategy-config` e `--set` no walk-forward, mais as métricas de gate (PF em R, t-stat, concentração por dia e por mês, saídas por motivo — ADR-019)
- [x] Comandos `status` e `journal`
- [x] Integração com IBKR via TWS API validada com conta paper (conexão, conta, posições, ordens abertas)
- [x] Paper trading live contra a conta paper da IBKR (ordens bracket server-side)
- [x] Dashboard (`trader-web`: painel read-only servido pelo app do umbrelOS — ADR-014)
- [ ] Gate estatístico do ADR-019 §7 adotado (dos cinco critérios propostos, o CLI imprime dois — PF em R ≥ 1,2 e 2 melhores meses ≤ 60% — sob o rótulo **proposta**; o IC do PF em blocos depende do ferramental ainda pendente. O gate que vale continua sendo o do ADR-010)
- [ ] Dimensionamento por liquidez e fração de capital (ADR-020 — **proposto**, não implementado)
- [ ] Operação real (futuro)

> **Nota:** A integração com Interactive Brokers (`IbkrBrokerAdapter`) está validada em conta paper: envio/cancelamento de ordens, resumo de conta, posições e ordens abertas funcionam. A subscrição de eventos de fill (`subscribe_order_events`) ainda é um stub controlado — exige um `Client` persistente no adapter (decisão arquitetural pendente).

---

## Licença

[Definir]

---

## Aviso de risco

Trading envolve risco significativo de perda. Este software é fornecido para fins educacionais e de pesquisa. Nenhum resultado passado garante resultado futuro. **Não opere dinheiro real sem validação extensiva em paper trading.**
