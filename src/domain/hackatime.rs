use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct HackatimeProject {
    pub name: String,
    #[serde(default)]
    pub total_heartbeats: Option<i64>,
    #[serde(default, alias = "total_seconds")]
    pub total_duration: Option<f64>,
    #[serde(default)]
    pub first_heartbeat: Option<f64>,
    #[serde(default)]
    pub last_heartbeat: Option<f64>,
    #[serde(default)]
    pub languages: Vec<String>,
    #[serde(default)]
    pub repo: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct HackatimeProjectsPayload {
    #[serde(default)]
    pub projects: Vec<HackatimeProject>,
}

#[derive(Debug, Deserialize)]
pub struct HackatimeMePayload {
    pub id: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ProjectHackatimeResponse {
    pub linked_project_names: Vec<String>,
    pub projects: Vec<HackatimeProject>,
}
