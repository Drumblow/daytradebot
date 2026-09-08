-- =============================================================================
-- 01-pdl-pdh-break.sql — Rompimento de PDL (short) / PDH (long) intradiário
-- -----------------------------------------------------------------------------
-- Setup    : abertura dentro do range de ontem; vende o rompimento da mínima de
--            ontem (PDL) / compra o rompimento da máxima (PDH); stop = extremo
--            do dia até ali; alvo k × R; saída no fechamento.
-- Fonte    : Murphy, Technical Analysis of the Financial Markets, Cap. 16
--            ("The Use of Intraday Pivot Points") — análise
--            docs/books/analysis/murphy-technical-analysis.md §3.2 e §5.2
--            (candidata `pivot-point-intraday-v1`). A condição TO < PDC / TO > PDC
--            (abertura de hoje abaixo/acima do fechamento de ontem) é a do livro.
-- Hipótese : níveis de ontem funcionam como pivôs intradiários; o rompimento
--            aceito continua na direção com R ≥ 1.
-- Parâmetros: janela de fill 10:00–14:30 ET (Q8c/Q8d/Q12: ex 12:00–13:00);
--            filtro de risco 0,3–1,5%; custo 4 bp ida e volta; alvos 1 / 1,5 / 2 /
--            99 (= segurar até o fechamento); universo 11 ETFs.
-- Blocos (N de variantes = 8 regras):
--   A  Q3  short/long — MODELO OTIMISTA de referência: fill exato no nível, stop
--          só a partir da barra seguinte, sem custo (é o que um scanner ingênuo
--          mostra — NÃO usar como evidência).
--   B  Q8/Q8b — fill no nível, stop avaliado desde a barra do fill, custo 4 bp,
--          filtro de risco.
--   C  Q8c/Q8d — condição de Murphy (TO<PDC / TO>PDC), ex 12h, FILL HONESTO
--          (min/max(open, nível)).
--   D  Q12 — sem condição de PDC, ex 12h, fill honesto, dividido por TO vs PDC;
--      Q12b — variante confirmada: primeira barra que FECHA abaixo da PDL,
--          entrada abaixo da mínima dela (válida 2 barras), stop na máxima do dia.
-- Resultado 06/09/2026 (designer de setups; Q12 re-executada pelos críticos):
--   PDL-break short com fill no nível: avg R +0,063 (2025, PF_R 1,21) / +0,134
--   (2026, PF_R 1,50). Com fill honesto (Q12, k=1): −0,034 (n=499, PF_R 0,90) /
--   +0,034 (n=344, PF_R 1,11). A condição TO<PDC do livro piora. PDH-break long
--   PF 0,64 em 2026. Q12b: sem número registrado nos relatórios — a medir.
--   Veredito: REPROVADO — o "edge" era o gap atravessado no gatilho (ADR-015).
-- Uso      : docker exec -i trader-postgres psql -U trader -d trader_db < sql/screens/01-pdl-pdh-break.sql
-- Fuso     : America/New_York em tudo. Cópia fiel das consultas de 06/09/2026
--            (q2.sql Q3, q3.sql Q8/Q8b, q4.sql Q8c/Q8d, q5.sql Q12/Q12b).
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

