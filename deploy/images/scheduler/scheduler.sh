#!/bin/bash
# Scheduler do app do umbrelOS (ADR-013): liga e desliga as instancias na janela
# de pregao e faz o backup diario do banco.
#
# Herda a licao do trader-containers.sh: NUNCA confiar no codigo de saida de um
# comando de start para concluir que os containers subiram. Em 2026-08-28
# descobrimos que `docker compose start` retorna 0 mesmo quando o container nao
# existe — o timer teria falhado em silencio e custado um quinto pregao.
set -uo pipefail

INSTANCES="${INSTANCES:?INSTANCES obrigatorio}"
POSTGRES_CONTAINER="${POSTGRES_CONTAINER:-daytradebot_postgres_1}"
BACKUP_DIR="${BACKUP_DIR:-/backups}"
PGUSER_NAME="${PGUSER_NAME:-trader}"
PGDATABASE_NAME="${PGDATABASE_NAME:-trader_db}"
PGPORT_NUM="${PGPORT_NUM:-5433}"

log() { echo "[scheduler $(date '+%F %T %Z')] $*"; }

# ── alertas ──────────────────────────────────────────────────────────────────
# O scheduler detectava problemas e so escrevia no proprio log: backup que
# falhou, pg_dump que devolveu um arquivo de 200 bytes, "subiram 7 de 11 no
# open". Nada disso chegava a ninguem (A9 da auditoria de 30/08/2026).
#
# O segredo vem montado por volume, como nas instancias — nunca na imagem.
ALERTS_ENV="${ALERTS_ENV_FILE:-/run/trader-secrets/alerts.env}"
if [ -f "$ALERTS_ENV" ]; then
  set -a
  # shellcheck disable=SC1090
  . "$ALERTS_ENV"
  set +a
fi

# Envia um alerta critico. Sem webhook configurado, so loga — nunca falha e
# nunca interrompe a operacao por causa de um alerta.
alerta() {
  local msg="$1" corpo url
  url="${TRADER__ALERTS__WEBHOOK_URL:-}"
  [ -n "$url" ] || return 0

  # Mesma deteccao do Alerter em Rust: Discord quer {"content"}, Slack quer
  # {"text"}, e o endpoint compativel .../slack do Discord quer o do Slack.
  case "$url" in
    */slack) corpo=$(printf '{"text":"%s"}' "$msg") ;;
    *discord.com/api/webhooks*|*discordapp.com/api/webhooks*)
      corpo=$(printf '{"content":"%s"}' "$msg") ;;
    *) corpo=$(printf '{"text":"%s"}' "$msg") ;;
  esac

  # --fail para que 4xx/5xx nao passem por sucesso; a URL carrega token e por
  # isso nunca entra no log.
  if ! curl -sS --fail --max-time 10 -H 'Content-Type: application/json'        -d "$corpo" "$url" >/dev/null 2>&1; then
    log "AVISO: webhook recusou o alerta"
  fi
}

expected_count() { echo "$INSTANCES" | wc -w | tr -d ' '; }

# Estamos dentro da janela de pregao? Usado para distinguir "as instancias nao
# subiram porque quebraram" de "as instancias sairam pela guarda porque e fora de
# hora" — que e o comportamento correto e nao pode virar alarme.
dentro_da_janela() {
  [ "$(date +%u)" -le 5 ] || return 1
  local agora inicio fim
  agora=$(date +%H%M)
  inicio=$(printf '%02d%02d' "${START_HOUR}" "${START_MIN}")
  fim=$(printf '%02d%02d' "${END_HOUR}" "${END_MIN}")
  [ "$agora" -ge "$inicio" ] && [ "$agora" -lt "$fim" ]
}

