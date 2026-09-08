-- =============================================================================
-- 04-gap-fade.sql — Gap de abertura: continuação × preenchimento (gap-fade)
-- -----------------------------------------------------------------------------
-- Setup    : gap-down ≥ 1% (ou ≥ 0,8 × ATRd) na abertura; se a 1ª hora "dirige
--            de volta" (fechamento da barra 10:15 acima da abertura), compra o
--            rompimento (a) da máxima da 1ª hora (Q2) ou (b) da máxima da barra
--            das 10:15, válido 2 barras (Q2b); stop na mínima da 1ª hora; alvo
--            k × R; saída no fechamento.
-- Fonte    : Dalton, Mind over Markets, Cap. 4 Seção I (tipos de abertura;
--            Open-Drive / Open-Test-Drive) e Setup C "Gap (continuação ou
--            preenchimento)" — análise docs/books/analysis/dalton-mind-over-markets.md
--            §2.5 e "Setup C — Gap" (candidata `gap-continuation-v1`). O lado
--            "fade" (comprar o gap-down que reverte) é o Setup C invertido =
--            interpretação nossa; limiares 1% / 0,8 × ATRd idem.
-- Hipótese : gap grande com resposta na 1ª hora preenche em direção ao
--            fechamento de ontem com R ≥ 1.
-- Parâmetros: Q1 buckets de gap por ano, ex mar–abr/2025 e por "drive back" vs
--            "continua" (estatística, sem modelo de fill); Q2 fill no extremo
--            da 1ª hora, sem custo, válido 10:30–13:00 (otimista); Q2b fill
--            honesto (max(open, máx. 10:15)), válido 10:30–10:45, 4 bp, alvos
--            1 / 1,5 / 2, com corte "crash" (mar–abr/2025).
-- Blocos (N de variantes = 2 regras de trade + 1 estatística com 4 buckets):
--   Q1, Q2, Q2b.
-- Resultado 06/09/2026 (designer de setups): Q2b — avg R +0,09 (2025 ex-crash)
--   / −0,04 (2026). Q1 e Q2: sem número registrado nos relatórios — a medir.
--   Veredito: ≈ 0, REPROVADO ("gap-continuation/gap-fade", 3º no ranking de
--   Dalton). Referência do banco (sql/stats/05-gaps-de-abertura.sql): gap
--   mediano e taxa de preenchimento por símbolo.
-- Uso      : docker exec -i trader-postgres psql -U trader -d trader_db < sql/screens/04-gap-fade.sql
-- Fuso     : America/New_York. Cópia fiel de q1.sql, q2.sql Q2 e q4.sql Q2b.
-- =============================================================================
\set ON_ERROR_STOP on

-- Q1 — estatística condicional por bucket de gap (autocontida, sem tabelas temp)
with c as (
  select a.symbol, c.timestamp at time zone 'America/New_York' as ts, c.open, c.high, c.low, c.close
  from candles c join assets a on a.id=c.asset_id where c.timeframe='15m'
  and a.symbol in ('AVUV','IJS','IWM','IWN','IWO','IWV','SLYV','VBR','IJR','SCHA','VB')
), d as (
  select symbol, ts::date as d,
    (array_agg(open order by ts))[1] as o, max(high) h, min(low) l,
    (array_agg(close order by ts desc))[1] as cl,
    max(high) filter (where ts::time < '10:30') h1h,
    min(low) filter (where ts::time < '10:30') h1l,
    (array_agg(close order by ts))[4] as h1c,
    max(high) filter (where ts::time >= '10:30') rh,
    min(low) filter (where ts::time >= '10:30') rl,
    count(*) n
  from c group by 1,2
), dd as (
  select *, lag(cl) over w pc, lag(h) over w ph, lag(l) over w pl,
    avg((h-l)/cl) over (partition by symbol order by d rows between 14 preceding and 1 preceding) atrd
  from d window w as (partition by symbol order by d)
), g as (
  select *, (o-pc)/pc gap, (cl-h1c)/h1c ret_after_h1, (h1c-h1l)/h1c dist_h1l, (h1h-h1c)/h1c dist_h1h, (h1h-h1l)/h1c h1r
  from dd where n>=20 and pc is not null and atrd is not null
), s as (
  select case
    when gap < -0.01 then 'A gapdown<-1%'
    when gap < -0.8*atrd then 'B gapdown<-0.8atrd(not<-1%)'
    when gap > 0.01 then 'C gapup>+1%'
    when gap > 0.8*atrd then 'D gapup>+0.8atrd(not>1%)'
    else 'Z rest' end k, * from g
)
select k, extract(year from d)::int yr, count(*) n,
  round((avg(ret_after_h1)*100)::numeric,3) ret_after_h1_pct,
  round((avg((cl>h1c)::int)*100)::numeric,1) pos_pct,
  round((avg((rl<h1l)::int)*100)::numeric,1) h1low_broken_pct,
  round((avg((rh>h1h)::int)*100)::numeric,1) h1high_broken_pct,
  round(((percentile_cont(0.5) within group (order by dist_h1l))*100)::numeric,3) stop_to_h1low_med_pct,
  round(((percentile_cont(0.5) within group (order by dist_h1h))*100)::numeric,3) stop_to_h1high_med_pct,
  round(((percentile_cont(0.5) within group (order by h1r))*100)::numeric,3) h1range_med_pct
