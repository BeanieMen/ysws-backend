use std::time::Duration;
use sqlx::{PgPool, postgres::PgPoolOptions};

pub async fn connect_and_migrate(database_url: &str) -> anyhow::Result<PgPool> {
    let pool = PgPoolOptions::new()
        .max_connections(50)
        .min_connections(10)
        .acquire_timeout(Duration::from_secs(5))
        .idle_timeout(Duration::from_secs(600))
        .max_lifetime(Duration::from_secs(1800))
        .connect(database_url)
        .await?;
    sqlx::migrate!("./database/migrations").run(&pool).await?;
    Ok(pool)
}
