#!/bin/bash
# Entrypoint do runner self-hosted dentro do app do umbrelOS (ADR-013).
#
# Dois modos:
#   configure  — registra o runner uma vez, com token de curta duracao
#   (padrao)   — executa o runner ja registrado
#
# A configuracao (.runner, .credentials) fica em $RUNNER_DIR, que e um volume
# persistente. A imagem so carrega os binarios. Isso evita guardar um PAT de
# longa duracao no dispositivo: o token de registro vale ~1 hora e e usado uma vez.
set -euo pipefail

RUNNER_DIR="${RUNNER_DIR:-/home/runner/config}"
RUNNER_HOME=/opt/actions-runner
CONFIG_FILES=(.runner .credentials .credentials_rsakey)

mkdir -p "$RUNNER_DIR/_work"

# O runner le e escreve a configuracao no proprio diretorio. Como os binarios
# estao na imagem (read-only na pratica) e a configuracao precisa persistir,
# ligamos um no outro por symlink. Escrever num symlink pendente cria o alvo,
# entao isso funciona tanto para ler quanto para registrar.
for f in "${CONFIG_FILES[@]}"; do
  ln -sfn "$RUNNER_DIR/$f" "$RUNNER_HOME/$f"
done
ln -sfn "$RUNNER_DIR/_work" "$RUNNER_HOME/_work"

if [[ "${1:-run}" == "configure" ]]; then
  : "${GITHUB_URL:?GITHUB_URL obrigatorio (ex.: https://github.com/dono/repo)}"
  : "${REGISTRATION_TOKEN:?REGISTRATION_TOKEN obrigatorio}"

  echo "[runner] registrando ${RUNNER_NAME:-daytradebot-app} em $GITHUB_URL"
  cd "$RUNNER_HOME"
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

Depois reinicie a app. Nenhum token de longa duracao e guardado no dispositivo.
MSG
  exit 78  # EX_CONFIG
fi

# Acesso ao socket do Docker: se o gid do grupo docker do host nao for o que a
# imagem assumiu, falha aqui com uma mensagem util em vez de "permission denied"
# no meio de um deploy.
if ! docker info >/dev/null 2>&1; then
  echo "AVISO: sem acesso ao socket do Docker — o deploy vai falhar." >&2
  echo "       Confira se o gid do grupo docker do host bate com o DOCKER_GID da imagem." >&2
fi

cd "$RUNNER_HOME"
echo "[runner] iniciando"
exec ./run.sh