-- -----------------------------------------------------------------------------
-- A. Q3 — MODELO OTIMISTA (comparação): fill exato no nível, stop só depois da
--    barra do fill, sem custo. Serve para mostrar o tamanho do artefato.
-- -----------------------------------------------------------------------------
-- Q3: Murphy pivot break: open inside yesterday range; short on break of PDL after 10:00 (stop = today high so far); long on break of PDH (stop = today low so far)
with dy as (select * from dd where n>=20 and pl is not null and o between pl and ph),
 bs as (select b.*, max(high) over (partition by b.symbol, b.d order by ts rows unbounded preceding) th, min(low) over (partition by b.symbol, b.d order by ts rows unbounded preceding) tl from bars b join dy using(symbol,d)),
 fs as (select bs.symbol, bs.d, min(ts) t_fill from bs join dy using(symbol,d) where ts::time >= '10:00' and ts::time <= '14:30' and low < dy.pl group by 1,2),
 fsb as (select fs.*, bs.th th_fill from fs join bs on bs.symbol=fs.symbol and bs.d=fs.d and bs.ts=fs.t_fill),
 sts as (select b.symbol, b.d, min(b.ts) t_stop from bars b join fsb using(symbol,d) where b.ts > fsb.t_fill and b.high > fsb.th_fill group by 1,2),
 k as (select * from (values (1.0),(2.0),(99.0)) v(k)),
 tgs as (select b.symbol, b.d, k.k, min(b.ts) t_tgt from bars b join fsb using(symbol,d) join dy using(symbol,d) cross join k where b.ts >= fsb.t_fill and b.low <= dy.pl - k.k*(fsb.th_fill-dy.pl) group by 1,2,3),
 rs as (select dy.symbol, dy.d, extract(year from dy.d)::int yr, k.k, (fsb.th_fill-dy.pl)/dy.pl risk_pct,
   case when sts.t_stop is not null and (tgs.t_tgt is null or sts.t_stop <= tgs.t_tgt) then -1.0 when tgs.t_tgt is not null then k.k else (dy.pl-dy.cl)/(fsb.th_fill-dy.pl) end r_mult
   from dy join fsb using(symbol,d) cross join k left join sts using(symbol,d) left join tgs on tgs.symbol=dy.symbol and tgs.d=dy.d and tgs.k=k.k)
select 'Q3 PDL-break SHORT stop=TH (OTIMISTA)' q, k, yr, count(*) fills, round(avg(r_mult)::numeric,3) avg_r, round((avg((r_mult>0)::int)*100)::numeric,1) wr,
 round(((percentile_cont(0.5) within group (order by risk_pct))*100)::numeric,3) risk_med_pct, round(sum(r_mult)::numeric,1) sum_r,
 count(*) filter (where risk_pct between 0.003 and 0.015) n_risk_ok, round(avg(r_mult) filter (where risk_pct between 0.003 and 0.015)::numeric,3) avg_r_risk_ok
from rs group by 1,2,3 order by 2,3;

with dy as (select * from dd where n>=20 and pl is not null and o between pl and ph),
 bs as (select b.*, max(high) over (partition by b.symbol, b.d order by ts rows unbounded preceding) th, min(low) over (partition by b.symbol, b.d order by ts rows unbounded preceding) tl from bars b join dy using(symbol,d)),
 fl as (select bs.symbol, bs.d, min(ts) t_fill from bs join dy using(symbol,d) where ts::time >= '10:00' and ts::time <= '14:30' and high > dy.ph group by 1,2),
 flb as (select fl.*, bs.tl tl_fill from fl join bs on bs.symbol=fl.symbol and bs.d=fl.d and bs.ts=fl.t_fill),
 stl as (select b.symbol, b.d, min(b.ts) t_stop from bars b join flb using(symbol,d) where b.ts > flb.t_fill and b.low < flb.tl_fill group by 1,2),
 k as (select * from (values (1.0),(2.0),(99.0)) v(k)),
 tgl as (select b.symbol, b.d, k.k, min(b.ts) t_tgt from bars b join flb using(symbol,d) join dy using(symbol,d) cross join k where b.ts >= flb.t_fill and b.high >= dy.ph + k.k*(dy.ph-flb.tl_fill) group by 1,2,3),
 rl as (select dy.symbol, dy.d, extract(year from dy.d)::int yr, k.k, (dy.ph-flb.tl_fill)/dy.ph risk_pct,
   case when stl.t_stop is not null and (tgl.t_tgt is null or stl.t_stop <= tgl.t_tgt) then -1.0 when tgl.t_tgt is not null then k.k else (dy.cl-dy.ph)/(dy.ph-flb.tl_fill) end r_mult
   from dy join flb using(symbol,d) cross join k left join stl using(symbol,d) left join tgl on tgl.symbol=dy.symbol and tgl.d=dy.d and tgl.k=k.k)
select 'Q3 PDH-break LONG stop=TL (OTIMISTA)' q, k, yr, count(*) fills, round(avg(r_mult)::numeric,3) avg_r, round((avg((r_mult>0)::int)*100)::numeric,1) wr,
 round(((percentile_cont(0.5) within group (order by risk_pct))*100)::numeric,3) risk_med_pct, round(sum(r_mult)::numeric,1) sum_r,
 count(*) filter (where risk_pct between 0.003 and 0.015) n_risk_ok, round(avg(r_mult) filter (where risk_pct between 0.003 and 0.015)::numeric,3) avg_r_risk_ok
