-- =============================================================================
-- 03-orb-ib-estreito.sql — Opening Range Breakout / Initial Balance estreito
-- -----------------------------------------------------------------------------
-- Setup    : primeira hora (09:30–10:29 ET, 4 barras) = Initial Balance (IB).
--            Quando o IB é estreito, opera-se o primeiro rompimento entre 10:30
--            e 14:00: entrada no extremo rompido, stop no extremo oposto do IB,
--            alvo k × R (R = largura do IB), saída no fechamento.
-- Fonte    : Rosewag Cap. 16 (Opening Range Breakout) — análise
--            docs/books/analysis/rosewag-new-age-ta.md §3.2 e §4.2 (candidata
--            `opening-range-breakout-v1`); Dalton, Mind over Markets, Cap. 2
--            (Initial Balance e range extension) — análise
--            docs/books/analysis/dalton-mind-over-markets.md §2.1. Os limiares
--            (0,5% absoluto; 0,45 / 0,35 / 0,30 × ATRd) são interpretação nossa.
-- Hipótese : IB estreito → extensão de range com continuação suficiente para
--            pagar 1–1,5 R depois do custo.
-- Parâmetros: janela de fill 10:30–14:00 ET; primeiro lado rompido decide a
--            direção; Q10/Q10b/Q13 com fill honesto e 4 bp; Q4 sem custo e com
--            fill no extremo (modelo otimista de referência).
-- Blocos (N de variantes = 5 regras):
--   Q4   IB < 0,5% do preço, ambos os lados, fill no extremo, sem custo (otimista);
--   Q10  IB < 0,5%, só short, k = 1,5, custo 4 bp, por símbolo;
--   Q10b IB < 0,45 × ATRd, ambos os lados, fill honesto, 4 bp, por ano e símbolo;
--   Q11  sobreposição entre dias de IB estreito (relativo) e dias de PDL-break;
--   Q13  IB < 0,30 e < 0,35 × ATRd, ambos os lados, fill honesto, 4 bp.
-- Resultado 06/09/2026 (designer de setups): com limiar relativo (< 0,45 × ATRd)
--   short +0,02 (2025) / −0,05 (2026), long +0,08 / −0,17; H1 < 0,30 × ATRd
--   short −0,20 / +0,10. Limiar absoluto 0,5% (Q4/Q10): sem número registrado
--   nos relatórios — a medir. Veredito: ≈ 0, REPROVADO como gatilho; IB só como
--   contexto/instrumentação (plano-mestre §6.6). A 1ª hora contém 64–68% do
--   range diário e é rompida em 91–95% dos dias (mapa do banco) — a extensão é
--   quase universal, então não separa nada sozinha.
-- Uso      : docker exec -i trader-postgres psql -U trader -d trader_db < sql/screens/03-orb-ib-estreito.sql
-- Fuso     : America/New_York. Cópia fiel de q2.sql Q4, q3.sql Q10, q4.sql Q10b/Q11, q5.sql Q13.
-- =============================================================================
\set ON_ERROR_STOP on

create temp table bars as
select a.symbol, c.timestamp at time zone 'America/New_York' as ts, (c.timestamp at time zone 'America/New_York')::date d, c.open, c.high, c.low, c.close, c.volume
from candles c join assets a on a.id=c.asset_id where c.timeframe='15m' and a.symbol in ('AVUV','IJS','IWM','IWN','IWO','IWV','SLYV','VBR','IJR','SCHA','VB');
create index on bars(symbol,d,ts);
create temp table days as select symbol, d, (array_agg(open order by ts))[1] o, max(high) h, min(low) l, (array_agg(close order by ts desc))[1] cl,
 (array_agg(high order by ts))[1] b1h, (array_agg(low order by ts))[1] b1l,
 max(high) filter (where ts::time<'10:30') h1h, min(low) filter (where ts::time<'10:30') h1l, (array_agg(close order by ts))[4] h1c,
 (array_agg(high order by ts))[4] b4h, count(*) n from bars group by 1,2;
create temp table dd as select *, lag(cl) over w pc, lag(h) over w ph, lag(l) over w pl, lag(o) over w po, lag(cl,2) over w pc2,
 avg((h-l)/cl) over (partition by symbol order by d rows between 14 preceding and 1 preceding) atrd from days window w as (partition by symbol order by d);

