#!/usr/bin/env bash
# sql/screens/run.sh — roda um screen 09+ (lib + setup + eval) contra o banco dev.
# Uso: sql/screens/run.sh 17-fade-extremo-ndias.sql [-v hold_days=1] [-v stop_floor_atr=1.0] ...
# Saída bruta em stdout; guarde com `mkdir -p docs/reports/screens-raw && ... | tee docs/reports/screens-raw/<arquivo>.txt`
# (o diretório não existe no repo e não deve ser commitado bruto; o que entra no repo é o resumo em docs/reports/screens-<data>.md).
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
setup="$1"; shift
cat "$here/_lib.sql" "$here/$setup" "$here/_eval.sql" \
  | docker exec -i trader-postgres psql -U trader -d trader_db -q -X "$@"