from rl group by 1,2,3 order by 2,3;

-- -----------------------------------------------------------------------------
-- B. Q8 / Q8b — fill no nível, stop desde a barra do fill, custo 4 bp, risco 0,3–1,5%
-- -----------------------------------------------------------------------------
-- Q8: PDL-break SHORT honest: stop = TH of bars BEFORE the fill bar (+0 buffer); fill bar high > stop => -1R; cost 4bp subtracted in R; risk filter 0.3-1.5%; by year and by symbol (k=1)
with dy as (select * from dd where n>=20 and pl is not null and o between pl and ph),
 bs as (select b.*, max(high) over (partition by b.symbol, b.d order by ts rows between unbounded preceding and 1 preceding) th_prev from bars b join dy using(symbol,d)),
 fs as (select bs.symbol, bs.d, min(ts) t_fill from bs join dy using(symbol,d) where ts::time >= '10:00' and ts::time <= '14:30' and low < dy.pl group by 1,2),
 fsb as (select fs.*, bs.th_prev, bs.high fill_high, bs.open fill_open from fs join bs on bs.symbol=fs.symbol and bs.d=fs.d and bs.ts=fs.t_fill),
 sts as (select b.symbol, b.d, min(b.ts) t_stop from bars b join fsb using(symbol,d) where b.ts >= fsb.t_fill and b.high > fsb.th_prev group by 1,2),
 k as (select * from (values (1.0),(2.0),(99.0)) v(k)),
 tgs as (select b.symbol, b.d, k.k, min(b.ts) t_tgt from bars b join fsb using(symbol,d) join dy using(symbol,d) cross join k where b.ts >= fsb.t_fill and b.low <= dy.pl - k.k*(fsb.th_prev-dy.pl) group by 1,2,3),
 rs as (select dy.symbol, dy.d, extract(year from dy.d)::int yr, k.k, fsb.t_fill, (fsb.th_prev-dy.pl)/dy.pl risk_pct,
   case when sts.t_stop is not null and (tgs.t_tgt is null or sts.t_stop <= tgs.t_tgt) then -1.0 when tgs.t_tgt is not null then k.k else (dy.pl-dy.cl)/(fsb.th_prev-dy.pl) end
   - 0.0004/((fsb.th_prev-dy.pl)/dy.pl) r_net
   from dy join fsb using(symbol,d) cross join k left join sts using(symbol,d) left join tgs on tgs.symbol=dy.symbol and tgs.d=dy.d and tgs.k=k.k
   where (fsb.th_prev-dy.pl)/dy.pl between 0.003 and 0.015)
select 'Q8 PDL-break SHORT fill=PDL, risk 0.3-1.5%, net 4bp' q, k, yr::text grp, count(*) fills, round(avg(r_net)::numeric,3) avg_r_net, round((avg((r_net>0)::int)*100)::numeric,1) wr,
 round(((percentile_cont(0.5) within group (order by risk_pct))*100)::numeric,3) risk_med_pct, round(sum(r_net)::numeric,1) sum_r,
 round((sum(r_net) filter (where r_net>0) / nullif(-sum(r_net) filter (where r_net<0),0))::numeric,2) pf_r
from rs group by 1,2,3
union all
select 'Q8 PDL-break SHORT by symbol (k=1)' q, k, symbol, count(*), round(avg(r_net)::numeric,3), round((avg((r_net>0)::int)*100)::numeric,1),
 round(((percentile_cont(0.5) within group (order by risk_pct))*100)::numeric,3), round(sum(r_net)::numeric,1),
 round((sum(r_net) filter (where r_net>0) / nullif(-sum(r_net) filter (where r_net<0),0))::numeric,2)
from rs where k=1.0 group by 1,2,3
union all
select 'Q8 PDL-break SHORT by fill hour (k=1)' q, k, to_char(t_fill,'HH24'), count(*), round(avg(r_net)::numeric,3), round((avg((r_net>0)::int)*100)::numeric,1),
 round(((percentile_cont(0.5) within group (order by risk_pct))*100)::numeric,3), round(sum(r_net)::numeric,1),
 round((sum(r_net) filter (where r_net>0) / nullif(-sum(r_net) filter (where r_net<0),0))::numeric,2)
