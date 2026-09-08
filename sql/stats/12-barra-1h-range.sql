-- 12-barra-1h-range.sql — range mediano de uma barra de 1h derivada do 15m (alinhada a 09:30 ET),
-- com e sem a primeira hora, para os 8 ativos vivos. Proxy do stop de 1 barra no timeframe 1h
-- (plano-mestre §7: 1h derivado via `resample`, condicional a resultados da Onda B).
-- Fuso America/New_York. Origem: designer de setups de 06/09/2026 (q6.sql Q17).
-- Uso: docker exec -i trader-postgres psql -U trader -d trader_db < sql/stats/12-barra-1h-range.sql
WITH bars AS (
 SELECT a.symbol, c.timestamp AT TIME ZONE 'America/New_York' ts, (c.timestamp AT TIME ZONE 'America/New_York')::date d, c.high, c.low, c.close
 FROM candles c JOIN assets a ON a.id=c.asset_id
 WHERE c.timeframe='15m' AND a.symbol IN ('AVUV','IJS','IWM','IWN','IWO','IWV','SLYV','VBR')
), h AS (
 SELECT symbol, d, date_trunc('hour', ts - interval '30 minutes') + interval '30 minutes' hts, max(high) hh, min(low) hl, avg(close) c
 FROM bars GROUP BY 1,2,3
)
SELECT 'Q17 1h bar range' q, symbol, count(*) n,
 round(((percentile_cont(0.5) WITHIN GROUP (ORDER BY (hh-hl)/c))*100)::numeric,3) med_1h_range_pct,
 round(((percentile_cont(0.5) WITHIN GROUP (ORDER BY (hh-hl)/c) FILTER (WHERE hts::time >= '10:30'))*100)::numeric,3) med_1h_range_ex_first_pct
FROM h GROUP BY 2 ORDER BY 2;