-- Q4: narrow IB (H1 range < 0.5% of price): first break after 10:30 (<=14:00), entry at H1 extreme, stop = opposite extreme, targets k*R  (OTIMISTA: fill no extremo, sem custo)
with dy as (select * from dd where n>=20 and (h1h-h1l)/h1c < 0.005),
 fu as (select b.symbol, b.d, min(b.ts) t from bars b join dy using(symbol,d) where b.ts::time >= '10:30' and b.ts::time <= '14:00' and b.high > dy.h1h group by 1,2),
 fd as (select b.symbol, b.d, min(b.ts) t from bars b join dy using(symbol,d) where b.ts::time >= '10:30' and b.ts::time <= '14:00' and b.low < dy.h1l group by 1,2),
 f as (select dy.symbol, dy.d, case when fu.t is not null and (fd.t is null or fu.t <= fd.t) then 1 when fd.t is not null then -1 end dir, least(coalesce(fu.t,fd.t), coalesce(fd.t,fu.t)) t_fill
       from dy left join fu using(symbol,d) left join fd using(symbol,d) where fu.t is not null or fd.t is not null),
 st as (select b.symbol, b.d, min(b.ts) t_stop from bars b join f using(symbol,d) join dy using(symbol,d) where b.ts >= f.t_fill and ((f.dir=1 and b.low < dy.h1l) or (f.dir=-1 and b.high > dy.h1h)) group by 1,2),
 k as (select * from (values (1.0),(1.5),(2.0),(99.0)) v(k)),
 tg as (select b.symbol, b.d, k.k, min(b.ts) t_tgt from bars b join f using(symbol,d) join dy using(symbol,d) cross join k where b.ts >= f.t_fill and ((f.dir=1 and b.high >= dy.h1h + k.k*(dy.h1h-dy.h1l)) or (f.dir=-1 and b.low <= dy.h1l - k.k*(dy.h1h-dy.h1l))) group by 1,2,3),
 r as (select dy.symbol, dy.d, extract(year from dy.d)::int yr, f.dir, k.k, (dy.h1h-dy.h1l)/dy.h1c risk_pct,
   case when st.t_stop is not null and (tg.t_tgt is null or st.t_stop <= tg.t_tgt) then -1.0 when tg.t_tgt is not null then k.k
        else f.dir*(dy.cl - case when f.dir=1 then dy.h1h else dy.h1l end)/(dy.h1h-dy.h1l) end r_mult
   from dy join f using(symbol,d) cross join k left join st using(symbol,d) left join tg on tg.symbol=dy.symbol and tg.d=dy.d and tg.k=k.k)
select 'Q4 narrowIB<0.5% breakout, stop=opp extreme (OTIMISTA)' q, k, yr, dir, count(*) fills, round(avg(r_mult)::numeric,3) avg_r, round((avg((r_mult>0)::int)*100)::numeric,1) wr,
 round(((percentile_cont(0.5) within group (order by risk_pct))*100)::numeric,3) risk_med_pct, round(sum(r_mult)::numeric,1) sum_r
from r group by 1,2,3,4 order by 2,3,4;

-- Q10: narrow IB by symbol, short side only, k=1.5, honest net 4bp
with dy as (select * from dd where n>=20 and (h1h-h1l)/h1c < 0.005),
 fd as (select b.symbol, b.d, min(b.ts) t from bars b join dy using(symbol,d) where b.ts::time >= '10:30' and b.ts::time <= '14:00' and b.low < dy.h1l group by 1,2),
 fu as (select b.symbol, b.d, min(b.ts) t from bars b join dy using(symbol,d) where b.ts::time >= '10:30' and b.ts::time <= '14:00' and b.high > dy.h1h group by 1,2),
 f as (select fd.symbol, fd.d, fd.t t_fill from fd left join fu using(symbol,d) where fu.t is null or fd.t < fu.t),
 st as (select b.symbol, b.d, min(b.ts) t_stop from bars b join f using(symbol,d) join dy using(symbol,d) where b.ts >= f.t_fill and b.high > dy.h1h group by 1,2),
 tg as (select b.symbol, b.d, min(b.ts) t_tgt from bars b join f using(symbol,d) join dy using(symbol,d) where b.ts >= f.t_fill and b.low <= dy.h1l - 1.5*(dy.h1h-dy.h1l) group by 1,2),
 r as (select dy.symbol, dy.d, extract(year from dy.d)::int yr, (dy.h1h-dy.h1l)/dy.h1c risk_pct,
   case when st.t_stop is not null and (tg.t_tgt is null or st.t_stop <= tg.t_tgt) then -1.0 when tg.t_tgt is not null then 1.5 else (dy.h1l-dy.cl)/(dy.h1h-dy.h1l) end - 0.0004/((dy.h1h-dy.h1l)/dy.h1c) r_net
   from dy join f using(symbol,d) left join st using(symbol,d) left join tg using(symbol,d))
