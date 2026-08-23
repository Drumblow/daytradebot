#!/bin/bash
# Entrypoint do container trader-<instancia>
# Lê SYMBOL/STRATEGY do env da instância e executa o trader-cli como na VM.
set -e

if [[ -z "$SYMBOL" || -z "$STRATEGY" ]]; then
  echo "ERRO: SYMBOL e STRATEGY obrigatórios no env" >&2
  exit 1
fi

exec /opt/trader/bin/trader-cli paper --mode live --symbol "$SYMBOL" --strategy "$STRATEGY"