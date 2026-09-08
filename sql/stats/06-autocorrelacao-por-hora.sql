-- 06-autocorrelacao-por-hora.sql — ACF(1) dos retornos de 15m por hora do dia (small caps:
-- exclui SPY/QQQ/MDY). Bloco A: Pearson, desvio e |retorno| médio. Bloco B: Spearman (por ranks),
-- 2025 vs 2026 e % de barras que revertem o sinal da anterior.
-- Só pares de barras do MESMO dia ET. Origem: leitor de banco de 06/09/2026 (q5d_acf_hour, q9b_spearman_hour).
-- Uso: docker exec -i trader-postgres psql -U trader -d trader_db < sql/stats/06-autocorrelacao-por-hora.sql

-- A. Pearson
WITH b AS (
 SELECT a.symbol, c.timestamp ts, (c.timestamp AT TIME ZONE 'America/New_York')::date d, (c.timestamp AT TIME ZONE 'America/New_York')::time t, c.close,
   c.close/lag(c.close) OVER (PARTITION BY a.symbol ORDER BY c.timestamp)-1 r,
   lag((c.timestamp AT TIME ZONE 'America/New_York')::date) OVER (PARTITION BY a.symbol ORDER BY c.timestamp) pd
 FROM candles c JOIN assets a ON a.id=c.asset_id WHERE c.timeframe='15m' AND a.symbol NOT IN ('SPY','QQQ','MDY')
), r AS (
 SELECT symbol, d, t, r, lag(r) OVER (PARTITION BY symbol ORDER BY ts) r1, lag(pd) OVER (PARTITION BY symbol ORDER BY ts) pd1
 FROM b WHERE pd=d
)
SELECT t bar_open_et, count(*) n, round(corr(r, r1)::numeric,4) acf1_smallcaps,
 round((100*stddev(r))::numeric,4) sd_ret_pct, round((100*avg(abs(r)))::numeric,4) avg_abs_ret_pct
FROM r WHERE r1 IS NOT NULL AND pd1=d
GROUP BY 1 ORDER BY 1;

-- B. Spearman, por ano, e % de reversão
WITH b AS (
 SELECT a.symbol, c.timestamp ts, (c.timestamp AT TIME ZONE 'America/New_York')::date d, (c.timestamp AT TIME ZONE 'America/New_York')::time t,
   c.close/lag(c.close) OVER (PARTITION BY a.symbol ORDER BY c.timestamp)-1 r,
   lag((c.timestamp AT TIME ZONE 'America/New_York')::date) OVER (PARTITION BY a.symbol ORDER BY c.timestamp) pd
 FROM candles c JOIN assets a ON a.id=c.asset_id WHERE c.timeframe='15m' AND a.symbol NOT IN ('SPY','QQQ','MDY')
), r AS (
 SELECT symbol, d, t, r, lag(r) OVER (PARTITION BY symbol ORDER BY ts) r1, lag(pd) OVER (PARTITION BY symbol ORDER BY ts) pd1 FROM b WHERE pd=d
), x AS (
 SELECT *, rank() OVER (PARTITION BY t ORDER BY r) rk, rank() OVER (PARTITION BY t ORDER BY r1) rk1,
  rank() OVER (PARTITION BY t, (d>='2026-01-01') ORDER BY r) rky, rank() OVER (PARTITION BY t, (d>='2026-01-01') ORDER BY r1) rky1
 FROM r WHERE r1 IS NOT NULL AND pd1=d
)
SELECT t bar_open_et, count(*) n, round(corr(rk,rk1)::numeric,3) spearman_acf1,
 round(corr(rky,rky1) FILTER (WHERE d<'2026-01-01')::numeric,3) sp_2025, round(corr(rky,rky1) FILTER (WHERE d>='2026-01-01')::numeric,3) sp_2026,
 round(100*avg(CASE WHEN r1<>0 AND r<>0 THEN (sign(r)=-sign(r1))::int END),1) reversal_pct
FROM x GROUP BY 1 ORDER BY 1;
