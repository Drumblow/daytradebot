#!/bin/bash
# start|stop dos containers trader-* (11 instâncias).
# Usado pelos timers systemd trader-start/stop (ADR-012, host umbrelOS).
set -euo pipefail

ACTION="${1:?uso: trader-containers.sh start|stop}"
COMPOSE="docker compose -f /data/trader/docker-compose.yml"
INSTANCES="trader-iwm-pullback trader-iwv-pullback trader-iwo-pullback \
trader-ijs-balance trader-vbr-balance trader-avuv-balance \
trader-iwm-openrev trader-iwn-openrev \
trader-avuv-rangefade trader-slyv-rangefade trader-iwv-rangefade"

case "$ACTION" in
  start)
    # NÃO usar `compose start`: ele retorna 0 mesmo quando o container NÃO
    # existe. Verificado em 2026-08-28 — depois do reboot que apagou os
    # containers (incidente 08-23), o script imprimia "started" sem subir nada
    # e o timer das 9h25 teria falhado em silêncio.
    # `up -d --no-recreate` é idempotente: cria o que falta, inicia o que está
    # parado e não recria o que já roda.
    $COMPOSE up -d --no-recreate $INSTANCES

    # Falha explícita se alguma instância não ficou de pé — o timer precisa
    # aparecer como failed no systemd, não terminar "com sucesso" sem bot.
    running=0
    for svc in $INSTANCES; do
      if [ "$(docker inspect -f '{{.State.Running}}' "$svc" 2>/dev/null)" = "true" ]; then
        running=$((running + 1))
      else
        echo "AVISO: $svc não está rodando" >&2
      fi
    done
    echo "trader containers started ($running/11 rodando)"
    [ "$running" -eq 11 ] || { echo "ERRO: esperava 11 instâncias, subiram $running" >&2; exit 1; }
    ;;
  stop)
    $COMPOSE stop $INSTANCES
    echo "trader containers stopped"
    ;;
  *)
    echo "uso: trader-containers.sh start|stop" >&2
    exit 1
    ;;
esac
