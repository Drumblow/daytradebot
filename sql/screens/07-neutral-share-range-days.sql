-- =============================================================================
-- 07-neutral-share-range-days.sql — Participação do contexto "neutral" em dias
--                                    de range (não é setup: é a medição que
--                                    motiva a range-extreme-fade-v2)
-- -----------------------------------------------------------------------------
-- O que mede: proxy do contexto global do bot (crates/trader-core/src/context/
--            mod.rs:79-81 exige Uptrend/Downtrend para `is_tradeable`) usando
--            SMA20/SMA200 do 15m: up = close > SMA20 > SMA200; down = close <
--            SMA20 < SMA200; senão neutral. Cruza com "dia de range" = range do
--            dia < 1,5 × ATRd (o próprio filtro da range-extreme-fade-v1) e
--            reporta a participação de cada estado, nas barras 09:45–15:15 dos
--            8 ativos vivos, após 200 barras de aquecimento.
-- Fonte    : plano-mestre §2.3 achado 5 e §6.1; docs/strategies/range-extreme-
--            fade-v2.md. O contexto real usa EMA20/SMA200 (classify_trend) —
--            o proxy com SMA20 é aproximação; a variante B abaixo (dos críticos)
--            usa ATR em preço absoluto em vez de relativo, mesma leitura.
-- Blocos (N de variantes = 1 medição + 1 variante de verificação):
--   A  Q14 (designer, re-executado pelos críticos);
--   B  variante autocontida dos críticos (crit_neutral).
-- Resultado 06/09/2026 (re-simulação dos críticos): em dias de range, 44,5%
--   das barras são neutral (26.686 de 59.976) contra 41,1% nos dias de
--   tendência; com a regra real EMA20/SMA200 por símbolo: AVUV 47%, SLYV 46%,
--   IJS 46%, VBR 45%, IWN 44%, IWV 41%. Produção 31/08–04/09: 36% neutral
--   (docs/reports/pregoes-2026-08-31_a_09-02.md). Medido com log de debug:
--   NoContext rejeitou 48/36/62 sinais da range-fade em AVUV/SLYV/IWV contra
--   31/30/35 entradas. Ressalva dos críticos: é participação de BARRAS, não de
--   SINAIS — o PF dos sinais bloqueados é desconhecido (plano §6.1 passo 1).
-- Uso      : docker exec -i trader-postgres psql -U trader -d trader_db < sql/screens/07-neutral-share-range-days.sql
-- Fuso     : America/New_York. Cópia fiel de q5.sql Q14 e crit_neutral.sql.
-- =============================================================================
\set ON_ERROR_STOP on

create temp table bars as
select a.symbol, c.timestamp at time zone 'America/New_York' as ts, (c.timestamp at time zone 'America/New_York')::date d, c.open, c.high, c.low, c.close, c.volume
from candles c join assets a on a.id=c.asset_id where c.timeframe='15m' and a.symbol in ('AVUV','IJS','IWM','IWN','IWO','IWV','SLYV','VBR','IJR','SCHA','VB');
create index on bars(symbol,d,ts);
create temp table days as select symbol, d, (array_agg(open order by ts))[1] o, max(high) h, min(low) l, (array_agg(close order by ts desc))[1] cl,
 max(high) filter (where ts::time<'10:30') h1h, min(low) filter (where ts::time<'10:30') h1l, (array_agg(close order by ts))[4] h1c, count(*) n from bars group by 1,2;
create temp table dd as select *, lag(cl) over w pc, lag(h) over w ph, lag(l) over w pl, lag(o) over w po,
 avg((h-l)/cl) over (partition by symbol order by d rows between 14 preceding and 1 preceding) atrd from days window w as (partition by symbol order by d);

-- A. Q14: context proxy (close>SMA20>SMA200 up / close<SMA20<SMA200 down / else neutral) share overall and inside range-like days (day range < 1.5*ATRd), 8 active symbols
with x as (select symbol, ts, d, close, avg(close) over (partition by symbol order by ts rows between 19 preceding and current row) s20, avg(close) over (partition by symbol order by ts rows between 199 preceding and current row) s200,
           row_number() over (partition by symbol order by ts) rn from bars where symbol in ('AVUV','IJS','IWM','IWN','IWO','IWV','SLYV','VBR')),
 y as (select x.*, case when close > s20 and s20 > s200 then 'up' when close < s20 and s20 < s200 then 'down' else 'neutral' end st, ((dd.h-dd.l)/dd.cl < 1.5*dd.atrd) rangeday from x join dd using(symbol,d) where rn > 200 and dd.atrd is not null and ts::time between '09:45' and '15:15')
select 'Q14 context proxy' q, rangeday, st, count(*) bars, round((100.0*count(*)/sum(count(*)) over (partition by rangeday))::numeric,1) pct_within_group from y group by 2,3 order by 2,3;

-- B. Variante de verificação dos críticos (autocontida; ATR14 em preço absoluto)
with b as (
  select a.symbol, c.timestamp, (c.timestamp at time zone 'America/New_York') as ts_et,
         (c.timestamp at time zone 'America/New_York')::date as d, c.high, c.low, c.close,
         avg(c.close) over (partition by c.asset_id order by c.timestamp rows between 19 preceding and current row) sma20,
         avg(c.close) over (partition by c.asset_id order by c.timestamp rows between 199 preceding and current row) sma200,
         row_number() over (partition by c.asset_id order by c.timestamp) rn
  from candles c join assets a on a.id=c.asset_id
  where c.timeframe='15m' and a.symbol in ('AVUV','IJS','IWM','IWN','IWO','IWV','SLYV','VBR')
),
days2 as (
  select symbol, d, max(high)-min(low) rng, max(close) lc from b group by 1,2
),
atrd as (
  select symbol, d, rng, avg(rng) over (partition by symbol order by d rows between 14 preceding and 1 preceding) atr14 from days2
),
cls as (
  select b.*, case when close>sma20 and sma20>sma200 then 'up' when close<sma20 and sma20<sma200 then 'down' else 'neutral' end st,
         case when a.rng < 1.5*a.atr14 then 'range' else 'trend' end dt
  from b join atrd a using(symbol,d)
  where rn>200 and ts_et::time between '09:45' and '15:15' and a.atr14 is not null
)
select 'Q14b criticos (ATR abs)' q, dt, st, count(*) n, round(100.0*count(*)/sum(count(*)) over (partition by dt),1) pct from cls group by 1,2,3 order by 2,3;
