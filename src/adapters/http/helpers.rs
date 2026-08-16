use crate::{
    adapters::http::AppState,
    domain::{HackClubIdentity, HackatimeProjectsPayload, SessionUser, UserRole},
    error::{ApiError, ApiResult},
    ports::CryptoPort,
};
use axum::http::HeaderMap;
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Row};
use std::time::Duration;
use uuid::Uuid;

pub async fn current_session_user(state: &AppState, headers: &HeaderMap) -> ApiResult<SessionUser> {
    let token = session_token(headers)
        .ok_or_else(|| ApiError::Unauthorized("sign in with Hack Club first".into()))?;
    let user = sqlx::query_as::<_, SessionUser>(
        "SELECT u.id, u.email, u.first_name, u.last_name, u.role FROM sessions s JOIN users u ON s.user_id = u.id WHERE s.token_hash = $1 AND s.expires_at > now()",
    )
    .bind(token_hash(token))
    .fetch_optional(&state.db)
    .await?;
    user.ok_or_else(|| ApiError::Unauthorized("your session has expired; sign in again".into()))
}

pub async fn current_user(state: &AppState, headers: &HeaderMap) -> ApiResult<Uuid> {
    let user = current_session_user(state, headers).await?;
    Ok(user.id)
}

pub async fn own_project(db: &PgPool, project_id: Uuid, user_id: Uuid) -> ApiResult<()> {
    let owner: Option<Uuid> = sqlx::query_scalar("SELECT owner_id FROM projects WHERE id = $1")
        .bind(project_id)
        .fetch_optional(db)
        .await?;
    match owner {
        Some(id) if id == user_id => Ok(()),
        Some(_) => Err(ApiError::Forbidden("you do not own this project".into())),
        None => Err(ApiError::NotFound("project not found".into())),
    }
}

pub async fn own_project_or_admin(db: &PgPool, project_id: Uuid, session_user: &SessionUser) -> ApiResult<()> {
    if session_user.role == UserRole::Admin {
        let exists: Option<Uuid> = sqlx::query_scalar("SELECT id FROM projects WHERE id = $1")
            .bind(project_id)
            .fetch_optional(db)
            .await?;
        return match exists {
            Some(_) => Ok(()),
            None => Err(ApiError::NotFound("project not found".into())),
        };
    }
    own_project(db, project_id, session_user.id).await
}

pub fn require_admin(user: &SessionUser) -> ApiResult<()> {
    if user.role == UserRole::Admin {
        Ok(())
    } else {
        Err(ApiError::Forbidden("admin privileges required".into()))
    }
}

pub fn require_reviewer_or_admin(user: &SessionUser) -> ApiResult<()> {
    if user.role == UserRole::Reviewer || user.role == UserRole::Admin {
        Ok(())
    } else {
        Err(ApiError::Forbidden("reviewer or admin privileges required".into()))
    }
}

