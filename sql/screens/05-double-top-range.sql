-- =============================================================================
-- 05-double-top-range.sql — Double-top / double-bottom "solto" em dia de range
-- -----------------------------------------------------------------------------
-- Setup    : em dia de range (range do dia até a barra < 1,2 × ATRd), a partir
--            da 5ª barra, entre 10:30 e 15:00 ET: uma barra que volta à máxima
--            do dia (± 0,10 × ATRd) depois de um recuo ≥ 0,30 × ATRd nas 6 barras
--            anteriores, com BARRA DE SINAL de baixa (fecha no terço inferior,
--            corpo ≥ 30% do range) → sell stop abaixo da mínima dela (válido 2
--            barras); stop = max(máxima da barra, máxima do dia); alvo k × R.
--            Simétrico para double-bottom (long). Fill honesto, 4 bp.
-- Fonte    : Brooks, Reading Price Charts Bar by Bar — "Double Bottom/Double Top
--            Pullback" (análise docs/books/analysis/brooks-bar-by-bar.md §2.4);
--            barra de sinal como na `range-extreme-fade-v1` (corpo ≥ 30%,
--            fechamento no terço). Tolerâncias em ATRd = interpretação nossa.
-- Hipótese : a segunda visita ao extremo do dia em dia de range falha e paga
--            1–1,5 R — mesma família da range-extreme-fade, sem o detector de
--            "novo extremo ≤ 0,5 × ATR14" nem os vetos dela.
-- Parâmetros: alvos 1 / 1,5 / 2; risco mínimo 0,15%; sem teto de risco.
-- Blocos (N de variantes = 1 regra, 2 direções, 3 alvos): Q16.
-- Resultado 06/09/2026 (designer de setups): PF 0,37–0,86 com stops de ~0,3%.
--   Veredito: REPROVADO — o edge da range-fade está no detector de dia de range
--   + barra de sinal + vetos, não em padrões soltos (plano-mestre §2.1, §6.1).
-- Uso      : docker exec -i trader-postgres psql -U trader -d trader_db < sql/screens/05-double-top-range.sql
-- Fuso     : America/New_York. Cópia fiel de q6.sql Q16.
-- =============================================================================
\set ON_ERROR_STOP on

create temp table bars as
select a.symbol, c.timestamp at time zone 'America/New_York' as ts, (c.timestamp at time zone 'America/New_York')::date d, c.open, c.high, c.low, c.close, c.volume
from candles c join assets a on a.id=c.asset_id where c.timeframe='15m' and a.symbol in ('AVUV','IJS','IWM','IWN','IWO','IWV','SLYV','VBR','IJR','SCHA','VB');
create index on bars(symbol,d,ts);
create temp table days as select symbol, d, max(high) h, min(low) l, (array_agg(close order by ts desc))[1] cl, count(*) n from bars group by 1,2;
create temp table dd as select *, avg((h-l)/cl) over (partition by symbol order by d rows between 14 preceding and 1 preceding) atrd from days;

