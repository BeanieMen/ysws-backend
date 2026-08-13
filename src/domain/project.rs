use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct CreateProjectRequest {
    pub title: String,
    pub description: Option<String>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Project {
    pub id: Uuid,
    pub owner_id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub banner_url: Option<String>,
    pub submission_status: String,
    pub submitted_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct SetHackatimeProjectsRequest {
    pub names: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct DashboardProject {
    pub id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub banner_url: Option<String>,
    pub submission_status: String,
    pub submitted_at: Option<DateTime<Utc>>,
    pub linked_project_names: Vec<String>,
    pub total_seconds: f64,
}

#[derive(Debug, Serialize)]
pub struct DashboardProjectsResponse {
    pub projects: Vec<DashboardProject>,
    pub total_seconds: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ProjectReview {
    pub id: Uuid,
    pub project_id: Uuid,
    pub reviewer_id: Uuid,
    pub status: String,
    pub comment: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateProjectReviewRequest {
    pub status: String,
    pub comment: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ProjectBannerResponse {
    pub project_id: Uuid,
    pub banner_url: String,
}

#[derive(Debug, Serialize)]
pub struct SubmitProjectResponse {
    pub project_id: Uuid,
    pub submission_status: String,
    pub submitted_at: DateTime<Utc>,
}
