use std::{env, time::Duration};

#[derive(Clone)]
pub struct Config {
    pub database_url: String,
    pub redis_url: String,
    pub app_url: String,
    pub backend_url: String,
    pub port: u16,
    pub encryption_key: [u8; 32],
    #[allow(dead_code)]
    pub attend_api_base_url: String,
    #[allow(dead_code)]
    pub attend_api_key: String,
    pub hackatime_api_base_url: String,
    pub hackclub_client_id: String,
    pub hackclub_client_secret: String,
    pub hackclub_redirect_uri: String,
    pub hackatime_client_id: String,
    pub hackatime_client_secret: String,
    pub hackatime_redirect_uri: String,
    pub cookie_secure: bool,
    pub lapse_api_base_url: String,
    pub lapse_api_token: Option<String>,
    pub airtable_api_key: Option<String>,
    pub airtable_base_id: Option<String>,
    pub airtable_participants_table: String,
    pub airtable_projects_table: String,
    pub airtable_participant_id_field: String,
    pub airtable_project_id_field: String,
    pub airtable_fraud_approval_field: String,
    pub airtable_sync_interval: Duration,
    pub provider_timeout: Duration,
}

impl Config {
    /// Loads application configuration from environment variables.
    ///
    /// # Errors
    ///
    /// Returns an error if a required environment variable is missing, if
    /// `APP_ENCRYPTION_KEY` is not valid hexadecimal or is not exactly 32 bytes,
    /// or if `PORT` cannot be parsed as a valid port number.
    pub fn from_env() -> anyhow::Result<Self> {
        dotenvy::dotenv().ok();

        let encryption_key = env::var("APP_ENCRYPTION_KEY")
            .map_err(|_| anyhow::anyhow!("APP_ENCRYPTION_KEY must be configured"))?;

        let bytes = hex::decode(encryption_key)
            .map_err(|_| anyhow::anyhow!("APP_ENCRYPTION_KEY must be hexadecimal"))?;

        let encryption_key: [u8; 32] = bytes
            .try_into()
            .map_err(|_| anyhow::anyhow!("APP_ENCRYPTION_KEY must be exactly 32 bytes"))?;

        let app_url = env_or("APP_URL", "http://localhost:3000");
        let backend_url = env_or("BACKEND_URL", "http://localhost:8000");

        let hackclub_redirect_uri = env::var("HACKCLUB_REDIRECT_URI")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| format!("{app_url}/auth/hackclub/callback"));

        let hackatime_redirect_uri = env::var("HACKATIME_REDIRECT_URI")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| format!("{app_url}/auth/hackatime/callback"));

        Ok(Self {
            database_url: required("DATABASE_URL")?,
            redis_url: required("REDIS_URL")?,
            app_url: app_url.clone(),
            backend_url: backend_url.clone(),
            port: env::var("PORT").unwrap_or_else(|_| "8000".into()).parse()?,
            encryption_key,
            attend_api_base_url: env_or("ATTEND_API_BASE_URL", "https://attend.hackclub.com"),
            attend_api_key: env_or("ATTEND_API_KEY", ""),
            hackatime_api_base_url: env_or(
                "HACKATIME_API_BASE_URL",
                "https://hackatime.hackclub.com",
            ),
            hackclub_client_id: required("HACKCLUB_CLIENT_ID")?,
            hackclub_client_secret: required("HACKCLUB_CLIENT_SECRET")?,
            hackclub_redirect_uri,
            hackatime_client_id: required("HACKATIME_CLIENT_ID")?,
            hackatime_client_secret: required("HACKATIME_CLIENT_SECRET")?,
            hackatime_redirect_uri,
            cookie_secure: env::var("COOKIE_SECURE")
                .is_ok_and(|value| value == "true" || value == "1"),
            lapse_api_base_url: env_or("LAPSE_API_BASE_URL", "https://api.lapse.hackclub.com"),
            lapse_api_token: env::var("LAPSE_API_TOKEN").ok().filter(|s| !s.is_empty()),
            airtable_api_key: optional("AIRTABLE_API_KEY"),
            airtable_base_id: optional("AIRTABLE_BASE_ID"),
            airtable_participants_table: env_or("AIRTABLE_PARTICIPANTS_TABLE", "Participants"),
            airtable_projects_table: env_or("AIRTABLE_PROJECTS_TABLE", "Projects"),
            airtable_participant_id_field: env_or("AIRTABLE_PARTICIPANT_ID_FIELD", "Participant ID"),
            airtable_project_id_field: env_or("AIRTABLE_PROJECT_ID_FIELD", "Project ID"),
            airtable_fraud_approval_field: env_or("AIRTABLE_FRAUD_APPROVAL_FIELD", "Fraud Approval"),
            airtable_sync_interval: Duration::from_secs(
                env::var("AIRTABLE_SYNC_INTERVAL_SECONDS")
                    .ok()
                    .and_then(|value| value.parse().ok())
                    .filter(|seconds: &u64| *seconds > 0)
                    .unwrap_or(300),
            ),
            provider_timeout: Duration::from_secs(10),
        })
    }
}

fn required(name: &str) -> anyhow::Result<String> {
    env::var(name).map_err(|_| anyhow::anyhow!("{name} must be configured"))
}

fn env_or(name: &str, default: &str) -> String {
    env::var(name)
        .unwrap_or_else(|_| default.into())
        .trim_end_matches('/')
        .into()
}

fn optional(name: &str) -> Option<String> {
    env::var(name).ok().map(|value| value.trim().to_owned()).filter(|value| !value.is_empty())
}