from rs where k=1.0 group by 1,2,3
order by 1,2,3;

-- Q8b: PDH-break LONG (stop = TL before fill bar)
with dy as (select * from dd where n>=20 and pl is not null and o between pl and ph),
 bs as (select b.*, min(low) over (partition by b.symbol, b.d order by ts rows between unbounded preceding and 1 preceding) tl_prev from bars b join dy using(symbol,d)),
 fl as (select bs.symbol, bs.d, min(ts) t_fill from bs join dy using(symbol,d) where ts::time >= '10:00' and ts::time <= '14:30' and high > dy.ph group by 1,2),
 flb as (select fl.*, bs.tl_prev from fl join bs on bs.symbol=fl.symbol and bs.d=fl.d and bs.ts=fl.t_fill),
 stl as (select b.symbol, b.d, min(b.ts) t_stop from bars b join flb using(symbol,d) where b.ts >= flb.t_fill and b.low < flb.tl_prev group by 1,2),
 k as (select * from (values (1.0),(2.0),(99.0)) v(k)),
 tgl as (select b.symbol, b.d, k.k, min(b.ts) t_tgt from bars b join flb using(symbol,d) join dy using(symbol,d) cross join k where b.ts >= flb.t_fill and b.high >= dy.ph + k.k*(dy.ph-flb.tl_prev) group by 1,2,3),
 rl as (select dy.symbol, dy.d, extract(year from dy.d)::int yr, k.k, (dy.ph-flb.tl_prev)/dy.ph risk_pct,
   case when stl.t_stop is not null and (tgl.t_tgt is null or stl.t_stop <= tgl.t_tgt) then -1.0 when tgl.t_tgt is not null then k.k else (dy.cl-dy.ph)/(dy.ph-flb.tl_prev) end
   - 0.0004/((dy.ph-flb.tl_prev)/dy.ph) r_net
   from dy join flb using(symbol,d) cross join k left join stl using(symbol,d) left join tgl on tgl.symbol=dy.symbol and tgl.d=dy.d and tgl.k=k.k
   where (dy.ph-flb.tl_prev)/dy.ph between 0.003 and 0.015)
select 'Q8b PDH-break LONG fill=PDH, risk 0.3-1.5%, net 4bp' q, k, yr, count(*) fills, round(avg(r_net)::numeric,3) avg_r_net, round((avg((r_net>0)::int)*100)::numeric,1) wr,
 round(((percentile_cont(0.5) within group (order by risk_pct))*100)::numeric,3) risk_med_pct, round(sum(r_net)::numeric,1) sum_r,
 round((sum(r_net) filter (where r_net>0) / nullif(-sum(r_net) filter (where r_net<0),0))::numeric,2) pf_r
from rl group by 1,2,3 order by 2,3;

-- -----------------------------------------------------------------------------
-- C. Q8c / Q8d — condição de Murphy (TO<PDC / TO>PDC), ex 12h, FILL HONESTO
-- -----------------------------------------------------------------------------
-- Q8c: PDL-break SHORT with Murphy condition TO<PDC (and TO>PDL), fills 10:00-14:30 excluding 12h, stop=TH before fill bar, fill=min(open,PDL), net 4bp; by year and freq by symbol
with dy as (select * from dd where n>=20 and pl is not null and o > pl and o < pc),
 bs as (select b.*, max(high) over (partition by b.symbol, b.d order by ts rows between unbounded preceding and 1 preceding) th_prev from bars b join dy using(symbol,d)),
 fs as (select bs.symbol, bs.d, min(ts) t_fill from bs join dy using(symbol,d) where ts::time >= '10:00' and ts::time <= '14:30' and not (ts::time >= '12:00' and ts::time < '13:00') and low < dy.pl group by 1,2),
 fsb as (select fs.*, bs.th_prev, least(bs.open, dy.pl) fill_px from fs join bs on bs.symbol=fs.symbol and bs.d=fs.d and bs.ts=fs.t_fill join dy on dy.symbol=fs.symbol and dy.d=fs.d),
 sts as (select b.symbol, b.d, min(b.ts) t_stop from bars b join fsb using(symbol,d) where b.ts >= fsb.t_fill and b.high > fsb.th_prev group by 1,2),
 k as (select * from (values (1.0),(1.5),(2.0)) v(k)),
 tgs as (select b.symbol, b.d, k.k, min(b.ts) t_tgt from bars b join fsb using(symbol,d) join dy using(symbol,d) cross join k where b.ts >= fsb.t_fill and b.low <= fsb.fill_px - k.k*(fsb.th_prev-fsb.fill_px) group by 1,2,3),
 rs as (select dy.symbol, dy.d, extract(year from dy.d)::int yr, k.k, (fsb.th_prev-fsb.fill_px)/fsb.fill_px risk_pct,
   case when sts.t_stop is not null and (tgs.t_tgt is null or sts.t_stop <= tgs.t_tgt) then -1.0 when tgs.t_tgt is not null then k.k else (fsb.fill_px-dy.cl)/(fsb.th_prev-fsb.fill_px) end
   - 0.0004/((fsb.th_prev-fsb.fill_px)/fsb.fill_px) r_net
   from dy join fsb using(symbol,d) cross join k left join sts using(symbol,d) left join tgs on tgs.symbol=dy.symbol and tgs.d=dy.d and tgs.k=k.k
   where (fsb.th_prev-fsb.fill_px)/fsb.fill_px between 0.003 and 0.015)
