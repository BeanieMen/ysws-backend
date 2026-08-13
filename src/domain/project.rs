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
    pub linked_project_names: Vec<String>,
    pub total_seconds: f64,
}

#[derive(Debug, Serialize)]
pub struct DashboardProjectsResponse {
    pub projects: Vec<DashboardProject>,
    pub total_seconds: f64,
}
