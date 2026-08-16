use crate::{
    adapters::http::{AppState, helpers::{current_user, validate_len, ensure_user, user_hackatime_projects, placeholder_name, current_session_user, normalized_names, own_project_or_admin, linked_connection}},
    domain::{
        CreateProjectRequest, DashboardProject, DashboardProjectsResponse,
        HackatimeProjectsPayload, Project, ProjectBannerResponse, ProjectHackatimeResponse,
        ProjectLapsesResponse, SetHackatimeProjectsRequest, ShipProjectResponse,
    },
    error::{ApiError, ApiResult},
};
use axum::{
    Json,
    extract::{Multipart, Path, State},
    http::{HeaderMap, StatusCode},
};
use std::{sync::Arc, time::Duration};
use uuid::Uuid;

/// Creates a new project.
///
/// # Errors
///
/// Returns an error if input validation fails or database query fails.
pub async fn create_project(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(body): Json<CreateProjectRequest>,
) -> ApiResult<(StatusCode, Json<Project>)> {
    let user_id = current_user(&state, &headers).await?;
    validate_len(&body.title, "title", 120)?;
    ensure_user(&state.db, user_id).await?;
    let mut transaction = state.db.begin().await?;
    let project = sqlx::query_as::<_, Project>("INSERT INTO projects (id, owner_id, title, description) VALUES ($1, $2, $3, $4) RETURNING id, owner_id, title, description, banner_url, submission_status, submitted_at, shipped_at, 'pending'::text AS project_approval_status, 'pending'::text AS fraud_approval_status, created_at, updated_at")
        .bind(Uuid::new_v4()).bind(user_id).bind(body.title.trim()).bind(body.description.map(|d| d.trim().to_owned()).filter(|d| !d.is_empty()))
        .fetch_one(&mut *transaction).await?;
    sqlx::query("INSERT INTO project_shipments (project_id) VALUES ($1)")
        .bind(project.id)
        .execute(&mut *transaction)
        .await?;
    transaction.commit().await?;
    Ok((StatusCode::CREATED, Json(project)))
}