pub async fn ensure_user(db: &PgPool, user_id: Uuid) -> ApiResult<()> {
    let found: Option<Uuid> = sqlx::query_scalar("SELECT id FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_optional(db)
        .await?;
    found
        .map(|_| ())
        .ok_or_else(|| ApiError::NotFound("user not found".into()))
}

pub async fn linked_connection(
    db: &PgPool,
    cipher: &CryptoPort,
    project_id: Uuid,
    user_id: Uuid,
) -> ApiResult<(Vec<String>, String, String)> {
    let names = sqlx::query_scalar("SELECT hackatime_project_name FROM project_hackatime_projects WHERE project_id = $1 ORDER BY hackatime_project_name")
        .bind(project_id)
        .fetch_all(db)
        .await?;
    let row = sqlx::query(
        "SELECT account_id, access_token_ciphertext FROM hackatime_connections WHERE user_id = $1",
    )
    .bind(user_id)
    .fetch_optional(db)
    .await?
    .ok_or_else(|| ApiError::BadRequest("connect Hackatime before fetching project data".into()))?;
    let token: String = row.get("access_token_ciphertext");
    let token = cipher.decrypt(&token).map_err(ApiError::Internal)?;
    Ok((names, token, row.get("account_id")))
}

pub async fn user_hackatime_projects(
    state: &AppState,
    user_id: Uuid,
) -> ApiResult<HackatimeProjectsPayload> {
    let cache_key = format!("user:{user_id}:hackatime-projects:v2");
    if let Some(cached) = state.cache.get_json(&cache_key).await {
        return Ok(cached);
    }
    let row =
        sqlx::query("SELECT access_token_ciphertext FROM hackatime_connections WHERE user_id = $1")
            .bind(user_id)
            .fetch_optional(&state.db)
            .await?
            .ok_or_else(|| ApiError::BadRequest("connect Hackatime to track time".into()))?;
    let encrypted_token: String = row.get("access_token_ciphertext");
    let token = state
        .cipher
        .decrypt(&encrypted_token)
        .map_err(ApiError::Internal)?;
    let projects = state.providers.hackatime_projects(&token).await?;
    state
        .cache
        .set_json(&cache_key, &projects, Duration::from_mins(5))
        .await;
    Ok(projects)
}

pub async fn upsert_hackclub_user(db: &PgPool, identity: HackClubIdentity) -> ApiResult<Uuid> {
    let email = identity
        .primary_email
        .ok_or_else(|| ApiError::BadRequest("Hack Club did not grant access to your email".into()))?
        .trim()
        .to_lowercase();
    validate_email(&email)?;
    let first_name = identity.first_name.unwrap_or_else(|| "Hacker".into());
    let last_name = identity.last_name.unwrap_or_default();
    let existing_id: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM users WHERE hca_id = $1 OR email = $2 ORDER BY (hca_id = $1) DESC LIMIT 1",
    )
    .bind(&identity.id)
    .bind(&email)
    .fetch_optional(db)
    .await?;
    if let Some(id) = existing_id {
        sqlx::query("UPDATE users SET hca_id = $1, email = $2, first_name = $3, last_name = $4, updated_at = now() WHERE id = $5")
            .bind(identity.id).bind(email).bind(first_name.trim()).bind(last_name.trim()).bind(id).execute(db).await?;
        return Ok(id);
    }
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO users (id, hca_id, email, first_name, last_name, role) VALUES ($1, $2, $3, $4, $5, 'user')",
    )
    .bind(id)
    .bind(identity.id)
    .bind(email)
    .bind(first_name.trim())
    .bind(last_name.trim())
    .execute(db)
    .await?;
    Ok(id)
}

pub async fn create_session(db: &PgPool, user_id: Uuid) -> ApiResult<String> {
    let token = Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO sessions (id, user_id, token_hash, expires_at) VALUES ($1, $2, $3, now() + interval '21 days')")
        .bind(Uuid::new_v4()).bind(user_id).bind(token_hash(&token)).execute(db).await?;
    Ok(token)
}

pub fn session_token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(axum::http::header::COOKIE)
        .and_then(|value| value.to_str().ok())?
        .split(';')
        .map(str::trim)
        .find_map(|cookie| cookie.strip_prefix("session="))
}

#[must_use]
pub fn token_hash(token: &str) -> String {
    hex::encode(Sha256::digest(token.as_bytes()))
}

pub fn normalized_names(names: Vec<String>) -> ApiResult<Vec<String>> {
    if names.len() > 50 {
        return Err(ApiError::BadRequest(
            "at most 50 Hackatime projects may be linked".into(),
        ));
    }
    let mut names: Vec<_> = names
        .into_iter()
        .map(|n| n.trim().to_owned())
        .filter(|n| !n.is_empty() && !placeholder_name(n))
        .collect();
    if names.iter().any(|name| name.len() > 255) {
        return Err(ApiError::BadRequest(
            "Hackatime project name must be at most 255 characters".into(),
        ));
    }
    names.sort();
    names.dedup();
    Ok(names)
}

#[must_use]
pub fn placeholder_name(name: &str) -> bool {
    name.trim().starts_with("<<") && name.trim().ends_with(">>")
}

pub fn validate_email(value: &str) -> ApiResult<()> {
    if value.len() > 254 || !value.contains('@') || value.starts_with('@') || value.ends_with('@') {
        return Err(ApiError::BadRequest("enter a valid email address".into()));
    }
    Ok(())
}

/// Validates length of string field.
///
/// # Errors
///
/// Returns an error if the length is 0 or exceeds `max`.
pub fn validate_len(value: &str, field: &str, max: usize) -> ApiResult<()> {
    let length = value.trim().chars().count();
    if length == 0 || length > max {
        return Err(ApiError::BadRequest(format!(
            "{field} must be between 1 and {max} characters"
        )));
    }
    Ok(())
}

#[must_use]
pub fn idempotency_cache_key(user_id: Uuid, event_id: Uuid, key: &str) -> String {
    format!(
        "idempotency:attendance:{}",
        hex::encode(Sha256::digest(
            format!("{user_id}:{event_id}:{key}").as_bytes()
        ))
    )
}
