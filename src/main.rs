pub mod adapters;
pub mod airtable_sync;
pub mod cache;
pub mod config;
pub mod crypto;
pub mod database;
pub mod domain;
pub mod error;
pub mod ports;
pub mod providers;
pub mod test;

use crate::{
    adapters::http::{AppState, router},
    cache::Cache,
    config::Config,
    crypto::TokenCipher,
    providers::Providers,
};
use axum::extract::DefaultBodyLimit;
use std::time::Duration;
use tower_http::{
    cors::CorsLayer,
    request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer},
    timeout::TimeoutLayer,
    trace::TraceLayer,
};
use tracing::info;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("test_instance=info".parse()?),
        )
        .init();
    let config = Config::from_env()?;
    let db = database::connect_and_migrate(&config.database_url).await?;
    let cache = Cache::connect(&config.redis_url).await?;
    let providers = Providers::new(config.clone())?;

    tokio::fs::create_dir_all("uploads/banners").await.ok();

    let airtable_sync_interval = config.airtable_sync_interval;
    let app_state = AppState {
        db,
        cache,
        cipher: TokenCipher::new(config.encryption_key),
        providers,
        cookie_secure: config.cookie_secure,
    };
    if app_state.providers.airtable_configured() {
        tokio::spawn(airtable_sync::run_scheduler(app_state.clone(), airtable_sync_interval));
    }
    let app = router(app_state)
    .layer(DefaultBodyLimit::max(15 * 1024 * 1024))
    .layer(TimeoutLayer::with_status_code(
        axum::http::StatusCode::REQUEST_TIMEOUT,
        Duration::from_secs(30),
    ))
    .layer(PropagateRequestIdLayer::new(
        axum::http::HeaderName::from_static("x-request-id"),
    ))
    .layer(SetRequestIdLayer::new(
        axum::http::HeaderName::from_static("x-request-id"),
        MakeRequestUuid,
    ))
    .layer(TraceLayer::new_for_http())
    .layer(CorsLayer::very_permissive());
    let listener = tokio::net::TcpListener::bind(("0.0.0.0", config.port)).await?;
    info!(port = config.port, "test-instance API listening");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

async fn shutdown_signal() {
    if let Err(err) = tokio::signal::ctrl_c().await {
        eprintln!("failed to install Ctrl+C handler: {err}");
    }
}
