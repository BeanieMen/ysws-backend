use crate::{
    cache::Cache,
    crypto::TokenCipher,
    error::{ApiError, ApiResult},
    models::{
        AttendanceRegistrationResponse, CreateProjectRequest, CurrentUserResponse,
        DashboardProject, DashboardProjectsResponse, HackClubIdentity, HackatimeProjectsPayload,
        Project, ProjectHackatimeResponse, ProjectLapsesResponse, RegisterAttendanceRequest,
        SetHackatimeProjectsRequest,
    },
    providers::Providers,
};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, HeaderName, HeaderValue, StatusCode, header::SET_COOKIE},
    response::{IntoResponse, Redirect, Response},
    routing::{get, post, put},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Row};
use std::{sync::Arc, time::Duration};
use uuid::Uuid;

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub cache: Cache,
    pub cipher: TokenCipher,
    pub providers: Providers,
    pub cookie_secure: bool,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/healthz", get(health))
        .route("/auth/hackclub/login", get(hackclub_login))
        .route("/auth/hackclub/callback", get(hackclub_callback))
        .route("/auth/hackatime/login", get(hackatime_login))
        .route("/auth/hackatime/callback", get(hackatime_callback))
        .route("/auth/logout", post(logout))
        .route("/api/v1/me", get(current_user_profile))
        .route("/api/v1/projects", get(list_projects).post(create_project))
        .route("/api/v1/hackatime/projects", get(list_hackatime_projects))
        .route(
            "/api/v1/projects/{project_id}/hackatime-projects",
            put(set_hackatime_projects),
        )
        .route(
            "/api/v1/projects/{project_id}/hackatime",
            get(get_project_hackatime),
        )
        .route(
            "/api/v1/projects/{project_id}/lapses",
            get(get_project_lapses),
        )
        .route(
            "/api/v1/attendance/events/{event_id}/register",
            post(register_attendance),
        )
        .with_state(Arc::new(state))
}

