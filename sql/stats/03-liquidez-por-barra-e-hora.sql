-- 03-liquidez-por-barra-e-hora.sql — US$ por barra na abertura, no meio do dia e no fechamento;
-- fração de uma barra mediana de meio-dia que uma posição de US$ 250k representa; p10 por barra.
-- Fuso America/New_York. Origem: leitor de banco da pesquisa de 06/09/2026 (q9c_liq_bar).
-- Uso: docker exec -i trader-postgres psql -U trader -d trader_db < sql/stats/03-liquidez-por-barra-e-hora.sql
SELECT a.symbol,
 round(avg(c.volume*c.close) FILTER (WHERE (c.timestamp AT TIME ZONE 'America/New_York')::time='09:30')/1e3,0) k_0930,
 round(avg(c.volume*c.close) FILTER (WHERE (c.timestamp AT TIME ZONE 'America/New_York')::time BETWEEN '11:00' AND '14:00')/1e3,0) k_midday,
 round(avg(c.volume*c.close) FILTER (WHERE (c.timestamp AT TIME ZONE 'America/New_York')::time='15:45')/1e3,0) k_1545,
 round(100*250000/avg(c.volume*c.close) FILTER (WHERE (c.timestamp AT TIME ZONE 'America/New_York')::time BETWEEN '11:00' AND '14:00'),0) pct_of_midday_bar_for_250k,
 round((percentile_cont(0.1) WITHIN GROUP (ORDER BY c.volume*c.close))::numeric/1e3,0) p10_bar_k
FROM candles c JOIN assets a ON a.id=c.asset_id WHERE c.timeframe='15m'
GROUP BY 1 ORDER BY k_midday;
