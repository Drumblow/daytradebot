-- =============================================================================
-- sql/screens/_lib.sql — biblioteca compartilhada dos screens 09+ (07/09/2026)
-- -----------------------------------------------------------------------------
-- Uso    : cat sql/screens/_lib.sql sql/screens/NN-setup.sql sql/screens/_eval.sql \
--            | docker exec -i trader-postgres psql -U trader -d trader_db \
--                -v hold_days=0 -v gap_cancel=true -v stop_floor_atr=0 -v max_bars_hold=0
--          (ou sql/screens/run.sh NN-setup.sql [-v var=valor ...]).
-- O que cria (tudo temp, sessão única):
--   p     parâmetros (vars psql com default: hold_days, gap_cancel, stop_floor_atr,
--         max_bars_hold, k_main, t_to, risk_min)
--   bars  candles 15m em ET dos 11 ETFs dos screens de 06/09 + flag `live`
--         (8 símbolos com dado até 02/09/2026: AVUV IJS IWM IWN IWO IWV SLYV VBR)
--   dd    OHLC diário, níveis de ontem (PDH/PDL/PDC/PO), extremos de 2 e 3 dias
--         anteriores, ATR diário (preço e relativo), 1ª hora (IB = 4 barras)
--   bs    barras + indicadores por barra, como a v1 os calcula:
--         atr14 (média simples do TR das 14 barras incl. a atual —
--         range_extreme_fade_v1/context.rs:90-108), ema20 (indicators/mod.rs:23-38:
--         seed = SMA20, 19 iterações sobre a mesma janela), inclinação da EMA20
--         em 12 barras (context.rs ema_slope_per_bar), sma20, extremos do dia
--         antes da barra (th_prev/tl_prev) e até a barra (th_cur/tl_cur), área
--         de balanço de 78 barras excluindo a atual (balance_area_breakout_v1),
--         estocástico lento %D(14,3,3) (Murphy Cap. 10), canal Donchian de 26
--         barras (Murphy Cap. 9), sma200 (contexto global: context/mod.rs:120-128),
--         pivôs de 2 barras (is_ph/is_pl — range_extreme_fade_v1/context.rs pivots),
--         rn = índice da barra no dia, seq = índice da barra na série do símbolo.
-- Fuso   : America/New_York em tudo (padrão do repo).
-- Modelo de fill/custo: ver _eval.sql e README §2.
-- =============================================================================
\set ON_ERROR_STOP on
\if :{?hold_days}      \else \set hold_days 0        \endif
\if :{?gap_cancel}     \else \set gap_cancel true    \endif
\if :{?stop_floor_atr} \else \set stop_floor_atr 0   \endif
\if :{?max_bars_hold}  \else \set max_bars_hold 0    \endif
\if :{?k_main}         \else \set k_main 1.5         \endif
\if :{?t_to}           \else \set t_to 15:15         \endif
\if :{?risk_min}       \else \set risk_min 0.0020    \endif

create temp table p as select
  time :'t_to'               as t_to,            -- última barra (abertura ET) em que a entrada pode encher
  :risk_min::numeric         as risk_min,        -- |fill − stop| / fill mínimo (20 bp = piso da lente)
  0.015::numeric             as risk_max,        -- 1,5% = "não é day trade de 15 min" (interpretação nossa)
  0.0004::numeric            as cost_bp,         -- 4 bp ida e volta (ADR-016: 2 bp por lado)
  0.01::numeric              as cost_share,      -- comissão IBKR Canada 0,005/ação × 2 pernas ($/ação)
  0.25::numeric              as gap_tol,         -- entry_overshoot_tolerance (config/default.toml)
  :gap_cancel::boolean       as gap_cancel,      -- true = replica a ADR-015
  :hold_days::int            as hold_days,       -- 0 = flatten no fechamento da 15:45 (live)
  :stop_floor_atr::numeric   as stop_floor_atr,  -- >0 = stop afastado para >= floor × ATR14 (Grimes Cap. 8); 0 = off
  :max_bars_hold::int        as max_bars_hold,   -- >0 = barreira vertical em N barras (AFML Cap. 3); 0 = off
  :k_main::numeric           as k_main;

create temp table k as select * from (values (1.0), (1.5), (2.0), (99.0)) v(k);

-- EMA como a v1 calcula (indicators/mod.rs:23-38): seed = SMA das `period`
-- últimas barras; depois itera sobre as mesmas barras pulando a primeira.
create function pg_temp.ema_v1(arr numeric[], period int) returns numeric
language sql immutable as $$
  with x as (select v, i from unnest(arr) with ordinality as u(v, i))
  select case when coalesce(array_length(arr, 1), 0) < period then null else
    (select avg(v) from x) * power(1 - 2.0 / (period + 1), period - 1)
    + 2.0 / (period + 1) * (select sum(v * power(1 - 2.0 / (period + 1), period - i)) from x where i > 1)
  end
$$;

create temp table bars as
select a.symbol,
       a.symbol in ('AVUV','IJS','IWM','IWN','IWO','IWV','SLYV','VBR') as live,
       c.timestamp at time zone 'America/New_York'         as ts,
       (c.timestamp at time zone 'America/New_York')::date as d,
       c.open, c.high, c.low, c.close, c.volume
