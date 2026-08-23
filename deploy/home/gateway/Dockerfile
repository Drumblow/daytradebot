# Dockerfile do IB Gateway para o host umbrelOS da casa
# Replica a instalação da VM Oracle (Gateway 10.45 + IBC 3.24.1 + Xvfb, paper)
# ⚠️ IBC 3.24.1/Gateway 10.45 exige Java 17 (rejeita 21: "No java executable found").
#    Usamos Temurin 17 — a JRE que a IBKR recomenda para TWS/Gateway.
# ⚠️ O Gateway/JxBrowser (Chromium) NÃO roda como root (falha silenciosa no login
#    dialog → IBC exit 1112). Roda como usuário trader, como na VM Oracle.
FROM debian:13-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
        xvfb \
        xauth \
        x11-utils \
        procps \
        ca-certificates \
        wget \
        tar \
        # libs do JxBrowser (Chromium embutido) e do Swing/GTK que o Gateway usa
        libnss3 \
        libnspr4 \
        libatk1.0-0 \
        libatk-bridge2.0-0 \
        libcups2 \
        libdrm2 \
        libxkbcommon0 \
        libxcomposite1 \
        libxdamage1 \
        libxfixes3 \
        libxrandr2 \
        libxinerama1 \
        libgbm1 \
        libasound2 \
        libpango-1.0-0 \
        libcairo2 \
        libglib2.0-0 \
        libgtk-3-0 \
        libx11-6 \
        libxext6 \
        libxi6 \
        libxtst6 \
        libxss1 \
        libexpat1 \
        libcups2 \
        fonts-liberation \
        fontconfig \
    && rm -rf /var/lib/apt/lists/*

# JRE 17 **com JavaFX** (ZuluFX — mesma família que a IBKR embute no instalador;
# o Gateway 10.45 exige javafx/embed/swing/JFXPanel no login. Temurin puro → NoClassDefFoundError).
# O IBC testa ${JAVA_PATH}/java (JAVA_PATH = diretório que CONTÉM o binário java → /opt/java17/bin)
RUN set -eux; \
    wget -qO /tmp/jre17.tar.gz \
      "https://cdn.azul.com/zulu/bin/zulu17.68.17-ca-fx-jre17.0.20-linux_x64.tar.gz"; \
    mkdir -p /opt/java17 && tar xzf /tmp/jre17.tar.gz -C /opt/java17 --strip-components=1; \
    rm /tmp/jre17.tar.gz; \
    /opt/java17/bin/java -version && \
    /opt/java17/bin/java --list-modules 2>/dev/null | grep -c javafx

# Usuário trader (uid 1001 = mesmo do host umbrelOS, para casar com os volumes
# /data/trader montados; umbrel é 1000 no host)
RUN useradd -m -u 1001 -s /bin/bash trader

# Estrutura idêntica à da VM Oracle
RUN mkdir -p /opt/trader/{ibc,gateway-settings,logs/ibc,backups}

# IBC + scripts + credenciais (copiados do kit de migração)
COPY ibc/ /opt/trader/ibc/
# Instalação do Gateway 10.45 (extraída do gateway-full.tgz)
COPY ibgateway/ /opt/trader/ibgateway/
# Settings (jts.ini) — TrustedIPs=127.0.0.1, UseSSL=true, ApiOnly=true
COPY gateway-settings/ /opt/trader/gateway-settings/
COPY jts.ini /opt/trader/jts.ini

# Entry controla o Xvfb explicitamente (xvfb-run como PID 1 travava)
COPY gateway-entry.sh /usr/local/bin/gateway-entry.sh
RUN chmod +x /usr/local/bin/gateway-entry.sh

# Tudo do trader (arquivos copiados como root)
RUN chown -R trader:trader /opt/trader /opt/java17

WORKDIR /opt/trader
ENV IBC_INI=/opt/trader/ibc/ibc.ini \
    TRADING_MODE=paper \
    TWOFA_TIMEOUT_ACTION=exit \
    IBC_PATH=/opt/trader/ibc \
    TWS_PATH=/opt/trader \
    TWS_SETTINGS_PATH= \
    LOG_PATH=/opt/trader/logs/ibc \
    JAVA_PATH=/opt/java17/bin

# A API do Gateway escuta em 127.0.0.1:4002 dentro do container.
# Com network_mode: host no compose, vira 127.0.0.1:4002 no host.
EXPOSE 4002

USER trader

CMD ["/usr/local/bin/gateway-entry.sh"]