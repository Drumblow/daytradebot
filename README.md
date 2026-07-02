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
| Frontend (futuro) | Next.js / React / Tailwind |
| Deploy | VPS / Docker (futuro) |

---

## Arquitetura

O projeto segue o padrão **Ports & Adapters** com workspace Cargo:

```text
crates/
├── trader-domain/      # Entidades, enums, traits
├── trader-core/        # Lógica de estratégia, contexto, risco, execução
├── trader-adapters/    # Integrações com broker e data provider
├── trader-infra/       # Banco, config, logging, event bus
├── trader-journal/     # Diário e analytics
├── trader-backtest/    # Engine de backtest
└── trader-cli/         # Interface de linha de comando
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
trader-cli backtest --strategy pullback-trend-v1 --symbol SPY --from 2025-01-01 --to 2025-12-31
trader-cli paper --strategy pullback-trend-v1 --symbol SPY
trader-cli ingest --symbol SPY --timeframe 15m
trader-cli status
trader-cli journal --date 2026-07-01
```

---

## Status do projeto

- [x] Planejamento arquitetural
- [ ] Fundação do workspace Rust
- [ ] Conexão com dados e broker
- [ ] Motor de contexto de mercado
- [ ] Primeira estratégia
- [ ] Paper trading
- [ ] Backtest
- [ ] Dashboard
- [ ] Operação real (futuro)

---

## Licença

[Definir]

---

## Aviso de risco

Trading envolve risco significativo de perda. Este software é fornecido para fins educacionais e de pesquisa. Nenhum resultado passado garante resultado futuro. **Não opere dinheiro real sem validação extensiva em paper trading.**
