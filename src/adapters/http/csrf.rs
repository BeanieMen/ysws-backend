use crate::{adapters::http::AppState, error::ApiError};
use axum::{
    extract::{Request, State},
    http::{Method, header::ORIGIN},
    middleware::Next,
    response::{IntoResponse, Response},
};
use std::sync::Arc;

/// Rejects cross-site unsafe requests that could otherwise carry the session cookie.
pub async fn require_same_origin(
    State(state): State<Arc<AppState>>,
    request: Request,
    next: Next,
) -> Response {
    if !matches!(
        request.method(),
        &Method::POST | &Method::PUT | &Method::PATCH | &Method::DELETE
    ) {
        return next.run(request).await;
    }

    let origin = request
        .headers()
        .get(ORIGIN)
        .and_then(|value| value.to_str().ok());
    if origin == Some(state.app_url.as_str()) {
        return next.run(request).await;
    }

    ApiError::Forbidden("unsafe requests must originate from the application".into())
        .into_response()
}
