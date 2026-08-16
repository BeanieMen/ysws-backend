#![allow(clippy::unwrap_used, clippy::unused_async, clippy::must_use_candidate, clippy::missing_panics_doc)]

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use serde_json::Value;

use crate::domain::{
    HackClubIdentity, HackatimeProject, HackatimeProjectsPayload, LapseTimelapsesResponse, LapseUser,
};
use crate::error::{ApiError, ApiResult};

#[derive(Debug, Clone, Default)]
pub struct MockProviders {
    pub oauth_codes: Arc<Mutex<HashMap<String, HackClubIdentity>>>,
    pub hackatime_codes: Arc<Mutex<HashMap<String, (String, String)>>>,
    pub hackatime_projects_data: Arc<Mutex<HashMap<String, Vec<HackatimeProject>>>>,
    pub lapse_users: Arc<Mutex<HashMap<String, LapseUser>>>,
    pub lapse_timelapses_data: Arc<Mutex<HashMap<String, LapseTimelapsesResponse>>>,
}

impl MockProviders {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a mock Hack Club OAuth code.
    pub fn register_hackclub_code(&self, code: impl Into<String>, identity: HackClubIdentity) {
        self.oauth_codes.lock().unwrap().insert(code.into(), identity);
    }

    /// Simulates Hack Club identity retrieval.
    ///
    /// # Errors
    ///
    /// Returns `ApiError::Unauthorized` if the code is invalid or not registered.
    pub async fn hackclub_identity(&self, code: &str) -> ApiResult<HackClubIdentity> {
        self.oauth_codes
            .lock()
            .unwrap()
            .get(code)
            .cloned()
            .ok_or_else(|| ApiError::Unauthorized("Invalid authorization code".into()))
    }

    /// Registers a mock Hackatime OAuth code.
    pub fn register_hackatime_code(&self, code: impl Into<String>, account_id: impl Into<String>, token: impl Into<String>) {
        self.hackatime_codes
            .lock()
            .unwrap()
            .insert(code.into(), (account_id.into(), token.into()));
    }

    /// Simulates Hackatime connection token exchange.
    ///
    /// # Errors
    ///
    /// Returns `ApiError::Unauthorized` if the code is invalid.
    pub async fn hackatime_connection(&self, code: &str) -> ApiResult<(String, String)> {
        self.hackatime_codes
            .lock()
            .unwrap()
            .get(code)
            .cloned()
            .ok_or_else(|| ApiError::Unauthorized("Invalid hackatime code".into()))
    }

    /// Registers mock projects for a Hackatime access token.
    pub fn set_hackatime_projects(&self, token: impl Into<String>, projects: Vec<HackatimeProject>) {
        self.hackatime_projects_data.lock().unwrap().insert(token.into(), projects);
    }

    /// Simulates fetching Hackatime projects.
    ///
    /// # Errors
    ///
    /// Returns `ApiError::Upstream` if the access token is invalid or has no projects.
    pub async fn hackatime_projects(&self, access_token: &str) -> ApiResult<HackatimeProjectsPayload> {
        let projects = self
            .hackatime_projects_data
            .lock()
            .unwrap()
            .get(access_token)
            .cloned()
            .ok_or_else(|| ApiError::Upstream("Invalid token or no projects found".into()))?;
        Ok(HackatimeProjectsPayload { projects })
    }

    /// Simulates registering attendance.
    ///
    /// # Errors
    ///
    /// Returns `ApiResult` with dummy status payload.
    pub async fn register_attendance(&self) -> ApiResult<(Option<String>, Value)> {
        Ok((Some("part_123".into()), serde_json::json!({"status": "ok"})))
    }
}
