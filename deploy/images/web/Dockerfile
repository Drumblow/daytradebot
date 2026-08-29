# Imagem do painel web (trader-web) para o app do umbrelOS (ADR-014).
#
# Mesmo padrao da imagem do bot: o binario e compilado pelo CI e chega no
# contexto de build; a imagem so embala. O dashboard (HTML/CSS/JS) ja esta
# embutido no binario via include_str!, entao nao ha assets soltos aqui.
#
# O painel e read-only: le o Postgres com default_transaction_read_only=on
# e nao fala com o broker (so um probe TCP de liveness na porta do Gateway).
#
# Debian 13 (trixie): mesma glibc do build, o binario precisa de libc.so.6.
FROM debian:13-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
        ca-certificates tzdata \
    && rm -rf /var/lib/apt/lists/*

COPY trader-web /usr/local/bin/trader-web
RUN chmod +x /usr/local/bin/trader-web

# Horario de NY por consistencia com o resto do app (agregacoes diarias e
# janela de pregao ja sao calculadas em America/New_York pelo proprio binario).
ENV TZ=America/New_York \
    TRADER_WEB_BIND=0.0.0.0:8551

EXPOSE 8551

ENTRYPOINT ["/usr/local/bin/trader-web"]
