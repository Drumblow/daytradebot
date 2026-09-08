-- 0005: índice para a busca de baseline do gate B (ADR-019 §3).
--
-- `analyze` passou a escolher o run de referência por
-- (strategy_id, símbolo, config_hash) em vez de "o mais recente da
-- estratégia, em qualquer par e qualquer config" — que era como a primeira
-- ablação virava baseline do gate B em silêncio.
--
-- O índice da 0003 é `(strategy_id, created_at DESC)` e não cobre a chave
-- nova. São ~690 runs hoje, então o seq scan não dói; o índice existe para o
-- caminho não piorar quando o harness multiplicar as ablações (que é
-- justamente o que o ADR-019 torna barato).

CREATE INDEX IF NOT EXISTS idx_backtest_runs_baseline
    ON backtest_runs (strategy_id, asset_id, config_hash, created_at DESC);

-- O que esta migração NÃO faz: índice ÚNICO e dedupe.
--
-- O plano (§5.6) e o ADR-019 §3 pedem um índice único em
-- (strategy_id, asset_id, config_hash, period_start, period_end, label) com
-- dedupe prévio. A auditoria de 07/09 mediu o que aconteceria: dentro do
-- maior grupo duplicado (24 linhas) existem SETE valores distintos de
-- `final_equity`. Não são re-execuções idênticas — o `config_hash` cobre só o
-- TOML da estratégia, e não a versão do motor, o slippage nem a régua de fim
-- de sessão. Deduplicar por essa chave apagaria resultados diferentes entre
-- si e destruiria a única evidência de que o motor mudou.
--
-- A partir deste ADR os runs novos gravam `slippage_bps`, `session_flatten`,
-- `experimental` e `overrides` no jsonb `metrics`, então a identidade passa a
-- ser recuperável. O dedupe e o índice único ficam para quando existir uma
-- chave que de fato identifique um run — e o dono tiver aprovado apagar
-- linhas (a migração seria destrutiva).