from candles c join assets a on a.id = c.asset_id
where c.timeframe = '15m'
  and a.symbol in ('AVUV','IJS','IWM','IWN','IWO','IWV','SLYV','VBR','IJR','SCHA','VB');
create index on bars (symbol, d, ts);

create temp table days as
select symbol, d,
       (array_agg(open  order by ts))[1]           as o,
       max(high)                                   as h,
       min(low)                                    as l,
       (array_agg(close order by ts desc))[1]      as cl,
       (array_agg(high  order by ts))[1]           as b1h,
       (array_agg(low   order by ts))[1]           as b1l,
       max(high) filter (where ts::time < '10:30') as h1h,
       min(low)  filter (where ts::time < '10:30') as h1l,
       count(*)                                    as n
from bars group by 1, 2;

create temp table dd as
select *,
       row_number() over w                                   as dn,
       lag(cl) over w                                        as pc,
       lag(h)  over w                                        as ph,
       lag(l)  over w                                        as pl,
       lag(o)  over w                                        as po,
       lag(h1h) over w                                       as ph1h,
       lag(h1l) over w                                       as ph1l,
       max(h) over (partition by symbol order by d rows between 2 preceding and 1 preceding) as h2,
       min(l) over (partition by symbol order by d rows between 2 preceding and 1 preceding) as l2,
       max(h) over (partition by symbol order by d rows between 3 preceding and 1 preceding) as h3,
       min(l) over (partition by symbol order by d rows between 3 preceding and 1 preceding) as l3,
       avg(h - l)        over (partition by symbol order by d rows between 14 preceding and 1 preceding) as atrd_px,
       avg((h - l) / cl) over (partition by symbol order by d rows between 14 preceding and 1 preceding) as atrd,
       lag((cl - l) / nullif(h - l, 0)) over w               as pclpos,   -- posição do fechamento de ontem no range
       lag(n) over w                                         as pn
from days window w as (partition by symbol order by d);
create index on dd (symbol, dn);

create temp table b1 as
select b.*,
       greatest(high - low, abs(high - lag(close) over w), abs(low - lag(close) over w)) as tr
from bars b window w as (partition by symbol order by ts);

create temp table b2 as
select b.*,
       row_number() over (partition by symbol, d order by ts)                                   as rn,
       row_number() over (partition by symbol order by ts)                                      as seq,
       avg(close) over (partition by symbol order by ts rows between 199 preceding and current row) as sma200,
       avg(tr)   over (partition by symbol order by ts rows between 13 preceding and current row) as atr14,
       pg_temp.ema_v1(array_agg(close) over (partition by symbol order by ts rows between 19 preceding and current row), 20) as ema20,
       avg(close) over (partition by symbol order by ts rows between 19 preceding and current row) as sma20,
       max(high) over (partition by symbol, d order by ts rows between unbounded preceding and 1 preceding) as th_prev,
       min(low)  over (partition by symbol, d order by ts rows between unbounded preceding and 1 preceding) as tl_prev,
       max(high) over (partition by symbol, d order by ts rows between unbounded preceding and current row) as th_cur,
       min(low)  over (partition by symbol, d order by ts rows between unbounded preceding and current row) as tl_cur,
       max(high) over (partition by symbol order by ts rows between 78 preceding and 1 preceding) as ba_h,
       min(low)  over (partition by symbol order by ts rows between 78 preceding and 1 preceding) as ba_l,
       avg(close) over (partition by symbol order by ts rows between 78 preceding and 1 preceding) as ba_mean,
       max(high) over (partition by symbol order by ts rows between 13 preceding and current row) as h14,
       min(low)  over (partition by symbol order by ts rows between 13 preceding and current row) as l14,
       max(high) over (partition by symbol order by ts rows between 25 preceding and 1 preceding) as dc_h26,
       min(low)  over (partition by symbol order by ts rows between 25 preceding and 1 preceding) as dc_l26,
       min(low)  over (partition by symbol order by ts rows between 6 preceding and 1 preceding)  as l_n4,
       max(high) over (partition by symbol order by ts rows between 6 preceding and 1 preceding)  as h_n4
from b1 b;

create temp table b3 as
select b.*,
       (high > lag(high, 1) over w and high > lag(high, 2) over w
          and high >= lead(high, 1) over w and high >= lead(high, 2) over w)                as is_ph,
       (low < lag(low, 1) over w and low < lag(low, 2) over w
          and low <= lead(low, 1) over w and low <= lead(low, 2) over w)                    as is_pl,
       abs(ema20 - lag(ema20, 11) over w) / 12 / nullif(ema20, 0)                      as ema_slope,
       100 * (close - l14) / nullif(h14 - l14, 0)                                       as stoch_k,
       avg(100 * (close - l14) / nullif(h14 - l14, 0)) over (w rows between 2 preceding and current row) as stoch_d
from b2 b window w as (partition by symbol order by ts);

create temp table bs as
select b.*,
       avg(stoch_d) over (partition by symbol order by ts rows between 2 preceding and current row) as stoch_d_slow,
       dd.dn, dd.atrd, dd.atrd_px, dd.ph, dd.pl, dd.pc, dd.po, dd.h2, dd.l2, dd.h3, dd.l3,
       dd.h1h, dd.h1l, dd.o as day_open, dd.cl as day_close, dd.n as day_n
from b3 b join dd using (symbol, d);
create index on bs (symbol, dn, ts);
create index on bs (symbol, d, ts);
