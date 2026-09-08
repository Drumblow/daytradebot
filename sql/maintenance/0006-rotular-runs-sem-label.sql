-- Dá rótulo aos `backtest_runs` que nasceram sem nenhum (§5.6 do plano).
--
-- POR QUE: 586 dos 745 runs do banco dev têm `label = NULL`. A causa é o
-- comando `backtest`, que gravava `label: None` SEMPRE — o `--label` existe
-- só no `walkforward`, desde o ADR-019. A causa foi corrigida em 08/09/2026:
-- o `backtest` ganhou `--label` e, sem ele, grava `backtest-<data>` em vez de
-- NULL. Este script trata o passivo.
--
-- O QUE O RÓTULO AFIRMA: apenas a data em que o run foi criado. **Não**
-- afirma para que ele serviu — isso exigiria inspecionar run a run, e um
-- rótulo com propósito inventado é pior que rótulo nenhum. Quem quiser
-- atribuir propósito tem `created_at`, `strategy_id`, `config_hash` e
-- `metrics` para cruzar com os relatórios em `docs/reports/`.
--
-- SEGURANÇA: escreve só a coluna `label`. Não toca em métricas, equity nem
-- trades. `label` não participa da seleção de baseline do gate B
-- (`latest_for` filtra por estratégia, par, `config_hash` e `experimental`),
-- então isto não muda veredito nenhum — é rastreabilidade. Idempotente.
--
-- Rodar no banco DEV:
--   docker exec -i trader-postgres psql -U trader -d trader_db \
--     -f - < sql/maintenance/0006-rotular-runs-sem-label.sql
--
-- No banco de PRODUÇÃO é decisão do dono, como o 0004 e o 0005.

BEGIN;

SELECT count(*) AS runs_sem_label_antes
FROM backtest_runs
WHERE label IS NULL;

UPDATE backtest_runs
SET label = 'retro-' || to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD')
WHERE label IS NULL;

SELECT count(*) AS runs_sem_label_depois
FROM backtest_runs
WHERE label IS NULL;

SELECT label, count(*)
FROM backtest_runs
WHERE label LIKE 'retro-%'
GROUP BY label
ORDER BY label;

COMMIT;
