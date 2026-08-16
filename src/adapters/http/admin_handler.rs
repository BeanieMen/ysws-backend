use crate::{
    adapters::http::{
        AppState,
        helpers::{current_session_user, ensure_user, require_admin, validate_email},
    },
    domain::{AdminUpdateUserRequest, AdminUserResponse, UpdateUserRoleRequest},
    error::{ApiError, ApiResult},
};
use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
};
use std::sync::Arc;
use uuid::Uuid;

pub async fn list_users(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> ApiResult<Json<Vec<AdminUserResponse>>> {
    let session_user = current_session_user(&state, &headers).await?;
    require_admin(&session_user)?;
    let users = sqlx::query_as::<_, AdminUserResponse>(
        "SELECT id, email, first_name, last_name, role, hca_id FROM users ORDER BY created_at DESC",
    )
    .fetch_all(&state.db)
    .await?;
    Ok(Json(users))
}

pub async fn update_user_role(
    State(state): State<Arc<AppState>>,
    Path(user_id): Path<Uuid>,
    headers: HeaderMap,
    Json(body): Json<UpdateUserRoleRequest>,
) -> ApiResult<StatusCode> {
    let session_user = current_session_user(&state, &headers).await?;
    require_admin(&session_user)?;
    ensure_user(&state.db, user_id).await?;
    sqlx::query("UPDATE users SET role = $1, updated_at = now() WHERE id = $2")
        .bind(body.role)
        .bind(user_id)
        .execute(&state.db)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn admin_update_user(
    State(state): State<Arc<AppState>>,
    Path(user_id): Path<Uuid>,
    headers: HeaderMap,
    Json(body): Json<AdminUpdateUserRequest>,
) -> ApiResult<Json<AdminUserResponse>> {
    let session_user = current_session_user(&state, &headers).await?;
    require_admin(&session_user)?;
    ensure_user(&state.db, user_id).await?;

    if let Some(email) = &body.email {
        validate_email(email)?;
    }

    let user = sqlx::query_as::<_, AdminUserResponse>(
        "UPDATE users SET email = COALESCE($1, email), first_name = COALESCE($2, first_name), last_name = COALESCE($3, last_name), role = COALESCE($4, role), updated_at = now() WHERE id = $5 RETURNING id, email, first_name, last_name, role, hca_id",
    )
    .bind(body.email.map(|e| e.trim().to_lowercase()))
    .bind(body.first_name.map(|f| f.trim().to_string()))
    .bind(body.last_name.map(|l| l.trim().to_string()))
    .bind(body.role)
    .bind(user_id)
    .fetch_one(&state.db)
    .await?;

    Ok(Json(user))
}

pub async fn admin_delete_user(
    State(state): State<Arc<AppState>>,
    Path(user_id): Path<Uuid>,
    headers: HeaderMap,
) -> ApiResult<StatusCode> {
    let session_user = current_session_user(&state, &headers).await?;
    require_admin(&session_user)?;
    ensure_user(&state.db, user_id).await?;
    sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(user_id)
        .execute(&state.db)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn admin_delete_project(
    State(state): State<Arc<AppState>>,
    Path(project_id): Path<Uuid>,
    headers: HeaderMap,
) -> ApiResult<StatusCode> {
    let session_user = current_session_user(&state, &headers).await?;
    require_admin(&session_user)?;
    let result = sqlx::query("DELETE FROM projects WHERE id = $1")
        .bind(project_id)
        .execute(&state.db)
        .await?;
    if result.rows_affected() == 0 {
        return Err(ApiError::NotFound("project not found".into()));
    }
    Ok(StatusCode::NO_CONTENT)
}
