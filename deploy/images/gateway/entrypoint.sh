#!/bin/bash
# Entrypoint do container do IB Gateway no app do umbrelOS (ADR-013).
#
# Substitui o gatewaystart-vm.sh: exporta o que o IBC espera, sobe o Xvfb e
# executa o launcher do IBC. As credenciais vem de um arquivo montado do host
# e existem SO na memoria deste processo — nunca sao escritas em disco.
set -euo pipefail

SECRETS_FILE="${SECRETS_FILE:-/run/trader-secrets/ibkr.env}"

# ── credenciais ───────────────────────────────────────────────────────────────
# O IBC usa TWSUSERID/TWSPASSWORD quando IbLoginId/IbPassword estao vazios no
# ibc.ini — que e exatamente como a imagem publica e construida.
if [[ -r "$SECRETS_FILE" ]]; then
  # shellcheck disable=SC1090
  set -a; source "$SECRETS_FILE"; set +a
fi

if [[ -z "${TWSUSERID:-}" || -z "${TWSPASSWORD:-}" ]]; then
  cat >&2 <<MSG
ERRO: credenciais da Interactive Brokers ausentes.

Preencha no servidor, com a app instalada:

  \${APP_DATA_DIR}/secrets/ibkr.env

    TWSUSERID=<seu usuario IBKR>
    TWSPASSWORD=<sua senha IBKR>

e reinicie a app. O arquivo fica so no seu dispositivo — nao vai para o
repositorio nem para a imagem.
MSG
  exit 78  # EX_CONFIG
fi

# ── instalacao do Gateway (montada do host, nao vem na imagem) ───────────────
GATEWAY_DIR="${TWS_PATH}/ibgateway/${TWS_MAJOR_VRSN}"
if [[ ! -d "$GATEWAY_DIR" ]]; then
  cat >&2 <<MSG
ERRO: IB Gateway ${TWS_MAJOR_VRSN} nao encontrado em ${GATEWAY_DIR}.

A imagem nao redistribui o instalador da IBKR. Instale o IB Gateway em:

  \${APP_DATA_DIR}/gateway/ibgateway/${TWS_MAJOR_VRSN}/

MSG
  exit 78
fi

# ── Xvfb ─────────────────────────────────────────────────────────────────────
# Controlado explicitamente: xvfb-run como PID 1 travava sem executar o comando.
DISP="${DISPLAY:-:97}"
export DISPLAY="$DISP"
rm -f "/tmp/.X${DISP#:}-lock" "/tmp/.X11-unix/X${DISP#:}" 2>/dev/null || true

mkdir -p /tmp/xvfb-auth
XAUTH=/tmp/xvfb-auth/authority
export XAUTHORITY="$XAUTH"
rm -f "$XAUTH"; touch "$XAUTH"

echo "[gateway] iniciando Xvfb em $DISP"
Xvfb "$DISP" -screen 0 1280x1024x24 -nolisten tcp -auth "$XAUTH" &

for _ in $(seq 1 30); do
  xdpyinfo -display "$DISP" >/dev/null 2>&1 && break
  sleep 1
done
xdpyinfo -display "$DISP" >/dev/null 2>&1 || { echo "[gateway] ERRO: Xvfb nao subiu" >&2; exit 1; }
echo "[gateway] Xvfb pronto"

# ── IBC ──────────────────────────────────────────────────────────────────────
export APP=GATEWAY
export IBC_INI IBC_PATH TWS_PATH TWS_MAJOR_VRSN TWS_SETTINGS_PATH LOG_PATH
export JAVA_PATH TRADING_MODE TWOFA_TIMEOUT_ACTION
export TWSUSERID TWSPASSWORD

echo "[gateway] iniciando IBC (modo ${TRADING_MODE}, API 4002)"
exec "${IBC_PATH}/scripts/displaybannerandlaunch.sh"
