#!/bin/bash
# Entry do container do IB Gateway (umbrelOS, ADR-012).
# Controla Xvfb explicitamente (xvfb-run como PID 1 travava sem executar o comando)
set -euo pipefail

DISP="${DISPLAY:-:97}"
export DISPLAY="$DISP"

# Limpa locks antigos de displays mortos
rm -f "/tmp/.X${DISP#:}-lock" "/tmp/.X11-unix/X${DISP#:}" 2>/dev/null || true

mkdir -p /tmp/xvfb-auth
XAUTH=/tmp/xvfb-auth/authority
export XAUTHORITY="$XAUTH"
rm -f "$XAUTH"
touch "$XAUTH"

echo "[gateway-entry] iniciando Xvfb em $DISP"
Xvfb "$DISP" -screen 0 1280x1024x24 -nolisten tcp -auth "$XAUTH" &
XVFB_PID=$!

# espera o display ficar pronto
for i in $(seq 1 30); do
  if xdpyinfo -display "$DISP" >/dev/null 2>&1; then
    echo "[gateway-entry] Xvfb pronto (pid $XVFB_PID)"
    break
  fi
  sleep 1
done

# IBC/Gateway (bloqueia; sinais propagados)
exec /opt/trader/ibc/gatewaystart-vm.sh -inline