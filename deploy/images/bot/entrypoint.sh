#!/bin/bash
# Entrypoint de uma instancia do bot no app do umbrelOS (ADR-013).
set -euo pipefail

if [[ -z "${SYMBOL:-}" || -z "${STRATEGY:-}" ]]; then
  echo "ERRO: SYMBOL e STRATEGY sao obrigatorios no env" >&2
  exit 1
fi

# ── guarda de janela de pregao ───────────────────────────────────────────────
# Por que isto existe: no host antigo quem ligava e desligava as instancias eram
# timers systemd, que o rugix apaga a cada boot. Na app quem faz isso e o
# container scheduler — mas o umbreld sobe TODOS os servicos da app no boot.
# Sem esta guarda, um religamento as 3h da manha (queda de energia, que e
# exatamente o cenario do incidente de 2026-08-23) colocaria 11 instancias
# conectando na IBKR fora de hora.
#
# Sai com 0, nao com erro: combinado com `restart: on-failure` no compose, um bot
# que sai limpo fica parado e o scheduler o liga as 9h25. Um bot que quebra de
# verdade reinicia.
if [[ "${SKIP_WINDOW_GUARD:-false}" != "true" ]]; then
  dow=$(date +%u)          # 1=segunda ... 7=domingo
  now=$(date +%H:%M)

  if (( dow > 5 )); then
    echo "[$SYMBOL/$STRATEGY] fim de semana ($(date +%a)) — nada a fazer, saindo."
    exit 0
  fi

  if [[ "$now" < "${TRADING_START}" || "$now" > "${TRADING_END}" ]]; then
    echo "[$SYMBOL/$STRATEGY] fora da janela ${TRADING_START}-${TRADING_END} ET (agora ${now}) — saindo."
    echo "[$SYMBOL/$STRATEGY] o scheduler liga esta instancia as ${TRADING_START} ET."
    exit 0
  fi

  echo "[$SYMBOL/$STRATEGY] dentro da janela ${TRADING_START}-${TRADING_END} ET (agora ${now})."
fi

# ── webhook de alertas (opcional) ────────────────────────────────────────────
# Segredo do dispositivo, montado por volume (nunca na imagem publica). Se o
# arquivo existir e definir TRADER__ALERTS__WEBHOOK_URL, o bot passa a enviar
# alertas criticos (circuit breaker, live_stopped) para o webhook — sem ele, a
# unica forma de saber que o bot caiu e olhar o painel/logs (HANDOFF §5).
ALERTS_ENV="${ALERTS_ENV_FILE:-/run/trader-secrets/alerts.env}"
if [[ -f "$ALERTS_ENV" ]]; then
  set -a
  # shellcheck disable=SC1090
  source "$ALERTS_ENV"
  set +a
  if [[ -n "${TRADER__ALERTS__WEBHOOK_URL:-}" ]]; then
    echo "[$SYMBOL/$STRATEGY] webhook de alertas configurado."
  fi
fi

exec /opt/trader/bin/trader-cli paper --mode live --symbol "$SYMBOL" --strategy "$STRATEGY"