from s group by 1,2
union all
select k||' exMarAbr25', 0, count(*),
  round((avg(ret_after_h1)*100)::numeric,3), round((avg((cl>h1c)::int)*100)::numeric,1),
  round((avg((rl<h1l)::int)*100)::numeric,1), round((avg((rh>h1h)::int)*100)::numeric,1),
  round(((percentile_cont(0.5) within group (order by dist_h1l))*100)::numeric,3),
  round(((percentile_cont(0.5) within group (order by dist_h1h))*100)::numeric,3),
  round(((percentile_cont(0.5) within group (order by h1r))*100)::numeric,3)
from s where d not between '2025-03-01' and '2025-04-30' and k <> 'Z rest' group by 1
union all
select k||' & H1 drive back (gapdown: h1c>o; gapup: h1c<o)', 0, count(*),
  round((avg(ret_after_h1)*100)::numeric,3), round((avg((cl>h1c)::int)*100)::numeric,1),
  round((avg((rl<h1l)::int)*100)::numeric,1), round((avg((rh>h1h)::int)*100)::numeric,1),
  round(((percentile_cont(0.5) within group (order by dist_h1l))*100)::numeric,3),
  round(((percentile_cont(0.5) within group (order by dist_h1h))*100)::numeric,3),
  round(((percentile_cont(0.5) within group (order by h1r))*100)::numeric,3)
from s where k in ('A gapdown<-1%','B gapdown<-0.8atrd(not<-1%)') and h1c > o or k in ('C gapup>+1%','D gapup>+0.8atrd(not>1%)') and h1c < o group by 1
union all
select k||' & H1 continues (gapdown: h1c<=o; gapup: h1c>=o)', 0, count(*),
  round((avg(ret_after_h1)*100)::numeric,3), round((avg((cl>h1c)::int)*100)::numeric,1),
  round((avg((rl<h1l)::int)*100)::numeric,1), round((avg((rh>h1h)::int)*100)::numeric,1),
  round(((percentile_cont(0.5) within group (order by dist_h1l))*100)::numeric,3),
  round(((percentile_cont(0.5) within group (order by dist_h1h))*100)::numeric,3),
  round(((percentile_cont(0.5) within group (order by h1r))*100)::numeric,3)
from s where k in ('A gapdown<-1%','B gapdown<-0.8atrd(not<-1%)') and h1c <= o or k in ('C gapup>+1%','D gapup>+0.8atrd(not>1%)') and h1c >= o group by 1
order by 1,2;

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

