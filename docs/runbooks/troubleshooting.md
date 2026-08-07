# Runbook — Troubleshooting

## Bot não opera (nenhum sinal há muito tempo)

1. É horário de pregão (14:30–21:00 UTC, config da estratégia)?
2. `status`/`journal`: há sinais rejeitados? `analyze` mostra a distribuição de motivos.
3. Limite diário atingido? Veja `daily_trades`/`daily_pnl` no log de boot ("estado de risco reconstruído").
4. Mercado sem tendência de alta → rejeição `NoContext` é comportamento esperado.

## Falha ao buscar candles / reconectar na IBKR

- Sintoma: warns "falha ao buscar candles na IBKR". O ciclo é pulado e retentado.
- 10 falhas consecutivas → circuit breaker encerra o live (com alerta).
- Causas comuns: IB Gateway deslogado, reinício diário do gateway, firewall, `client_id` em conflito com outra sessão.

## Ordem enviada mas fill não aparece no banco

- O polling de execuções roda a cada 15s; aguarde um ciclo.
- Verifique na TWS se a ordem foi executada de fato (limit de entrada pode não pegar).
- Fills de outras contas/símbolos são ignorados por design (log "fill de outro símbolo ignorado").

## Restart perdeu o trade aberto?

- Não deveria: no boot, o bot recupera ordens abertas do banco e re-lê os fills.
- Se a ordem foi colocada **antes** desta versão (sem persistência de ordem), o fill de saída será logado como "fill sem ordem rastreada" e ignorado — feche/audite manualmente na TWS.

## Backtest sem dados

- `backtest` falha sem candles reais (a menos que `--allow-synthetic`).
- Rode `trader-cli ingest --symbol SPY --days 180 --provider ibkr`.
- Verifique qualidade: tabela `ingestions` (gaps_detected) por execução de ingest.

## Banco indisponível

- Modo live **não sobe** sem banco (falha fechada — auditoria obrigatória).
- Modos simulated/replay seguem sem persistência, com aviso.
- `docker compose up -d postgres` e confira `DATABASE_URL`.
