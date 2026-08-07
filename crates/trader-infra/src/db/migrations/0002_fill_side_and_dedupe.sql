-- 0002: fills ganham `side` (o fill precisa ser autodescritivo, pois fills de
-- saída do bracket — stop/alvo — pertencem a ordens filhas que não existem no
-- banco) e dedupe por `broker_fill_id` para idempotência no replay de
-- execuções do dia (subscribe_order_events via polling de reqExecutions).

ALTER TABLE fills ADD COLUMN IF NOT EXISTS side TEXT NOT NULL DEFAULT 'buy';

CREATE UNIQUE INDEX IF NOT EXISTS idx_fills_broker_fill_id
    ON fills (broker_fill_id)
    WHERE broker_fill_id IS NOT NULL;
