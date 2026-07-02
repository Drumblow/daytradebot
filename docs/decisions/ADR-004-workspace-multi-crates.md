# ADR-004: Workspace com múltiplos crates

**Status:** Aprovado  
**Data:** 2026-07-02  
**Autor:** Software Architect  

---

## Contexto

O sistema possui domínios bem separados: regras de trading, integração com broker, persistência, analytics e interfaces de usuário. É necessário organizar o código para evitar dependências cíclicas e facilitar testes.

## Decisão

Usar um **workspace Cargo** com crates internas especializadas.

## Estrutura

```text
crates/
├── trader-domain/      # Entidades e traits
├── trader-core/        # Lógica de estratégia, risco, execução
├── trader-adapters/    # Implementações de broker e data provider
├── trader-infra/       # DB, config, logging, event bus
├── trader-journal/     # Diário e analytics
├── trader-backtest/    # Engine de backtest
└── trader-cli/         # Entrypoint
```

## Justificativa

- **Separação de responsabilidades:** cada crate tem um propósito claro.
- **Testabilidade:** `trader-core` pode ser testado sem banco ou broker.
- **Build incremental:** alterações em um crate não recompilam todo o projeto.
- **Dependências controladas:** `trader-domain` não depende de async, sqlx ou HTTP.
- **Portabilidade:** trocar um adapter não afeta o core.

## Consequências

- Overhead inicial de configuração de workspace.
- Necessidade de disciplina para não criar dependências cíclicas.
- Publicação interna de crates requer versionamento cuidadoso.

## Decisões relacionadas

- ADR-001: Backend em Rust.
- `docs/ARCHITECTURE.md`.
