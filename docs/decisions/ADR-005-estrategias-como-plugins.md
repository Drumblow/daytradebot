# ADR-005: Estratégias como plugins via trait

**Status:** Aprovado  
**Data:** 2026-07-02  
**Autor:** Software Architect  

---

## Contexto

O sistema deve suportar múltiplas estratégias de trading baseadas em livros de Price Action. Cada estratégia deve ser testável isoladamente e deve compartilhar infraestrutura comum (dados, execução, risco).

## Decisão

Implementar estratégias como **plugins que implementam uma trait comum** (`Strategy`).

## Contrato

```rust
pub trait Strategy {
    fn id(&self) -> StrategyId;
    fn name(&self) -> &'static str;
    fn source(&self) -> &'static str;  // livro/capítulo
    fn version(&self) -> &'static str;
    fn analyze(&self, ctx: &MarketContext, state: &StrategyState) -> SignalResult;
}
```

## Justificativa

- **Uniformidade:** backtest, paper e live usam a mesma implementação.
- **Testabilidade:** cada estratégia pode ser testada com candles sintéticos.
- **Auditabilidade:** `source` e `version` rastreiam a origem de cada regra.
- **Extensibilidade:** novas estratégias não alteram o core.
- **Versionamento:** mudanças em regras criam novas versões, sem invalidar histórico.

## Consequências

- Estratégias precisam respeitar o contrato (entradas/saídas padronizadas).
- Lógica de execução e risco permanece fora da estratégia.
- Configurações de estratégia são serializáveis (TOML/JSON) para versionamento.

## Decisões relacionadas

- ADR-001: Backend em Rust.
- ADR-004: Workspace com múltiplos crates.
- `docs/strategy-analysis-framework.md`.
