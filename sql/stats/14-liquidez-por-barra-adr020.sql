-- Barra mediana de 15m por símbolo — a base do cap de liquidez (ADR-020 §3).
--
-- **O feed esparso, medido (08/09/2026):** o plano já registra, no achado 7
-- do §2.3, que o Gateway entrega 3–10% do volume desde a troca de TWS →
-- Gateway em 07/08/2026. Medindo a barra mediana sobre TODOS os pregões, a
-- dispersão é bem maior do que a estimativa: IWM guarda 3,2% do volume
-- anterior, AVUV 7,1%, IWN 20%, IWV 32%, IJS 41%, VBR 51% e SLYV **89%** —
-- com as 26 barras de sempre em cada pregão e o mesmo `source = 'ibkr'`.
-- Os pregões de agosto foram reingeridos em 03/09 e voltaram iguais:
-- reingerir do Gateway não repara o histórico.
--
-- Por isso as duas janelas abaixo:
--   * PRÉ-GATEWAY  — 60 pregões até 06/08/2026. É a única liquidez medida em
--                    que dá para acreditar hoje, e é a que a ADR-020 usou;
--   * ÚLTIMAS 600 BARRAS — o que o código calcularia AGORA, que é o número
--                    que o cap usaria se alguém o ligasse.
--
-- A razão entre as duas é o tamanho da armadilha: ligar o cap de liquidez
-- hoje aplicaria um teto medido num volume que não existe.
--
-- Rodar no banco DEV:
--   docker exec -i trader-postgres psql -U trader -d trader_db \
--     -f - < sql/stats/14-liquidez-por-barra-adr020.sql

WITH base AS (
    SELECT a.symbol,
           c.timestamp,
           c.close * c.volume AS notional,
           (c.timestamp AT TIME ZONE 'America/New_York')::date AS pregao
    FROM candles c
    JOIN assets a ON a.id = c.asset_id
    WHERE c.timeframe = '15m'
      AND c.volume > 0
),
pregoes_pre AS (
    SELECT symbol, pregao,
           row_number() OVER (PARTITION BY symbol ORDER BY pregao DESC) AS n
    FROM (SELECT DISTINCT symbol, pregao FROM base WHERE pregao < DATE '2026-08-07') d
),
pre_gateway AS (
    SELECT b.symbol,
           percentile_cont(0.5) WITHIN GROUP (ORDER BY b.notional) AS mediana,
           percentile_cont(0.1) WITHIN GROUP (ORDER BY b.notional) AS p10,
           count(DISTINCT b.pregao) AS pregoes,
           min(b.pregao) AS de,
           max(b.pregao) AS ate
    FROM base b
    JOIN pregoes_pre p ON p.symbol = b.symbol AND p.pregao = b.pregao
    WHERE p.n <= 60
    GROUP BY b.symbol
),
ordenado AS (
    SELECT symbol, notional,
           row_number() OVER (PARTITION BY symbol ORDER BY timestamp DESC) AS n
    FROM base
),
ultimas_600 AS (
    SELECT symbol,
           percentile_cont(0.5) WITHIN GROUP (ORDER BY notional) AS mediana,
           count(*) AS barras
    FROM ordenado
    WHERE n <= 600
    GROUP BY symbol
)
SELECT p.symbol,
       p.de,
       p.ate,
       round(p.mediana::numeric)       AS mediana_pre_gateway,
       round(p.p10::numeric)           AS p10_pre_gateway,
       round((p.mediana / 3)::numeric) AS teto_um_terco,
       round(u.mediana::numeric)       AS mediana_hoje_600b,
       round((u.mediana / nullif(p.mediana, 0))::numeric, 3) AS razao_hoje_pre,
       -- Quanto uma posição de 1× a equity paper (≈ US$ 238k) ocupa da barra
       -- mediana. Acima de 33% é o que o cap de 1/3 recusaria.
       round((238000 / nullif(p.mediana, 0) * 100)::numeric, 1) AS pct_da_barra_1x
FROM pre_gateway p
JOIN ultimas_600 u USING (symbol)
WHERE p.symbol IN ('SLYV', 'IJS', 'VBR', 'IWN', 'IWO', 'AVUV', 'IWM', 'IWV')
ORDER BY p.mediana;
