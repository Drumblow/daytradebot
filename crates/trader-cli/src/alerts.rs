//! Alertas operacionais via webhook (Slack/Discord/Teams compatível).
//!
//! Fire-and-forget: falha no envio nunca interrompe o trading — o alerta
//! permanece no log estruturado independentemente do webhook.

use tracing::{error, info};

/// Cliente de alertas. Sem webhook configurado, apenas loga.
#[derive(Debug, Clone)]
pub struct Alerter {
    webhook_url: Option<String>,
    client: reqwest::Client,
}

impl Alerter {
    pub fn new(webhook_url: &str) -> Self {
        let webhook_url = if webhook_url.trim().is_empty() {
            None
        } else {
            Some(webhook_url.trim().to_string())
        };
        Self {
            webhook_url,
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
        tokio::spawn(async move {
            let result = client
                .post(&url)
                .json(&serde_json::json!({ "text": text }))
                .send()
                .await;
            if let Err(e) = result {
                error!(error = %e, "falha ao enviar alerta para webhook");
            }
        });
    }

    async fn send_await(&self, text: String) {
        let Some(url) = self.webhook_url.clone() else {
            return;
        };
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            self.client
                .post(&url)
                .json(&serde_json::json!({ "text": text }))
                .send(),
        )
        .await;
        match result {
            Ok(Ok(_)) => {}
            Ok(Err(e)) => error!(error = %e, "falha ao enviar alerta para webhook"),
            Err(_) => error!("timeout (5s) ao enviar alerta para webhook"),
        }
    }
}
