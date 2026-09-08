-- =============================================================================
-- sql/screens/_eval.sql — avaliação com fill honesto (07/09/2026)
-- -----------------------------------------------------------------------------
-- Espera a temp table `sel` criada pelo arquivo do setup, uma linha por SINAL:
--   tag         text     variante (base, param −20%, param +20%, filtro…)
--   symbol, d, dn        símbolo, data ET, índice do pregão (de bs)
--   dir         int      +1 long / −1 short
--   trigger_px  numeric  gatilho da ordem stop (1 tick além da barra de sinal)
--   stop_px     numeric  stop estrutural (fixo no sinal)
--   ts_sig      ts       barra de sinal (a entrada só pode encher DEPOIS dela)
--   valid_bars  int      validade da ordem em barras (entry_validity_candles)
--   atr_px      numeric  ATR14 da barra de sinal (para o piso de stop)
-- Modelo (README §2, SimulatedBroker pós-ADR-015):
--   fill  = 1ª barra após ts_sig, dentro da validade e até p.t_to, que atravessa
--           o gatilho; preço = max(open, gatilho) long / min(open, gatilho) short;
--           gap_cancel: abertura além do gatilho > gap_tol × |gatilho − stop| = sem trade;
--   1 fill por (tag, symbol, d): o primeiro do dia (uma posição por símbolo);
--   stop  = avaliado PRIMEIRO, da barra do fill inclusive (pior caso); com
--           stop_floor_atr > 0 o stop é afastado até >= floor × atr_px;
--   alvo  = k × R (k = 1 / 1,5 / 2 / 99 = segurar);
--   saída = fechamento do último pregão permitido (hold_days) ou, com
--           max_bars_hold > 0, fechamento da N-ésima barra após o fill
--           (barreira vertical, AFML Cap. 3);
--   custo = (4 bp + 0,01 $/ação ÷ preço) ÷ risk_pct, em R;
--   filtro= risk_min <= risk_pct <= risk_max.
-- Saídas: S0 sinais/fills brutos; S1 por ano; S2 por DATA (t-stat); S3 melhor
--   trimestre; S4 por símbolo; S5 por hora; S6 por direção × ano; S7 cancelados
--   por gap; S8 dias com >= 2 sinais. Critério (README §5): t >= 1,5 no total,
--   mesmo sinal 2025/2026, melhor trimestre <= 50%, >= 40 datas/ano nos 8
--   vivos, sinal estável nas tags ±20%.
-- =============================================================================

create temp table f0 as
select s.tag, s.symbol, s.d, s.dn, s.dir, s.trigger_px, s.stop_px, s.ts_sig, s.atr_px,
       min(b.ts) as t_fill
from sel s
join bars b on b.symbol = s.symbol and b.d = s.d
cross join p
where b.ts > s.ts_sig
  and b.ts <= s.ts_sig + (s.valid_bars * interval '15 minutes')
  and b.ts::time <= p.t_to
  and ((s.dir = 1 and b.high >= s.trigger_px) or (s.dir = -1 and b.low <= s.trigger_px))
group by 1, 2, 3, 4, 5, 6, 7, 8, 9;

-- uma posição por símbolo: o primeiro fill do dia por (tag, symbol, d)
create temp table f as
select * from (
  select f0.*, row_number() over (partition by tag, symbol, d order by t_fill, ts_sig) as rk from f0
) z where rk = 1;

create temp table fb as
select f.tag, f.symbol, f.d, f.dn, f.dir, f.trigger_px, f.ts_sig, f.t_fill, f.atr_px,
       b.open as fill_open, b.live,
       case when f.dir = 1 then greatest(b.open, f.trigger_px)
            else               least(b.open, f.trigger_px) end                          as fill_px,
       case when p.stop_floor_atr > 0 and f.dir = 1
              then least(f.stop_px, greatest(b.open, f.trigger_px) - p.stop_floor_atr * f.atr_px)
            when p.stop_floor_atr > 0 and f.dir = -1
              then greatest(f.stop_px, least(b.open, f.trigger_px) + p.stop_floor_atr * f.atr_px)
            else f.stop_px end                                                          as stop_px,
       f.stop_px                                                                        as stop_raw,
       case when f.dir = 1 then greatest(b.open - f.trigger_px, 0)
            else               greatest(f.trigger_px - b.open, 0) end                    as overshoot
from f join bs b on b.symbol = f.symbol and b.d = f.d and b.ts = f.t_fill
cross join p;

create temp table fb2 as
select fb.*,
       abs(fill_px - stop_px) / fill_px                                                  as risk_pct,
       (p.gap_cancel and abs(trigger_px - stop_raw) > 0
          and overshoot > p.gap_tol * abs(trigger_px - stop_raw))                       as cancelled,
       fb.dn + p.hold_days                                                               as last_dn,
       case when p.max_bars_hold > 0 and p.hold_days = 0
            then fb.t_fill + (p.max_bars_hold * interval '15 minutes') end               as t_vert
