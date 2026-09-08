-- =============================================================================
-- 09-controle-pos-range-fade.sql — CONTROLE POSITIVO: range-extreme-fade-v1
-- -----------------------------------------------------------------------------
-- Setup  : modelo da v1 (docs/strategies/range-extreme-fade-v1.md §4–6;
--          config/strategies/range-extreme-fade-v1.toml):
--          dia de range = inclinação da EMA20 em 12 barras < 0,05%/barra E range
--          do dia (incl. a barra) < 1,5 × ATR diário(14) E (tag *_piv) sem
--          estrutura HH/HL ou LH/LL por pivôs de 2 barras nas 12 últimas;
--          NÃO modela o veto Barb Wire (omissão registrada);
--          rompimento do extremo do dia por 0 < ext <= 0,5 × ATR14 (barra);
--          barra de sinal contra (corpo >= 30%, sombra >= 1/3, fechamento no
--          terço favorável); veto meio-do-dia (11:30–14:00 ET) ∧ terço central
--          do range (em ET — o bug do v1 é em UTC); entrada stop 1 tick além da
--          barra de sinal, validade 2 barras; stop 1 tick do outro lado; alvo
--          1,5R; sinais 09:45–15:15 ET.
-- Fonte  : Brooks, Reading Price Charts Bar by Bar, Cap. 9 (failed HH/LL
--          breakouts) + Cap. 5 (Barb Wire) — docs/books/analysis/brooks-bar-by-bar.md.
-- Tags   : base        sinais da estratégia (todos, como o debug de 06/09 contou);
--          ctx         base ∧ is_tradeable do contexto global (close > EMA20 > SMA200
--                      ou close < EMA20 < SMA200 no 15m — context/mod.rs:79-81,120-128):
--                      é o que o RiskManager deixa executar (NoContext bloqueia o resto);
--          ctx_piv     ctx ∧ veto de estrutura por pivôs (context.rs has_trend_structure)
--                      = o modelo mais fiel à v1 executada;
--          ext_m20 / ext_p20  ctx_piv com max_extension 0,4 / 0,6 × ATR14 (±20%);
--          osc / osc30 ctx_piv ∧ oscillator-presignal %D_slow(14,3,3) < 20 long / > 80
--                      short (Murphy Cap. 10/12, análise §3.10–3.11) / 30–70.
-- Serve para: (1) calibrar o screener (README §4: ctx_piv tem de PASSAR §5);
--          (2) medir o PF dos sinais que o contexto bloqueia (base − ctx);
--          (3) veredito do oscillator-presignal como filtro (osc vs ctx_piv);
--          (4) triple-barrier-exit via -v max_bars_hold=N (delta vs ctx_piv);
--          (5) sem piso de stop: -v risk_min=0.0005 (a v1 não tem piso).
-- Referência a bater: gate A OOS AVUV PF 1,95 / SLYV 2,95 / IWV 1,32; com
--          flatten 15:45 e 2 bp, PF 1,74 no agregado dos 3 pares (69 trades).
-- N de variantes neste arquivo: 7 tags (base, ctx, ctx_piv, ±20%, osc, osc30).
-- =============================================================================

create temp table cand as
with s as (
  select b.*,
         (high - th_prev)  as ext_up,
         (tl_prev - low)   as ext_dn,
         abs(close - open) / nullif(high - low, 0)                    as body_pct,
         (high - greatest(open, close)) / nullif(high - low, 0)        as uwick_pct,
         (least(open, close) - low) / nullif(high - low, 0)            as lwick_pct,
         (th_cur - tl_cur)                                             as day_range_px,
         (ts::time between '11:30' and '13:59')
           and close > tl_cur + (th_cur - tl_cur) / 3
           and close < th_cur - (th_cur - tl_cur) / 3                  as midday_midrange,
         ((close > ema20 and ema20 > sma200) or (close < ema20 and ema20 < sma200)) as tradeable
  from bs b
  where rn >= 2 and ts::time between '09:45' and '15:15'
    and atr14 is not null and atrd_px is not null and ema_slope is not null and sma200 is not null
    and high > low
)
select s.*,
       case when ext_up > 0 and ext_up <= 0.6 * atr14
                 and close < open and body_pct >= 0.30 and uwick_pct >= 0.334
                 and close <= high - (high - low) * 2 / 3 then -1
            when ext_dn > 0 and ext_dn <= 0.6 * atr14
                 and close > open and body_pct >= 0.30 and lwick_pct >= 0.334
                 and close >= low + (high - low) * 2 / 3 then 1 end as dir
from s
where ema_slope < 0.0005
  and day_range_px < 1.5 * atrd_px
  and not midday_midrange;

-- veto de estrutura por pivôs (v1 context.rs has_trend_structure): últimos 2
-- pivot highs e 2 pivot lows nas barras [seq-11, seq-2]; HH∧HL ou LH∧LL = veto
create temp table cand2 as
select c.*,
       (ph.n = 2 and pl.n = 2 and ((ph.last > ph.prev and pl.last > pl.prev)
                                   or (ph.last < ph.prev and pl.last < pl.prev))) as trend_struct
from cand c
left join lateral (
  select count(*) as n, (array_agg(high order by seq desc))[1] as last, (array_agg(high order by seq desc))[2] as prev
  from (select high, seq from bs x where x.symbol = c.symbol and x.seq between c.seq - 11 and c.seq - 2 and x.is_ph
        order by seq desc limit 2) z) ph on true
left join lateral (
  select count(*) as n, (array_agg(low order by seq desc))[1] as last, (array_agg(low order by seq desc))[2] as prev
  from (select low, seq from bs x where x.symbol = c.symbol and x.seq between c.seq - 11 and c.seq - 2 and x.is_pl
        order by seq desc limit 2) z) pl on true
where c.dir is not null;

create temp table sel as
select v.tag, symbol, d, dn, dir,
       case when dir = 1 then high + 0.01 else low - 0.01 end as trigger_px,
       case when dir = 1 then low - 0.01  else high + 0.01 end as stop_px,
       ts as ts_sig, 2 as valid_bars, atr14 as atr_px
from cand2 c
cross join (values ('base', 0.5, false, false, 0),
                   ('ctx', 0.5, true, false, 0),
                   ('ctx_piv', 0.5, true, true, 0),
                   ('ext_m20', 0.4, true, true, 0),
                   ('ext_p20', 0.6, true, true, 0),
                   ('osc', 0.5, true, true, 20),
                   ('osc30', 0.5, true, true, 30)) v(tag, ext_mult, need_ctx, need_piv, osc)
where ((dir = -1 and ext_up <= v.ext_mult * atr14) or (dir = 1 and ext_dn <= v.ext_mult * atr14))
  and (not v.need_ctx or tradeable)
  and (not v.need_piv or not coalesce(trend_struct, false))
  and (v.osc = 0 or (dir = 1 and stoch_d_slow < v.osc) or (dir = -1 and stoch_d_slow > 100 - v.osc));
