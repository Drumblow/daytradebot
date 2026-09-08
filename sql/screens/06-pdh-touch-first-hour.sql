-- =============================================================================
-- 06-pdh-touch-first-hour.sql — Toque bruto da PDH na primeira hora
-- -----------------------------------------------------------------------------
-- Setup    : dia que abre abaixo da máxima de ontem (PDH); primeira barra entre
--            09:45 e 10:15 ET que abre abaixo da PDH e cuja máxima chega a
--            ≥ 0,997 × PDH ("toque"). Mede o resultado de um SHORT hipotético
--            do fechamento da barra de toque até o fechamento do dia, quantas
--            vezes a máxima da barra de toque é rompida depois, o range da barra
--            (proxy do stop de 1 barra) e o MFE mediano. NÃO há modelo de fill —
--            é a medição do nível sozinho, sem barra de sinal nem vetos.
-- Fonte    : base da `opening-reversal-v1` — Brooks, Reading Price Charts Bar by
--            Bar, Cap. 11/10 (opening reversal) e Dalton Cap. 4 (Open-Test-Drive)
--            via docs/strategies/opening-reversal-v1.md. Tolerância 0,3% =
--            interpretação nossa.
-- Hipótese : o nível de ontem, por si só, produz reversão vendável na 1ª hora.
-- Parâmetros: 8 símbolos vivos no denominador de per_symbol; janela 09:45–10:15
--            (exclui a barra 09:30).
-- Blocos (N de variantes = 1 medição): Q15.
-- Resultado 06/09/2026 (designer de setups): n = 1.119 (587 em 2025, 532 em
--   2026; ~70 por símbolo/ano); retorno short até o fechamento +0,03% / −0,06%;
--   78% das barras de toque são rompidas depois; MFE mediano 0,44–0,46%; range
--   mediano da barra de toque 0,33–0,37% (stop de 1 barra ≈ 33 bp → custo ≈ 12%
--   do R). Veredito: o nível sozinho NÃO tem edge — se a opening-reversal tem
--   edge, ele está na barra de sinal + vetos (plano-mestre §6.3: a v2 endurece
--   a confirmação, não afrouxa).
-- Uso      : docker exec -i trader-postgres psql -U trader -d trader_db < sql/screens/06-pdh-touch-first-hour.sql
-- Fuso     : America/New_York. Cópia fiel de q5.sql Q15.
-- =============================================================================
\set ON_ERROR_STOP on

create temp table bars as
select a.symbol, c.timestamp at time zone 'America/New_York' as ts, (c.timestamp at time zone 'America/New_York')::date d, c.open, c.high, c.low, c.close, c.volume
from candles c join assets a on a.id=c.asset_id where c.timeframe='15m' and a.symbol in ('AVUV','IJS','IWM','IWN','IWO','IWV','SLYV','VBR','IJR','SCHA','VB');
create index on bars(symbol,d,ts);
create temp table days as select symbol, d, (array_agg(open order by ts))[1] o, max(high) h, min(low) l, (array_agg(close order by ts desc))[1] cl,
 max(high) filter (where ts::time<'10:30') h1h, min(low) filter (where ts::time<'10:30') h1l, (array_agg(close order by ts))[4] h1c, count(*) n from bars group by 1,2;
create temp table dd as select *, lag(cl) over w pc, lag(h) over w ph, lag(l) over w pl, lag(o) over w po,
 avg((h-l)/cl) over (partition by symbol order by d rows between 14 preceding and 1 preceding) atrd from days window w as (partition by symbol order by d);

-- Q15: PDH touch (high within 0.3% below/above PDH) in first hour bars 09:45-10:15 (excluding 9:30 bar), short outcome: from touch bar close to day close; and stop = touch bar high + 0.3*ATRd? report distances
with dy as (select * from dd where n>=20 and ph is not null and o < ph),
 t as (select b.symbol, b.d, min(b.ts) t_touch from bars b join dy using(symbol,d) where b.ts::time between '09:45' and '10:15' and b.high >= dy.ph*0.997 and b.open < dy.ph group by 1,2),
 tb as (select t.*, b.close tc, b.high th_bar, b.low tl_bar, dy.cl, dy.ph, dy.atrd, extract(year from t.d)::int yr from t join bars b on b.symbol=t.symbol and b.d=t.d and b.ts=t.t_touch join dy on dy.symbol=t.symbol and dy.d=t.d),
 post as (select tb.*, max(b.high) rest_h, min(b.low) rest_l from tb join bars b on b.symbol=tb.symbol and b.d=tb.d and b.ts > tb.t_touch group by tb.symbol, tb.d, tb.t_touch, tb.tc, tb.th_bar, tb.tl_bar, tb.cl, tb.ph, tb.atrd, tb.yr)
select 'Q15 PDH touch 09:45-10:15' q, yr, count(*) n, round((count(*)::numeric/8),1) per_symbol, round((avg((tc-cl)/tc)*100)::numeric,3) short_ret_to_close_pct, round((avg((cl<tc)::int)*100)::numeric,1) pct_close_below,
 round((avg((rest_h > th_bar)::int)*100)::numeric,1) pct_touch_high_broken, round(((percentile_cont(0.5) within group (order by (th_bar-tl_bar)/tc))*100)::numeric,3) touch_bar_range_med_pct,
 round(((percentile_cont(0.5) within group (order by (tc-rest_l)/tc))*100)::numeric,3) mfe_med_pct
from post group by 2 order by 2;
