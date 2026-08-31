//! Alertas operacionais via webhook (Slack/Discord/Teams compatível).
//!
//! Fire-and-forget: falha no envio nunca interrompe o trading — o alerta
//! permanece no log estruturado independentemente do webhook.

use tracing::{error, info};

/// Formato do corpo aceito pelo destino do webhook.
///
/// Slack usa `{"text": ...}` e o Discord usa `{"content": ...}` — mandar o
/// formato errado devolve 400 e o alerta some. O Discord também expõe um
/// endpoint compatível com Slack quando a URL termina em `/slack`; nesse
/// caso o corpo correto é o do Slack.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WebhookFlavor {
    Slack,
    Discord,
}

/// Limite de caracteres do corpo, por destino. O Discord corta em 2000 e
/// recusa o envio inteiro acima disso; a margem cobre o prefixo de emoji.
const DISCORD_MAX_CHARS: usize = 1900;
const SLACK_MAX_CHARS: usize = 3000;

impl WebhookFlavor {
    fn detect(url: &str) -> Self {
        let url = url.trim().trim_end_matches('/');
        if url.ends_with("/slack") {
            return Self::Slack;
        }
        if url.contains("discord.com/api/webhooks") || url.contains("discordapp.com/api/webhooks") {
            return Self::Discord;
        }
        Self::Slack
    }

    fn max_chars(self) -> usize {
        match self {
            Self::Slack => SLACK_MAX_CHARS,
            Self::Discord => DISCORD_MAX_CHARS,
        }
    }

    fn payload(self, text: &str) -> serde_json::Value {
        // Corta em fronteira de char: as mensagens levam emoji e acentos.
        let limit = self.max_chars();
        let body = if text.chars().count() > limit {
            text.chars().take(limit).collect::<String>()
        } else {
            text.to_string()
        };
        match self {
            Self::Slack => serde_json::json!({ "text": body }),
            Self::Discord => serde_json::json!({ "content": body }),
        }
    }
}

/// Cliente de alertas. Sem webhook configurado, apenas loga.
#[derive(Debug, Clone)]
pub struct Alerter {
    webhook_url: Option<String>,
    flavor: WebhookFlavor,
    client: reqwest::Client,
}

impl Alerter {
    pub fn new(webhook_url: &str) -> Self {
        let webhook_url = if webhook_url.trim().is_empty() {
            None
        } else {
            Some(webhook_url.trim().to_string())
        };
        let flavor = webhook_url
            .as_deref()
            .map(WebhookFlavor::detect)
            .unwrap_or(WebhookFlavor::Slack);
        Self {
            webhook_url,
            flavor,
            client: reqwest::Client::new(),
        }
    }

    /// Envia alerta informativo (webhook + log).
    pub fn info(&self, message: &str) {
        info!(alert = %message, "alerta");
        self.send(format!("ℹ️ {message}"));
    }

    /// Envia alerta crítico AGUARDANDO a entrega (timeout de 5s).
    ///
    /// Usar em caminhos que encerram o processo em seguida (ex.: circuit
    /// breaker): no fire-and-forget o runtime desliga antes da task de
    /// envio rodar e o alerta — justamente o mais importante — se perde.
    pub async fn critical_await(&self, message: &str) {
        error!(alert = %message, "ALERTA CRÍTICO");
        self.send_await(format!("🚨 {message}")).await;
    }

    /// Variante aguardada de [`Self::info`], para o encerramento do live.
    pub async fn info_await(&self, message: &str) {
        info!(alert = %message, "alerta");
        self.send_await(format!("ℹ️ {message}")).await;
    }

    fn send(&self, text: String) {
        let Some(url) = self.webhook_url.clone() else {
            return;
        };
        let client = self.client.clone();
        let payload = self.flavor.payload(&text);
        tokio::spawn(async move {
            match client.post(&url).json(&payload).send().await {
                Ok(response) => check_status(response),
                Err(e) => {
                    error!(error = %e.without_url(), "falha ao enviar alerta para webhook")
                }
            }
        });
    }

    async fn send_await(&self, text: String) {
        let Some(url) = self.webhook_url.clone() else {
            return;
        };
        let payload = self.flavor.payload(&text);
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            self.client.post(&url).json(&payload).send(),
        )
        .await;
        match result {
            Ok(Ok(response)) => check_status(response),
            Ok(Err(e)) => error!(error = %e.without_url(), "falha ao enviar alerta para webhook"),
            Err(_) => error!("timeout (5s) ao enviar alerta para webhook"),
        }
    }
}

/// Um 4xx/5xx do webhook NÃO é erro de transporte: sem esta checagem, um
/// corpo no formato errado (Discord recebendo `{"text": ...}`) devolvia 400 e
/// o alerta sumia em silêncio — que é exatamente o modo de falha que o
/// webhook existe para evitar.
fn check_status(response: reqwest::Response) {
    let status = response.status();
    if let Err(e) = response.error_for_status() {
        error!(
            status = %status,
            error = %e.without_url(),
            "webhook recusou o alerta"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DISCORD: &str = "https://discord.com/api/webhooks/123/abc";

    #[test]
    fn detecta_discord_e_slack_pela_url() {
        assert_eq!(WebhookFlavor::detect(DISCORD), WebhookFlavor::Discord);
        assert_eq!(
            WebhookFlavor::detect("https://discordapp.com/api/webhooks/123/abc"),
            WebhookFlavor::Discord
        );
        // O endpoint compatível do Discord quer o corpo do Slack.
        assert_eq!(
            WebhookFlavor::detect(&format!("{DISCORD}/slack")),
            WebhookFlavor::Slack
        );
        assert_eq!(
            WebhookFlavor::detect("https://hooks.slack.com/services/T/B/x"),
            WebhookFlavor::Slack
        );
        // Destino desconhecido cai no formato mais comum.
        assert_eq!(
            WebhookFlavor::detect("https://exemplo.invalido/hook"),
            WebhookFlavor::Slack
        );
    }

    #[test]
    fn corpo_usa_a_chave_do_destino() {
        assert_eq!(
            WebhookFlavor::Discord.payload("oi")["content"],
            serde_json::json!("oi")
        );
        assert_eq!(
            WebhookFlavor::Slack.payload("oi")["text"],
            serde_json::json!("oi")
        );
    }

    /// O corte é por char, não por byte: as mensagens levam emoji e acentos,
    /// e cortar no meio de um char geraria JSON inválido.
    #[test]
    fn corta_mensagem_longa_em_fronteira_de_char() {
        let longa: String = "🚨á".repeat(2_000);
        let corpo = WebhookFlavor::Discord.payload(&longa);
        let texto = corpo["content"].as_str().unwrap();
        assert_eq!(texto.chars().count(), DISCORD_MAX_CHARS);
        assert!(texto.starts_with("🚨á"));
    }

    #[test]
    fn alerter_sem_url_nao_tem_webhook() {
        let alerter = Alerter::new("   ");
        assert!(alerter.webhook_url.is_none());
    }

    #[test]
    fn alerter_detecta_o_formato_na_construcao() {
        assert_eq!(Alerter::new(DISCORD).flavor, WebhookFlavor::Discord);
    }
}
