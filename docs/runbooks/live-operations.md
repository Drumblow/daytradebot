# Runbook — Operação do live paper (IBKR)

## Pré-requisitos

1. Postgres no ar: `docker compose up -d postgres`.
2. IB Gateway/TWS aberto, logado na conta **paper**, API habilitada (porta 7497 TWS / 4002 Gateway).
3. `.env` com `DATABASE_URL` apontando para o Postgres.
4. `config/default.toml`: `app.mode = "paper"`, `ibkr.paper = true`, `ibkr.port = 7497` (ou 4002).

O bot **falha no boot** se: banco indisponível, `ibkr.paper = false`, ou porta de conta real (7496/4001). Isso é intencional — falha fechada.

## Subir

```bash
cargo run -p trader-cli -- paper --mode live --symbol SPY
```

No boot o bot:
- roda as migrações do banco;
- reconstrói o estado de risco do dia (P&L, trades, perdas consecutivas) a partir do banco;
- recupera ordem em aberto de sessão anterior (se houver) e religa o rastreamento de fills;
- sincroniza o cursor de candles (não opera setups antigos).

## Monitorar

- Logs: saída do processo (pretty) — sinais, rejeições, ordens, trades.
- `cargo run -p trader-cli -- status` — últimos sinais/trades.
- `cargo run -p trader-cli -- journal` — trades do dia + P&L.
- `cargo run -p trader-cli -- analyze` — métricas do live vs backtest + critérios de aceitação.
- Banco: `signals`, `orders`, `fills`, `trades`, `system_events`.

## Alertas

Configure `[alerts].webhook_url` (Slack/Discord/Teams) para receber:
- início/encerramento do live;
- trade fechado (com P&L);
- circuit breaker (10 falhas consecutivas de dados/reconciliação → o live encerra com erro).

## Parar

`Ctrl+C` — shutdown gracioso. Stop e alvo das posições abertas ficam **server-side na IBKR** (bracket): posições abertas continuam protegidas mesmo com o bot desligado.

## Restart no meio do pregão

Seguro. O bot reconstrói limites diários do banco e reconecta o rastreamento de fills. Fills já persistidos nunca são contados em dobro (dedupe por `broker_fill_id`).

## Circuit breaker

Se o bot encerrar com `circuit breaker: ...`:
1. Verifique se o IB Gateway está no ar e logado.
2. Verifique conectividade (`cargo run -p trader-cli -- test-connection --provider ibkr`).
3. Veja `system_events` para o histórico de falhas.
4. Corrija a causa e suba de novo — o estado se recupera do banco.
