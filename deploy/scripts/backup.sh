#!/bin/bash
# Backup diário do banco do bot (VM Oracle). Instalado em /opt/trader/bin/backup.sh
# pelo deploy (ver .github/workflows/deploy.yml) e agendado pelo timer systemd
# trader-backup.timer (21:30 UTC). O agendamento original era /etc/cron.d, mas a
# VM nem tinha o pacote cron instalado — o banco ficou sem backup de 08-07 a
# 08-17 (ver docs/reports/day9-2026-08-14.md §4).
set -e
TS=$(date +%Y%m%d-%H%M%S)
docker exec trader-postgres pg_dump -U trader trader_db | gzip > /opt/trader/backups/trader_db-$TS.sql.gz
find /opt/trader/backups -name 'trader_db-*.sql.gz' -mtime +7 -delete
