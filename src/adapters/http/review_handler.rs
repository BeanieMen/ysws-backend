use crate::{
    adapters::http::{
        AppState,
        helpers::{current_session_user, own_project_or_admin, require_reviewer_or_admin},
    },
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
    let projects = if session_user.role == crate::domain::UserRole::Admin {
        sqlx::query_as::<_, Project>(
            "SELECT p.id, p.owner_id, p.title, p.description, p.banner_url, p.submission_status, p.submitted_at, p.shipped_at, COALESCE(s.project_approval_status, 'pending') AS project_approval_status, COALESCE(s.fraud_approval_status, 'pending') AS fraud_approval_status, p.created_at, p.updated_at FROM projects p LEFT JOIN project_shipments s ON s.project_id = p.id ORDER BY p.shipped_at DESC NULLS LAST, p.created_at DESC",
        )
        .fetch_all(&state.db)
        .await?
    } else {
        sqlx::query_as::<_, Project>(
            "SELECT p.id, p.owner_id, p.title, p.description, p.banner_url, p.submission_status, p.submitted_at, p.shipped_at, COALESCE(s.project_approval_status, 'pending') AS project_approval_status, COALESCE(s.fraud_approval_status, 'pending') AS fraud_approval_status, p.created_at, p.updated_at FROM projects p LEFT JOIN project_shipments s ON s.project_id = p.id WHERE p.shipped_at IS NOT NULL ORDER BY p.shipped_at DESC",
        )
        .fetch_all(&state.db)
        .await?
    };
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
            "invalid review status; must be pending, approved, rejected, or changes_requested"
                .into(),
        ));
    }

    let shipped_at: Option<Option<chrono::DateTime<chrono::Utc>>> =
        sqlx::query_scalar("SELECT shipped_at FROM projects WHERE id = $1")
            .bind(project_id)
            .fetch_optional(&state.db)
            .await?;
    let Some(shipped_at) = shipped_at else {
        return Err(ApiError::NotFound("project not found".into()));
    };
    if session_user.role != crate::domain::UserRole::Admin && shipped_at.is_none() {
        return Err(ApiError::Forbidden(
            "reviewers may only review shipped projects".into(),
        ));
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

    sqlx::query(
        "UPDATE project_shipments SET project_approval_status = $1, project_reviewed_at = now(), project_reviewer_id = $2, updated_at = now() WHERE project_id = $3",
    )
    .bind(&review.status)
    .bind(session_user.id)
    .bind(project_id)
    .execute(&state.db)
    .await?;

    // Either fraud approval or reviewer approval may arrive first. The helper
    // re-checks both states under a row lock and records an idempotent credit.
    if review.status == "approved" {
        crate::approved_hours::award_if_fully_approved(&state, project_id).await?;
    }

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
    if own_project_or_admin(&state.db, project_id, &session_user)
        .await
        .is_err()
    {
        require_reviewer_or_admin(&session_user)?;
        let shipped_at: Option<Option<chrono::DateTime<chrono::Utc>>> =
            sqlx::query_scalar("SELECT shipped_at FROM projects WHERE id = $1")
                .bind(project_id)
                .fetch_optional(&state.db)
                .await?;
        match shipped_at {
            Some(Some(_)) => {}
            Some(None) => {
                return Err(ApiError::Forbidden(
                    "reviewers may only view reviews for shipped projects".into(),
                ));
            }
            None => return Err(ApiError::NotFound("project not found".into())),
        }
    }

    let reviews = sqlx::query_as::<_, ProjectReview>(
        "SELECT id, project_id, reviewer_id, status, comment, created_at, updated_at FROM project_reviews WHERE project_id = $1 ORDER BY updated_at DESC",
    )
    .bind(project_id)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(reviews))
}