select 'Q10 narrowIB SHORT by symbol k=1.5 net' q, symbol, count(*) fills, round(avg(r_net)::numeric,3) avg_r, round((avg((r_net>0)::int)*100)::numeric,1) wr, round(sum(r_net)::numeric,1) sum_r,
 round((sum(r_net) filter (where r_net>0) / nullif(-sum(r_net) filter (where r_net<0),0))::numeric,2) pf_r
from r group by 1,2 order by 2;

-- Q10b: narrow IB relative to daily ATR (H1 range < 0.45*atrd): short and long, honest fill, net 4bp, by year and symbol
with dy as (select * from dd where n>=20 and atrd is not null and (h1h-h1l)/h1c < 0.45*atrd),
 fd as (select b.symbol, b.d, min(b.ts) t from bars b join dy using(symbol,d) where b.ts::time >= '10:30' and b.ts::time <= '14:00' and b.low < dy.h1l group by 1,2),
 fu as (select b.symbol, b.d, min(b.ts) t from bars b join dy using(symbol,d) where b.ts::time >= '10:30' and b.ts::time <= '14:00' and b.high > dy.h1h group by 1,2),
 f as (select dy.symbol, dy.d, case when fu.t is not null and (fd.t is null or fu.t <= fd.t) then 1 when fd.t is not null then -1 end dir, least(coalesce(fu.t,fd.t), coalesce(fd.t,fu.t)) t_fill
       from dy left join fu using(symbol,d) left join fd using(symbol,d) where fu.t is not null or fd.t is not null),
 fb as (select f.*, case when f.dir=1 then greatest(b.open, dy.h1h) else least(b.open, dy.h1l) end fill_px from f join bars b on b.symbol=f.symbol and b.d=f.d and b.ts=f.t_fill join dy on dy.symbol=f.symbol and dy.d=f.d),
 st as (select b.symbol, b.d, min(b.ts) t_stop from bars b join fb using(symbol,d) join dy using(symbol,d) where b.ts >= fb.t_fill and ((fb.dir=1 and b.low < dy.h1l) or (fb.dir=-1 and b.high > dy.h1h)) group by 1,2),
 k as (select * from (values (1.0),(1.5),(2.0)) v(k)),
 tg as (select b.symbol, b.d, k.k, min(b.ts) t_tgt from bars b join fb using(symbol,d) join dy using(symbol,d) cross join k where b.ts >= fb.t_fill and ((fb.dir=1 and b.high >= fb.fill_px + k.k*(fb.fill_px-dy.h1l)) or (fb.dir=-1 and b.low <= fb.fill_px - k.k*(dy.h1h-fb.fill_px))) group by 1,2,3),
 r as (select dy.symbol, dy.d, extract(year from dy.d)::int yr, fb.dir, k.k, case when fb.dir=1 then (fb.fill_px-dy.h1l)/fb.fill_px else (dy.h1h-fb.fill_px)/fb.fill_px end risk_pct,
   case when st.t_stop is not null and (tg.t_tgt is null or st.t_stop <= tg.t_tgt) then -1.0 when tg.t_tgt is not null then k.k
        else fb.dir*(dy.cl - fb.fill_px)/(case when fb.dir=1 then fb.fill_px-dy.h1l else dy.h1h-fb.fill_px end) end
        - 0.0004/(case when fb.dir=1 then (fb.fill_px-dy.h1l)/fb.fill_px else (dy.h1h-fb.fill_px)/fb.fill_px end) r_net
   from dy join fb using(symbol,d) cross join k left join st using(symbol,d) left join tg on tg.symbol=dy.symbol and tg.d=dy.d and tg.k=k.k)
select 'Q10b narrowIB<0.45*ATRd, honest, net 4bp' q, k, yr::text grp, dir, count(*) fills, round(avg(r_net)::numeric,3) avg_r_net, round((avg((r_net>0)::int)*100)::numeric,1) wr,
 round(((percentile_cont(0.5) within group (order by risk_pct))*100)::numeric,3) risk_med_pct, round(sum(r_net)::numeric,1) sum_r,
 round((sum(r_net) filter (where r_net>0) / nullif(-sum(r_net) filter (where r_net<0),0))::numeric,2) pf_r
from r group by 1,2,3,4
union all
select 'Q10b by symbol SHORT k=1.5' q, k, symbol, dir, count(*), round(avg(r_net)::numeric,3), round((avg((r_net>0)::int)*100)::numeric,1),
 round(((percentile_cont(0.5) within group (order by risk_pct))*100)::numeric,3), round(sum(r_net)::numeric,1),
 round((sum(r_net) filter (where r_net>0) / nullif(-sum(r_net) filter (where r_net<0),0))::numeric,2)
from r where k=1.5 and dir=-1 group by 1,2,3,4
order by 1,2,3,4;

