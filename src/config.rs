use std::{env, time::Duration};

#[derive(Clone)]
pub struct Config {
    pub database_url: String,
    pub redis_url: String,
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
    pub provider_timeout: Duration,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        dotenvy::dotenv().ok();
        let encryption_key = env::var("APP_ENCRYPTION_KEY")
            .map_err(|_| anyhow::anyhow!("APP_ENCRYPTION_KEY must be configured"))?;
        let bytes = hex::decode(encryption_key)
            .map_err(|_| anyhow::anyhow!("APP_ENCRYPTION_KEY must be hexadecimal"))?;
        let encryption_key: [u8; 32] = bytes
            .try_into()
            .map_err(|_| anyhow::anyhow!("APP_ENCRYPTION_KEY must be exactly 32 bytes"))?;

        Ok(Self {
            database_url: required("DATABASE_URL")?,
            redis_url: required("REDIS_URL")?,
            port: env::var("PORT").unwrap_or_else(|_| "3000".into()).parse()?,
            encryption_key,
            attend_api_base_url: env_or("ATTEND_API_BASE_URL", "https://attend.hackclub.com"),
            attend_api_key: env_or("ATTEND_API_KEY", ""),
            hackatime_api_base_url: env_or(
                "HACKATIME_API_BASE_URL",
                "https://hackatime.hackclub.com",
            ),
            hackclub_client_id: required("HACKCLUB_CLIENT_ID")?,
            hackclub_client_secret: required("HACKCLUB_CLIENT_SECRET")?,
            hackclub_redirect_uri: required("HACKCLUB_REDIRECT_URI")?,
            hackatime_client_id: required("HACKATIME_CLIENT_ID")?,
            hackatime_client_secret: required("HACKATIME_CLIENT_SECRET")?,
            hackatime_redirect_uri: required("HACKATIME_REDIRECT_URI")?,
            cookie_secure: env::var("COOKIE_SECURE")
                .map(|value| value == "true" || value == "1")
                .unwrap_or(false),
            lapse_api_base_url: env_or("LAPSE_API_BASE_URL", "https://api.lapse.hackclub.com"),
            lapse_api_token: env::var("LAPSE_API_TOKEN").ok().filter(|s| !s.is_empty()),
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
