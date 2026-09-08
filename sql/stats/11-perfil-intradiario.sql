-- 11-perfil-intradiario.sql — Bloco A: range, corpo, US$ e retorno médios por barra do dia (small
-- caps: exclui SPY/QQQ/MDY). Bloco B: "junção" entre barras consecutivas do mesmo dia — gap médio e
-- p99 entre o fechamento anterior e a abertura seguinte, e % de barras com |retorno| > 1% / > 2%
-- (o que um stop de 1 barra pode atravessar). Fuso America/New_York.
-- Origem: leitor de banco de 06/09/2026 (q5e_profile, q6_junction).
-- Uso: docker exec -i trader-postgres psql -U trader -d trader_db < sql/stats/11-perfil-intradiario.sql

-- A. perfil por barra
SELECT (c.timestamp AT TIME ZONE 'America/New_York')::time bar_open_et, count(*) n,
 round(100*avg((c.high-c.low)/c.close),4) avg_range_pct_smallcaps,
 round(100*avg(abs(c.close-c.open)/c.open),4) avg_abs_body_pct,
 round(avg(c.volume*c.close)/1e6,2) avg_dollar_vol_M,
 round(100*avg((c.close-c.open)/c.open),4) avg_ret_pct
FROM candles c JOIN assets a ON a.id=c.asset_id
WHERE c.timeframe='15m' AND a.symbol NOT IN ('SPY','QQQ','MDY')
GROUP BY 1 ORDER BY 1;

-- B. junção entre barras
WITH b AS (
 SELECT a.symbol, c.timestamp ts, (c.timestamp AT TIME ZONE 'America/New_York')::date d, (c.timestamp AT TIME ZONE 'America/New_York')::time t, c.open, c.close,
   lag(c.close) OVER (PARTITION BY a.symbol ORDER BY c.timestamp) pc,
   lag((c.timestamp AT TIME ZONE 'America/New_York')::date) OVER (PARTITION BY a.symbol ORDER BY c.timestamp) pd
 FROM candles c JOIN assets a ON a.id=c.asset_id WHERE c.timeframe='15m' AND a.symbol NOT IN ('SPY','QQQ','MDY')
)
SELECT t bar_open_et, count(*) n,
 round(100*avg(abs(open-pc)/pc),4) avg_junction_gap_pct,
 round((100*percentile_cont(0.99) WITHIN GROUP (ORDER BY abs(open-pc)/pc))::numeric,3) p99_junction_gap_pct,
 round(100*avg((abs(close/pc-1)>0.01)::int),2) pct_bars_ret_gt1pct,
 round(100*avg((abs(close/pc-1)>0.02)::int),3) pct_bars_ret_gt2pct
FROM b WHERE pd=d GROUP BY 1 ORDER BY 1;
