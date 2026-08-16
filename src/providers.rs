use crate::{
    config::Config,
    domain::{
        HackClubIdentity, HackClubMePayload, HackatimeMePayload, HackatimeProjectsPayload,
        LapseTimelapsesResponse, LapseUser, LapseUserResponse, RegisterAttendanceRequest,
    },
    error::{ApiError, ApiResult},
};
use reqwest::Client;
use serde_json::Value;
use std::collections::BTreeMap;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct AirtableParticipantSync {
    pub id: Uuid,
    pub email: String,
    pub first_name: String,
    pub last_name: String,
    pub record_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AirtableProjectSync {
    pub id: Uuid,
    pub owner_id: Uuid,
    pub owner_email: String,
    pub title: String,
    pub description: Option<String>,
    pub shipped_at: chrono::DateTime<chrono::Utc>,
    pub project_approval_status: String,
    pub fraud_approval_status: String,
    pub record_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AirtableFraudStatus {
    pub record_id: String,
    pub project_id: String,
    pub status: String,
}

#[derive(Clone)]
pub struct Providers {
    client: Client,
    config: Config,
}

impl Providers {
    /// Creates a new `Providers` instance.
    ///
    /// # Errors
    ///
    /// Returns an error if the HTTP client fails to build.
    pub fn new(config: Config) -> anyhow::Result<Self> {
        let client = Client::builder().timeout(config.provider_timeout).build()?;
        Ok(Self { client, config })
    }

    #[must_use]
    pub const fn airtable_configured(&self) -> bool {
        self.config.airtable_api_key.is_some() && self.config.airtable_base_id.is_some()
    }

    fn airtable_credentials(&self) -> ApiResult<(&str, &str)> {
        let token = self
            .config
            .airtable_api_key
            .as_deref()
            .ok_or_else(|| ApiError::BadRequest("AIRTABLE_API_KEY is not configured".into()))?;
        let base_id = self
            .config
            .airtable_base_id
            .as_deref()
            .ok_or_else(|| ApiError::BadRequest("AIRTABLE_BASE_ID is not configured".into()))?;
        Ok((token, base_id))
    }

    fn airtable_table_url(&self, table: &str) -> ApiResult<reqwest::Url> {
        let (_, base_id) = self.airtable_credentials()?;
        let mut url = reqwest::Url::parse("https://api.airtable.com")
            .map_err(|error| ApiError::Internal(error.into()))?;
        let mut segments = url
            .path_segments_mut()
            .map_err(|()| ApiError::Internal(anyhow::anyhow!("invalid Airtable URL")))?;
        segments.push("v0");
        segments.push(base_id);
        segments.push(table);
        drop(segments);
        Ok(url)
    }

    async fn upsert_airtable_record(
        &self,
        table: &str,
        record_id: Option<&str>,
        fields: BTreeMap<String, Value>,
    ) -> ApiResult<String> {
        let (token, _) = self.airtable_credentials()?;
        let mut url = self.airtable_table_url(table)?;
        if let Some(record_id) = record_id {
            let mut segments = url
                .path_segments_mut()
                .map_err(|()| ApiError::Internal(anyhow::anyhow!("invalid Airtable URL")))?;
            segments.push(record_id);
        }
        let request = if record_id.is_some() {
            self.client.patch(url)
        } else {
            self.client.post(url)
        };
        let response = request
            .bearer_auth(token)
            .json(&serde_json::json!({ "fields": fields }))
            .send()
            .await?;
        if !response.status().is_success() {
            return Err(ApiError::Upstream(format!(
                "Airtable record write failed with {}",
                response.status()
            )));
        }
        let body: Value = response.json().await?;
        body.get("id")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| {
                ApiError::Upstream("Airtable response did not include a record id".into())
            })
    }

    /// Creates or updates the participant record that corresponds to a local user.
    pub async fn upsert_airtable_participant(
        &self,
        participant: &AirtableParticipantSync,
    ) -> ApiResult<String> {
        let mut fields = BTreeMap::new();
        fields.insert(
            self.config.airtable_participant_id_field.clone(),
            Value::String(participant.id.to_string()),
        );
        fields.insert("Email".into(), Value::String(participant.email.clone()));
        fields.insert(
            "First Name".into(),
            Value::String(participant.first_name.clone()),
        );
        fields.insert(
            "Last Name".into(),
            Value::String(participant.last_name.clone()),
        );
        self.upsert_airtable_record(
            &self.config.airtable_participants_table,
            participant.record_id.as_deref(),
            fields,
        )
        .await
    }

    /// Creates or updates a shipped project record in Airtable. The fraud-approval
    /// field is intentionally excluded: Airtable is the authoritative source for
    /// that field and writing the local value would overwrite whatever a reviewer
    /// set there, causing the status to silently revert on every sync cycle.
    pub async fn upsert_airtable_project(
        &self,
        project: &AirtableProjectSync,
    ) -> ApiResult<String> {
        let mut fields = BTreeMap::new();
        fields.insert(
            self.config.airtable_project_id_field.clone(),
            Value::String(project.id.to_string()),
        );
        fields.insert(
            "Owner ID".into(),
            Value::String(project.owner_id.to_string()),
        );
        fields.insert(
            "Participant Email".into(),
            Value::String(project.owner_email.clone()),
        );
        fields.insert("Title".into(), Value::String(project.title.clone()));
        fields.insert(
            "Description".into(),
            Value::String(project.description.clone().unwrap_or_default()),
        );
        fields.insert(
            "Shipped At".into(),
            Value::String(project.shipped_at.to_rfc3339()),
        );
        fields.insert(
            "Project Approval".into(),
            Value::String(project.project_approval_status.clone()),
        );
        self.upsert_airtable_record(
            &self.config.airtable_projects_table,
            project.record_id.as_deref(),
            fields,
        )
        .await
    }

    /// Retrieves project IDs and fraud decisions from the Airtable project table.
    pub async fn airtable_fraud_statuses(&self) -> ApiResult<Vec<AirtableFraudStatus>> {
        let (token, _) = self.airtable_credentials()?;
        let mut offset: Option<String> = None;
        let mut statuses = Vec::new();
        loop {
            let mut url = self.airtable_table_url(&self.config.airtable_projects_table)?;
            {
                let mut query = url.query_pairs_mut();
                query.append_pair("pageSize", "100");
                query.append_pair("fields[]", &self.config.airtable_project_id_field);
                query.append_pair("fields[]", &self.config.airtable_fraud_approval_field);
                if let Some(offset) = &offset {
                    query.append_pair("offset", offset);
                }
            }
            let response = self.client.get(url).bearer_auth(token).send().await?;
            if !response.status().is_success() {
                return Err(ApiError::Upstream(format!(
                    "Airtable fraud-status request failed with {}",
                    response.status()
                )));
            }
            let body: Value = response.json().await?;
            let records = body
                .get("records")
                .and_then(Value::as_array)
                .ok_or_else(|| {
                    ApiError::Upstream("Airtable response did not include records".into())
                })?;
            for record in records {
                let Some(record_id) = record.get("id").and_then(Value::as_str) else {
                    continue;
                };
                let Some(fields) = record.get("fields").and_then(Value::as_object) else {
                    continue;
                };
                let Some(project_id) = fields
                    .get(&self.config.airtable_project_id_field)
                    .and_then(airtable_field_text)
                else {
                    continue;
                };
                let Some(status) = fields
                    .get(&self.config.airtable_fraud_approval_field)
                    .and_then(airtable_field_text)
                else {
                    continue;
                };
                statuses.push(AirtableFraudStatus {
                    record_id: record_id.to_owned(),
                    project_id,
                    status,
                });
            }
            offset = body
                .get("offset")
                .and_then(Value::as_str)
                .map(str::to_owned);
            if offset.is_none() {
                return Ok(statuses);
            }
        }
    }

    /// Registers attendance for an event.
    ///
    /// # Errors
    ///
    /// Returns an error if registering attendance fails.
    pub async fn register_attendance(
        &self,
        _event_id: Uuid,
        _attendee: &RegisterAttendanceRequest,
    ) -> ApiResult<(Option<String>, Value)> {
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        Ok((None, serde_json::json!({})))
    }

    #[must_use]
    pub fn hackclub_authorize_url(&self, state: &str, email: Option<&str>) -> String {
        let Ok(mut url) = reqwest::Url::parse("https://auth.hackclub.com/oauth/authorize") else {
            return String::new();
        };

        let mut query = url.query_pairs_mut();
        query.append_pair("client_id", &self.config.hackclub_client_id);
        query.append_pair("redirect_uri", &self.config.hackclub_redirect_uri);
        query.append_pair("response_type", "code");
        query.append_pair("scope", "openid profile email name");
        query.append_pair("state", state);
        if let Some(email) = email {
            query.append_pair("login_hint", email);
        }
        drop(query);
        url.into()
    }

    /// Retrieves Hack Club identity using an authorization code.
    ///
    /// # Errors
    ///
    /// Returns an error if token exchange or identity fetch fails.
    pub async fn hackclub_identity(&self, code: &str) -> ApiResult<HackClubIdentity> {
        let token_response = self
            .client
            .post("https://auth.hackclub.com/oauth/token")
            .form(&[
                ("client_id", self.config.hackclub_client_id.as_str()),
                ("client_secret", self.config.hackclub_client_secret.as_str()),
                ("redirect_uri", self.config.hackclub_redirect_uri.as_str()),
                ("code", code),
                ("grant_type", "authorization_code"),
            ])
            .send()
            .await?;
        if !token_response.status().is_success() {
            return Err(ApiError::Unauthorized(
                "Hack Club declined the sign-in request".into(),
            ));
        }
        let token: Value = token_response.json().await?;
        let access_token = token
            .get("access_token")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                ApiError::Upstream(
                    "Hack Club token response did not include an access token".into(),
                )
            })?;
        let response = self
            .client
            .get("https://auth.hackclub.com/api/v1/me")
            .bearer_auth(access_token)
            .send()
            .await?;
        if !response.status().is_success() {
            return Err(ApiError::Unauthorized(
                "Hack Club could not provide your identity".into(),
            ));
        }
        Ok(response.json::<HackClubMePayload>().await?.identity)
    }

    /// Builds the Hackatime OAuth authorization URL.
    ///
    /// # Panics
    ///
    /// Panics if `hackatime_api_base_url` cannot be parsed as a valid URL.
    #[must_use]
    pub fn hackatime_authorize_url(&self, state: &str) -> String {
        let mut url;

        if let Ok(parsed_url) = reqwest::Url::parse(&format!(
            "{}/oauth/authorize",
            self.config.hackatime_api_base_url
        )) {
            url = parsed_url;
        } else {
            eprintln!("Failed to parse Hackatime URL");
            return String::new();
        }

        let mut query = url.query_pairs_mut();
        query.append_pair("client_id", &self.config.hackatime_client_id);
        query.append_pair("redirect_uri", &self.config.hackatime_redirect_uri);
        query.append_pair("response_type", "code");
        query.append_pair("scope", "profile read");
        query.append_pair("state", state);
        drop(query);
        url.into()
    }

    /// Establishes a Hackatime connection using an authorization code.
    ///
    /// # Errors
    ///
    /// Returns an error if token exchange or account fetch fails.
    pub async fn hackatime_connection(&self, code: &str) -> ApiResult<(String, String)> {
        let token_response = self
            .client
            .post(format!(
                "{}/oauth/token",
                self.config.hackatime_api_base_url
            ))
            .form(&[
                ("client_id", self.config.hackatime_client_id.as_str()),
                (
                    "client_secret",
                    self.config.hackatime_client_secret.as_str(),
                ),
                ("redirect_uri", self.config.hackatime_redirect_uri.as_str()),
                ("code", code),
                ("grant_type", "authorization_code"),
            ])
            .send()
            .await?;
        if !token_response.status().is_success() {
            return Err(ApiError::Unauthorized(
                "Hackatime declined the connection request".into(),
            ));
        }
        let token: Value = token_response.json().await?;
        let access_token = token
            .get("access_token")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                ApiError::Upstream(
                    "Hackatime token response did not include an access token".into(),
                )
            })?
            .to_owned();
        let response = self
            .client
            .get(format!(
                "{}/api/v1/authenticated/me",
                self.config.hackatime_api_base_url
            ))
            .bearer_auth(&access_token)
            .send()
            .await?;
        if !response.status().is_success() {
            return Err(ApiError::Unauthorized(
                "Hackatime could not provide your account".into(),
            ));
        }
        let account = response.json::<HackatimeMePayload>().await?;
        let account_id = match account.id {
            Value::String(value) => value,
            value => value.to_string(),
        };
        Ok((account_id, access_token))
    }

    /// Fetches Hackatime projects for the given access token.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or deserialization fails.
    pub async fn hackatime_projects(
        &self,
        access_token: &str,
    ) -> ApiResult<HackatimeProjectsPayload> {
        let response = self
            .client
            .get(format!(
                "{}/api/v1/authenticated/projects",
                self.config.hackatime_api_base_url
            ))
            .bearer_auth(access_token)
            .send()
            .await?;
        if !response.status().is_success() {
            return Err(ApiError::Upstream(format!(
                "Hackatime projects request failed with {}",
                response.status()
            )));
        }
        let body: Value = response.json().await?;
        if body.is_array() {
            let projects = serde_json::from_value(body)?;
            Ok(HackatimeProjectsPayload { projects })
        } else {
            Ok(serde_json::from_value(body)?)
        }
    }

    /// Looks up a Lapse user by Hackatime ID.
    ///
    /// # Errors
    ///
    /// Returns an error if the Lapse API token is missing or request fails.
    pub async fn lapse_user(&self, hackatime_id: &str) -> ApiResult<Option<LapseUser>> {
        let token = self
            .config
            .lapse_api_token
            .as_deref()
            .ok_or_else(|| ApiError::BadRequest("Lapse is not configured".into()))?;
        let response = self
            .client
            .get(format!("{}/user/query", self.config.lapse_api_base_url))
            .bearer_auth(token)
            .query(&[("hackatimeId", hackatime_id)])
            .send()
            .await?;
        if !response.status().is_success() {
            return Err(ApiError::Upstream(format!(
                "Lapse user lookup failed with {}",
                response.status()
            )));
        }
        let body: Value = response.json().await?;
        if body.get("ok") == Some(&Value::Bool(false)) {
            return Err(ApiError::Upstream(
                body.get("error")
                    .and_then(Value::as_str)
                    .unwrap_or("Lapse lookup failed")
                    .into(),
            ));
        }
        Ok(
            serde_json::from_value::<LapseUserResponse>(body.get("data").cloned().unwrap_or(body))?
                .user,
        )
    }

    /// Fetches timelapses for a given Lapse user ID.
    ///
    /// # Errors
    ///
    /// Returns an error if the Lapse API token is missing or request fails.
    pub async fn lapse_timelapses(
        &self,
        lapse_user_id: &str,
    ) -> ApiResult<LapseTimelapsesResponse> {
        let token = self
            .config
            .lapse_api_token
            .as_deref()
            .ok_or_else(|| ApiError::BadRequest("Lapse is not configured".into()))?;
        let response = self
            .client
            .get(format!(
                "{}/timelapse/findByUser",
                self.config.lapse_api_base_url
            ))
            .bearer_auth(token)
            .query(&[("user", lapse_user_id)])
            .send()
            .await?;
        if !response.status().is_success() {
            return Err(ApiError::Upstream(format!(
                "Lapse timelapse lookup failed with {}",
                response.status()
            )));
        }
        let body: Value = response.json().await?;
        if body.get("ok") == Some(&Value::Bool(false)) {
            return Err(ApiError::Upstream(
                body.get("error")
                    .and_then(Value::as_str)
                    .unwrap_or("Lapse lookup failed")
                    .into(),
            ));
        }
        Ok(serde_json::from_value(
            body.get("data").cloned().unwrap_or(body),
        )?)
    }
}

fn airtable_field_text(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => Some(value.clone()),
        Value::Bool(value) => Some(value.to_string()),
        Value::Array(values) => values.first().and_then(airtable_field_text),
        _ => None,
    }
}
