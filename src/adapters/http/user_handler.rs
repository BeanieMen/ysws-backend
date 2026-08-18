use crate::{
    adapters::http::{AppState, helpers::current_session_user},
    domain::{CurrentUserResponse, minutes_as_hours},
    error::ApiResult,
};
use axum::{Json, extract::State, http::HeaderMap};
use std::sync::Arc;

/// Retrieves the profile for the currently authenticated user.
///
/// # Errors
///
/// Returns an error if the user session is invalid or database query fails.
pub async fn current_user_profile(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> ApiResult<Json<CurrentUserResponse>> {
    let user = current_session_user(&state, &headers).await?;
    let hackatime_connected: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM hackatime_connections WHERE user_id = $1)",
    )
    .bind(user.id)
    .fetch_one(&state.db)
    .await?;
    let minutes: i64 =
        sqlx::query_scalar("SELECT available_minutes FROM user_wallets WHERE user_id = $1")
            .bind(user.id)
            .fetch_optional(&state.db)
            .await?
            .unwrap_or(0);
    Ok(Json(CurrentUserResponse {
        id: user.id,
        email: user.email,
        first_name: user.first_name,
        last_name: user.last_name,
        role: user.role,
        hackatime_connected,
        available_hours: minutes_as_hours(minutes),
    }))
}
