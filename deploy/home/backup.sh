#!/bin/bash
# Backup diário do banco do bot (host umbrelOS da casa).
# Instalado em /data/trader/bin/backup.sh e agendado pelo timer systemd
# trader-backup.timer (21:30 UTC). Mesma lógica da VM Oracle (ADR-011),
# adaptada para o compose da casa (ADR-012).
set -e
TS=$(date +%Y%m%d-%H%M%S)
docker exec trader-postgres pg_dump -U trader -p 5433 -d trader_db | gzip > /data/trader/backups/trader_db-$TS.sql.gz
find /data/trader/backups -name 'trader_db-*.sql.gz' -mtime +7 -delete