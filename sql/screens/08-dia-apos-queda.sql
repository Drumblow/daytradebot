-- =============================================================================
-- 08-dia-apos-queda.sql — Dia seguinte a uma queda close-to-close ≥ 2%
--                          ("dia após clímax de venda")
-- -----------------------------------------------------------------------------
-- Setup    : ontem caiu ≥ 2% (fechamento a fechamento). Hoje, compra o
--            rompimento (a) da máxima da 1ª barra (09:30), válido 09:45–11:00,
--            stop na mínima dela (Q7); ou (b) da máxima da 1ª hora, válido
--            10:30–12:00, stop na mínima da 1ª hora (Q7b). Alvo k × R ou
--            segurar até o fechamento. Q6 é a estatística de referência:
--            retorno open→close de hoje por bucket da variação de ontem.
-- Fonte    : Murphy, Cap. 4 — reversal day / key reversal / selling climax
--            (análise docs/books/analysis/murphy-technical-analysis.md §3.6).
--            Limiar 2% e mecânica de entrada = interpretação nossa.
-- Hipótese : após clímax de venda, o dia seguinte tem drift positivo suficiente
--            para pagar 1–1,5 R num rompimento cedo.
-- Parâmetros: Q7 sem custo e fill no extremo (otimista); Q7b idem; ambos com
--            corte "crash" (mar–abr/2025) para separar regime; alvos 1 / 1,5 /
--            2 / 99. Nenhum dos dois aplica custo nem fill honesto — o veredito
--            já é negativo em 2026 mesmo assim.
-- Blocos (N de variantes = 2 regras + 1 estatística com 5 buckets): Q6, Q7, Q7b.
-- Resultado 06/09/2026 (designer de setups): avg R +0,55 (2025 ex-crash) /
--   −0,09 (2026). Veredito: REGIME, NÃO EDGE — o sinal vem do bull de 2025 e
--   some em 2026 (mesma leitura do lado short da opening-reversal, plano §2.3
--   achado 2).
-- Uso      : docker exec -i trader-postgres psql -U trader -d trader_db < sql/screens/08-dia-apos-queda.sql
-- Fuso     : America/New_York. Cópia fiel de q2.sql Q6 e q3.sql Q7/Q7b.
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

-- Q6: next-day open->close return conditioned on previous day close-to-close return
select 'Q6 prev-day cc bucket' q, case when (pc-pc2)/pc2 < -0.02 then 'a <-2%' when (pc-pc2)/pc2 < -0.01 then 'b -2..-1%' when (pc-pc2)/pc2 > 0.02 then 'e >+2%' when (pc-pc2)/pc2 > 0.01 then 'd +1..+2%' else 'c mid' end b,
 count(*) n, round((avg((cl-o)/o)*100)::numeric,3) oc_ret_pct, round((avg((cl>o)::int)*100)::numeric,1) pos_pct,
 round((avg((cl-o)/o) filter (where d not between '2025-03-01' and '2025-04-30')*100)::numeric,3) oc_ret_ex_marabr25, count(*) filter (where d not between '2025-03-01' and '2025-04-30') n_ex,
 round((avg((cl-h1c)/h1c)*100)::numeric,3) ret_after_h1_pct
from dd where n>=20 and pc2 is not null group by 2 order by 2;