-- Q2: gap-down >=1% + drive back; long buy stop at H1 high (valid 10:30-13:00), stop H1 low, targets k*R or hold to close  (OTIMISTA: fill no extremo, sem custo)
with dy as (select * from dd where n>=20 and pc is not null and (o-pc)/pc < -0.01 and h1c > o),
 f as (select b.symbol, b.d, min(b.ts) t_fill from bars b join dy using(symbol,d) where b.ts::time >= '10:30' and b.ts::time <= '13:00' and b.high > dy.h1h group by 1,2),
 st as (select b.symbol, b.d, min(b.ts) t_stop from bars b join f using(symbol,d) join dy using(symbol,d) where b.ts >= f.t_fill and b.low < dy.h1l group by 1,2),
 k as (select * from (values (1.0),(1.5),(2.0),(99.0)) v(k)),
 tg as (select b.symbol, b.d, k.k, min(b.ts) t_tgt from bars b join f using(symbol,d) join dy using(symbol,d) cross join k where b.ts >= f.t_fill and b.high >= dy.h1h + k.k*(dy.h1h-dy.h1l) group by 1,2,3),
 r as (select dy.symbol, dy.d, extract(year from dy.d)::int yr, k.k, (dy.h1h-dy.h1l)/dy.h1h risk_pct, f.t_fill, st.t_stop, tg.t_tgt,
   case when st.t_stop is not null and (tg.t_tgt is null or st.t_stop <= tg.t_tgt) then -1.0
        when tg.t_tgt is not null then k.k
        else (dy.cl - dy.h1h)/(dy.h1h-dy.h1l) end r_mult
   from dy join f using(symbol,d) cross join k left join st using(symbol,d) left join tg on tg.symbol=dy.symbol and tg.d=dy.d and tg.k=k.k)
select 'Q2 gapdown-driveback LONG entry h1h stop h1l (OTIMISTA)' q, k, yr, count(*) fills, round(avg(r_mult)::numeric,3) avg_r, round((avg((r_mult>0)::int)*100)::numeric,1) wr,
 round(((percentile_cont(0.5) within group (order by risk_pct))*100)::numeric,3) risk_med_pct, round(sum(r_mult)::numeric,1) sum_r
from r group by 1,2,3 order by 2,3;

-- Q2b: gap-down<-1% drive back; entry above the 10:15 bar high (valid 2 bars), stop H1 low, targets 1R / gap-fill(pc) capped at 2R  (fill honesto, 4 bp)
with dy as (select * from dd where n>=20 and pc is not null and (o-pc)/pc < -0.01 and h1c > o),
 f as (select b.symbol, b.d, min(b.ts) t_fill from bars b join dy using(symbol,d) where b.ts::time >= '10:30' and b.ts::time <= '10:45' and b.high > dy.b4h group by 1,2),
 fb as (select f.*, greatest(b.open, dy.b4h) fill_px from f join bars b on b.symbol=f.symbol and b.d=f.d and b.ts=f.t_fill join dy on dy.symbol=f.symbol and dy.d=f.d),
 st as (select b.symbol, b.d, min(b.ts) t_stop from bars b join fb using(symbol,d) join dy using(symbol,d) where b.ts >= fb.t_fill and b.low < dy.h1l group by 1,2),
 k as (select * from (values (1.0),(1.5),(2.0)) v(k)),
 tg as (select b.symbol, b.d, k.k, min(b.ts) t_tgt from bars b join fb using(symbol,d) join dy using(symbol,d) cross join k where b.ts >= fb.t_fill and b.high >= fb.fill_px + k.k*(fb.fill_px-dy.h1l) group by 1,2,3),
 r as (select dy.symbol, dy.d, extract(year from dy.d)::int yr, (dy.d between '2025-03-01' and '2025-04-30') crash, k.k, (fb.fill_px-dy.h1l)/fb.fill_px risk_pct,
   case when st.t_stop is not null and (tg.t_tgt is null or st.t_stop <= tg.t_tgt) then -1.0 when tg.t_tgt is not null then k.k else (dy.cl - fb.fill_px)/(fb.fill_px-dy.h1l) end - 0.0004/((fb.fill_px-dy.h1l)/fb.fill_px) r_net
   from dy join fb using(symbol,d) cross join k left join st using(symbol,d) left join tg on tg.symbol=dy.symbol and tg.d=dy.d and tg.k=k.k)
select 'Q2b gapdown driveback LONG entry above 10:15 bar, stop h1l, net' q, k, yr, crash, count(*) fills, round(avg(r_net)::numeric,3) avg_r, round((avg((r_net>0)::int)*100)::numeric,1) wr,
 round(((percentile_cont(0.5) within group (order by risk_pct))*100)::numeric,3) risk_med_pct, round(sum(r_net)::numeric,1) sum_r
from r group by 1,2,3,4 order by 2,3,4;
