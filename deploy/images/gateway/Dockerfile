# Imagem do IB Gateway para o app do umbrelOS (ADR-013).
#
# DIFERENCA CRITICA para deploy/home/gateway/Dockerfile (o antigo):
# esta imagem e PUBLICADA em registry publico, entao NAO pode conter:
#   - credenciais IBKR (o ibc.ini aqui vai com IbLoginId/IbPassword VAZIOS;
#     o IBC le as credenciais de TWSUSERID/TWSPASSWORD em runtime)
#   - o instalador do IB Gateway (binario da IBKR, nao redistribuivel) — ele e
#     montado por volume a partir de ${APP_DATA_DIR}/gateway/ibgateway
#
# O que fica na imagem: SO software livre e configuracao sem segredo.
#
# Armadilhas ja pagas na VM Oracle e mantidas aqui:
#   - IBC 3.24.1 / Gateway 10.45 exige Java 17 (rejeita 21: "No java executable found")
#   - precisa ser ZuluFX (com JavaFX): o Gateway usa javafx/embed/swing/JFXPanel
#     no login; Temurin puro da NoClassDefFoundError
#   - o Gateway/JxBrowser NAO roda como root (falha silenciosa no login → IBC exit 1112)
FROM debian:13-slim

ARG IBC_VERSION=3.24.1
ARG ZULU_TARBALL=zulu17.68.17-ca-fx-jre17.0.20-linux_x64.tar.gz

RUN apt-get update && apt-get install -y --no-install-recommends \
        xvfb xauth x11-utils procps ca-certificates wget tar unzip tzdata \
        # libs do JxBrowser (Chromium embutido) e do Swing/GTK que o Gateway usa
        libnss3 libnspr4 libatk1.0-0 libatk-bridge2.0-0 libcups2 libdrm2 \
        libxkbcommon0 libxcomposite1 libxdamage1 libxfixes3 libxrandr2 \
        libxinerama1 libgbm1 libasound2 libpango-1.0-0 libcairo2 libglib2.0-0 \
        libgtk-3-0 libx11-6 libxext6 libxi6 libxtst6 libxss1 libexpat1 \
        fonts-liberation fontconfig \
    && rm -rf /var/lib/apt/lists/*

# JRE 17 com JavaFX. O IBC testa ${JAVA_PATH}/java, entao JAVA_PATH e o diretorio
# que CONTEM o binario (/opt/java17/bin), nao o JAVA_HOME.
RUN set -eux; \
    wget -qO /tmp/jre17.tar.gz "https://cdn.azul.com/zulu/bin/${ZULU_TARBALL}"; \
    mkdir -p /opt/java17 && tar xzf /tmp/jre17.tar.gz -C /opt/java17 --strip-components=1; \
    rm /tmp/jre17.tar.gz; \
    /opt/java17/bin/java -version; \
    /opt/java17/bin/java --list-modules 2>/dev/null | grep -q javafx

# IBC — software livre, baixado da release oficial (nao vendorizado no repo)
RUN set -eux; \
    wget -qO /tmp/ibc.zip \
      "https://github.com/IbcAlpha/IBC/releases/download/${IBC_VERSION}/IBCLinux-${IBC_VERSION}.zip"; \
    mkdir -p /opt/trader/ibc; \
    unzip -q /tmp/ibc.zip -d /opt/trader/ibc; \
    rm /tmp/ibc.zip; \
    chmod +x /opt/trader/ibc/*.sh /opt/trader/ibc/scripts/*.sh; \
    test -f /opt/trader/ibc/IBC.jar

# Pontos de montagem. Ficam vazios na imagem — o conteudo vem do app-data:
#   ibgateway/       instalacao do Gateway 10.45 (ro)   <- binario da IBKR
#   gateway-settings/ jts.ini e settings do Gateway (rw)
#   logs/ibc/        logs do IBC (rw)
RUN mkdir -p /opt/trader/ibgateway /opt/trader/gateway-settings /opt/trader/logs/ibc

# ibc.ini SEM credencial. IbLoginId/IbPassword ficam vazios de proposito:
# o IBC usa TWSUSERID/TWSPASSWORD do ambiente quando eles estao em branco.
COPY ibc.ini /opt/trader/ibc/ibc.ini
COPY entrypoint.sh /usr/local/bin/entrypoint.sh
RUN chmod +x /usr/local/bin/entrypoint.sh

# uid 1001 = mesmo do usuario trader no host, para casar com os volumes montados
RUN useradd -m -u 1001 -s /bin/bash trader \
    && chown -R trader:trader /opt/trader /opt/java17

WORKDIR /opt/trader
ENV IBC_INI=/opt/trader/ibc/ibc.ini \
    IBC_PATH=/opt/trader/ibc \
    TWS_PATH=/opt/trader \
    TWS_MAJOR_VRSN=1045 \
    TWS_SETTINGS_PATH= \
    LOG_PATH=/opt/trader/logs/ibc \
    JAVA_PATH=/opt/java17/bin \
    TRADING_MODE=paper \
    TWOFA_TIMEOUT_ACTION=exit \
    DISPLAY=:97

EXPOSE 4002

# Healthcheck: a porta da API do Gateway tem de aceitar conexao. Pega o caso
# comum de "Gateway caiu / nao subiu" e deixa o estado visivel no docker ps
# em vez de so no log. NAO pega o gateway wedged (socket aberto, sem
# responder) — contra esse cenario a defesa e o timeout na borda do adapter
# (A5 da auditoria de 30/08/2026), nao o healthcheck.
# start-period generoso: o login com 2FA do Gateway leva minutos.
HEALTHCHECK --interval=30s --timeout=5s --start-period=5m --retries=3     CMD bash -c 'exec 3<>/dev/tcp/127.0.0.1/4002' || exit 1

USER trader
ENTRYPOINT ["/usr/local/bin/entrypoint.sh"]