-- Q7: day after cc drop >= 2% (and >=1.5*atrd variant): long buy stop above first bar (9:30) high, valid 9:45-11:00, stop = first bar low, targets k*R / hold to close
with dy as (select *, (pc-pc2)/pc2 ccr from dd where n>=20 and pc2 is not null),
 sel as (select * from dy where ccr < -0.02),
 f as (select b.symbol, b.d, min(b.ts) t_fill from bars b join sel using(symbol,d) where b.ts::time >= '09:45' and b.ts::time <= '11:00' and b.high > sel.b1h group by 1,2),
 st as (select b.symbol, b.d, min(b.ts) t_stop from bars b join f using(symbol,d) join sel using(symbol,d) where b.ts >= f.t_fill and b.low < sel.b1l group by 1,2),
 k as (select * from (values (1.0),(1.5),(2.0),(99.0)) v(k)),
 tg as (select b.symbol, b.d, k.k, min(b.ts) t_tgt from bars b join f using(symbol,d) join sel using(symbol,d) cross join k where b.ts >= f.t_fill and b.high >= sel.b1h + k.k*(sel.b1h-sel.b1l) group by 1,2,3),
 r as (select sel.symbol, sel.d, extract(year from sel.d)::int yr, (sel.d between '2025-03-01' and '2025-04-30') crash, k.k, (sel.b1h-sel.b1l)/sel.b1h risk_pct,
   case when st.t_stop is not null and (tg.t_tgt is null or st.t_stop <= tg.t_tgt) then -1.0 when tg.t_tgt is not null then k.k else (sel.cl - sel.b1h)/(sel.b1h-sel.b1l) end r_mult
   from sel join f using(symbol,d) cross join k left join st using(symbol,d) left join tg on tg.symbol=sel.symbol and tg.d=sel.d and tg.k=k.k)
select 'Q7 after cc<-2%: LONG entry b1h stop b1l' q, k, yr, crash, count(*) fills, round(avg(r_mult)::numeric,3) avg_r, round((avg((r_mult>0)::int)*100)::numeric,1) wr,
 round(((percentile_cont(0.5) within group (order by risk_pct))*100)::numeric,3) risk_med_pct, round(sum(r_mult)::numeric,1) sum_r
from r group by 1,2,3,4 order by 2,3,4;

-- Q7b: same but entry above H1 high (drive back), valid 10:30-12:00, stop H1 low
with dy as (select *, (pc-pc2)/pc2 ccr from dd where n>=20 and pc2 is not null),
 sel as (select * from dy where ccr < -0.02),
 f as (select b.symbol, b.d, min(b.ts) t_fill from bars b join sel using(symbol,d) where b.ts::time >= '10:30' and b.ts::time <= '12:00' and b.high > sel.h1h group by 1,2),
 st as (select b.symbol, b.d, min(b.ts) t_stop from bars b join f using(symbol,d) join sel using(symbol,d) where b.ts >= f.t_fill and b.low < sel.h1l group by 1,2),
 k as (select * from (values (1.0),(2.0),(99.0)) v(k)),
 tg as (select b.symbol, b.d, k.k, min(b.ts) t_tgt from bars b join f using(symbol,d) join sel using(symbol,d) cross join k where b.ts >= f.t_fill and b.high >= sel.h1h + k.k*(sel.h1h-sel.h1l) group by 1,2,3),
 r as (select sel.symbol, sel.d, extract(year from sel.d)::int yr, (sel.d between '2025-03-01' and '2025-04-30') crash, k.k, (sel.h1h-sel.h1l)/sel.h1h risk_pct,
   case when st.t_stop is not null and (tg.t_tgt is null or st.t_stop <= tg.t_tgt) then -1.0 when tg.t_tgt is not null then k.k else (sel.cl - sel.h1h)/(sel.h1h-sel.h1l) end r_mult
   from sel join f using(symbol,d) cross join k left join st using(symbol,d) left join tg on tg.symbol=sel.symbol and tg.d=sel.d and tg.k=k.k)
select 'Q7b after cc<-2%: LONG entry h1h stop h1l' q, k, yr, crash, count(*) fills, round(avg(r_mult)::numeric,3) avg_r, round((avg((r_mult>0)::int)*100)::numeric,1) wr,
 round(((percentile_cont(0.5) within group (order by risk_pct))*100)::numeric,3) risk_med_pct, round(sum(r_mult)::numeric,1) sum_r
from r group by 1,2,3,4 order by 2,3,4;
