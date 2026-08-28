#!/bin/bash
# Entrypoint do runner self-hosted dentro do app do umbrelOS (ADR-013).
#
# Dois modos:
#   configure  — registra o runner uma vez, com token de curta duracao
#   (padrao)   — executa o runner ja registrado
#
# O runner roda A PARTIR do volume persistente ($RUNNER_DIR), nao de /opt.
# A imagem so carrega os binarios "de fabrica" e os semeia no volume quando a
# versao muda. A primeira tentativa foi manter os binarios em /opt e ligar a
# configuracao por symlink — nao funciona: o runner .NET le .credentials logo
# na inicializacao e aborta num symlink pendente (core dump, verificado
# em 2026-08-28).
set -euo pipefail

RUNNER_DIR="${RUNNER_DIR:-/home/runner/config}"
PRISTINE=/opt/actions-runner

mkdir -p "$RUNNER_DIR"

# Semeia (ou atualiza) os binarios. `cp -a origem/. destino/` sobrescreve os
# arquivos da imagem e preserva o que so existe no volume — .runner,
# .credentials e _work sobrevivem a uma troca de versao do runner.
img_ver=$(cat "$PRISTINE/.image-version" 2>/dev/null || echo desconhecida)
cur_ver=$(cat "$RUNNER_DIR/.image-version" 2>/dev/null || echo nenhuma)
if [[ "$img_ver" != "$cur_ver" ]]; then
  echo "[runner] semeando binarios: $cur_ver -> $img_ver"
  cp -a "$PRISTINE/." "$RUNNER_DIR/"
fi

cd "$RUNNER_DIR"

if [[ "${1:-run}" == "configure" ]]; then
  : "${GITHUB_URL:?GITHUB_URL obrigatorio (ex.: https://github.com/dono/repo)}"
  : "${REGISTRATION_TOKEN:?REGISTRATION_TOKEN obrigatorio}"

  echo "[runner] registrando ${RUNNER_NAME:-daytradebot-app} em $GITHUB_URL"
  ./config.sh \
    --unattended --replace \
    --url "$GITHUB_URL" \
    --token "$REGISTRATION_TOKEN" \
    --name "${RUNNER_NAME:-daytradebot-app}" \
    --labels "${RUNNER_LABELS:-self-hosted,linux,x64,home}" \
    --work _work
  echo "[runner] registrado. A configuracao ficou em $RUNNER_DIR e sobrevive a reboot."
  exit 0
fi

if [[ ! -f "$RUNNER_DIR/.runner" ]]; then
  cat >&2 <<MSG
ERRO: runner nao registrado.

Registre uma vez, com um token de curta duracao (validade ~1h):

  TOKEN=\$(gh api -X POST repos/<dono>/<repo>/actions/runners/registration-token --jq .token)

  docker run --rm \\
    -v <APP_DATA_DIR>/runner:/home/runner/config \\
    -e GITHUB_URL=https://github.com/<dono>/<repo> \\
    -e REGISTRATION_TOKEN="\$TOKEN" \\
    ghcr.io/drumblow/trader-runner:latest configure

Depois reinicie a app. Nenhum token de longa duracao fica no dispositivo.
MSG
  exit 78  # EX_CONFIG
fi

# Acesso ao socket do Docker: falha aqui com mensagem util em vez de
# "permission denied" no meio de um deploy.
if ! docker info >/dev/null 2>&1; then
  echo "AVISO: sem acesso ao socket do Docker — o deploy vai falhar." >&2
  echo "       Confira se o gid do grupo docker do host bate com o DOCKER_GID da imagem." >&2
fi

echo "[runner] iniciando"
exec ./run.sh
