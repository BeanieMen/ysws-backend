use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "text", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum UserRole {
    User,
    Reviewer,
    Admin,
}

impl Default for UserRole {
    fn default() -> Self {
        Self::User
    }
}

impl fmt::Display for UserRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::User => write!(f, "user"),
            Self::Reviewer => write!(f, "reviewer"),
            Self::Admin => write!(f, "admin"),
        }
    }
}

impl std::str::FromStr for UserRole {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_lowercase().as_str() {
            "user" => Ok(Self::User),
            "reviewer" => Ok(Self::Reviewer),
            "admin" => Ok(Self::Admin),
            _ => Err(format!("invalid role: {s}")),
        }
    }
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct SessionUser {
    pub id: Uuid,
    pub email: String,
    pub first_name: String,
    pub last_name: String,
    pub role: UserRole,
}

#[derive(Debug, Serialize)]
pub struct CurrentUserResponse {
    pub id: Uuid,
    pub email: String,
    pub first_name: String,
    pub last_name: String,
    pub role: UserRole,
    pub hackatime_connected: bool,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct AdminUserResponse {
    pub id: Uuid,
    pub email: String,
    pub first_name: String,
    pub last_name: String,
    pub role: UserRole,
    pub hca_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateUserRoleRequest {
    pub role: UserRole,
}

#[derive(Debug, Deserialize)]
pub struct AdminUpdateUserRequest {
    pub email: Option<String>,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub role: Option<UserRole>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_and_deserializes_user_roles() {
        let role = UserRole::Admin;
        let json = serde_json::to_string(&role).unwrap();
        assert_eq!(json, "\"admin\"");
        let parsed: UserRole = serde_json::from_str("\"reviewer\"").unwrap();
        assert_eq!(parsed, UserRole::Reviewer);
    }
}
