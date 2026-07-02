# ADR-001: Backend em Rust

**Status:** Aprovado  
**Data:** 2026-07-02  
**Autor:** Software Architect  

---

## Contexto

O sistema é um robô trader que processa dados de mercado, executa regras de estratégia e envia ordens. Precisa de performance previsível, segurança de memória e precisão financeira.

## Decisão

Desenvolver o backend inteiramente em **Rust**, usando workspace com múltiplos crates.

## Alternativas consideradas

| Alternativa | Prós | Contras |
|-------------|------|---------|
| Python | Rápido de prototipar, ecossistema de dados | GIL, tipagem fraca, difícil garantir precisão e performance |
| Node.js/TypeScript | Familiaridade comum | GC imprevisível, tipagem menos rigorosa, não ideal para finanças |
| Go | Concorrência simples, compilação rápida | Menos rigoroso com tipos numéricos, ecossistema financeiro menor |
| Java/Kotlin | Maduro, forte | JVM com GC, overhead, menos adequado para binário único |
| C++ | Performance máxima | Segurança de memória manual, curva íngreme, bugs caros |

## Justificativa

- **Performance previsível:** sem garbage collector, latência mais determinística.
- **Segurança de memória:** elimina classes inteiras de bugs críticos em tempo de execução.
- **Concorrência segura:** ownership e borrowing evitam data races em código assíncrono.
- **Tipagem forte:** `Decimal` para dinheiro, enums para estados, `chrono` para tempo.
- **Ecossistema:** `tokio`, `sqlx`, `serde`, `tracing`, `rust_decimal` atendem todos os domínios do projeto.
- **Binário único:** fácil deploy e versionamento.

## Consequências

- Curva de aprendizado maior para desenvolvedores não familiarizados com Rust.
- Build inicial mais lento que linguagens interpretadas.
- Menor oferta de bibliotecas de trading prontas (ex.: não há equivalente maduro ao pandas), o que exige implementação própria de indicadores.

## Decisões relacionadas

- ADR-004: Workspace com múltiplos crates.
- ADR-005: Estratégias como plugins via trait.
