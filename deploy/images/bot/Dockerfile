# Imagem de uma instancia do bot para o app do umbrelOS (ADR-013).
#
# DIFERENCA para deploy/home/bot/Dockerfile (o antigo): la o binario e as configs
# vinham por volume de /data/trader e o CI trocava o arquivo no host. Aqui eles
# sao ASSADOS na imagem, publicada em ghcr.io. Isso faz uma instalacao limpa a
# partir da app store funcionar sem CI, e faz a versao que roda ser rastreavel
# por digest — some o truque de trocar o inode do binario embaixo do processo.
#
# O binario e compilado pelo CI (precisa de Postgres: o sqlx valida query em
# tempo de compilacao) e chega no contexto de build. Ver .github/workflows/images.yml.
#
# Debian 13 (trixie): mesma glibc do build, o binario precisa de libc.so.6/libgcc.
FROM debian:13-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
        ca-certificates tzdata \
    && rm -rf /var/lib/apt/lists/*

RUN mkdir -p /opt/trader/bin /opt/trader/config /opt/trader/logs

WORKDIR /opt/trader

COPY trader-cli /opt/trader/bin/trader-cli
COPY config/ /opt/trader/config/
COPY entrypoint.sh /usr/local/bin/entrypoint.sh
RUN chmod +x /opt/trader/bin/trader-cli /usr/local/bin/entrypoint.sh

# Janela de pregao, em horario de Nova York. O entrypoint sai com 0 fora dela —
# ver o comentario no entrypoint sobre por que isso importa no boot.
ENV TZ=America/New_York \
    TRADING_START=09:25 \
    TRADING_END=16:10

ENTRYPOINT ["/usr/local/bin/entrypoint.sh"]
