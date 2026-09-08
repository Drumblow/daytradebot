-- OPCIONAL — requer decisão do dono (ADR-018, decisão 2).
--
-- Reclassifica os encerramentos de fim de pregão já gravados como `manual`
-- para `end_of_day`. Antes do ADR-018 o live não tinha variante própria e
-- marcava o flatten no journal (`forced_exit = 'session_flatten'`); o
-- predicado abaixo é exato — nenhum outro caminho grava essa marca.
--
-- Enquanto este script NÃO for rodado, o código continua correto: o
-- `Trade::effective_exit_reason()` reconhece o flatten pelo journal, então o
-- gate B não mistura categorias. Rodar isto é higiene, não correção.
--
-- Reversível: a marca no journal permanece após o UPDATE.
--   UPDATE trades SET exit_reason = 'manual'
--    WHERE exit_reason = 'end_of_day' AND journal->>'forced_exit' = 'session_flatten';
--
-- Antes de rodar: dump da tabela e registro dos ids afetados no relatório de
-- higiene (§5.6 do plano).

BEGIN;

-- 1. O que será afetado (conferir antes do UPDATE).
SELECT id, symbol_id_placeholder.symbol, entry_time, exit_time, exit_reason
  FROM trades
  JOIN (SELECT id AS aid, symbol FROM assets) AS symbol_id_placeholder
    ON symbol_id_placeholder.aid = trades.asset_id
 WHERE exit_reason = 'manual'
   AND journal ->> 'forced_exit' = 'session_flatten'
 ORDER BY exit_time;

-- 2. A reclassificação.
UPDATE trades
   SET exit_reason = 'end_of_day'
 WHERE exit_reason = 'manual'
   AND journal ->> 'forced_exit' = 'session_flatten';

-- COMMIT;  -- descomentar após conferir a contagem do passo 1
ROLLBACK;