-- Q11: overlap between narrow-IB short days (relative) and PDL-break short days
with nb as (select symbol, d from dd where n>=20 and atrd is not null and (h1h-h1l)/h1c < 0.45*atrd and l < h1l),
 pb as (select symbol, d from dd where n>=20 and pl is not null and o > pl and o < pc and l < pl)
select 'Q11 overlap' q, (select count(*) from nb) narrow_ib_short_days, (select count(*) from pb) pdl_break_days, (select count(*) from nb join pb using(symbol,d)) both_days;

-- Q13: very narrow IB (H1 < 0.30 / 0.35 x ATRd), short & long, honest, net 4bp, by year
with thr as (select * from (values (0.30),(0.35)) v(t)),
 dy as (select dd.*, thr.t from dd cross join thr where n>=20 and atrd is not null and (h1h-h1l)/h1c < thr.t*atrd),
 fd as (select b.symbol, b.d, dy.t, min(b.ts) tt from bars b join dy on dy.symbol=b.symbol and dy.d=b.d where b.ts::time >= '10:30' and b.ts::time <= '14:00' and b.low < dy.h1l group by 1,2,3),
 fu as (select b.symbol, b.d, dy.t, min(b.ts) tt from bars b join dy on dy.symbol=b.symbol and dy.d=b.d where b.ts::time >= '10:30' and b.ts::time <= '14:00' and b.high > dy.h1h group by 1,2,3),
 f as (select dy.symbol, dy.d, dy.t, dy.h1h, dy.h1l, dy.cl, case when fu.tt is not null and (fd.tt is null or fu.tt <= fd.tt) then 1 when fd.tt is not null then -1 end dir, least(coalesce(fu.tt,fd.tt), coalesce(fd.tt,fu.tt)) t_fill
       from dy left join fu on fu.symbol=dy.symbol and fu.d=dy.d and fu.t=dy.t left join fd on fd.symbol=dy.symbol and fd.d=dy.d and fd.t=dy.t where fu.tt is not null or fd.tt is not null),
 fb as (select f.*, case when f.dir=1 then greatest(b.open, f.h1h) else least(b.open, f.h1l) end fill_px from f join bars b on b.symbol=f.symbol and b.d=f.d and b.ts=f.t_fill),
 st as (select fb.symbol, fb.d, fb.t, min(b.ts) t_stop from bars b join fb on b.symbol=fb.symbol and b.d=fb.d where b.ts >= fb.t_fill and ((fb.dir=1 and b.low < fb.h1l) or (fb.dir=-1 and b.high > fb.h1h)) group by 1,2,3),
 k as (select * from (values (1.0),(1.5)) v(k)),
 tg as (select fb.symbol, fb.d, fb.t, k.k, min(b.ts) t_tgt from bars b join fb on b.symbol=fb.symbol and b.d=fb.d cross join k where b.ts >= fb.t_fill and ((fb.dir=1 and b.high >= fb.fill_px + k.k*(fb.fill_px-fb.h1l)) or (fb.dir=-1 and b.low <= fb.fill_px - k.k*(fb.h1h-fb.fill_px))) group by 1,2,3,4),
 r as (select fb.symbol, fb.d, fb.t, extract(year from fb.d)::int yr, fb.dir, k.k, case when fb.dir=1 then (fb.fill_px-fb.h1l)/fb.fill_px else (fb.h1h-fb.fill_px)/fb.fill_px end risk_pct,
   case when st.t_stop is not null and (tg.t_tgt is null or st.t_stop <= tg.t_tgt) then -1.0 when tg.t_tgt is not null then k.k
        else fb.dir*(fb.cl - fb.fill_px)/(case when fb.dir=1 then fb.fill_px-fb.h1l else fb.h1h-fb.fill_px end) end
        - 0.0004/(case when fb.dir=1 then (fb.fill_px-fb.h1l)/fb.fill_px else (fb.h1h-fb.fill_px)/fb.fill_px end) r_net
   from fb cross join k left join st on st.symbol=fb.symbol and st.d=fb.d and st.t=fb.t left join tg on tg.symbol=fb.symbol and tg.d=fb.d and tg.t=fb.t and tg.k=k.k)
select 'Q13 very narrow IB honest net' q, t, k, yr, dir, count(*) fills, round(avg(r_net)::numeric,3) avg_r_net, round((avg((r_net>0)::int)*100)::numeric,1) wr,
 round(((percentile_cont(0.5) within group (order by risk_pct))*100)::numeric,3) risk_med_pct, round(sum(r_net)::numeric,1) sum_r,
 round((sum(r_net) filter (where r_net>0) / nullif(-sum(r_net) filter (where r_net<0),0))::numeric,2) pf_r
from r group by 1,2,3,4,5 order by 2,3,4,5;
