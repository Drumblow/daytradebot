-- =============================================================================
-- sql/screens/_template.sql — Fase 0: screener SQL com fill honesto (template)
-- -----------------------------------------------------------------------------
-- Status : proposto / especificado — NÃO adotado como gate (07/09/2026).
--          O arquivo roda; a calibração contra controles (README §4) não foi
--          executada. Até lá, ferramenta de pesquisa.
-- Origem : docs/cto-plano-lucratividade-2026-09.md §5.7;
--          docs/strategy-analysis-framework.md §3 "Fase 0";
--          derivado dos screens de 06/09/2026 (01-…08-… neste diretório).
-- Uso    : docker exec -i trader-postgres psql -U trader -d trader_db < sql/screens/_template.sql
-- Fuso   : todo timestamp é convertido para America/New_York; janelas e horas
--          de fill são ET (padrão do repo).
--
-- Modelo (README §2), na forma do SimulatedBroker após a ADR-015:
--   entrada = max(open, gatilho) long / min(open, gatilho) short, na primeira
--             barra da janela [t_from, t_to] que atravessa o gatilho;
--   stop    = avaliado PRIMEIRO, a partir da barra do fill inclusive
--             (pior caso; simulated/broker.rs:351-353); toque = low <= stop;
--   alvo    = k × R para cada k da tabela `k` (99 = segurar até o fechamento);
--   saída   = fechamento do último pregão permitido: hold_days = 0 → fechamento
--             do dia do fill (= flatten do live, NÃO o engine sem EOD);
--   custo   = cost / risk_pct, subtraído em R (0,0004 = 4 bp ida e volta);
--   filtro  = risk_min <= |fill − stop| / fill <= risk_max;
--   gap     = gap_cancel = true → abertura além do gatilho > gap_tol × distância
--             do stop = SEM trade (ADR-015, config/default.toml:42);
--             false → enche no preço pior (comportamento dos screens de 06/09).
--
-- Blocos : p → bars → days → dd → bs → sel → f → fb → fb2 → st → tg → ex → r
--          → S1 (ano) S2 (por DATA: t-stat) S3 (melhor trimestre) S4 (símbolo)
--          S5 (hora do fill) S6 (direção) S7 (cancelados por gap).
-- Para um setup novo normalmente só `sel` muda (uma linha por (symbol, d) com
-- dir, trigger_px e stop_px). Se o gatilho depende da barra (ex.: barra de
-- sinal), escreva `sel` por barra e ajuste `f` — veja 05-double-top-range.sql.
-- =============================================================================
\set ON_ERROR_STOP on

-- -----------------------------------------------------------------------------
-- p — PARÂMETROS (edite aqui)
-- -----------------------------------------------------------------------------
create temp table p as select
  time '10:00'     as t_from,      -- primeira barra (abertura ET) em que a entrada pode encher
  time '14:30'     as t_to,        -- última barra
  0.003::numeric   as risk_min,    -- filtro de risco: |fill − stop| / fill, mínimo (0,3%)
  0.015::numeric   as risk_max,    -- máximo (1,5%)
  0.0004::numeric  as cost,        -- 4 bp ida e volta (ADR-016: 2 bp por lado)
  0.25::numeric    as gap_tol,     -- entry_overshoot_tolerance (config/default.toml:42)
  true             as gap_cancel,  -- true = replica a ADR-015; false = enche pior (screens de 06/09)
  0                as hold_days,   -- 0 = sai no fechamento do dia do fill; N = carrega N pregões
  1.5::numeric     as k_main;      -- alvo usado nos blocos por símbolo/hora/direção

create temp table k as select * from (values (1.0), (1.5), (2.0), (99.0)) v(k);

-- -----------------------------------------------------------------------------
-- bars — candles 15m em ET
-- -----------------------------------------------------------------------------
create temp table bars as
select a.symbol,
       c.timestamp at time zone 'America/New_York'         as ts,
       (c.timestamp at time zone 'America/New_York')::date as d,
       c.open, c.high, c.low, c.close, c.volume
from candles c join assets a on a.id = c.asset_id
where c.timeframe = '15m'
  and a.symbol in ('AVUV','IJS','IWM','IWN','IWO','IWV','SLYV','VBR','IJR','SCHA','VB');
