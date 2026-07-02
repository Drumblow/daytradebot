# Fase 2 — Conexão com dados e broker (paper)

**Status:** ✅ Concluída (com stubs controlados para operações de conta)  
**Data de conclusão:** 2026-07-02  

---

## Objetivo

Estabelecer comunicação bidirecional com a Interactive Brokers em paper trading, criando a camada de adapters e o entrypoint CLI.

---

## Entregáveis

### `trader-adapters`

Novo crate com integrações externas:

- ✅ `ibkr::IbkrMarketDataProvider` — candles históricos, realtime bars (5s), cotação, health check.
- ✅ `ibkr::IbkrBrokerAdapter` — envio de ordens market/limit/stop/bracket, cancelamento.
- ✅ `simulated::SimulatedBroker` — broker em memória para testes e paper trading.
- ✅ `simulated::SimulatedMarketDataProvider` — provedor de dados em memória para testes.

### `trader-cli`

Novo binário CLI com comandos:

- ✅ `test-connection [--provider ibkr|simulated]`
- ✅ `account [--provider ibkr|simulated]`
- ✅ `ingest --symbol <SYMBOL> --timeframe <TF> --days <N> [--provider ibkr|simulated]`
- ✅ `paper --symbol <SYMBOL> --strategy <STRATEGY> [--mode simulated|replay]`
- ✅ `backtest --symbol <SYMBOL> --strategy <STRATEGY> --from <DATE> --to <DATE>`
- ✅ `status`
- ✅ `journal --date <DATE>`

### Configuração

- ✅ `config/default.toml` com seção `[ibkr]` para TWS API.
- ✅ `.env.example` com variáveis `TRADER__IBKR__*` e `TRADER_PROVIDER`.
- ✅ `trader-infra/src/config/mod.rs` com `IbkrSettings`.

### Decisões

- ✅ ADR-007 registrando a escolha da TWS API/IB Gateway via crate `ibapi` v3.x.

---

## Limitações conhecidas

A conta na Interactive Brokers ainda **não está liberada**. Por isso:

- As operações de `IbkrBrokerAdapter` que dependem de conta/posições (`get_open_orders`, `get_positions`, `get_account_summary`, `subscribe_order_events`) estão como **stubs controlados** que retornam valores vazios, zeros ou avisos.
- O envio de ordens reais para a IBKR (`place_order`, `cancel_order`) está codificado com o crate `ibapi`, mas só poderá ser validado quando a conta for liberada.
- Todos os testes automatizados usam os adapters **simulados**.
- A validação real com IB Gateway/TWS será feita assim que a conta for liberada.

---

## Como testar

```bash
# Build e testes
cargo build --workspace
cargo test --workspace
cargo clippy --all-targets --all-features -- -D warnings

# Comandos simulados (não requerem conta IBKR)
cargo run --bin trader-cli -- test-connection --provider simulated
cargo run --bin trader-cli -- account --provider simulated
cargo run --bin trader-cli -- ingest --symbol SPY --timeframe 15m --days 7 --provider simulated
cargo run --bin trader-cli -- paper --symbol SPY --strategy pullback-trend-v1 --mode simulated
cargo run --bin trader-cli -- paper --symbol SPY --strategy pullback-trend-v1 --mode replay
cargo run --bin trader-cli -- backtest --symbol SPY --strategy pullback-trend-v1 --from 2025-01-01 --to 2025-12-31 --timeframe 15m
cargo run --bin trader-cli -- status
cargo run --bin trader-cli -- journal --date 2026-07-02
```

---

## Próxima fase

**Fase 3 — Motor de contexto de mercado**

Implementar indicadores (EMA/SMA, ATR, volume relativo, máximas/mínimas de swing) e o `MarketContextAnalyzer` para classificar tendência, volatilidade e fase do mercado.
