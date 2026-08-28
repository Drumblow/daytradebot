# Scheduler do app do umbrelOS (ADR-013).
#
# Substitui os tres timers systemd (trader-start / trader-stop / trader-backup),
# que viviam em /etc/systemd/system e eram apagados a cada boot pelo rugix — a
# causa raiz do incidente de 2026-08-23.
#
# Base docker:cli: traz o cliente docker, que e como o scheduler liga e desliga
# as instancias irmas atraves do socket montado.
FROM docker:28-cli

# Sem cliente do postgres de proposito: o backup roda `docker exec` no container
# do banco, entao o pg_dump usado e o da propria versao do servidor.
RUN apk add --no-cache tzdata bash

COPY scheduler.sh /usr/local/bin/scheduler.sh
RUN chmod +x /usr/local/bin/scheduler.sh

# Toda a agenda em horario de Nova York: o pregao manda, e assim o horario de
# verao americano e tratado pelo tzdata em vez de na mao (a config das
# estrategias ainda precisa de ajuste manual na virada — ver os TOMLs).
ENV TZ=America/New_York \
    TRADING_START="25 9" \
    TRADING_END="10 16" \
    BACKUP_AT="30 16" \
    BACKUP_RETENTION_DAYS=7

ENTRYPOINT ["/usr/local/bin/scheduler.sh"]