-- Q16: Brooks double-top second entry (short) / double-bottom (long) inside range-like days, honest fill, net 4bp
with b as (select b.*, dd.atrd*dd.cl atr_px, dd.cl dcl,
   max(high) over (partition by b.symbol, b.d order by ts rows between unbounded preceding and 1 preceding) dh_prev,
   min(low) over (partition by b.symbol, b.d order by ts rows between unbounded preceding and 1 preceding) dl_prev,
   min(low) over (partition by b.symbol, b.d order by ts rows between 6 preceding and 1 preceding) l6,
   max(high) over (partition by b.symbol, b.d order by ts rows between 6 preceding and 1 preceding) h6,
   row_number() over (partition by b.symbol, b.d order by ts) rn
  from bars b join dd using(symbol,d) where dd.atrd is not null),
 sig as (select *,
   case when ts::time between '10:30' and '15:00' and rn >= 5 and (dh_prev-dl_prev) < 1.2*atr_px
         and high >= dh_prev - 0.10*atr_px and high <= dh_prev + 0.10*atr_px and l6 <= dh_prev - 0.30*atr_px
         and high > low and close <= low + (high-low)/3 and abs(close-open) >= 0.30*(high-low) then -1
        when ts::time between '10:30' and '15:00' and rn >= 5 and (dh_prev-dl_prev) < 1.2*atr_px
         and low <= dl_prev + 0.10*atr_px and low >= dl_prev - 0.10*atr_px and h6 >= dl_prev + 0.30*atr_px
         and high > low and close >= high - (high-low)/3 and abs(close-open) >= 0.30*(high-low) then 1 end dir
   from b),
 s as (select symbol, d, ts, dir, high sh, low sl, atr_px, greatest(high, dh_prev) stop_short, least(low, dl_prev) stop_long from sig where dir is not null),
 f as (select s.*, min(x.ts) t_fill from s join bars x on x.symbol=s.symbol and x.d=s.d and x.ts > s.ts and x.ts <= s.ts + interval '30 minutes'
       where (s.dir=-1 and x.low < s.sl) or (s.dir=1 and x.high > s.sh) group by s.symbol, s.d, s.ts, s.dir, s.sh, s.sl, s.atr_px, s.stop_short, s.stop_long),
 fb as (select f.*, case when f.dir=-1 then least(x.open, f.sl) else greatest(x.open, f.sh) end fill_px,
        case when f.dir=-1 then f.stop_short else f.stop_long end stop_px from f join bars x on x.symbol=f.symbol and x.d=f.d and x.ts=f.t_fill),
 st as (select fb.symbol, fb.d, fb.ts, min(x.ts) t_stop from bars x join fb on x.symbol=fb.symbol and x.d=fb.d and x.ts >= fb.t_fill where (fb.dir=-1 and x.high > fb.stop_px) or (fb.dir=1 and x.low < fb.stop_px) group by 1,2,3),
 k as (select * from (values (1.0),(1.5),(2.0)) v(k)),
 tg as (select fb.symbol, fb.d, fb.ts, k.k, min(x.ts) t_tgt from bars x join fb on x.symbol=fb.symbol and x.d=fb.d and x.ts >= fb.t_fill cross join k
        where (fb.dir=-1 and x.low <= fb.fill_px - k.k*(fb.stop_px-fb.fill_px)) or (fb.dir=1 and x.high >= fb.fill_px + k.k*(fb.fill_px-fb.stop_px)) group by 1,2,3,4),
 r as (select fb.symbol, fb.d, extract(year from fb.d)::int yr, fb.dir, k.k, abs(fb.stop_px-fb.fill_px)/fb.fill_px risk_pct,
   case when st.t_stop is not null and (tg.t_tgt is null or st.t_stop <= tg.t_tgt) then -1.0 when tg.t_tgt is not null then k.k else 0.0 end - 0.0004/nullif(abs(fb.stop_px-fb.fill_px)/fb.fill_px,0) r_net
   from fb cross join k left join st on st.symbol=fb.symbol and st.d=fb.d and st.ts=fb.ts left join tg on tg.symbol=fb.symbol and tg.d=fb.d and tg.ts=fb.ts and tg.k=k.k
   where abs(fb.stop_px-fb.fill_px)/fb.fill_px >= 0.0015)
select 'Q16 double-top/bottom 2nd entry in range day, honest net' q, k, yr, dir, count(*) fills, round(avg(r_net)::numeric,3) avg_r_net, round((avg((r_net>0)::int)*100)::numeric,1) wr,
 round(((percentile_cont(0.5) within group (order by risk_pct))*100)::numeric,3) risk_med_pct, round(sum(r_net)::numeric,1) sum_r,
 round((sum(r_net) filter (where r_net>0) / nullif(-sum(r_net) filter (where r_net<0),0))::numeric,2) pf_r
from r group by 1,2,3,4
union all
select 'Q16 by symbol k=1.5 both dirs' q, k, null, null, count(*), round(avg(r_net)::numeric,3), round((avg((r_net>0)::int)*100)::numeric,1),
 round(((percentile_cont(0.5) within group (order by risk_pct))*100)::numeric,3), round(sum(r_net)::numeric,1),
 round((sum(r_net) filter (where r_net>0) / nullif(-sum(r_net) filter (where r_net<0),0))::numeric,2)
from r where k=1.5 group by 1,2, symbol
order by 1,2,3,4;
