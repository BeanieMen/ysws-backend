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

#[derive(Debug, Deserialize, Serialize)]
pub struct RegisterAttendanceRequest {
    pub first_name: String,
    pub last_name: String,
    pub email: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AttendanceRegistrationResponse {
    pub registration_id: Uuid,
    pub event_id: Uuid,
    pub status: String,
    pub participant_id: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct HackatimeProject {
    pub name: String,
    #[serde(default)]
    pub total_heartbeats: Option<i64>,
    /// Hackatime's current project response calls this `total_seconds`; older
    /// responses called it `total_duration`. Normalize both to seconds.
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
pub struct HackClubMePayload {
    pub identity: HackClubIdentity,
}

#[derive(Debug, Deserialize)]
pub struct HackClubIdentity {
    pub id: String,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub primary_email: Option<String>,
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

#[derive(Debug, Serialize)]
pub struct CurrentUserResponse {
    pub id: Uuid,
    pub email: String,
    pub first_name: String,
    pub last_name: String,
    pub hackatime_connected: bool,
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

#[derive(Debug, Deserialize)]
pub struct LapseUserResponse {
    pub user: Option<LapseUser>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct LapseUser {
    pub id: String,
    pub handle: String,
    #[serde(rename = "displayName")]
    pub display_name: String,
}

#[derive(Debug, Deserialize)]
pub struct LapseTimelapsesResponse {
    #[serde(default)]
    pub timelapses: Vec<LapseTimelapse>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct LapseTimelapse {
    pub id: String,
    pub name: String,
    #[serde(rename = "createdAt")]
    pub created_at: i64,
    pub duration: f64,
    #[serde(rename = "playbackUrl")]
    pub playback_url: Option<String>,
    #[serde(rename = "thumbnailUrl")]
    pub thumbnail_url: Option<String>,
    #[serde(default, rename = "private")]
    pub private_data: Option<LapsePrivate>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct LapsePrivate {
    #[serde(rename = "hackatimeProject")]
    pub hackatime_project: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ProjectLapsesResponse {
    pub lapse_user: Option<LapseUser>,
    pub timelapses: Vec<LapseTimelapse>,
    pub other_timelapse_count: usize,
}

#[cfg(test)]
mod tests {
    use super::HackatimeProjectsPayload;

    #[test]
    fn accepts_current_hackatime_total_seconds() {
        let parsed: HackatimeProjectsPayload = serde_json::from_str(
            r#"{"projects":[{"name":"PartyLink-mobile","total_seconds":7325}]}"#,
        )
        .unwrap();
        assert_eq!(parsed.projects[0].total_duration, Some(7325.0));
    }

    #[test]
    fn accepts_legacy_hackatime_total_duration() {
        let parsed: HackatimeProjectsPayload = serde_json::from_str(
            r#"{"projects":[{"name":"PartyLink-backend","total_duration":3600}]}"#,
        )
        .unwrap();
        assert_eq!(parsed.projects[0].total_duration, Some(3600.0));
    }
}
