use sqlx::postgres::{PgPool, PgPoolOptions};
use anyhow::Result;

pub async fn create_pool(database_url: &str) -> Result<PgPool> {
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(database_url)
        .await?;
    Ok(pool)
}

pub async fn run_migrations(pool: &PgPool) -> Result<()> {
    let migrations_path = std::env::var("MIGRATIONS_DIR")
        .unwrap_or_else(|_| "./migrations".to_string());

    let migrator = sqlx::migrate::Migrator::new(std::path::Path::new(&migrations_path)).await?;
    migrator.run(pool).await?;
    Ok(())
}