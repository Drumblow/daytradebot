# ADR-008: Paper Trading com Replay de Candles do Banco

**Status:** Aprovado  
**Data:** 2026-07-02  
**Autor:** CTO

---

## Contexto

O comando `paper` do HumanStyle Trader Bot precisa operar em ambiente simulado antes da integração real com a Interactive Brokers. Inicialmente, o comando usava apenas candles sintéticos gerados em memória, o que limitava a validação da estratégia em dados mais realistas e impedia a persistência de um histórico auditável.

Era necessário um modo de paper trading que:

1. Não dependesse de conta na corretora liberada.
2. Permitisse testar a estratégia com dados históricos armazenados no PostgreSQL.
3. Mantivesse a mesma lógica de execução usada no backtest e no live futuro.
4. Persistisse sinais, ordens, trades e contexto para auditoria.

## Decisão

Adotar dois modos de execução para o comando `trader-cli paper`:

- **`simulated`**: gera candles sintéticos em memória e opera em loop contínuo.
- **`replay`**: carrega candles históricos do PostgreSQL e os alimenta à estratégia em ordem cronológica, simulando o tempo real.

Ambos os modos usam o mesmo `ExecutionEngine`, `RiskManager` e `SimulatedBroker`, garantindo consistência com o backtest e com o futuro live trading.

## Motivos

- **Validação sem risco**: permite testar a estratégia com dados reais antes de enviar ordens para a corretora.
- **Auditabilidade**: todo sinal, ordem, trade e contexto é persistido no banco.
- **Consistência**: backtest, replay e live compartilham a mesma lógica de execução.
- **Preparação para IBKR**: o mesmo loop de paper pode ser reconectado ao `IbkrMarketDataProvider` quando a conta for liberada.

## Consequências

- O comando `paper` depende de `DATABASE_URL` no modo `replay`.
- A ingestão de dados históricos (`trader-cli ingest`) passa a ser pré-requisito para o modo `replay`.
- O loop contínuo requer mecanismo de shutdown gracioso (Ctrl+C).
- Dados de replay não são perfeitos: não reproduzem slippage real nem latência de mercado.

## Decisões relacionadas

- ADR-002: PostgreSQL como datastore.
- ADR-005: Estratégias como plugins via trait.
- `docs/TECHNICAL-ROADMAP.md`