select 'Q8c PDL-break SHORT Murphy TO<PDC, ex12h, honest fill' q, k, yr::text grp, count(*) fills, round(avg(r_net)::numeric,3) avg_r_net, round((avg((r_net>0)::int)*100)::numeric,1) wr,
 round(((percentile_cont(0.5) within group (order by risk_pct))*100)::numeric,3) risk_med_pct, round(sum(r_net)::numeric,1) sum_r,
 round((sum(r_net) filter (where r_net>0) / nullif(-sum(r_net) filter (where r_net<0),0))::numeric,2) pf_r
from rs group by 1,2,3
union all
select 'Q8c by symbol k=1.5 (fills/18m)' q, k, symbol, count(*), round(avg(r_net)::numeric,3), round((avg((r_net>0)::int)*100)::numeric,1),
 round(((percentile_cont(0.5) within group (order by risk_pct))*100)::numeric,3), round(sum(r_net)::numeric,1),
 round((sum(r_net) filter (where r_net>0) / nullif(-sum(r_net) filter (where r_net<0),0))::numeric,2)
from rs where k=1.5 group by 1,2,3
order by 1,2,3;

-- Q8d: PDH-break LONG with Murphy TO>PDC (and TO<PDH), ex 12h, honest
with dy as (select * from dd where n>=20 and pl is not null and o < ph and o > pc),
 bs as (select b.*, min(low) over (partition by b.symbol, b.d order by ts rows between unbounded preceding and 1 preceding) tl_prev from bars b join dy using(symbol,d)),
 fl as (select bs.symbol, bs.d, min(ts) t_fill from bs join dy using(symbol,d) where ts::time >= '10:00' and ts::time <= '14:30' and not (ts::time >= '12:00' and ts::time < '13:00') and high > dy.ph group by 1,2),
 flb as (select fl.*, bs.tl_prev, greatest(bs.open, dy.ph) fill_px from fl join bs on bs.symbol=fl.symbol and bs.d=fl.d and bs.ts=fl.t_fill join dy on dy.symbol=fl.symbol and dy.d=fl.d),
 stl as (select b.symbol, b.d, min(b.ts) t_stop from bars b join flb using(symbol,d) where b.ts >= flb.t_fill and b.low < flb.tl_prev group by 1,2),
 k as (select * from (values (1.0),(1.5),(2.0)) v(k)),
 tgl as (select b.symbol, b.d, k.k, min(b.ts) t_tgt from bars b join flb using(symbol,d) cross join k where b.ts >= flb.t_fill and b.high >= flb.fill_px + k.k*(flb.fill_px-flb.tl_prev) group by 1,2,3),
 rl as (select dy.symbol, dy.d, extract(year from dy.d)::int yr, k.k, (flb.fill_px-flb.tl_prev)/flb.fill_px risk_pct,
   case when stl.t_stop is not null and (tgl.t_tgt is null or stl.t_stop <= tgl.t_tgt) then -1.0 when tgl.t_tgt is not null then k.k else (dy.cl-flb.fill_px)/(flb.fill_px-flb.tl_prev) end
   - 0.0004/((flb.fill_px-flb.tl_prev)/flb.fill_px) r_net
   from dy join flb using(symbol,d) cross join k left join stl using(symbol,d) left join tgl on tgl.symbol=dy.symbol and tgl.d=dy.d and tgl.k=k.k
   where (flb.fill_px-flb.tl_prev)/flb.fill_px between 0.003 and 0.015)
