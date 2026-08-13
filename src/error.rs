use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("{0}")]
    BadRequest(String),
    #[error("{0}")]
    Forbidden(String),
    #[error("{0}")]
    Unauthorized(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    Conflict(String),
    #[error("{0}")]
    Upstream(String),
    #[error("internal server error")]
    Internal(#[from] anyhow::Error),
}

#[derive(Serialize)]
struct ErrorBody<'a> {
    error: &'a str,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = match self {
            Self::BadRequest(_) => StatusCode::BAD_REQUEST,
            Self::Forbidden(_) => StatusCode::FORBIDDEN,
            Self::Unauthorized(_) => StatusCode::UNAUTHORIZED,
            Self::NotFound(_) => StatusCode::NOT_FOUND,
            Self::Conflict(_) => StatusCode::CONFLICT,
            Self::Upstream(_) => StatusCode::BAD_GATEWAY,
            Self::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };
        let message = self.to_string();
        (status, Json(ErrorBody { error: &message })).into_response()
    }
}

impl From<sqlx::Error> for ApiError {
    fn from(error: sqlx::Error) -> Self {
        Self::Internal(error.into())
    }
}

impl From<reqwest::Error> for ApiError {
    fn from(error: reqwest::Error) -> Self {
        Self::Upstream(error.to_string())
    }
}

impl From<serde_json::Error> for ApiError {
    fn from(error: serde_json::Error) -> Self {
        Self::Upstream(format!("invalid response from provider: {error}"))
    }
}

pub type ApiResult<T> = Result<T, ApiError>;
