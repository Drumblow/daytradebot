# Dockerfile do bot (trader-cli) para o host umbrelOS da casa
# Base Debian 13 (trixie) — glibc compatível com o binário (requer libc.so.6, libgcc)
FROM debian:13-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
        ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Estrutura espelhando a VM Oracle (paths de config iguais)
RUN mkdir -p /opt/trader/bin /opt/trader/config /opt/trader/backups /opt/trader/logs

WORKDIR /opt/trader

# Binário e configs vêm por volume bind (/data/trader/bin e /data/trader/config)
# para o deploy não precisar rebuildar imagem (mesmo padrão da VM: troca binário + restart).

COPY entrypoint.sh /usr/local/bin/entrypoint.sh
RUN chmod +x /usr/local/bin/entrypoint.sh

ENTRYPOINT ["/usr/local/bin/entrypoint.sh"]