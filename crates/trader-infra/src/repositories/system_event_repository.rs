//! Implementação sqlx de repositório de eventos de sistema.

use sqlx::PgPool;

use trader_domain::RepositoryError;

/// Implementação sqlx de repositório de eventos de sistema.
#[derive(Debug, Clone)]
pub struct SqlxSystemEventRepository {
    pool: PgPool,
}

impl SqlxSystemEventRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Registra um evento de sistema (start/stop, circuit breaker, limites).
    pub async fn record(
        &self,
        level: &str,
        component: &str,
        event_type: &str,
        message: &str,
        payload: Option<serde_json::Value>,
    ) -> Result<(), RepositoryError> {
        sqlx::query!(
            r#"
            INSERT INTO system_events (level, component, event_type, message, payload)
            VALUES ($1, $2, $3, $4, $5)
            "#,
            level,
            component,
            event_type,
            message,
            payload,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| RepositoryError::Query(e.to_string()))?;

        Ok(())
    }
}
