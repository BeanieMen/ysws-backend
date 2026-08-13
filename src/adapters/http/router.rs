use crate::{
    adapters::http::{
        admin_handler, attendance_handler, auth_handler, health_handler, project_handler,
        review_handler, user_handler,
    },
    ports::{CachePort, CryptoPort, DbPort, ProvidersPort},
};
use axum::{
    Router,
    routing::{delete, get, post, put},
};
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub db: DbPort,
    pub cache: CachePort,
    pub cipher: CryptoPort,
    pub providers: ProvidersPort,
    pub cookie_secure: bool,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/healthz", get(health_handler::health))
        .route("/auth/hackclub/login", get(auth_handler::hackclub_login))
        .route(
            "/auth/hackclub/callback",
            get(auth_handler::hackclub_callback),
        )
        .route("/auth/hackatime/login", get(auth_handler::hackatime_login))
        .route(
            "/auth/hackatime/callback",
            get(auth_handler::hackatime_callback),
        )
        .route("/auth/logout", post(auth_handler::logout))
        .route("/api/v1/me", get(user_handler::current_user_profile))
        .route(
            "/api/v1/projects",
            get(project_handler::list_projects).post(project_handler::create_project),
        )
        .route(
            "/api/v1/hackatime/projects",
            get(attendance_handler::list_hackatime_projects),
        )
        .route(
            "/api/v1/projects/{project_id}/hackatime-projects",
            put(project_handler::set_hackatime_projects),
        )
        .route(
            "/api/v1/projects/{project_id}/hackatime",
            get(project_handler::get_project_hackatime),
        )
        .route(
            "/api/v1/projects/{project_id}/lapses",
            get(project_handler::get_project_lapses),
        )
        .route(
            "/api/v1/attendance/events/{event_id}/register",
            post(attendance_handler::register_attendance),
        )
        // Reviewer & Admin endpoints
        .route(
            "/api/v1/reviews/projects",
            get(review_handler::list_projects_for_review),
        )
        .route(
            "/api/v1/projects/{project_id}/reviews",
            get(review_handler::get_project_reviews).post(review_handler::create_project_review),
        )
        // Admin-only endpoints
        .route("/api/v1/admin/users", get(admin_handler::list_users))
        .route(
            "/api/v1/admin/users/{user_id}/role",
            put(admin_handler::update_user_role),
        )
        .route(
            "/api/v1/admin/users/{user_id}",
            put(admin_handler::admin_update_user).delete(admin_handler::admin_delete_user),
        )
        .route(
            "/api/v1/admin/projects/{project_id}",
            delete(admin_handler::admin_delete_project),
        )
        .with_state(Arc::new(state))
}