create index on bars (symbol, d, ts);

-- -----------------------------------------------------------------------------
-- days / dd — OHLC diário, 1ª barra, 1ª hora (09:30–10:29), níveis de ontem, ATRd
-- -----------------------------------------------------------------------------
create temp table days as
select symbol, d,
       (array_agg(open  order by ts))[1]           as o,
       max(high)                                   as h,
       min(low)                                    as l,
       (array_agg(close order by ts desc))[1]      as cl,
       (array_agg(high  order by ts))[1]           as b1h,   -- 1ª barra (09:30)
       (array_agg(low   order by ts))[1]           as b1l,
       max(high) filter (where ts::time < '10:30') as h1h,   -- 1ª hora = Initial Balance
       min(low)  filter (where ts::time < '10:30') as h1l,
       (array_agg(close order by ts))[4]           as h1c,   -- fechamento da barra 10:15
       count(*)                                    as n
from bars group by 1, 2;

create temp table dd as
select *,
       row_number() over w                         as dn,    -- índice do pregão por símbolo
       lag(cl) over w as pc,                                  -- PDC
       lag(h)  over w as ph,                                  -- PDH
       lag(l)  over w as pl,                                  -- PDL
       lag(o)  over w as po,
       lag(cl, 2) over w as pc2,
       avg((h - l) / cl) over (partition by symbol order by d
                               rows between 14 preceding and 1 preceding) as atrd
from days window w as (partition by symbol order by d);

-- bs — barras com extremos acumulados do dia ANTES da barra (stop dinâmico)
create temp table bs as
select b.*, dd.dn,
       max(high) over (partition by b.symbol, b.d order by b.ts
                       rows between unbounded preceding and 1 preceding) as th_prev,
       min(low)  over (partition by b.symbol, b.d order by b.ts
                       rows between unbounded preceding and 1 preceding) as tl_prev
from bars b join dd using (symbol, d);
create index on bs (symbol, dn, ts);

-- -----------------------------------------------------------------------------
-- sel — O SETUP. Uma linha por (symbol, d) candidato:
--   dir        +1 long / −1 short
--   trigger_px gatilho (stop order)
--   stop_px    stop fixo, ou NULL = extremo do dia antes da barra do fill
--              (máxima acumulada para short, mínima para long)
-- Exemplo entregue: PDL-break short com abertura dentro do range de ontem
-- (Murphy Cap. 16 — docs/books/analysis/murphy-technical-analysis.md §3.2).
-- Filtro n >= 20 barras = pregão completo.
-- -----------------------------------------------------------------------------
create temp table sel as
select symbol, d, dn, cl as day_close,
       -1               as dir,
       pl               as trigger_px,
       null::numeric    as stop_px
from dd
where n >= 20 and pl is not null and o > pl and o < ph;

-- -----------------------------------------------------------------------------
-- f — primeira barra da janela que atravessa o gatilho
-- -----------------------------------------------------------------------------
create temp table f as
select s.symbol, s.d, s.dn, s.dir, s.trigger_px, s.stop_px as stop_fixed, s.day_close,
       min(b.ts) as t_fill
from sel s
join bars b using (symbol, d)
cross join p
where b.ts::time between p.t_from and p.t_to
  and ((s.dir = 1 and b.high >= s.trigger_px) or (s.dir = -1 and b.low <= s.trigger_px))
group by 1, 2, 3, 4, 5, 6, 7;

-- -----------------------------------------------------------------------------
-- fb / fb2 — fill honesto, stop, overshoot, risco, cancelamento por gap
-- -----------------------------------------------------------------------------
create temp table fb as
select f.symbol, f.d, f.dn, f.dir, f.trigger_px, f.day_close, f.t_fill,
       b.open as fill_open,
       case when f.dir = 1 then greatest(b.open, f.trigger_px)
            else               least(b.open, f.trigger_px) end                as fill_px,
       coalesce(f.stop_fixed, case when f.dir = 1 then b.tl_prev else b.th_prev end) as stop_px,
       case when f.dir = 1 then greatest(b.open - f.trigger_px, 0)
            else               greatest(f.trigger_px - b.open, 0) end          as overshoot
from f join bs b on b.symbol = f.symbol and b.d = f.d and b.ts = f.t_fill;