select 'Q8d PDH-break LONG Murphy TO>PDC, ex12h, honest fill' q, k, yr, count(*) fills, round(avg(r_net)::numeric,3) avg_r_net, round((avg((r_net>0)::int)*100)::numeric,1) wr,
 round(((percentile_cont(0.5) within group (order by risk_pct))*100)::numeric,3) risk_med_pct, round(sum(r_net)::numeric,1) sum_r,
 round((sum(r_net) filter (where r_net>0) / nullif(-sum(r_net) filter (where r_net<0),0))::numeric,2) pf_r
from rl group by 1,2,3 order by 2,3;

-- -----------------------------------------------------------------------------
-- D. Q12 / Q12b — sem condição de PDC, ex 12h, fill honesto (bloco re-executado
--    pelos críticos em 06/09/2026: k=1 → 2025 n=499 avgR −0,034 PF_R 0,90;
--    2026 n=344 +0,034 PF_R 1,11)
-- -----------------------------------------------------------------------------
-- Q12: PDL-break SHORT, open inside yesterday range (no PDC cond), fills 10:00-14:30 ex 12h, honest fill=min(open,PDL), stop=TH before fill bar, risk 0.3-1.5%, net 4bp; split by TO vs PDC; plus confirmed-close variant
with dy as (select * from dd where n>=20 and pl is not null and o > pl and o < ph),
 bs as (select b.*, max(high) over (partition by b.symbol, b.d order by ts rows between unbounded preceding and 1 preceding) th_prev from bars b join dy using(symbol,d)),
 fs as (select bs.symbol, bs.d, min(ts) t_fill from bs join dy using(symbol,d) where ts::time >= '10:00' and ts::time <= '14:30' and not (ts::time >= '12:00' and ts::time < '13:00') and low < dy.pl group by 1,2),
 fsb as (select fs.*, bs.th_prev, least(bs.open, dy.pl) fill_px, (dy.o < dy.pc) open_below_pdc from fs join bs on bs.symbol=fs.symbol and bs.d=fs.d and bs.ts=fs.t_fill join dy on dy.symbol=fs.symbol and dy.d=fs.d),
 sts as (select b.symbol, b.d, min(b.ts) t_stop from bars b join fsb using(symbol,d) where b.ts >= fsb.t_fill and b.high > fsb.th_prev group by 1,2),
 k as (select * from (values (1.0),(1.5),(99.0)) v(k)),
 tgs as (select b.symbol, b.d, k.k, min(b.ts) t_tgt from bars b join fsb using(symbol,d) cross join k where b.ts >= fsb.t_fill and b.low <= fsb.fill_px - k.k*(fsb.th_prev-fsb.fill_px) group by 1,2,3),
 rs as (select dy.symbol, dy.d, extract(year from dy.d)::int yr, k.k, fsb.open_below_pdc, (fsb.th_prev-fsb.fill_px)/fsb.fill_px risk_pct,
   case when sts.t_stop is not null and (tgs.t_tgt is null or sts.t_stop <= tgs.t_tgt) then -1.0 when tgs.t_tgt is not null then k.k else (fsb.fill_px-dy.cl)/(fsb.th_prev-fsb.fill_px) end
   - 0.0004/((fsb.th_prev-fsb.fill_px)/fsb.fill_px) r_net
   from dy join fsb using(symbol,d) cross join k left join sts using(symbol,d) left join tgs on tgs.symbol=dy.symbol and tgs.d=dy.d and tgs.k=k.k
   where (fsb.th_prev-fsb.fill_px)/fsb.fill_px between 0.003 and 0.015)
