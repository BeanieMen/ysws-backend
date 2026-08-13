use crate::{
    config::Config,
    error::{ApiError, ApiResult},
    domain::{
        HackClubIdentity, HackClubMePayload, HackatimeMePayload, HackatimeProjectsPayload,
        LapseTimelapsesResponse, LapseUser, LapseUserResponse, RegisterAttendanceRequest,
    },
};
use reqwest::Client;
use serde_json::Value;
use uuid::Uuid;

#[derive(Clone)]
pub struct Providers {
    client: Client,
    config: Config,
}

impl Providers {
    pub fn new(config: Config) -> anyhow::Result<Self> {
        let client = Client::builder().timeout(config.provider_timeout).build()?;
        Ok(Self { client, config })
    }

    pub async fn register_attendance(
        &self,
        _event_id: Uuid,
        _attendee: &RegisterAttendanceRequest,
    ) -> ApiResult<(Option<String>, Value)> {
        Ok((None, serde_json::json!({})))
    }

    pub fn hackclub_authorize_url(&self, state: &str, email: Option<&str>) -> String {
        let mut url = reqwest::Url::parse("https://auth.hackclub.com/oauth/authorize")
            .expect("Hack Club Auth URL is valid");
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

    pub fn hackatime_authorize_url(&self, state: &str) -> String {
        let mut url = reqwest::Url::parse(&format!(
            "{}/oauth/authorize",
            self.config.hackatime_api_base_url
        ))
        .expect("Hackatime URL is valid");
        let mut query = url.query_pairs_mut();
        query.append_pair("client_id", &self.config.hackatime_client_id);
        query.append_pair("redirect_uri", &self.config.hackatime_redirect_uri);
        query.append_pair("response_type", "code");
        query.append_pair("scope", "profile read");
        query.append_pair("state", state);
        drop(query);
        url.into()
    }

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
