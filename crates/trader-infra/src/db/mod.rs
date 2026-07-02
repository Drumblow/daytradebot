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
pub async fn create_pool(database_url: &str) -> Result<PgPool, RepositoryError> {
    PgPool::connect(database_url)
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
