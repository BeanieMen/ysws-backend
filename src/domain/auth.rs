use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct LoginQuery {
    pub email: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct OAuthCallbackQuery {
    pub code: Option<String>,
    pub state: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OAuthState {
    pub email: Option<String>,
    pub user_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct HackClubMePayload {
    pub identity: HackClubIdentity,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct HackClubIdentity {
    pub id: String,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub primary_email: Option<String>,
}
