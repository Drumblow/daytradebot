-- 07-autocorrelacao-por-simbolo.sql — Bloco A: ACF(1) e ACF(2) dos retornos de 15m por símbolo
-- (só pares do mesmo dia ET), % de barras com mesmo sinal / sinal oposto. Bloco B: ACF diária e
-- intradiária (open→close) robusta — Spearman, ex mar–abr/2025, só 2026 — e correlação gap × intradia.
-- Origem: leitor de banco de 06/09/2026 (q5c_acf, q7c_daily_acf_robust).
-- Uso: docker exec -i trader-postgres psql -U trader -d trader_db < sql/stats/07-autocorrelacao-por-simbolo.sql

-- A. intradiário, 15m
WITH b AS (
 SELECT a.symbol, c.timestamp ts, (c.timestamp AT TIME ZONE 'America/New_York')::date d, c.close,
   c.close/lag(c.close) OVER (PARTITION BY a.symbol ORDER BY c.timestamp)-1 r,
   lag((c.timestamp AT TIME ZONE 'America/New_York')::date) OVER (PARTITION BY a.symbol ORDER BY c.timestamp) pd
 FROM candles c JOIN assets a ON a.id=c.asset_id WHERE c.timeframe='15m'
), r AS (
 SELECT symbol, d, r,
   lag(r) OVER (PARTITION BY symbol ORDER BY ts) r1, lag(pd) OVER (PARTITION BY symbol ORDER BY ts) pd1,
   lag(r,2) OVER (PARTITION BY symbol ORDER BY ts) r2, lag(pd,2) OVER (PARTITION BY symbol ORDER BY ts) pd2
 FROM b WHERE pd=d
)
SELECT symbol, count(*) n,
 round(corr(r, r1)::numeric,4) acf1,
 round(corr(r, r2)::numeric,4) acf2,
 round((100*stddev(r))::numeric,4) sd_ret_pct,
 round(100*avg(CASE WHEN r1<>0 THEN (sign(r)=sign(r1))::int END),1) same_sign_pct,
 round(100*avg(CASE WHEN r1<>0 THEN (sign(r)=-sign(r1))::int END),1) opp_sign_pct
FROM r WHERE r1 IS NOT NULL AND pd1=d AND pd2=d
GROUP BY 1 ORDER BY acf1;

-- B. diário e intradiário (open→close), robusto
WITH day AS (
 SELECT a.symbol, (c.timestamp AT TIME ZONE 'America/New_York')::date d,
  (array_agg(c.open ORDER BY c.timestamp))[1] o, (array_agg(c.close ORDER BY c.timestamp DESC))[1] cl
 FROM candles c JOIN assets a ON a.id=c.asset_id WHERE c.timeframe='15m' GROUP BY 1,2
), r AS (
 SELECT symbol, d, cl/lag(cl) OVER (PARTITION BY symbol ORDER BY d)-1 rd, (cl-o)/o rid,
  lag((cl-o)/o) OVER (PARTITION BY symbol ORDER BY d) rid1,
  (o/lag(cl) OVER (PARTITION BY symbol ORDER BY d)-1) gap
 FROM day
), x AS (SELECT *, lag(rd) OVER (PARTITION BY symbol ORDER BY d) rd1,
   rank() OVER (PARTITION BY symbol ORDER BY rid) rk, rank() OVER (PARTITION BY symbol ORDER BY rid1) rk1 FROM r WHERE rid1 IS NOT NULL)
SELECT symbol, count(*) n,
 round(corr(rid, rid1)::numeric,3) intraday_acf1,
 round(corr(rk, rk1)::numeric,3) intraday_acf1_spearman,
 round(corr(rid, rid1) FILTER (WHERE d NOT BETWEEN '2025-03-01' AND '2025-04-30')::numeric,3) intraday_acf1_ex_mar_apr25,
 round(corr(rid, rid1) FILTER (WHERE d >= '2026-01-01')::numeric,3) intraday_acf1_2026,
 round(corr(rd, rd1) FILTER (WHERE d >= '2026-01-01')::numeric,3) daily_acf1_2026,
 round(corr(gap, rid)::numeric,3) corr_gap_intraday,
 round(corr(gap, rid) FILTER (WHERE d >= '2026-01-01')::numeric,3) corr_gap_intraday_2026,
 round(corr(rid1, gap)::numeric,3) corr_prev_intraday_vs_gap
FROM x WHERE rd1 IS NOT NULL GROUP BY 1 ORDER BY 1;
