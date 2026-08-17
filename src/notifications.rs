use crate::config::Config;
use reqwest::Client;
use serde_json::Value;

/// Best-effort transactional email and Slack notifications.
#[derive(Clone)]
pub struct Notifications {
    client: Client,
    resend_api_token: Option<String>,
    resend_from_email: Option<String>,
    slack_bot_token: Option<String>,
    slack_channel_id: Option<String>,
}

impl Notifications {
    /// Creates notification clients from optional environment configuration.
    pub fn new(config: &Config) -> anyhow::Result<Self> {
        Ok(Self {
            client: Client::builder().timeout(config.provider_timeout).build()?,
            resend_api_token: config.resend_api_token.clone(),
            resend_from_email: config.resend_from_email.clone(),
            slack_bot_token: config.slack_bot_token.clone(),
            slack_channel_id: config.slack_channel_id.clone(),
        })
    }

    /// Posts a best-effort service-started notification to Slack.
    pub async fn backend_started(&self) -> anyhow::Result<()> {
        self.post_slack("🟢 papa backend is up.").await
    }

    /// Sends notifications for a newly created Hack Club account.
    pub async fn new_user_signed_up(&self, email: &str) -> anyhow::Result<()> {
        if let Err(error) = self.send_welcome_email(email).await {
            tracing::warn!(error = %error, "failed to send welcome email");
        }
        if let Err(error) = self.post_slack("🎉 A new user signed up.").await {
            tracing::warn!(error = %error, "failed to post new-user notification to Slack");
        }
        Ok(())
    }

    async fn send_welcome_email(&self, email: &str) -> anyhow::Result<()> {
        let (Some(token), Some(from)) = (&self.resend_api_token, &self.resend_from_email) else {
            return Ok(());
        };
        let response = self
            .client
            .post("https://api.resend.com/emails")
            .bearer_auth(token)
            .json(&serde_json::json!({
                "from": from,
                "to": [email],
                "subject": "Welcome to papa!",
                "text": "Welcome to papa! Your account is ready—start building and tracking your project time.",
            }))
            .send()
            .await?;
        if response.status().is_success() {
            Ok(())
        } else {
            anyhow::bail!("Resend returned {}", response.status())
        }
    }

    async fn post_slack(&self, text: &str) -> anyhow::Result<()> {
        let (Some(token), Some(channel)) = (&self.slack_bot_token, &self.slack_channel_id) else {
            return Ok(());
        };
        let response = self
            .client
            .post("https://slack.com/api/chat.postMessage")
            .bearer_auth(token)
            .json(&serde_json::json!({ "channel": channel, "text": text }))
            .send()
            .await?;
        let status = response.status();
        let body: Value = response.json().await?;
        if status.is_success() && body.get("ok").and_then(Value::as_bool) == Some(true) {
            Ok(())
        } else {
            anyhow::bail!("Slack returned {status}")
        }
    }
}