select 'Q12 PDL-break SHORT no-PDC-cond ex12h honest' q, k, yr::text grp, open_below_pdc, count(*) fills, round(avg(r_net)::numeric,3) avg_r_net, round((avg((r_net>0)::int)*100)::numeric,1) wr,
 round(((percentile_cont(0.5) within group (order by risk_pct))*100)::numeric,3) risk_med_pct, round(sum(r_net)::numeric,1) sum_r,
 round((sum(r_net) filter (where r_net>0) / nullif(-sum(r_net) filter (where r_net<0),0))::numeric,2) pf_r
from rs group by 1,2,3,4
union all
select 'Q12 PDL-break SHORT no-PDC-cond ex12h honest ALL' q, k, yr::text, null, count(*), round(avg(r_net)::numeric,3), round((avg((r_net>0)::int)*100)::numeric,1),
 round(((percentile_cont(0.5) within group (order by risk_pct))*100)::numeric,3), round(sum(r_net)::numeric,1),
 round((sum(r_net) filter (where r_net>0) / nullif(-sum(r_net) filter (where r_net<0),0))::numeric,2)
from rs group by 1,2,3
order by 1,2,3,4;

-- Q12b: confirmed variant: first bar (10:00-14:30 ex 12h) that CLOSES below PDL; entry sell stop at that bar low - tick, valid next 2 bars; stop = TH before entry; target k*R
with dy as (select * from dd where n>=20 and pl is not null and o > pl and o < ph),
 bs as (select b.*, max(high) over (partition by b.symbol, b.d order by ts rows unbounded preceding) th_incl from bars b join dy using(symbol,d)),
 sig as (select bs.symbol, bs.d, min(ts) t_sig from bs join dy using(symbol,d) where ts::time >= '10:00' and ts::time <= '14:30' and not (ts::time >= '12:00' and ts::time < '13:00') and close < dy.pl group by 1,2),
 sgb as (select sig.*, bs.low sig_low, bs.th_incl th_sig from sig join bs on bs.symbol=sig.symbol and bs.d=sig.d and bs.ts=sig.t_sig),
 f as (select sgb.symbol, sgb.d, sgb.sig_low, sgb.th_sig, min(b.ts) t_fill from bars b join sgb on b.symbol=sgb.symbol and b.d=sgb.d and b.ts > sgb.t_sig and b.ts <= sgb.t_sig + interval '30 minutes' where b.low < sgb.sig_low group by 1,2,3,4),
 fb as (select f.*, least(b.open, f.sig_low) fill_px from f join bars b on b.symbol=f.symbol and b.d=f.d and b.ts=f.t_fill),
 st as (select b.symbol, b.d, min(b.ts) t_stop from bars b join fb using(symbol,d) where b.ts >= fb.t_fill and b.high > fb.th_sig group by 1,2),
 k as (select * from (values (1.0),(1.5),(99.0)) v(k)),
 tg as (select b.symbol, b.d, k.k, min(b.ts) t_tgt from bars b join fb using(symbol,d) cross join k where b.ts >= fb.t_fill and b.low <= fb.fill_px - k.k*(fb.th_sig-fb.fill_px) group by 1,2,3),
 rs as (select dy.symbol, dy.d, extract(year from dy.d)::int yr, k.k, (fb.th_sig-fb.fill_px)/fb.fill_px risk_pct,
   case when st.t_stop is not null and (tg.t_tgt is null or st.t_stop <= tg.t_tgt) then -1.0 when tg.t_tgt is not null then k.k else (fb.fill_px-dy.cl)/(fb.th_sig-fb.fill_px) end
   - 0.0004/((fb.th_sig-fb.fill_px)/fb.fill_px) r_net
   from dy join fb using(symbol,d) cross join k left join st using(symbol,d) left join tg on tg.symbol=dy.symbol and tg.d=dy.d and tg.k=k.k
   where (fb.th_sig-fb.fill_px)/fb.fill_px between 0.003 and 0.015)
select 'Q12b PDL close-confirmed SHORT entry below signal bar' q, k, yr, count(*) fills, round(avg(r_net)::numeric,3) avg_r_net, round((avg((r_net>0)::int)*100)::numeric,1) wr,
 round(((percentile_cont(0.5) within group (order by risk_pct))*100)::numeric,3) risk_med_pct, round(sum(r_net)::numeric,1) sum_r,
 round((sum(r_net) filter (where r_net>0) / nullif(-sum(r_net) filter (where r_net<0),0))::numeric,2) pf_r
from rs group by 1,2,3 order by 2,3;
