use std::time::Duration;

use sqlx::{PgPool, postgres::PgPoolOptions};

/// Creates the sole application database pool and applies the SQL migrations
/// before serving traffic. PostgreSQL holds the migration ledger in
/// `_sqlx_migrations`; this makes restarts safe and gives PostgREST the fully
/// migrated schema to introspect.
pub async fn connect_and_migrate(database_url: &str) -> anyhow::Result<PgPool> {
    let pool = PgPoolOptions::new()
        .max_connections(20)
        .acquire_timeout(Duration::from_secs(5))
        .connect(database_url)
        .await?;
    sqlx::migrate!("./database/migrations").run(&pool).await?;
    Ok(pool)
}