create temp table fb2 as
select fb.*,
       abs(fill_px - stop_px) / fill_px                                        as risk_pct,
       abs(trigger_px - stop_px)                                               as stop_dist,
       (p.gap_cancel and abs(trigger_px - stop_px) > 0
          and overshoot > p.gap_tol * abs(trigger_px - stop_px))              as cancelled,
       fb.dn + p.hold_days                                                     as last_dn
from fb cross join p
where stop_px is not null
  and ((dir = 1 and stop_px < fill_px) or (dir = -1 and stop_px > fill_px));

-- -----------------------------------------------------------------------------
-- st — primeira barra (do fill inclusive, até last_dn) que toca o stop
-- -----------------------------------------------------------------------------
create temp table st as
select fb.symbol, fb.d, fb.t_fill, min(b.ts) as t_stop
from fb2 fb
join bs b on b.symbol = fb.symbol and b.dn between fb.dn and fb.last_dn and b.ts >= fb.t_fill
where (fb.dir = 1 and b.low <= fb.stop_px) or (fb.dir = -1 and b.high >= fb.stop_px)
group by 1, 2, 3;

-- -----------------------------------------------------------------------------
-- tg — primeira barra que toca o alvo k × R
-- -----------------------------------------------------------------------------
create temp table tg as
select fb.symbol, fb.d, fb.t_fill, k.k, min(b.ts) as t_tgt
from fb2 fb
cross join k
join bs b on b.symbol = fb.symbol and b.dn between fb.dn and fb.last_dn and b.ts >= fb.t_fill
where (fb.dir = 1  and b.high >= fb.fill_px + k.k * (fb.fill_px - fb.stop_px))
   or (fb.dir = -1 and b.low  <= fb.fill_px - k.k * (fb.stop_px - fb.fill_px))
group by 1, 2, 3, 4;

-- ex — fechamento do último pregão permitido (hold_days)
create temp table ex as
select fb.symbol, fb.d, fb.t_fill, x.cl as exit_close
from fb2 fb
join lateral (select cl from dd x where x.symbol = fb.symbol and x.dn <= fb.last_dn
              order by x.dn desc limit 1) x on true;

-- -----------------------------------------------------------------------------
-- r — resultado em R por (fill, k): −1 se stop antes ou junto do alvo; k se alvo;
--     senão (exit_close − fill)/R na direção; menos o custo
-- -----------------------------------------------------------------------------
create temp table r as
select z.*, z.r_gross - p.cost / z.risk_pct as r_net
from (
  select fb.symbol, fb.d, fb.dn,
         extract(year from fb.d)::int         as yr,
         to_char(fb.d, 'YYYY"Q"Q')             as q,
         fb.dir, k.k, fb.t_fill,
         to_char(fb.t_fill, 'HH24')            as fill_hour,
         fb.risk_pct, fb.cancelled, fb.overshoot,
         case when st.t_stop is not null and (tg.t_tgt is null or st.t_stop <= tg.t_tgt) then -1.0
              when tg.t_tgt is not null then k.k
              else fb.dir * (ex.exit_close - fb.fill_px) / abs(fb.fill_px - fb.stop_px) end as r_gross
  from fb2 fb
  cross join k
  left join st on st.symbol = fb.symbol and st.d = fb.d and st.t_fill = fb.t_fill
  left join tg on tg.symbol = fb.symbol and tg.d = fb.d and tg.t_fill = fb.t_fill and tg.k = k.k
  join ex on ex.symbol = fb.symbol and ex.d = fb.d and ex.t_fill = fb.t_fill
) z cross join p
where z.risk_pct between p.risk_min and p.risk_max;

-- =============================================================================
-- SAÍDAS (fills cancelados por gap ficam fora de S1–S6; contados em S7)
-- =============================================================================

-- S1 — por ano e alvo
select 'S1 por ano' as bloco, k, yr::text as grp,
       count(*)                                                        as fills,
       count(distinct d)                                               as n_datas,
       round(avg(r_net)::numeric, 3)                                   as avg_r_net,
       round((avg((r_net > 0)::int) * 100)::numeric, 1)                as wr,
       round(((percentile_cont(0.5) within group (order by risk_pct)) * 100)::numeric, 3) as risk_med_pct,
       round(sum(r_net)::numeric, 1)                                   as sum_r,
       round((sum(r_net) filter (where r_net > 0)
              / nullif(-sum(r_net) filter (where r_net < 0), 0))::numeric, 2) as pf_r
