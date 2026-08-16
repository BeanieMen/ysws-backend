use sqlx::{PgPool, postgres::PgPoolOptions};
use std::time::Duration;

/// Connects to `PostgreSQL` database and runs pending migrations.
///
/// # Errors
///
/// Returns an error if database connection or migration fails.
pub async fn connect_and_migrate(database_url: &str) -> anyhow::Result<PgPool> {
    let pool = PgPoolOptions::new()
        .max_connections(50)
        .min_connections(10)
        .acquire_timeout(Duration::from_secs(5))
        .idle_timeout(Duration::from_mins(10))
        .max_lifetime(Duration::from_mins(30))
        .connect(database_url)
        .await?;
    sqlx::migrate!("./database/migrations").run(&pool).await?;
    Ok(pool)
}