start_instances() {
  local expected running=0 svc
  expected=$(expected_count)
  log "abrindo pregao: ligando $expected instancias"

  for svc in $INSTANCES; do
    docker start "$svc" >/dev/null 2>&1 || log "AVISO: falha ao ligar $svc"
  done

  # A instancia sai com 0 se estiver fora da janela (guarda do entrypoint), entao
  # damos um instante antes de conferir para nao contar um container que ja saiu.
  sleep 10

  local na_janela=0
  dentro_da_janela && na_janela=1

  for svc in $INSTANCES; do
    if [ "$(docker inspect -f '{{.State.Running}}' "$svc" 2>/dev/null)" = "true" ]; then
      running=$((running + 1))
    elif [ "$na_janela" -eq 1 ]; then
      log "AVISO: $svc NAO esta rodando"
    fi
  done

  if [ "$na_janela" -eq 0 ]; then
    log "fora da janela de pregao: as $expected instancias sairam pela guarda do"
    log "entrypoint ($running rodando). Comportamento esperado, nao e erro."
    return 0
  fi

  log "instancias rodando: $running/$expected"
  if [ "$running" -ne "$expected" ]; then
    log "ERRO: esperava $expected instancias, subiram $running"
    alerta "🚨 abertura do pregao: subiram $running de $expected instancias"
  fi
}

stop_instances() {
  log "fechando pregao: parando instancias"
  for svc in $INSTANCES; do
    docker stop "$svc" >/dev/null 2>&1 || log "AVISO: falha ao parar $svc"
  done
  log "instancias paradas"
}

backup_db() {
  local ts file
  ts=$(date +%Y%m%d-%H%M%S)
  file="$BACKUP_DIR/trader_db-$ts.sql.gz"
  mkdir -p "$BACKUP_DIR"

  if docker exec "$POSTGRES_CONTAINER" \
       pg_dump -U "$PGUSER_NAME" -p "$PGPORT_NUM" -d "$PGDATABASE_NAME" 2>/dev/null \
       | gzip > "$file"; then
    # Um pg_dump que falha no meio deixa um .gz pequeno e valido. Conferir o
    # tamanho evita "backup diario" que na verdade e um arquivo vazio.
    local size
    size=$(stat -c %s "$file" 2>/dev/null || echo 0)
    if [ "$size" -lt 10000 ]; then
      log "ERRO: backup suspeito ($size bytes) — mantido para inspecao: $file"
      alerta "🚨 backup do banco suspeito: apenas $size bytes em $file"
    else
      log "backup ok: $file ($size bytes)"
    fi
  else
    log "ERRO: pg_dump falhou"
    alerta "🚨 backup do banco FALHOU (pg_dump nao completou)"
    rm -f "$file"
  fi

  find "$BACKUP_DIR" -name 'trader_db-*.sql.gz' -mtime "+$BACKUP_RETENTION_DAYS" -delete 2>/dev/null
}

# ── agenda ───────────────────────────────────────────────────────────────────
# crond do busybox le o TZ do container, entao tudo fica em horario de NY e o
# horario de verao americano e resolvido pelo tzdata.
read -r START_MIN START_HOUR <<<"$TRADING_START"
read -r END_MIN END_HOUR <<<"$TRADING_END"
read -r BACKUP_MIN BACKUP_HOUR <<<"$BACKUP_AT"

mkdir -p /etc/crontabs
cat > /etc/crontabs/root <<CRON
$START_MIN $START_HOUR * * 1-5 /usr/local/bin/scheduler.sh start-instances
$END_MIN $END_HOUR * * 1-5 /usr/local/bin/scheduler.sh stop-instances
$BACKUP_MIN $BACKUP_HOUR * * * /usr/local/bin/scheduler.sh backup
CRON

case "${1:-run}" in
  start-instances) start_instances; exit 0 ;;
  stop-instances)  stop_instances;  exit 0 ;;
  backup)          backup_db;       exit 0 ;;
esac

log "agenda (TZ=$TZ):"
sed 's/^/[scheduler]   /' /etc/crontabs/root
log "instancias sob controle: $(expected_count)"

# Se o app subiu dentro da janela de pregao — por exemplo o servidor religou as
# 10h da manha de uma terca — as instancias precisam subir agora, sem esperar o
# proximo 9h25. A guarda do entrypoint delas decide se e hora ou nao.
if dentro_da_janela; then
  log "app subiu DENTRO da janela de pregao — ligando as instancias agora"
  start_instances
fi

exec crond -f -l 8
