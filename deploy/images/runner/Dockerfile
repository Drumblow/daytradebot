# Runner self-hosted do GitHub Actions, dentro do app do umbrelOS (ADR-013).
#
# Por que containerizar um runner que ja funcionava como servico systemd:
# o servico vivia em /etc/systemd/system e o runner em /data/trader — a unit
# era apagada a cada boot pelo rugix, e o CI ficava mudo ate alguem reinstalar.
# Dentro do app ele sobe junto com o resto.
#
# Efeito colateral util: como o runner tem o socket do Docker, ele e tambem o
# unico acesso administrativo ao host que SOBREVIVE a um reboot. O usuario
# trader e o sudoers dele nao sobrevivem.
#
# A registracao NAO acontece aqui e nao usa PAT: o runner e registrado uma vez
# com um token de curta duracao, e as credenciais resultantes ficam em
# ${APP_DATA_DIR}/runner (persistente). O container so executa o que ja existe.
FROM debian:13-slim

ARG RUNNER_VERSION=2.337.0

RUN apt-get update && apt-get install -y --no-install-recommends \
        ca-certificates curl git jq tar gzip sudo \
        # dependencias do runner .NET
        libicu76 liblttng-ust1 libkrb5-3 zlib1g libssl3 \
    && rm -rf /var/lib/apt/lists/*

# Cliente do Docker (so o CLI — o daemon e o do host, via socket montado)
RUN set -eux; \
    arch=$(dpkg --print-architecture); \
    case "$arch" in amd64) dockerarch=x86_64 ;; arm64) dockerarch=aarch64 ;; \
      *) echo "arquitetura nao suportada: $arch" >&2; exit 1 ;; esac; \
    curl -fsSL "https://download.docker.com/linux/static/stable/${dockerarch}/docker-28.5.2.tgz" \
      -o /tmp/docker.tgz; \
    tar xzf /tmp/docker.tgz -C /tmp; \
    mv /tmp/docker/docker /usr/local/bin/docker; \
    rm -rf /tmp/docker /tmp/docker.tgz; \
    docker --version

# Plugin do compose — o deploy recria as instancias com a imagem nova
RUN set -eux; \
    arch=$(dpkg --print-architecture); \
    case "$arch" in amd64) ca=x86_64 ;; arm64) ca=aarch64 ;; esac; \
    mkdir -p /usr/local/lib/docker/cli-plugins; \
    curl -fsSL "https://github.com/docker/compose/releases/download/v2.40.3/docker-compose-linux-${ca}" \
      -o /usr/local/lib/docker/cli-plugins/docker-compose; \
    chmod +x /usr/local/lib/docker/cli-plugins/docker-compose; \
    docker compose version

# O runner recusa rodar como root, entao usamos um usuario dedicado. O acesso ao
# socket do Docker vem do grupo, nao de privilegio: o gid 987 e o do grupo docker
# do host (verificado com getent group docker).
ARG DOCKER_GID=987
RUN groupadd -g "${DOCKER_GID}" hostdocker \
    && useradd -m -u 1002 -s /bin/bash -G hostdocker runner

# Binarios "de fabrica". O runner NAO roda daqui: o entrypoint semeia estes
# arquivos no volume persistente e executa de la, porque o runner .NET exige
# configuracao e binarios no MESMO diretorio.
RUN set -eux; \
    arch=$(dpkg --print-architecture); \
    case "$arch" in amd64) ra=x64 ;; arm64) ra=arm64 ;; esac; \
    mkdir -p /opt/actions-runner; \
    curl -fsSL "https://github.com/actions/runner/releases/download/v${RUNNER_VERSION}/actions-runner-linux-${ra}-${RUNNER_VERSION}.tar.gz" \
      -o /tmp/runner.tar.gz; \
    tar xzf /tmp/runner.tar.gz -C /opt/actions-runner; \
    rm /tmp/runner.tar.gz; \
    echo "${RUNNER_VERSION}" > /opt/actions-runner/.image-version; \
    chown -R runner:runner /opt/actions-runner

COPY entrypoint.sh /usr/local/bin/entrypoint.sh
RUN chmod +x /usr/local/bin/entrypoint.sh

ENV RUNNER_DIR=/home/runner/config
USER runner
WORKDIR /opt/actions-runner
ENTRYPOINT ["/usr/local/bin/entrypoint.sh"]
