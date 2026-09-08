-- 10-distribuicao-range-diario.sql — % de dias com range < 1% / < 0,75% / > 2% por símbolo e range
-- médio por período. Os períodos (ago/2026, mai–jul/2026, 2025) são os da pergunta de 06/09/2026 —
-- ajustar as datas antes de reutilizar. Só dias completos (>= 20 barras). Fuso America/New_York.
-- Origem: leitor de banco de 06/09/2026 (q9d_range_dist).
-- Uso: docker exec -i trader-postgres psql -U trader -d trader_db < sql/stats/10-distribuicao-range-diario.sql
WITH day AS (
 SELECT a.symbol, (c.timestamp AT TIME ZONE 'America/New_York')::date d, (array_agg(c.open ORDER BY c.timestamp))[1] o, max(high) hi, min(low) lo, count(*) nb
 FROM candles c JOIN assets a ON a.id=c.asset_id WHERE c.timeframe='15m' GROUP BY 1,2
)
SELECT symbol, count(*) n,
 round(100*avg(((hi-lo)/o<0.01)::int),1) pct_days_rng_lt1pct,
 round(100*avg(((hi-lo)/o<0.0075)::int),1) pct_days_rng_lt075,
 round(100*avg(((hi-lo)/o>0.02)::int),1) pct_days_rng_gt2pct,
 round(100*avg((hi-lo)/o) FILTER (WHERE d BETWEEN '2026-08-01' AND '2026-09-03'),3) rng_aug26,
 round(100*avg((hi-lo)/o) FILTER (WHERE d BETWEEN '2026-05-01' AND '2026-07-31'),3) rng_may_jul26,
 round(100*avg((hi-lo)/o) FILTER (WHERE d < '2026-01-01'),3) rng_2025,
 round(100*avg(((hi-lo)/o<0.01)::int) FILTER (WHERE d BETWEEN '2026-08-01' AND '2026-09-03'),1) pct_lt1_aug26
FROM day WHERE nb>=20 GROUP BY 1 ORDER BY 1;