/// Lists projects for the current user.
///
/// # Errors
///
/// Returns an error if authentication fails or database query fails.
pub async fn list_projects(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> ApiResult<Json<DashboardProjectsResponse>> {
    let user_id = current_user(&state, &headers).await?;
    let projects = sqlx::query_as::<_, Project>("SELECT p.id, p.owner_id, p.title, p.description, p.banner_url, p.submission_status, p.submitted_at, p.shipped_at, COALESCE(s.project_approval_status, 'pending') AS project_approval_status, COALESCE(s.fraud_approval_status, 'pending') AS fraud_approval_status, p.created_at, p.updated_at FROM projects p LEFT JOIN project_shipments s ON s.project_id = p.id WHERE p.owner_id = $1 ORDER BY p.created_at DESC")
        .bind(user_id).fetch_all(&state.db).await?;
    let available = user_hackatime_projects(&state, user_id)
        .await
        .unwrap_or(HackatimeProjectsPayload { projects: vec![] });
    let durations: std::collections::HashMap<_, _> = available
        .projects
        .into_iter()
        .filter(|project| !placeholder_name(&project.name))
        .map(|project| (project.name, project.total_duration.unwrap_or(0.0)))
        .collect();
    let mut response_projects = Vec::with_capacity(projects.len());
    let mut total_seconds = 0.0;
    for project in projects {
        let linked_project_names: Vec<String> = sqlx::query_scalar("SELECT hackatime_project_name FROM project_hackatime_projects WHERE project_id = $1 ORDER BY hackatime_project_name")
            .bind(project.id).fetch_all(&state.db).await?;
        let project_seconds: f64 = linked_project_names
            .iter()
            .filter_map(|name| durations.get(name))
            .sum();
        total_seconds += project_seconds;
        response_projects.push(DashboardProject {
            id: project.id,
            title: project.title,
            description: project.description,
            banner_url: project.banner_url,
            submission_status: project.submission_status,
            submitted_at: project.submitted_at,
            shipped_at: project.shipped_at,
            project_approval_status: project.project_approval_status,
            fraud_approval_status: project.fraud_approval_status,
            linked_project_names,
            total_seconds: project_seconds,
        });
    }
    Ok(Json(DashboardProjectsResponse {
        projects: response_projects,
        total_seconds,
    }))
}

/// Sets linked Hackatime projects for a project.
///
/// # Errors
///
/// Returns an error if authorization fails, lock acquisition fails, or database transaction fails.
pub async fn set_hackatime_projects(
    State(state): State<Arc<AppState>>,
    Path(project_id): Path<Uuid>,
    headers: HeaderMap,
    Json(body): Json<SetHackatimeProjectsRequest>,
) -> ApiResult<StatusCode> {
    let session_user = current_session_user(&state, &headers).await?;
    let names = normalized_names(body.names)?;
    own_project_or_admin(&state.db, project_id, &session_user).await?;
    let lock_key = format!("lock:project:{project_id}:hackatime-projects");
    let lock_token = Uuid::new_v4().to_string();
    if !state
        .cache
        .take_lock(&lock_key, &lock_token, Duration::from_secs(10))
        .await
    {
        return Err(ApiError::Conflict(
            "project links are being updated; retry shortly".into(),
        ));
    }
    let result = async {
        let mut tx = state.db.begin().await?;
        sqlx::query("DELETE FROM project_hackatime_projects WHERE project_id = $1").bind(project_id).execute(&mut *tx).await?;
        for name in names {
            sqlx::query("INSERT INTO project_hackatime_projects (project_id, hackatime_project_name) VALUES ($1, $2)").bind(project_id).bind(name).execute(&mut *tx).await?;
        }
        sqlx::query("UPDATE projects SET updated_at = now() WHERE id = $1").bind(project_id).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok::<(), ApiError>(())
    }.await;
    state.cache.release_lock(&lock_key, &lock_token).await;
    result?;
    state
        .cache
        .delete(&format!("project:{project_id}:hackatime"))
        .await;
    state
        .cache
        .delete(&format!("project:{project_id}:lapses"))
        .await;
    Ok(StatusCode::NO_CONTENT)
}

/// Retrieves Hackatime data for a project.
///
/// # Errors
///
/// Returns an error if authorization fails or upstream service request fails.
pub async fn get_project_hackatime(
    State(state): State<Arc<AppState>>,
    Path(project_id): Path<Uuid>,
    headers: HeaderMap,
) -> ApiResult<Json<ProjectHackatimeResponse>> {
    let session_user = current_session_user(&state, &headers).await?;
    own_project_or_admin(&state.db, project_id, &session_user).await?;
    let cache_key = format!("project:{project_id}:hackatime");
    if let Some(cached) = state.cache.get_json(&cache_key).await {
        return Ok(Json(cached));
    }
    let (names, token, _) =
        linked_connection(&state.db, &state.cipher, project_id, session_user.id).await?;
    let summary = state.providers.hackatime_projects(&token).await?;
    let linked: std::collections::HashSet<_> = names.iter().collect();
    let projects = summary
        .projects
        .into_iter()
        .filter(|p| linked.contains(&p.name))
        .filter(|p| !placeholder_name(&p.name))
        .collect();
    let result = ProjectHackatimeResponse {
        linked_project_names: names,
        projects,
    };
    state
        .cache
        .set_json(&cache_key, &result, Duration::from_mins(5))
        .await;
    Ok(Json(result))
}

/// Retrieves Lapse data for a project.
///
/// # Errors
///
/// Returns an error if authorization fails or upstream service request fails.
pub async fn get_project_lapses(
    State(state): State<Arc<AppState>>,
    Path(project_id): Path<Uuid>,
    headers: HeaderMap,
) -> ApiResult<Json<ProjectLapsesResponse>> {
    let session_user = current_session_user(&state, &headers).await?;
    own_project_or_admin(&state.db, project_id, &session_user).await?;
    let cache_key = format!("project:{project_id}:lapses");
    if let Some(cached) = state.cache.get_json(&cache_key).await {
        return Ok(Json(cached));
    }
    let (names, _, account_id) =
        linked_connection(&state.db, &state.cipher, project_id, session_user.id).await?;
    let Some(lapse_user) = state.providers.lapse_user(&account_id).await? else {
        return Ok(Json(ProjectLapsesResponse {
            lapse_user: None,
            timelapses: vec![],
            other_timelapse_count: 0,
        }));
    };
    let timelapses = state
        .providers
        .lapse_timelapses(&lapse_user.id)
        .await?
        .timelapses;
    let linked: std::collections::HashSet<_> = names.iter().collect();
    let mut matched = Vec::new();
    for timelapse in &timelapses {
        if timelapse
            .private_data
            .as_ref()
            .and_then(|private| private.hackatime_project.as_ref())
            .is_some_and(|name| linked.contains(name))
        {
            matched.push(timelapse.clone());
        }
    }
    matched.sort_by_key(|t| std::cmp::Reverse(t.created_at));
    let result = ProjectLapsesResponse {
        lapse_user: Some(lapse_user),
        other_timelapse_count: timelapses.len().saturating_sub(matched.len()),
        timelapses: matched,
    };
    state
        .cache
        .set_json(&cache_key, &result, Duration::from_mins(5))
        .await;
    Ok(Json(result))
}

/// Uploads a banner image for a project.
///
/// # Errors
///
/// Returns an error if file parsing fails, file size exceeds limit, or writing fails.
pub async fn upload_project_banner(
    State(state): State<Arc<AppState>>,
    Path(project_id): Path<Uuid>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> ApiResult<Json<ProjectBannerResponse>> {
    let session_user = current_session_user(&state, &headers).await?;
    own_project_or_admin(&state.db, project_id, &session_user).await?;

    let mut file_data = Vec::new();
    let mut file_ext = "png".to_string();

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| ApiError::BadRequest(format!("invalid multipart data: {e}")))?
    {
        let name = field.name().unwrap_or("");
        if name == "banner" || name == "file" || name == "image" || name == "upload" {
            if let Some(content_type) = field.content_type() {
                file_ext = match content_type {
                    "image/jpeg" | "image/jpg" => "jpg".into(),
                    "image/png" => "png".into(),
                    "image/webp" => "webp".into(),
                    "image/gif" => "gif".into(),
                    _ => {
                        return Err(ApiError::BadRequest(
                            "only JPEG, PNG, WebP, and GIF images are allowed".into(),
                        ));
                    }
                };
            }
            file_data = field
                .bytes()
                .await
                .map_err(|e| ApiError::BadRequest(format!("failed to read image file: {e}")))?
                .to_vec();
            break;
        }
    }

    if file_data.is_empty() {
        return Err(ApiError::BadRequest("no banner image file provided".into()));
    }

    if file_data.len() > 10 * 1024 * 1024 {
        return Err(ApiError::BadRequest("banner image exceeds maximum limit of 10MB".into()));
    }

    let upload_dir = std::path::Path::new("uploads/banners");
    tokio::fs::create_dir_all(upload_dir)
        .await
        .map_err(|e| ApiError::Internal(e.into()))?;

    let filename = format!("{project_id}_{}.{file_ext}", Uuid::new_v4());
    let filepath = upload_dir.join(&filename);
    tokio::fs::write(&filepath, file_data)
        .await
        .map_err(|e| ApiError::Internal(e.into()))?;

    let banner_url = format!("/uploads/banners/{filename}");

    sqlx::query("UPDATE projects SET banner_url = $1, updated_at = now() WHERE id = $2")
        .bind(&banner_url)
        .bind(project_id)
        .execute(&state.db)
        .await?;

    state
        .cache
        .delete(&format!("project:{project_id}:hackatime"))
        .await;

    Ok(Json(ProjectBannerResponse {
        project_id,
        banner_url,
    }))
}

/// Ships a project, making it visible to reviewers.
///
/// # Errors
///
/// Returns an error if authorization fails or database update fails.
pub async fn ship_project(
    State(state): State<Arc<AppState>>,
    Path(project_id): Path<Uuid>,
    headers: HeaderMap,
) -> ApiResult<Json<ShipProjectResponse>> {
    let session_user = current_session_user(&state, &headers).await?;
    // Shipping is an owner action. Admins can still inspect every project but
    // cannot accidentally publish work on somebody else's behalf.
    crate::adapters::http::helpers::own_project(&state.db, project_id, session_user.id).await?;

    let (submission_status, shipped_at, project_approval_status, fraud_approval_status): (String, chrono::DateTime<chrono::Utc>, String, String) = sqlx::query_as(
        "WITH updated_project AS (UPDATE projects SET submission_status = CASE WHEN submission_status = 'draft' THEN 'submitted' ELSE submission_status END, submitted_at = COALESCE(submitted_at, now()), shipped_at = COALESCE(shipped_at, now()), updated_at = now() WHERE id = $1 RETURNING id, submission_status, shipped_at), updated_shipment AS (INSERT INTO project_shipments (project_id, shipped_at) SELECT id, shipped_at FROM updated_project ON CONFLICT (project_id) DO UPDATE SET shipped_at = COALESCE(project_shipments.shipped_at, EXCLUDED.shipped_at), updated_at = now() RETURNING shipped_at, project_approval_status, fraud_approval_status) SELECT (SELECT submission_status FROM updated_project), shipped_at, project_approval_status, fraud_approval_status FROM updated_shipment",
    )
    .bind(project_id)
    .fetch_one(&state.db)
    .await?;

    Ok(Json(ShipProjectResponse {
        project_id,
        submission_status,
        shipped_at,
        project_approval_status,
        fraud_approval_status,
    }))
}
