use crate::{adapters::http::AppState, error::ApiResult};
use axum::{extract::State, http::StatusCode};
use std::sync::Arc;

pub async fn health(State(state): State<Arc<AppState>>) -> ApiResult<StatusCode> {
    sqlx::query("SELECT 1").execute(&state.db).await?;
    Ok(StatusCode::NO_CONTENT)
}
