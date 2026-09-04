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

# Usuario sem privilegio (SECURITY.md §8.3). O painel nao monta volume, nao le
# segredo e so precisa abrir a porta 8551 — nada aqui exige root.
#
# A imagem do BOT continua como root de proposito: ela le
# ${APP_DATA_DIR}/secrets/alerts.env, que e um diretorio 0700 do UID 1001.
# Trocar o usuario do bot sem alinhar esse UID faria o webhook parar de
# funcionar EM SILENCIO — o oposto do que a auditoria pediu. Fica para quando
# o compose e o pre-install da store forem ajustados juntos.
RUN useradd --system --no-create-home --uid 10001 web
USER web

# Horario de NY por consistencia com o resto do app (agregacoes diarias e
# janela de pregao ja sao calculadas em America/New_York pelo proprio binario).
ENV TZ=America/New_York \
    TRADER_WEB_BIND=0.0.0.0:8551

EXPOSE 8551

ENTRYPOINT ["/usr/local/bin/trader-web"]
