-- 01-liquidez-diaria.sql — volume, US$ negociados, preço e range diário por símbolo
-- Fuso America/New_York. Origem: leitor de banco da pesquisa de 06/09/2026 (q1_daily).
-- Uso: docker exec -i trader-postgres psql -U trader -d trader_db < sql/stats/01-liquidez-diaria.sql
WITH d AS (
  SELECT a.symbol, (c.timestamp AT TIME ZONE 'America/New_York')::date AS d,
         max(c.high) hi, min(c.low) lo, sum(c.volume) vol, avg(c.close) px,
         sum(c.volume*c.close) dollar_vol, count(*) nbars
  FROM candles c JOIN assets a ON a.id=c.asset_id
  WHERE c.timeframe='15m'
  GROUP BY 1,2
)
SELECT symbol, count(*) n_days,
       round(avg(px),2) avg_px,
       round(avg(vol)/1e6,2) avg_shares_M,
       round(avg(dollar_vol)/1e6,1) avg_dollar_vol_M,
       round(100*avg((hi-lo)/px),3) avg_day_range_pct,
       round((100*percentile_cont(0.5) WITHIN GROUP (ORDER BY (hi-lo)/px))::numeric,3) med_day_range_pct,
       round((100*percentile_cont(0.1) WITHIN GROUP (ORDER BY (hi-lo)/px))::numeric,3) p10_day_range_pct,
       round(avg(nbars),1) avg_bars
FROM d GROUP BY 1 ORDER BY avg_dollar_vol_M DESC;
