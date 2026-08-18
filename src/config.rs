use std::{env, time::Duration};

#[derive(Clone)]
pub struct Config {
    pub database_url: String,
    pub redis_url: String,
    pub app_url: String,
    pub backend_url: String,
    pub port: u16,
    pub encryption_key: [u8; 32],
    pub hackatime_api_base_url: String,
    pub hackclub_client_id: String,
    pub hackclub_client_secret: String,
    pub hackclub_redirect_uri: String,
    pub hackatime_client_id: String,
    pub hackatime_client_secret: String,
    pub hackatime_redirect_uri: String,
    pub cookie_secure: bool,
    pub resend_api_token: Option<String>,
    pub resend_from_email: Option<String>,
    pub resend_api_base_url: String,
    pub slack_bot_token: Option<String>,
    pub slack_channel_id: Option<String>,
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
        let encryption_key = encryption_key.trim();
        if encryption_key.is_empty()
            || encryption_key.eq_ignore_ascii_case("replace-with-64-hex-characters")
        {
            anyhow::bail!(
                "APP_ENCRYPTION_KEY must be set to a unique 64-character hexadecimal key"
            );
        }

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
            app_url,
            backend_url,
            port: env::var("PORT").unwrap_or_else(|_| "8000".into()).parse()?,
            encryption_key,
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
            // Secure cookies are the safe default. Local HTTP development can opt out
            // explicitly with COOKIE_SECURE=false.
            cookie_secure: optional_bool("COOKIE_SECURE")?.unwrap_or(true),
            resend_api_token: optional("RESEND_API_TOKEN"),
            resend_from_email: optional("RESEND_FROM_EMAIL"),
            resend_api_base_url: env_or("RESEND_API_BASE_URL", "https://api.resend.com/emails"),
            slack_bot_token: optional("SLACK_BOT_TOKEN"),
            slack_channel_id: optional("SLACK_CHANNEL_ID"),
            lapse_api_base_url: env_or("LAPSE_API_BASE_URL", "https://api.lapse.hackclub.com"),
            lapse_api_token: env::var("LAPSE_API_TOKEN").ok().filter(|s| !s.is_empty()),
            airtable_api_key: optional("AIRTABLE_API_KEY"),
            airtable_base_id: optional("AIRTABLE_BASE_ID"),
            airtable_participants_table: env_or("AIRTABLE_PARTICIPANTS_TABLE", "Participants"),
            airtable_projects_table: env_or("AIRTABLE_PROJECTS_TABLE", "Projects"),
            airtable_participant_id_field: env_or(
                "AIRTABLE_PARTICIPANT_ID_FIELD",
                "Participant ID",
            ),
            airtable_project_id_field: env_or("AIRTABLE_PROJECT_ID_FIELD", "Project ID"),
            airtable_fraud_approval_field: env_or(
                "AIRTABLE_FRAUD_APPROVAL_FIELD",
                "Fraud Approval",
            ),
            airtable_sync_interval: Duration::from_secs(
                env::var("AIRTABLE_SYNC_INTERVAL_SECONDS")
                    .ok()
                    .and_then(|value| value.parse().ok())
                    .filter(|seconds: &u64| *seconds > 0)
                    .unwrap_or(30),
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
    env::var(name)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn optional_bool(name: &str) -> anyhow::Result<Option<bool>> {
    let Some(value) = optional(name) else {
        return Ok(None);
    };
    match value.to_ascii_lowercase().as_str() {
        "true" | "1" => Ok(Some(true)),
        "false" | "0" => Ok(Some(false)),
        _ => anyhow::bail!("{name} must be true or false"),
    }
}
