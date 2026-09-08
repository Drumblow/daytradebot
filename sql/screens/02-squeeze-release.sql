-- =============================================================================
-- 02-squeeze-release.sql — Release de squeeze (compressão de volatilidade) em 15m
-- -----------------------------------------------------------------------------
-- Setup    : squeeze = Bollinger(20, 2σ) dentro do canal SMA20 ± 1,5 × média do
--            range de 20 barras (proxy de Keltner) por ≥ 6 barras; release = a
--            primeira barra fora do squeeze; direção = sinal de (close − SMA20).
--            Duas leituras: (a) retorno forward 8/16 barras NA direção do
--            release (o breakout do livro); (b) FADE do release: entrada stop
--            contra o extremo da barra de release (válida 2 barras), stop no
--            extremo oposto da barra, alvo k × R, fill honesto, 4 bp.
-- Fonte    : Rosewag, The New Age of Technical Analysis, Cap. 12 (TTM Squeeze /
--            "Slingshot Squeeze") — análise docs/books/analysis/rosewag-new-age-ta.md
--            §3.7 e §4.1 (candidata `ttm-squeeze-breakout-v1`). Fonte de menor
--            rigor (autopublicado, sem estatísticas). Proxy BB/KC com SMA e range
--            simples = interpretação nossa (sem true range, sem Wilder).
-- Hipótese : compressão precede expansão na direção do rompimento (livro); ou,
--            se o release reverte, o fade paga.
-- Parâmetros: releases entre 10:00 e 14:30 ET (Q5: < 14:00, inclui 09:30/09:45);
--            fade: validade 2 barras (30 min), alvos 1 / 1,5 / 2; custo 4 bp;
--            sem filtro de risco (stop = range da barra de release).
-- Blocos (N de variantes = 1 regra de trade × 3 alvos + 2 medições forward):
--   Q5  frequência de releases e retorno forward 8/16 barras na direção;
--   Q9  releases 10:00–14:30 (retorno forward) + FADE com fill honesto.
-- Resultado 06/09/2026 (designer de setups): o release REVERTE — retorno em 8
--   barras na direção do rompimento −0,30% (2025) / −0,13% (2026); o fade do
--   release com stop na barra dá PF 0,15–0,52. Veredito: REPROVADO nas duas
--   direções (Rosewag 1º no ranking dos livros → morre no screen).
-- Uso      : docker exec -i trader-postgres psql -U trader -d trader_db < sql/screens/02-squeeze-release.sql
-- Fuso     : America/New_York. Cópia fiel de q2.sql Q5 e q4.sql "Q9 fixed".
-- =============================================================================
\set ON_ERROR_STOP on

create temp table bars as
select a.symbol, c.timestamp at time zone 'America/New_York' as ts, (c.timestamp at time zone 'America/New_York')::date d, c.open, c.high, c.low, c.close, c.volume
from candles c join assets a on a.id=c.asset_id where c.timeframe='15m' and a.symbol in ('AVUV','IJS','IWM','IWN','IWO','IWV','SLYV','VBR','IJR','SCHA','VB');
create index on bars(symbol,d,ts);

-- Q5: squeeze (BB20 2sd inside KC SMA20 +-1.5*avg(range20)) release frequency, >=6 bars, fwd 8-bar return in direction of close-sma20
with x as (select symbol, ts, d, close, high, low, avg(close) over w sma20, stddev_samp(close) over w sd20, avg(high-low) over w atr20 from bars window w as (partition by symbol order by ts rows between 19 preceding and current row)),
 y as (select *, (2*sd20 < 1.5*atr20) sq from x),
 z as (select *, sum(sq::int) over (partition by symbol order by ts rows between 6 preceding and 1 preceding) sq6, lead(close,8) over (partition by symbol order by ts) c8, lead(close,16) over (partition by symbol order by ts) c16 from y)
