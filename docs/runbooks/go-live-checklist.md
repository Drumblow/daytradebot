# Runbook — Checklist para operar com dinheiro real

Gate formal (ADR-010, que substituiu o "mínimo 3 meses" de docs/OPERATIONS.md): **gate composto** — validação estatística com histórico + 4 semanas de paper live + aprovação documentada. Este checklist operacionaliza o gate.

## 1. Validação da estratégia (pode ser feita agora, com histórico)

- [ ] Backtest com dados reais ≥ 6 meses: ≥ 50 trades, win rate ≥ 40%, PF ≥ 1.3, DD ≤ 10%, avg R > 0.15
      (`cargo run -p trader-cli -- ingest --symbol SPY --days 365 --provider ibkr` antes;
      depois `backtest --from YYYY-MM-DD`)
- [ ] Walk-forward OOS com métricas sustentadas fora da amostra
      (`cargo run -p trader-cli -- walkforward --windows 4`)
- [ ] Resultados registrados em `docs/strategies/pullback-trend-v1.md` (checklist de validação)

## 2. Validação operacional (4 semanas, tempo real)

- [ ] 4 semanas de paper live contínuo, uptime ≥ 99%, sem circuit breaker
- [ ] ≥ 20 trades em paper dentro de ±30% das métricas do backtest
      (`cargo run -p trader-cli -- analyze`)
- [ ] Zero violações de risco (todo trade com stop; limites diários respeitados)
- [ ] Restart no meio do pregão testado: estado de risco e ordens recuperados do banco
- [ ] Alertas configurados e testados (`[alerts].webhook_url`)
- [ ] Reconciliação semanal bot vs IBKR sem divergências (posições + ordens)
- [ ] Se houver circuit breaker ou divergência: corrigir e reiniciar a contagem das 4 semanas

## 3. Segurança

- [ ] `app.mode` continua "paper" até a data de go-live planejada
- [ ] Revisão do código de guardas: `paper.rs` (bail em modo real/porta real), `risk/mod.rs` (NotInPaperMode)
- [ ] Sem credenciais no repositório; `.env` fora do git
- [ ] Limites de risco revisados em `config/default.toml` (risco 1%, diário 2%, 3 trades/dia)

## 4. Go-live

- [ ] Tamanho de posição mínimo no primeiro mês (reduzir `risk_per_trade_pct` para 0.25–0.5%)
- [ ] Acompanhamento manual das 5 primeiras sessões
- [ ] ADR registrando a decisão de go-live e os resultados do paper
- [ ] Plano de rollback: voltar `app.mode` para "paper" e cancelar ordens abertas
