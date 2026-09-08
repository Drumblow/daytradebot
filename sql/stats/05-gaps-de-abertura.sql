-- 05-gaps-de-abertura.sql — gap de abertura (média, p50, p90), % de gaps preenchidos no dia,
-- % de dias com gap > 50 bp e correlação gap × retorno intradiário, por símbolo.
-- Só dias completos (>= 20 barras) que começam às 09:30 ET.
-- Fuso America/New_York. Origem: leitor de banco da pesquisa de 06/09/2026 (q5_gaps).
-- Uso: docker exec -i trader-postgres psql -U trader -d trader_db < sql/stats/05-gaps-de-abertura.sql
WITH b AS (
 SELECT a.symbol, c.timestamp AT TIME ZONE 'America/New_York' ts, (c.timestamp AT TIME ZONE 'America/New_York')::date d, c.open, c.high, c.low, c.close
 FROM candles c JOIN assets a ON a.id=c.asset_id WHERE c.timeframe='15m'
), day AS (
 SELECT symbol, d, count(*) nb,
  (array_agg(open ORDER BY ts))[1] o, (array_agg(close ORDER BY ts DESC))[1] cl,
  max(high) hi, min(low) lo,
  (array_agg(ts ORDER BY ts))[1]::time t0
 FROM b GROUP BY 1,2
), dd AS (
 SELECT *, lag(cl) OVER (PARTITION BY symbol ORDER BY d) pcl FROM day
)
SELECT symbol, count(*) n,
 round(100*avg(abs(o-pcl)/pcl),3) gap_mean,
 round((100*percentile_cont(0.5) WITHIN GROUP (ORDER BY abs(o-pcl)/pcl))::numeric,3) gap_p50,
 round((100*percentile_cont(0.9) WITHIN GROUP (ORDER BY abs(o-pcl)/pcl))::numeric,3) gap_p90,
 round(100*avg((o>pcl)::int),1) gap_up_pct,
 round(100*avg(CASE WHEN o>pcl THEN (lo<=pcl)::int WHEN o<pcl THEN (hi>=pcl)::int END),1) gap_fill_pct,
 round(100*avg(CASE WHEN abs(o-pcl)/pcl>0.005 THEN (CASE WHEN o>pcl THEN (lo<=pcl)::int ELSE (hi>=pcl)::int END) END),1) fill_pct_gap_gt50bp,
 round(100*avg(CASE WHEN abs(o-pcl)/pcl>0.005 THEN 1 ELSE 0 END),1) pct_days_gap_gt50bp,
 round(corr((o-pcl)/pcl, (cl-o)/o)::numeric,3) corr_gap_vs_intraday
FROM dd WHERE pcl IS NOT NULL AND nb>=20 AND t0='09:30'
GROUP BY 1 ORDER BY 1;