async fn health(State(state): State<Arc<AppState>>) -> ApiResult<StatusCode> {
    sqlx::query("SELECT 1").execute(&state.db).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct LoginQuery {
    email: Option<String>,
}

#[derive(Deserialize)]
struct OAuthCallbackQuery {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct OAuthState {
    email: Option<String>,
    user_id: Option<Uuid>,
}

#[derive(sqlx::FromRow)]
struct SessionUser {
    id: Uuid,
    email: String,
    first_name: String,
    last_name: String,
}

async fn hackclub_login(
    State(state): State<Arc<AppState>>,
    Query(query): Query<LoginQuery>,
) -> ApiResult<Redirect> {
    let email = query.email.map(|email| email.trim().to_lowercase());
    if let Some(email) = &email {
        validate_email(email)?;
    }
    let oauth_state = Uuid::new_v4().to_string();
    state
        .cache
        .set_json(
            &format!("oauth:hackclub:{oauth_state}"),
            &OAuthState {
                email: email.clone(),
                user_id: None,
            },
            Duration::from_secs(600),
        )
        .await;
    Ok(Redirect::to(
        &state
            .providers
            .hackclub_authorize_url(&oauth_state, email.as_deref()),
    ))
}

async fn hackclub_callback(
    State(state): State<Arc<AppState>>,
    Query(query): Query<OAuthCallbackQuery>,
) -> ApiResult<Response> {
    if query.error.is_some() {
        return Err(ApiError::Unauthorized(
            "Hack Club sign-in was cancelled or denied".into(),
        ));
    }
    let state_key = query
        .state
        .ok_or_else(|| ApiError::BadRequest("missing OAuth state".into()))?;
    let state_cache_key = format!("oauth:hackclub:{state_key}");
    let _oauth_state: OAuthState = state
        .cache
        .get_json(&state_cache_key)
        .await
        .ok_or_else(|| ApiError::Unauthorized("sign-in link expired; start again".into()))?;
    state.cache.delete(&state_cache_key).await;
    let code = query
        .code
        .ok_or_else(|| ApiError::BadRequest("missing OAuth code".into()))?;
    let identity = state.providers.hackclub_identity(&code).await?;
    let user_id = upsert_hackclub_user(&state.db, identity).await?;
    let token = create_session(&state.db, user_id).await?;
    // The opaque session value stays in the HttpOnly cookie. Do not put it in
    // the redirect URL or localStorage: either would expose it to scripts,
    // browser history, and logs.
    let mut response = Redirect::to("/dashboard").into_response();
    response
        .headers_mut()
        .append(SET_COOKIE, session_cookie(&token, state.cookie_secure)?);
    Ok(response)
}

async fn hackatime_login(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> ApiResult<Redirect> {
    let user_id = current_user(&state, &headers).await?;
    let oauth_state = Uuid::new_v4().to_string();
    state
        .cache
        .set_json(
            &format!("oauth:hackatime:{oauth_state}"),
            &OAuthState {
                email: None,
                user_id: Some(user_id),
            },
            Duration::from_secs(600),
        )
        .await;
    Ok(Redirect::to(
        &state.providers.hackatime_authorize_url(&oauth_state),
    ))
}

async fn hackatime_callback(
    State(state): State<Arc<AppState>>,
    Query(query): Query<OAuthCallbackQuery>,
) -> ApiResult<Redirect> {
    if query.error.is_some() {
        return Err(ApiError::Unauthorized(
            "Hackatime connection was cancelled or denied".into(),
        ));
    }
    let state_key = query
        .state
        .ok_or_else(|| ApiError::BadRequest("missing OAuth state".into()))?;
    let state_cache_key = format!("oauth:hackatime:{state_key}");
    let oauth_state: OAuthState =
        state
            .cache
            .get_json(&state_cache_key)
            .await
            .ok_or_else(|| {
                ApiError::Unauthorized("Hackatime connection link expired; start again".into())
            })?;
    state.cache.delete(&state_cache_key).await;
    let user_id = oauth_state
        .user_id
        .ok_or_else(|| ApiError::Unauthorized("invalid Hackatime connection state".into()))?;
    let code = query
        .code
        .ok_or_else(|| ApiError::BadRequest("missing OAuth code".into()))?;
    let (account_id, access_token) = state.providers.hackatime_connection(&code).await?;
    let token = state
        .cipher
        .encrypt(&access_token)
        .map_err(ApiError::Internal)?;
    sqlx::query("INSERT INTO hackatime_connections (user_id, account_id, access_token_ciphertext) VALUES ($1, $2, $3) ON CONFLICT (user_id) DO UPDATE SET account_id = EXCLUDED.account_id, access_token_ciphertext = EXCLUDED.access_token_ciphertext, updated_at = now()")
        .bind(user_id).bind(account_id).bind(token).execute(&state.db).await?;
    state
        .cache
        .delete(&format!("user:{user_id}:hackatime-projects:v2"))
        .await;
    Ok(Redirect::to("/dashboard"))
}

async fn logout(State(state): State<Arc<AppState>>, headers: HeaderMap) -> ApiResult<Response> {
    if let Some(token) = session_token(&headers) {
        sqlx::query("DELETE FROM sessions WHERE token_hash = $1")
            .bind(token_hash(token))
            .execute(&state.db)
            .await?;
    }
    let mut response = StatusCode::NO_CONTENT.into_response();
    response
        .headers_mut()
        .append(SET_COOKIE, clear_session_cookie(state.cookie_secure)?);
    Ok(response)
}

async fn current_user_profile(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> ApiResult<Json<CurrentUserResponse>> {
    let user_id = current_user(&state, &headers).await?;
    let user = sqlx::query_as::<_, SessionUser>(
        "SELECT id, email, first_name, last_name FROM users WHERE id = $1",
    )
    .bind(user_id)
    .fetch_one(&state.db)
    .await?;
    let hackatime_connected: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM hackatime_connections WHERE user_id = $1)",
    )
    .bind(user_id)
    .fetch_one(&state.db)
    .await?;
    Ok(Json(CurrentUserResponse {
        id: user.id,
        email: user.email,
        first_name: user.first_name,
        last_name: user.last_name,
        hackatime_connected,
    }))
}

async fn create_project(
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

async fn list_hackatime_projects(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> ApiResult<Json<HackatimeProjectsPayload>> {
    let user_id = current_user(&state, &headers).await?;
    Ok(Json(user_hackatime_projects(&state, user_id).await?))
}

async fn list_projects(
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

async fn set_hackatime_projects(
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

async fn get_project_hackatime(
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

async fn get_project_lapses(
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

async fn register_attendance(
    State(state): State<Arc<AppState>>,
    Path(event_id): Path<Uuid>,
    headers: HeaderMap,
) -> ApiResult<Json<AttendanceRegistrationResponse>> {
    let user_id = current_user(&state, &headers).await?;
    if state
        .cache
        .increment_with_ttl(
            &format!("ratelimit:attendance:{user_id}"),
            Duration::from_secs(60),
        )
        .await
        .is_some_and(|count| count > 10)
    {
        return Err(ApiError::Conflict(
            "too many attendance registration attempts; retry in a minute".into(),
        ));
    }
    let idempotency_key = headers
        .get(HeaderName::from_static("idempotency-key"))
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    if let Some(key) = idempotency_key.as_deref() {
        let cache_key = idempotency_cache_key(user_id, event_id, key);
        if let Some(cached) = state.cache.get_json(&cache_key).await {
            return Ok(Json(cached));
        }
    }
    let user = sqlx::query("SELECT email, first_name, last_name FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(|| ApiError::NotFound("user not found".into()))?;
    let attendee = RegisterAttendanceRequest {
        email: user.get("email"),
        first_name: user.get("first_name"),
        last_name: user.get("last_name"),
    };
    let lock_key = format!("lock:attendance:{event_id}:{user_id}");
    let lock_token = Uuid::new_v4().to_string();
    if !state
        .cache
        .take_lock(&lock_key, &lock_token, Duration::from_secs(30))
        .await
    {
        return Err(ApiError::Conflict(
            "attendance registration is already in progress".into(),
        ));
    }
    let result = register_attendance_locked(&state, event_id, user_id, attendee).await;
    state.cache.release_lock(&lock_key, &lock_token).await;
    let response = result?;
    if let Some(key) = idempotency_key {
        state
            .cache
            .set_json(
                &idempotency_cache_key(user_id, event_id, &key),
                &response,
                Duration::from_secs(86_400),
            )
            .await;
    }
    Ok(Json(response))
}

async fn register_attendance_locked(
    state: &AppState,
    event_id: Uuid,
    user_id: Uuid,
    attendee: RegisterAttendanceRequest,
) -> ApiResult<AttendanceRegistrationResponse> {
    let mut registration_id = Uuid::new_v4();
    let inserted = sqlx::query("INSERT INTO attendance_registrations (id, event_id, user_id, attendee_email, attendee_first_name, attendee_last_name, status) VALUES ($1, $2, $3, $4, $5, $6, 'pending') ON CONFLICT (event_id, user_id) DO NOTHING")
        .bind(registration_id).bind(event_id).bind(user_id).bind(&attendee.email).bind(&attendee.first_name).bind(&attendee.last_name).execute(&state.db).await?.rows_affected() == 1;
    if !inserted {
        let row = sqlx::query("SELECT id, status, attend_participant_id FROM attendance_registrations WHERE event_id = $1 AND user_id = $2").bind(event_id).bind(user_id).fetch_one(&state.db).await?;
        let status: String = row.get("status");
        if status == "registered" {
            return Ok(AttendanceRegistrationResponse {
                registration_id: row.get("id"),
                event_id,
                status,
                participant_id: row.get("attend_participant_id"),
            });
        }
        if status == "pending" {
            return Err(ApiError::Conflict(
                "attendance registration is already in progress".into(),
            ));
        }
        registration_id = row.get("id");
        sqlx::query("UPDATE attendance_registrations SET status = 'pending', last_error = NULL, updated_at = now() WHERE event_id = $1 AND user_id = $2").bind(event_id).bind(user_id).execute(&state.db).await?;
    }
    match state
        .providers
        .register_attendance(event_id, &attendee)
        .await
    {
        Ok((participant_id, provider_response)) => {
            sqlx::query("UPDATE attendance_registrations SET status = 'registered', attend_participant_id = $1, provider_response = $2, updated_at = now() WHERE event_id = $3 AND user_id = $4")
                .bind(&participant_id).bind(provider_response).bind(event_id).bind(user_id).execute(&state.db).await?;
            Ok(AttendanceRegistrationResponse {
                registration_id,
                event_id,
                status: "registered".into(),
                participant_id,
            })
        }
        Err(error) => {
            let message = error.to_string();
            sqlx::query("UPDATE attendance_registrations SET status = 'failed', last_error = $1, updated_at = now() WHERE event_id = $2 AND user_id = $3").bind(&message).bind(event_id).bind(user_id).execute(&state.db).await?;
            Err(error)
        }
    }
}

async fn linked_connection(
    db: &PgPool,
    cipher: &TokenCipher,
    project_id: Uuid,
    user_id: Uuid,
) -> ApiResult<(Vec<String>, String, String)> {
    let names = sqlx::query_scalar("SELECT hackatime_project_name FROM project_hackatime_projects WHERE project_id = $1 ORDER BY hackatime_project_name").bind(project_id).fetch_all(db).await?;
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

async fn own_project(db: &PgPool, project_id: Uuid, user_id: Uuid) -> ApiResult<()> {
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

async fn ensure_user(db: &PgPool, user_id: Uuid) -> ApiResult<()> {
    let found: Option<Uuid> = sqlx::query_scalar("SELECT id FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_optional(db)
        .await?;
    found
        .map(|_| ())
        .ok_or_else(|| ApiError::NotFound("user not found".into()))
}

async fn user_hackatime_projects(
    state: &AppState,
    user_id: Uuid,
) -> ApiResult<HackatimeProjectsPayload> {
    // v2 invalidates payloads cached before `total_seconds` was normalized.
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
        .set_json(&cache_key, &projects, Duration::from_secs(300))
        .await;
    Ok(projects)
}

async fn current_user(state: &AppState, headers: &HeaderMap) -> ApiResult<Uuid> {
    let token = session_token(headers)
        .ok_or_else(|| ApiError::Unauthorized("sign in with Hack Club first".into()))?;
    let user_id: Option<Uuid> = sqlx::query_scalar(
        "SELECT user_id FROM sessions WHERE token_hash = $1 AND expires_at > now()",
    )
    .bind(token_hash(token))
    .fetch_optional(&state.db)
    .await?;
    user_id.ok_or_else(|| ApiError::Unauthorized("your session has expired; sign in again".into()))
}

async fn upsert_hackclub_user(db: &PgPool, identity: HackClubIdentity) -> ApiResult<Uuid> {
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
        "INSERT INTO users (id, hca_id, email, first_name, last_name) VALUES ($1, $2, $3, $4, $5)",
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

async fn create_session(db: &PgPool, user_id: Uuid) -> ApiResult<String> {
    let token = Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO sessions (id, user_id, token_hash, expires_at) VALUES ($1, $2, $3, now() + interval '21 days')")
        .bind(Uuid::new_v4()).bind(user_id).bind(token_hash(&token)).execute(db).await?;
    Ok(token)
}

fn session_token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(axum::http::header::COOKIE)
        .and_then(|value| value.to_str().ok())?
        .split(';')
        .map(str::trim)
        .find_map(|cookie| cookie.strip_prefix("session="))
}

fn token_hash(token: &str) -> String {
    hex::encode(Sha256::digest(token.as_bytes()))
}

fn session_cookie(token: &str, secure: bool) -> ApiResult<HeaderValue> {
    let secure = if secure { "; Secure" } else { "" };
    HeaderValue::from_str(&format!(
        "session={token}; Path=/; HttpOnly; SameSite=Lax; Max-Age=1814400{secure}"
    ))
    .map_err(|error| ApiError::Internal(error.into()))
}

fn clear_session_cookie(secure: bool) -> ApiResult<HeaderValue> {
    let secure = if secure { "; Secure" } else { "" };
    HeaderValue::from_str(&format!(
        "session=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0{secure}"
    ))
    .map_err(|error| ApiError::Internal(error.into()))
}

fn normalized_names(names: Vec<String>) -> ApiResult<Vec<String>> {
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

fn placeholder_name(name: &str) -> bool {
    name.trim().starts_with("<<") && name.trim().ends_with(">>")
}
fn validate_email(value: &str) -> ApiResult<()> {
    if value.len() > 254 || !value.contains('@') || value.starts_with('@') || value.ends_with('@') {
        return Err(ApiError::BadRequest("enter a valid email address".into()));
    }
    Ok(())
}
fn validate_len(value: &str, field: &str, max: usize) -> ApiResult<()> {
    let length = value.trim().chars().count();
    if length == 0 || length > max {
        return Err(ApiError::BadRequest(format!(
            "{field} must be between 1 and {max} characters"
        )));
    }
    Ok(())
}
fn idempotency_cache_key(user_id: Uuid, event_id: Uuid, key: &str) -> String {
    format!(
        "idempotency:attendance:{}",
        hex::encode(Sha256::digest(
            format!("{user_id}:{event_id}:{key}").as_bytes()
        ))
    )
}