from fb cross join p
where stop_px is not null
  and ((dir = 1 and stop_px < fill_px) or (dir = -1 and stop_px > fill_px));

create temp table st as
select fb.tag, fb.symbol, fb.d, fb.t_fill, min(b.ts) as t_stop
from fb2 fb
join bs b on b.symbol = fb.symbol and b.dn between fb.dn and fb.last_dn and b.ts >= fb.t_fill
where ((fb.dir = 1 and b.low <= fb.stop_px) or (fb.dir = -1 and b.high >= fb.stop_px))
  and (fb.t_vert is null or b.ts <= fb.t_vert)
group by 1, 2, 3, 4;

create temp table tg as
select fb.tag, fb.symbol, fb.d, fb.t_fill, k.k, min(b.ts) as t_tgt
from fb2 fb
cross join k
join bs b on b.symbol = fb.symbol and b.dn between fb.dn and fb.last_dn and b.ts >= fb.t_fill
where ((fb.dir = 1  and b.high >= fb.fill_px + k.k * (fb.fill_px - fb.stop_px))
    or (fb.dir = -1 and b.low  <= fb.fill_px - k.k * (fb.stop_px - fb.fill_px)))
  and (fb.t_vert is null or b.ts <= fb.t_vert)
group by 1, 2, 3, 4, 5;

create temp table ex as
select fb.tag, fb.symbol, fb.d, fb.t_fill,
       case when fb.t_vert is not null then v.close else x.cl end as exit_close
from fb2 fb
join lateral (select cl from dd x where x.symbol = fb.symbol and x.dn <= fb.last_dn
              order by x.dn desc limit 1) x on true
left join lateral (select close from bs v where v.symbol = fb.symbol and v.d = fb.d
                   and v.ts <= fb.t_vert order by v.ts desc limit 1) v on true;

create temp table r as
select z.*,
       z.r_gross - (p.cost_bp + p.cost_share / z.fill_px) / z.risk_pct as r_net,
       (p.cost_bp + p.cost_share / z.fill_px) / z.risk_pct              as cost_r
from (
  select fb.tag, fb.symbol, fb.live, fb.d, fb.dn,
         extract(year from fb.d)::int         as yr,
         to_char(fb.d, 'YYYY"Q"Q')             as q,
         fb.dir, k.k, fb.t_fill, fb.fill_px,
         to_char(fb.t_fill, 'HH24')            as fill_hour,
         fb.risk_pct, fb.cancelled, fb.overshoot,
         case when st.t_stop is not null and (tg.t_tgt is null or st.t_stop <= tg.t_tgt) then -1.0
              when tg.t_tgt is not null then k.k
              else fb.dir * (ex.exit_close - fb.fill_px) / abs(fb.fill_px - fb.stop_px) end as r_gross,
         case when st.t_stop is not null and (tg.t_tgt is null or st.t_stop <= tg.t_tgt) then 'stop'
              when tg.t_tgt is not null then 'target' else 'close' end                     as exit_kind
  from fb2 fb
  cross join k
  left join st on st.tag = fb.tag and st.symbol = fb.symbol and st.d = fb.d and st.t_fill = fb.t_fill
  left join tg on tg.tag = fb.tag and tg.symbol = fb.symbol and tg.d = fb.d and tg.t_fill = fb.t_fill and tg.k = k.k
  join ex on ex.tag = fb.tag and ex.symbol = fb.symbol and ex.d = fb.d and ex.t_fill = fb.t_fill
) z cross join p
where z.risk_pct between p.risk_min and p.risk_max;

-- =============================================================================
-- SAÍDAS
-- =============================================================================
\echo '--- S0 sinais e fills brutos (antes do filtro de risco e do gap) ---'
select 'S0' as bloco, s.tag,
       count(*)                                                    as sinais,
       count(distinct (s.symbol, s.d))                             as sinais_sym_dia,
       (select count(*) from f where f.tag = s.tag)                as fills,
       (select count(*) from fb2 where fb2.tag = s.tag and not cancelled
          and risk_pct < (select risk_min from p))                 as fills_stop_abaixo_piso,
       (select count(*) from fb2 where fb2.tag = s.tag and cancelled) as cancelados_gap
from sel s group by 1, 2 order by 2;

\echo '--- S1 por tag, alvo e ano (fills não cancelados, dentro do filtro de risco) ---'
select 'S1' as bloco, tag, k, yr::text as grp,
       count(*)                                                        as fills,
       count(distinct d) filter (where live)                           as n_datas_vivos,
       round(avg(r_net)::numeric, 3)                                   as avg_r_net,
       round((avg((r_net > 0)::int) * 100)::numeric, 1)                as wr,
       round(((percentile_cont(0.5) within group (order by risk_pct)) * 100)::numeric, 3) as risk_med_pct,
       round(avg(cost_r)::numeric, 3)                                  as custo_r,
       round(sum(r_net)::numeric, 1)                                   as sum_r,
       round((sum(r_net) filter (where r_net > 0)
              / nullif(-sum(r_net) filter (where r_net < 0), 0))::numeric, 2) as pf_r
