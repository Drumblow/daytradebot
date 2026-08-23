#!/bin/bash
# start|stop dos containers trader-* (11 instâncias).
# Usado pelos timers systemd trader-start/stop (ADR-012, host umbrelOS).
set -euo pipefail

ACTION="${1:?uso: trader-containers.sh start|stop}"
COMPOSE="docker compose -f /data/trader/docker-compose.yml"

case "$ACTION" in
  start)
    # Sobe apenas as instâncias (gateway/postgres ficam sempre ativos com restart policy)
    for svc in trader-iwm-pullback trader-iwv-pullback trader-iwo-pullback \
               trader-ijs-balance trader-vbr-balance trader-avuv-balance \
               trader-iwm-openrev trader-iwn-openrev \
               trader-avuv-rangefade trader-slyv-rangefade trader-iwv-rangefade; do
      $COMPOSE start "$svc" >/dev/null 2>&1 || $COMPOSE up -d --no-recreate "$svc" >/dev/null 2>&1 || true
    done
    echo "trader containers started"
    ;;
  stop)
    $COMPOSE stop trader-iwm-pullback trader-iwv-pullback trader-iwo-pullback \
                trader-ijs-balance trader-vbr-balance trader-avuv-balance \
                trader-iwm-openrev trader-iwn-openrev \
                trader-avuv-rangefade trader-slyv-rangefade trader-iwv-rangefade 2>/dev/null || true
    echo "trader containers stopped"
    ;;
  *)
    echo "uso: trader-containers.sh start|stop" >&2
    exit 1
    ;;
esac