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

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
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
