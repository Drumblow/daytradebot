-- 09-initial-balance.sql — quanto do range diário a 1ª barra (09:30) e a 1ª hora (09:30–10:29 ET,
-- Initial Balance de Dalton) contêm; em quantos dias a 1ª hora faz a máxima/mínima do dia; em
-- quantos o resto do dia rompe os dois lados ou fica inteiro dentro; correlação 1ª hora × resto.
-- Só dias completos (>= 20 barras) que começam às 09:30 ET. Fuso America/New_York.
-- Origem: leitor de banco de 06/09/2026 (q5b_orb).
-- Uso: docker exec -i trader-postgres psql -U trader -d trader_db < sql/stats/09-initial-balance.sql
WITH b AS (
 SELECT a.symbol, c.timestamp AT TIME ZONE 'America/New_York' ts, (c.timestamp AT TIME ZONE 'America/New_York')::date d, c.open, c.high, c.low, c.close
 FROM candles c JOIN assets a ON a.id=c.asset_id WHERE c.timeframe='15m'
), day AS (
 SELECT symbol, d, count(*) nb,
  (array_agg(open ORDER BY ts))[1] o, (array_agg(close ORDER BY ts DESC))[1] cl,
  max(high) hi, min(low) lo,
  (array_agg(high ORDER BY ts))[1] h1, (array_agg(low ORDER BY ts))[1] l1,
  max(high) FILTER (WHERE ts::time < '10:30') hh1, min(low) FILTER (WHERE ts::time < '10:30') lh1,
  max(high) FILTER (WHERE ts::time >= '10:30') hrest, min(low) FILTER (WHERE ts::time >= '10:30') lrest,
  (array_agg(close ORDER BY ts))[4] c1030,
  (array_agg(ts ORDER BY ts))[1]::time t0
 FROM b GROUP BY 1,2
)
SELECT symbol, count(*) n,
 round(100*avg((h1-l1)/(hi-lo)),1) bar1_over_day_pct,
 round(100*avg((hh1-lh1)/(hi-lo)),1) h1_over_day_pct,
 round(100*avg((hi-lo)/o),3) day_rng_pct,
 round(100*avg((h1-l1)/o),3) bar1_rng_pct,
 round(100*avg((hh1-lh1)/o),3) hour1_rng_pct,
 round(100*avg((hh1=hi)::int),1) hod_in_h1_pct,
 round(100*avg((lh1=lo)::int),1) lod_in_h1_pct,
 round(100*avg((hh1=hi OR lh1=lo)::int),1) either_in_h1_pct,
 round(100*avg((hh1=hi AND lh1=lo)::int),1) both_in_h1_pct,
 round(100*avg((cl>hh1)::int),1) close_above_h1_pct,
 round(100*avg((cl<lh1)::int),1) close_below_h1_pct,
 round(100*avg((hrest>hh1 AND lrest<lh1)::int),1) both_sides_broken_pct,
 round(100*avg((hrest<=hh1 AND lrest>=lh1)::int),1) inside_h1_pct,
 round(corr((c1030-o)/o,(cl-c1030)/c1030)::numeric,3) corr_h1_vs_rest
FROM day WHERE nb>=20 AND t0='09:30'
GROUP BY 1 ORDER BY 1;