select 'Q5 squeeze release (>=6 bars)' q, extract(year from d)::int yr, count(*) filter (where not sq and sq6>=6) releases, round(count(*) filter (where not sq and sq6>=6)::numeric/11,1) per_symbol,
 round((avg(sign(close-sma20)*(c8-close)/close) filter (where not sq and sq6>=6 and ts::time<'14:00')*100)::numeric,3) fwd8_dir_pct,
 round((avg(sign(close-sma20)*(c16-close)/close) filter (where not sq and sq6>=6 and ts::time<'14:00')*100)::numeric,3) fwd16_dir_pct,
 round(((percentile_cont(0.5) within group (order by 1.5*atr20/close) filter (where not sq and sq6>=6))*100)::numeric,3) kc_halfwidth_med_pct,
 round((avg(sq::int)*100)::numeric,1) pct_bars_in_squeeze
from z group by 2 order by 2;

-- Q9 fixed: squeeze release fade
with x as (select symbol, ts, d, open, close, high, low, avg(close) over w sma20, stddev_samp(close) over w sd20, avg(high-low) over w atr20 from bars window w as (partition by symbol order by ts rows between 19 preceding and current row)),
 y as (select *, (2*sd20 < 1.5*atr20) sq from x),
 z as (select *, sum(sq::int) over (partition by symbol order by ts rows between 6 preceding and 1 preceding) sq6, lead(close,8) over (partition by symbol order by ts) c8, lead(close,16) over (partition by symbol order by ts) c16 from y),
 rel as (select symbol, d, ts, open, close, high, low, sma20, sign(close-sma20) dir, c8, c16, (high-low)/close bar_range from z where not sq and sq6>=6 and ts::time >= '10:00' and ts::time <= '14:30' and high > low),
 f as (select r.symbol, r.d, r.ts, r.dir, r.high rh, r.low rl, min(b.ts) t_fill from rel r join bars b on b.symbol=r.symbol and b.d=r.d and b.ts > r.ts and b.ts <= r.ts + interval '30 minutes'
       where (r.dir=1 and b.low < r.low) or (r.dir=-1 and b.high > r.high) group by 1,2,3,4,5,6),
 st as (select f.symbol, f.d, f.ts, min(b.ts) t_stop from bars b join f on b.symbol=f.symbol and b.d=f.d and b.ts >= f.t_fill where (f.dir=1 and b.high > f.rh) or (f.dir=-1 and b.low < f.rl) group by 1,2,3),
 k as (select * from (values (1.0),(1.5),(2.0)) v(k)),
 tg as (select f.symbol, f.d, f.ts, k.k, min(b.ts) t_tgt from bars b join f on b.symbol=f.symbol and b.d=f.d and b.ts >= f.t_fill cross join k
        where (f.dir=1 and b.low <= f.rl - k.k*(f.rh-f.rl)) or (f.dir=-1 and b.high >= f.rh + k.k*(f.rh-f.rl)) group by 1,2,3,4),
 rr as (select f.symbol, f.d, extract(year from f.d)::int yr, k.k, f.dir, (f.rh-f.rl)/f.rl risk_pct,
   case when st.t_stop is not null and (tg.t_tgt is null or st.t_stop <= tg.t_tgt) then -1.0 when tg.t_tgt is not null then k.k else 0.0 end - 0.0004/nullif((f.rh-f.rl)/f.rl,0) r_net
   from f cross join k left join st on st.symbol=f.symbol and st.d=f.d and st.ts=f.ts left join tg on tg.symbol=f.symbol and tg.d=f.d and tg.ts=f.ts and tg.k=k.k)
select 'Q9 squeeze release 10:00-14:30 fwd in dir' q, null::numeric k, extract(year from d)::int yr, null::int dir, count(*) n, round((avg(dir*(c8-close)/close)*100)::numeric,3) fwd8_dir_pct, round((avg(dir*(c16-close)/close)*100)::numeric,3) fwd16_dir_pct,
 round(((percentile_cont(0.5) within group (order by bar_range))*100)::numeric,3) relbar_range_med_pct, null::numeric sum_r, null::numeric pf_r
from rel group by 3
union all
select 'Q9 squeeze-release FADE stop=bar extreme, net 4bp' q, k, yr, dir, count(*), round(avg(r_net)::numeric,3), round((avg((r_net>0)::int)*100)::numeric,1),
 round(((percentile_cont(0.5) within group (order by risk_pct))*100)::numeric,3), round(sum(r_net)::numeric,1),
 round((sum(r_net) filter (where r_net>0) / nullif(-sum(r_net) filter (where r_net<0),0))::numeric,2)
from rr group by 1,2,3,4 order by 1,2,3,4;
