//! Conexão PostgreSQL via sqlx e utilidades de pool.

use sqlx::{migrate::MigrateDatabase, PgPool, Postgres};

use trader_domain::RepositoryError;

/// Cria o banco de dados se não existir.
pub async fn create_database(database_url: &str) -> Result<(), RepositoryError> {
    if !Postgres::database_exists(database_url)
        .await
        .map_err(|e| RepositoryError::Connection(e.to_string()))?
    {
        Postgres::create_database(database_url)
            .await
            .map_err(|e| RepositoryError::Connection(e.to_string()))?;
    }
    Ok(())
}

/// Cria um pool de conexões PostgreSQL.
/// Conexões por processo.
///
/// O padrão do sqlx é 10. Com 11 instâncias na mesma máquina isso dá 110
/// conexões contra o `max_connections = 100` do Postgres — e o processo que
/// chega por último falha ao conectar, em silêncio, no meio do pregão. Cada
/// instância faz consultas curtas e sequenciais; 3 é folgado.
const MAX_CONNECTIONS_POR_INSTANCIA: u32 = 3;

/// Teto de tempo de uma query. Sem isso, uma consulta travada segura a conexão
/// para sempre e o pool seca sem nenhum erro aparecer.
const STATEMENT_TIMEOUT_MS: &str = "30000";

pub async fn create_pool(database_url: &str) -> Result<PgPool, RepositoryError> {
    sqlx::postgres::PgPoolOptions::new()
        .max_connections(MAX_CONNECTIONS_POR_INSTANCIA)
        .acquire_timeout(std::time::Duration::from_secs(10))
        .after_connect(|conn, _meta| {
            Box::pin(async move {
                sqlx::query(&format!("SET statement_timeout = {STATEMENT_TIMEOUT_MS}"))
                    .execute(conn)
                    .await?;
                Ok(())
            })
        })
        .connect(database_url)
        .await
        .map_err(|e| RepositoryError::Connection(e.to_string()))
}

/// Roda as migrations sqlx embutidas no diretório `migrations`.
pub async fn run_migrations(pool: &PgPool) -> Result<(), RepositoryError> {
    sqlx::migrate!("src/db/migrations")
        .run(pool)
        .await
        .map_err(|e| RepositoryError::Connection(e.to_string()))
}

#[cfg(test)]
mod tests {
    // Testes de integração usam sqlx::test no crate trader-infra (ver tests/).
}