from r where not cancelled
group by 1, 2, 3, 4 order by 2, 3, 4;

\echo '--- S2 por DATA (n efetivo): t-stat da soma diária de R, por ano e total ---'
with daily as (
  select tag, k, yr, d, sum(r_net) as r_day from r where not cancelled group by 1, 2, 3, 4
)
select 'S2' as bloco, tag, k, coalesce(yr::text, 'todos') as grp,
       count(*)                                                        as n_datas,
       round(avg(r_day)::numeric, 3)                                   as avg_r_dia,
       round((avg(r_day) / nullif(stddev_samp(r_day) / sqrt(count(*)), 0))::numeric, 2) as t_stat,
       round(sum(r_day)::numeric, 1)                                   as sum_r
from daily
group by tag, k, rollup(yr) order by 2, 3, 4;

\echo '--- S3 melhor trimestre (share <= 50% com P&L total positivo) ---'
with quart as (
  select tag, k, q, sum(r_net) as r_q from r where not cancelled group by 1, 2, 3
)
select 'S3' as bloco, tag, k,
       (array_agg(q order by r_q desc))[1]                              as melhor_trim,
       round(max(r_q)::numeric, 1)                                     as r_melhor_trim,
       round(sum(r_q)::numeric, 1)                                     as r_total,
       case when sum(r_q) > 0 then round((max(r_q) / sum(r_q))::numeric, 2) end as share
from quart group by 1, 2, 3 order by 2, 3;

\echo '--- S4 por símbolo (k = k_main, todas as tags) ---'
select 'S4' as bloco, tag, r.k, symbol as grp,
       count(*) as fills, round(avg(r_net)::numeric, 3) as avg_r_net,
       round((avg((r_net > 0)::int) * 100)::numeric, 1) as wr,
       round(sum(r_net)::numeric, 1) as sum_r,
       round((sum(r_net) filter (where r_net > 0)
              / nullif(-sum(r_net) filter (where r_net < 0), 0))::numeric, 2) as pf_r
from r cross join p where not cancelled and r.k = p.k_main
group by 1, 2, 3, 4 order by 2, 4;

\echo '--- S5 por hora ET do fill (k = k_main) ---'
select 'S5' as bloco, tag, r.k, fill_hour as grp,
       count(*) as fills, round(avg(r_net)::numeric, 3) as avg_r_net,
       round((avg((r_net > 0)::int) * 100)::numeric, 1) as wr,
       round(sum(r_net)::numeric, 1) as sum_r,
       round((sum(r_net) filter (where r_net > 0)
              / nullif(-sum(r_net) filter (where r_net < 0), 0))::numeric, 2) as pf_r
from r cross join p where not cancelled and r.k = p.k_main
group by 1, 2, 3, 4 order by 2, 4;

\echo '--- S6 por direção e ano (k = k_main) ---'
select 'S6' as bloco, tag, r.k, dir, yr,
       count(*) as fills, round(avg(r_net)::numeric, 3) as avg_r_net,
       round((avg((r_net > 0)::int) * 100)::numeric, 1) as wr,
       round(sum(r_net)::numeric, 1) as sum_r,
       round((sum(r_net) filter (where r_net > 0)
              / nullif(-sum(r_net) filter (where r_net < 0), 0))::numeric, 2) as pf_r,
       round((100.0 * count(*) filter (where exit_kind = 'stop') / count(*))::numeric, 1) as pct_stop,
       round((100.0 * count(*) filter (where exit_kind = 'close') / count(*))::numeric, 1) as pct_close
from r cross join p where not cancelled and r.k = p.k_main
group by 1, 2, 3, 4, 5 order by 2, 4, 5;

\echo '--- S7 cancelados por gap (ADR-015) e o que teriam rendido (k = k_main) ---'
select 'S7' as bloco, tag, r.k, yr::text as grp,
       count(*) filter (where cancelled)                                as cancelados,
       count(*)                                                         as fills_brutos,
       round((100.0 * count(*) filter (where cancelled) / nullif(count(*), 0))::numeric, 1) as pct,
       round(avg(r_net) filter (where cancelled)::numeric, 3)           as avg_r_se_enchesse
from r cross join p where r.k = p.k_main
group by 1, 2, 3, 4 order by 2, 4;

\echo '--- S8 dias com >= 2 sinais no mesmo símbolo (cluster) e resultado do 1º fill nesses dias (k = k_main) ---'
with c as (
  select tag, symbol, d, count(*) as n_sig from sel group by 1, 2, 3
)
select 'S8' as bloco, r.tag, r.k, (c.n_sig >= 2) as cluster,
       count(*) as fills, round(avg(r_net)::numeric, 3) as avg_r_net,
       round((sum(r_net) filter (where r_net > 0)
              / nullif(-sum(r_net) filter (where r_net < 0), 0))::numeric, 2) as pf_r
from r join c using (tag, symbol, d) cross join p
where not cancelled and r.k = p.k_main
group by 1, 2, 3, 4 order by 2, 4;
