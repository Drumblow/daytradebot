-- 0004: `end_of_day` como motivo de saída próprio (ADR-018).
--
-- O live encerra tudo a mercado entre 15h55 e 16h10 ET porque as pernas do
-- bracket vão com TIF Day e expiram no sino (C1 da auditoria de 30/08/2026).
-- Até aqui esse encerramento era gravado como `manual` com
-- `journal->>'forced_exit' = 'session_flatten'`, e o backtest não fechava nada
-- no fim do pregão — a régua do gate A comparava o live com um backtest que
-- ganha dinheiro dormindo posicionado.
--
-- Esta migração APENAS amplia o domínio do CHECK. Ela não reescreve trade
-- nenhum: a reclassificação das linhas já gravadas é decisão do dono
-- (ADR-018, decisão 2). Enquanto ela não for tomada, o código reconhece o
-- flatten histórico pelo journal — `Trade::effective_exit_reason()` em
-- `crates/trader-domain/src/trades.rs` — e o script opcional
-- `sql/maintenance/0004-reclassificar-flatten.sql` faz o UPDATE quando
-- autorizado.

-- O CHECK da 0001 é anônimo (`exit_reason TEXT NOT NULL CHECK (...)`), então
-- o nome é o implícito do Postgres. Verificado em dev (`trades_exit_reason_check`),
-- mas derrubar POR NOME é falha silenciosa se algum ambiente divergir: o DROP
-- não acha nada, o ADD cria o novo, o antigo continua valendo e o primeiro
-- INSERT de `end_of_day` quebra em produção (SQLSTATE 23514). Por isso o DROP
-- é por BUSCA: qualquer CHECK desta tabela que fale de `exit_reason` sai.
DO $$
DECLARE
    c RECORD;
BEGIN
    FOR c IN
        SELECT conname
          FROM pg_constraint
         WHERE conrelid = 'trades'::regclass
           AND contype = 'c'
           AND pg_get_constraintdef(oid) ILIKE '%exit_reason%'
    LOOP
        EXECUTE format('ALTER TABLE trades DROP CONSTRAINT %I', c.conname);
    END LOOP;
END $$;

ALTER TABLE trades
    ADD CONSTRAINT trades_exit_reason_check
    CHECK (exit_reason IN ('target', 'stop', 'time', 'manual', 'risk_manager', 'end_of_day'));
