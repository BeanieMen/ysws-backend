use crate::{
    adapters::http::{AppState, helpers::*},
    domain::{
        CreateProjectRequest, DashboardProject, DashboardProjectsResponse,
        HackatimeProjectsPayload, Project, ProjectHackatimeResponse, ProjectLapsesResponse,
        SetHackatimeProjectsRequest,
    },
    error::{ApiError, ApiResult},
};
use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
};
use std::{sync::Arc, time::Duration};
use uuid::Uuid;

pub async fn create_project(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(body): Json<CreateProjectRequest>,
) -> ApiResult<(StatusCode, Json<Project>)> {
    let user_id = current_user(&state, &headers).await?;
    validate_len(&body.title, "title", 120)?;
    ensure_user(&state.db, user_id).await?;
    let project = sqlx::query_as::<_, Project>("INSERT INTO projects (id, owner_id, title, description) VALUES ($1, $2, $3, $4) RETURNING id, owner_id, title, description, created_at, updated_at")
        .bind(Uuid::new_v4()).bind(user_id).bind(body.title.trim()).bind(body.description.map(|d| d.trim().to_owned()).filter(|d| !d.is_empty()))
        .fetch_one(&state.db).await?;
    Ok((StatusCode::CREATED, Json(project)))
}

pub async fn list_projects(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> ApiResult<Json<DashboardProjectsResponse>> {
    let user_id = current_user(&state, &headers).await?;
    let projects = sqlx::query_as::<_, Project>("SELECT id, owner_id, title, description, created_at, updated_at FROM projects WHERE owner_id = $1 ORDER BY created_at DESC")
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
            linked_project_names,
            total_seconds: project_seconds,
        });
    }
    Ok(Json(DashboardProjectsResponse {
        projects: response_projects,
        total_seconds,
    }))
}

pub async fn set_hackatime_projects(
    State(state): State<Arc<AppState>>,
    Path(project_id): Path<Uuid>,
    headers: HeaderMap,
    Json(body): Json<SetHackatimeProjectsRequest>,
) -> ApiResult<StatusCode> {
    let user_id = current_user(&state, &headers).await?;
    let names = normalized_names(body.names)?;
    own_project(&state.db, project_id, user_id).await?;
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

pub async fn get_project_hackatime(
    State(state): State<Arc<AppState>>,
    Path(project_id): Path<Uuid>,
    headers: HeaderMap,
) -> ApiResult<Json<ProjectHackatimeResponse>> {
    let owner_id = current_user(&state, &headers).await?;
    own_project(&state.db, project_id, owner_id).await?;
    let cache_key = format!("project:{project_id}:hackatime");
    if let Some(cached) = state.cache.get_json(&cache_key).await {
        return Ok(Json(cached));
    }
    let (names, token, _) =
        linked_connection(&state.db, &state.cipher, project_id, owner_id).await?;
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
        .set_json(&cache_key, &result, Duration::from_secs(300))
        .await;
    Ok(Json(result))
}

pub async fn get_project_lapses(
    State(state): State<Arc<AppState>>,
    Path(project_id): Path<Uuid>,
    headers: HeaderMap,
) -> ApiResult<Json<ProjectLapsesResponse>> {
    let owner_id = current_user(&state, &headers).await?;
    own_project(&state.db, project_id, owner_id).await?;
    let cache_key = format!("project:{project_id}:lapses");
    if let Some(cached) = state.cache.get_json(&cache_key).await {
        return Ok(Json(cached));
    }
    let (names, _, account_id) =
        linked_connection(&state.db, &state.cipher, project_id, owner_id).await?;
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
    for timelapse in timelapses.iter() {
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
        other_timelapse_count: timelapses.len() - matched.len(),
        timelapses: matched,
    };
    state
        .cache
        .set_json(&cache_key, &result, Duration::from_secs(300))
        .await;
    Ok(Json(result))
}
