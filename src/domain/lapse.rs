use serde::{Deserialize, Serialize};

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
