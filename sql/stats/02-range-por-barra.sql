-- 02-range-por-barra.sql — range e corpo médios da barra de 15m, US$ por barra, barras sem volume
-- Origem: leitor de banco da pesquisa de 06/09/2026 (q1b_bar15).
-- Uso: docker exec -i trader-postgres psql -U trader -d trader_db < sql/stats/02-range-por-barra.sql
SELECT a.symbol,
  round(100*avg((c.high-c.low)/c.close),4) avg_bar_range_pct,
  round((100*percentile_cont(0.5) WITHIN GROUP (ORDER BY (c.high-c.low)/c.close))::numeric,4) med_bar_range_pct,
  round(100*avg(abs(c.close-c.open)/c.open),4) avg_abs_ret_pct,
  round(avg(c.volume*c.close)/1e3,0) avg_bar_dollar_vol_k,
  round(100*avg(CASE WHEN c.volume=0 THEN 1 ELSE 0 END),2) pct_zero_vol_bars
FROM candles c JOIN assets a ON a.id=c.asset_id WHERE c.timeframe='15m'
GROUP BY 1 ORDER BY 1;
