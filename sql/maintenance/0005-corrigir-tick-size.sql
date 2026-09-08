-- Corrige o `tick_size` gravado a partir de f64 (§5.6 do plano de
-- lucratividade).
--
-- POR QUE: `ensure_asset` usava `Decimal::from_f64_retain(0.01)`, que grava o
-- binário mais próximo de 0,01 — 0,0100000000000000002081668171... — em TODOS
-- os ativos. É a violação mais literal possível da regra "Decimal, nunca f64,
-- para dinheiro" do AGENTS.md: o tamanho do tique.
--
-- EFEITO PRÁTICO HOJE: nenhum. As estratégias leem o `tick_size` do próprio
-- TOML (`Decimal::from(1) / Decimal::from(100)`, exato), não da tabela
-- `assets`; a coluna nunca é lida pelo motor. A correção é de higiene e para
-- que a divergência não vire bug no dia em que alguém passar a lê-la — o que
-- o log de divergência do `ingest` agora vigia.
--
-- SEGURANÇA: é UPDATE de metadado, não de resultado. Não toca em `trades`,
-- `backtest_runs` nem `candles`. Idempotente.
--
-- Rodar no banco DEV:
--   docker exec -i trader-postgres psql -U trader -d trader_db \
--     -f - < sql/maintenance/0005-corrigir-tick-size.sql
--
-- No banco de PRODUÇÃO (servidor) é decisão do dono, como o 0004.

BEGIN;

-- Antes: quantos estão errados.
SELECT count(*) AS ativos_com_tick_size_de_f64
FROM assets
WHERE tick_size <> 0.01;

UPDATE assets
SET tick_size = 0.01,
    updated_at = now()
WHERE tick_size <> 0.01;

-- Depois: tem de ser zero.
SELECT count(*) AS ainda_errados
FROM assets
WHERE tick_size <> 0.01;

COMMIT;
