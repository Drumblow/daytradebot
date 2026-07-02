# ADR-002: PostgreSQL como datastore

**Status:** Aprovado  
**Data:** 2026-07-02  
**Autor:** Software Architect  

---

## Contexto

O robô precisa armazenar grandes volumes de dados históricos (candles), decisões (sinais, ordens, trades) e contextos de mercado para auditoria, backtest e analytics.

## Decisão

Usar **PostgreSQL** como banco de dados principal.

## Alternativas consideradas

| Alternativa | Prós | Contras |
|-------------|------|---------|
| SQLite | Simples, sem servidor | Concorrência limitada, não ideal para múltiplos componentes |
| MySQL | Popular, bom desempenho | Menos rico em tipos avançados, JSON menos maduro |
| MongoDB | Flexível, schema-less | Consistência eventual, menos adequado para dados financeiros relacionais |
| TimescaleDB/Timescale | Otimizado para séries temporais | Adiciona complexidade; PostgreSQL puro é suficiente no MVP |
| InfluxDB | Série temporal nativa | Menos adequado para dados relacionais (ordens, trades) |

## Justificativa

- **Relacionalidade:** candles, ordens, fills, trades e contextos têm relações claras.
- **SQL puro:** queries auditáveis, análises ad-hoc fáceis.
- **JSONB:** flexibilidade para snapshots e metadados sem perder a estrutura relacional.
- **Ecossistema Rust:** `sqlx` permite queries verificadas em compile time.
- **Maturidade:** ACID, transações, backups, replicação bem estabelecidos.
- **Custo:** pode rodar localmente, em Docker ou em serviços gratuitos (Oracle Free Tier).

## Consequências

- Requer modelagem cuidadosa do schema.
- Escalabilidade horizontal limitada (mas não é necessária no MVP).
- Pode ser necessário particionamento futuro para grandes volumes de candles.

## Decisões relacionadas

- ADR-001: Backend em Rust.
- `docs/DATA-MODEL.md`.
