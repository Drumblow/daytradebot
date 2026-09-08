-- 04-range-diario-por-mes.sql — range diário médio, retorno open→close médio e retorno intradiário
-- acumulado por mês (7 símbolos: IWM e os 6 pares vivos/candidatos).
-- Fuso America/New_York. Origem: leitor de banco da pesquisa de 06/09/2026 (q5h_monthly_rng).
-- Uso: docker exec -i trader-postgres psql -U trader -d trader_db < sql/stats/04-range-diario-por-mes.sql
WITH day AS (
 SELECT a.symbol, (c.timestamp AT TIME ZONE 'America/New_York')::date d,
  (array_agg(c.open ORDER BY c.timestamp))[1] o, (array_agg(c.close ORDER BY c.timestamp DESC))[1] cl, max(high) hi, min(low) lo
 FROM candles c JOIN assets a ON a.id=c.asset_id WHERE c.timeframe='15m' AND a.symbol IN ('IWM','AVUV','IJS','VBR','SLYV','IWN','IWV') GROUP BY 1,2
)
SELECT to_char(d,'YYYY-MM') ym, count(*) n, round(100*avg((hi-lo)/o),3) avg_rng_pct, round(100*avg((cl-o)/o),3) avg_ret_pct,
 round((100*(exp(sum(ln(cl/o)))-1))::numeric,1) cum_intraday_pct
FROM day GROUP BY 1 ORDER BY 1;
