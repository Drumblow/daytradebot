#!/bin/bash
# Backup diário do banco do bot (VM Oracle). Instalado em /opt/trader/bin/backup.sh
# pelo deploy (ver .github/workflows/deploy.yml) e agendado em /etc/cron.d/trader-backup.
# ATENÇÃO: o serviço cron precisa estar ativo na VM (estava inativo até 2026-08-17,
# o que deixou o banco 10 dias sem backup — ver docs/reports/day9-2026-08-14.md §4).
set -e
TS=$(date +%Y%m%d-%H%M%S)
docker exec trader-postgres pg_dump -U trader trader_db | gzip > /opt/trader/backups/trader_db-$TS.sql.gz
find /opt/trader/backups -name 'trader_db-*.sql.gz' -mtime +7 -delete
