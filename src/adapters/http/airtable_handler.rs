use crate::{
    adapters::http::{AppState, helpers::{current_session_user, require_admin}},
    airtable_sync::{self, AirtableSyncReport},
    error::ApiResult,
};
use axum::{Json, extract::State, http::HeaderMap};
use std::sync::Arc;

/// Runs an Airtable sync immediately. Scheduled sync remains enabled when
/// Airtable credentials are configured.
pub async fn sync_airtable(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> ApiResult<Json<AirtableSyncReport>> {
    let session_user = current_session_user(&state, &headers).await?;
    require_admin(&session_user)?;
    Ok(Json(airtable_sync::sync(&state).await?))
}