from r where not cancelled
group by 1, 2, 3 order by 2, 3;

-- S2 — agregação por DATA (n efetivo): t-stat sobre a soma diária de R, por ano
--      e no total. Critério do README §5: t >= 1,5 no total; mesmo sinal nos anos.
with daily as (
  select k, yr, d, sum(r_net) as r_day from r where not cancelled group by 1, 2, 3
)
select 'S2 por data' as bloco, k, coalesce(yr::text, 'todos') as grp,
       count(*)                                                        as n_datas,
       round(avg(r_day)::numeric, 3)                                   as avg_r_dia,
       round((avg(r_day) / nullif(stddev_samp(r_day) / sqrt(count(*)), 0))::numeric, 2) as t_stat,
       round(sum(r_day)::numeric, 1)                                   as sum_r
from daily
group by k, rollup(yr) order by 2, 3;

-- S3 — participação do melhor trimestre no P&L em R (critério: <= 50%; só faz
--      sentido com P&L total positivo)
with quart as (
  select k, q, sum(r_net) as r_q from r where not cancelled group by 1, 2
)
select 'S3 melhor trimestre' as bloco, k,
       (array_agg(q order by r_q desc))[1]                              as melhor_trim,
       round(max(r_q)::numeric, 1)                                     as r_melhor_trim,
       round(sum(r_q)::numeric, 1)                                     as r_total,
       case when sum(r_q) > 0 then round((max(r_q) / sum(r_q))::numeric, 2) end as share
from quart group by k order by k;

-- S4 — por símbolo (k = k_main)
select 'S4 por simbolo' as bloco, r.k, symbol as grp,
       count(*) as fills, round(avg(r_net)::numeric, 3) as avg_r_net,
       round((avg((r_net > 0)::int) * 100)::numeric, 1) as wr,
       round(sum(r_net)::numeric, 1) as sum_r,
       round((sum(r_net) filter (where r_net > 0)
              / nullif(-sum(r_net) filter (where r_net < 0), 0))::numeric, 2) as pf_r
from r cross join p where not cancelled and r.k = p.k_main
group by 1, 2, 3 order by 3;

-- S5 — por hora ET do fill (k = k_main)
select 'S5 por hora' as bloco, r.k, fill_hour as grp,
       count(*) as fills, round(avg(r_net)::numeric, 3) as avg_r_net,
       round((avg((r_net > 0)::int) * 100)::numeric, 1) as wr,
       round(sum(r_net)::numeric, 1) as sum_r,
       round((sum(r_net) filter (where r_net > 0)
              / nullif(-sum(r_net) filter (where r_net < 0), 0))::numeric, 2) as pf_r
from r cross join p where not cancelled and r.k = p.k_main
group by 1, 2, 3 order by 3;

-- S6 — por direção (k = k_main)
select 'S6 por direcao' as bloco, r.k, dir,
       count(*) as fills, round(avg(r_net)::numeric, 3) as avg_r_net,
       round((avg((r_net > 0)::int) * 100)::numeric, 1) as wr,
       round(sum(r_net)::numeric, 1) as sum_r,
       round((sum(r_net) filter (where r_net > 0)
              / nullif(-sum(r_net) filter (where r_net < 0), 0))::numeric, 2) as pf_r
from r cross join p where not cancelled and r.k = p.k_main
group by 1, 2, 3 order by 3;

-- S7 — fills cancelados por gap (ADR-015) e o que teriam rendido se enchessem
--      no preço pior (comportamento dos screens de 06/09)
select 'S7 cancelados por gap' as bloco, r.k, yr::text as grp,
       count(*) filter (where cancelled)                                as cancelados,
       count(*)                                                         as fills_brutos,
       round((100.0 * count(*) filter (where cancelled) / nullif(count(*), 0))::numeric, 1) as pct,
       round(avg(r_net) filter (where cancelled)::numeric, 3)           as avg_r_se_enchesse
from r cross join p where r.k = p.k_main
group by 1, 2, 3 order by 3;
