use crate::{
    adapters::http::{AppState, helpers::{current_session_user, require_reviewer_or_admin, own_project_or_admin}},
    domain::{CreateProjectReviewRequest, Project, ProjectReview},
    error::{ApiError, ApiResult},
};
use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
};
use std::sync::Arc;
use uuid::Uuid;

/// Lists projects for review.
///
/// # Errors
///
/// Returns an error if authorization fails or database query fails.
pub async fn list_projects_for_review(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> ApiResult<Json<Vec<Project>>> {
    let session_user = current_session_user(&state, &headers).await?;
    require_reviewer_or_admin(&session_user)?;
    let projects = sqlx::query_as::<_, Project>(
        "SELECT id, owner_id, title, description, created_at, updated_at FROM projects ORDER BY created_at DESC",
    )
    .fetch_all(&state.db)
    .await?;
    Ok(Json(projects))
}

/// Creates or updates a project review.
///
/// # Errors
///
/// Returns an error if input validation fails, authorization fails, or database query fails.
pub async fn create_project_review(
    State(state): State<Arc<AppState>>,
    Path(project_id): Path<Uuid>,
    headers: HeaderMap,
    Json(body): Json<CreateProjectReviewRequest>,
) -> ApiResult<(StatusCode, Json<ProjectReview>)> {
    let session_user = current_session_user(&state, &headers).await?;
    require_reviewer_or_admin(&session_user)?;

    let valid_statuses = ["pending", "approved", "rejected", "changes_requested"];
    if !valid_statuses.contains(&body.status.as_str()) {
        return Err(ApiError::BadRequest(
            "invalid review status; must be pending, approved, rejected, or changes_requested".into(),
        ));
    }

    let project_exists: Option<Uuid> = sqlx::query_scalar("SELECT id FROM projects WHERE id = $1")
        .bind(project_id)
        .fetch_optional(&state.db)
        .await?;
    if project_exists.is_none() {
        return Err(ApiError::NotFound("project not found".into()));
    }

    let review_id = Uuid::new_v4();
    let review = sqlx::query_as::<_, ProjectReview>(
        "INSERT INTO project_reviews (id, project_id, reviewer_id, status, comment) VALUES ($1, $2, $3, $4, $5) ON CONFLICT (project_id, reviewer_id) DO UPDATE SET status = EXCLUDED.status, comment = EXCLUDED.comment, updated_at = now() RETURNING id, project_id, reviewer_id, status, comment, created_at, updated_at",
    )
    .bind(review_id)
    .bind(project_id)
    .bind(session_user.id)
    .bind(&body.status)
    .bind(body.comment.map(|c| c.trim().to_owned()).filter(|c| !c.is_empty()))
    .fetch_one(&state.db)
    .await?;

    Ok((StatusCode::CREATED, Json(review)))
}

/// Retrieves reviews for a project.
///
/// # Errors
///
/// Returns an error if authorization fails or database query fails.
pub async fn get_project_reviews(
    State(state): State<Arc<AppState>>,
    Path(project_id): Path<Uuid>,
    headers: HeaderMap,
) -> ApiResult<Json<Vec<ProjectReview>>> {
    let session_user = current_session_user(&state, &headers).await?;
    own_project_or_admin(&state.db, project_id, &session_user).await
        .or_else(|_| require_reviewer_or_admin(&session_user))?;

    let reviews = sqlx::query_as::<_, ProjectReview>(
        "SELECT id, project_id, reviewer_id, status, comment, created_at, updated_at FROM project_reviews WHERE project_id = $1 ORDER BY updated_at DESC",
    )
    .bind(project_id)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(reviews))
}
