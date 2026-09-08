-- 08-proxy-de-tendencia.sql — proxy de `trend_state` do contexto global usando SMA20/SMA50 do 15m
-- (up = close > SMA20 > SMA50; down = close < SMA20 < SMA50; senão neutral). O contexto real usa
-- EMA20/SMA200 (crates/trader-core/src/context/mod.rs) — isto é uma aproximação para o histórico.
-- Bloco A: por símbolo (após 50 barras de aquecimento). Bloco B: por mês, 7 símbolos.
-- Origem: leitor de banco de 06/09/2026 (q7_trend_proxy, q7b_trend_proxy_month).
-- Uso: docker exec -i trader-postgres psql -U trader -d trader_db < sql/stats/08-proxy-de-tendencia.sql

-- A. por símbolo
WITH b AS (
 SELECT a.symbol, c.timestamp ts, c.close,
  avg(c.close) OVER (PARTITION BY a.symbol ORDER BY c.timestamp ROWS BETWEEN 19 PRECEDING AND CURRENT ROW) s20,
  avg(c.close) OVER (PARTITION BY a.symbol ORDER BY c.timestamp ROWS BETWEEN 49 PRECEDING AND CURRENT ROW) s50,
  row_number() OVER (PARTITION BY a.symbol ORDER BY c.timestamp) rn
 FROM candles c JOIN assets a ON a.id=c.asset_id WHERE c.timeframe='15m'
)
SELECT symbol, count(*) n,
 round(100*avg((close>s20 AND s20>s50)::int),1) up_proxy_pct,
 round(100*avg((close<s20 AND s20<s50)::int),1) down_proxy_pct,
 round(100*avg((NOT (close>s20 AND s20>s50) AND NOT (close<s20 AND s20<s50))::int),1) neutral_proxy_pct,
 round(100*avg((close>s50)::int),1) above_s50_pct
FROM b WHERE rn>50 GROUP BY 1 ORDER BY 1;

-- B. por mês (ET)
WITH b AS (
 SELECT a.symbol, c.timestamp ts, c.close,
  avg(c.close) OVER (PARTITION BY a.symbol ORDER BY c.timestamp ROWS BETWEEN 19 PRECEDING AND CURRENT ROW) s20,
  avg(c.close) OVER (PARTITION BY a.symbol ORDER BY c.timestamp ROWS BETWEEN 49 PRECEDING AND CURRENT ROW) s50,
  row_number() OVER (PARTITION BY a.symbol ORDER BY c.timestamp) rn
 FROM candles c JOIN assets a ON a.id=c.asset_id WHERE c.timeframe='15m' AND a.symbol IN ('IWM','AVUV','IJS','VBR','SLYV','IWN','IWV')
)
SELECT to_char(ts AT TIME ZONE 'America/New_York','YYYY-MM') ym, count(*) n,
 round(100*avg((close>s20 AND s20>s50)::int),1) up_proxy_pct,
 round(100*avg((close<s20 AND s20<s50)::int),1) down_proxy_pct,
 round(100*avg((NOT (close>s20 AND s20>s50) AND NOT (close<s20 AND s20<s50))::int),1) neutral_proxy_pct
FROM b WHERE rn>50 GROUP BY 1 ORDER BY 1;
