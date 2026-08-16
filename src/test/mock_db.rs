#![allow(
    clippy::unwrap_used,
    clippy::unused_async,
    clippy::must_use_candidate,
    clippy::missing_panics_doc,
    clippy::significant_drop_tightening
)]

use chrono::{DateTime, Utc};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

use crate::domain::{HackClubIdentity, UserRole};

#[derive(Debug, Clone)]
pub struct MockUser {
    pub id: Uuid,
    pub email: String,
    pub first_name: String,
    pub last_name: String,
    pub role: UserRole,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct MockProject {
    pub id: Uuid,
    pub owner_id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub banner_url: Option<String>,
    pub submission_status: String,
    pub submitted_at: Option<DateTime<Utc>>,
    pub shipped_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct MockHackatimeConnection {
    pub user_id: Uuid,
    pub account_id: String,
    pub access_token_ciphertext: String,
    pub connected_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct MockAttendanceRegistration {
    pub id: Uuid,
    pub event_id: Uuid,
    pub user_id: Uuid,
    pub attendee_email: String,
    pub attendee_first_name: String,
    pub attendee_last_name: String,
    pub attend_participant_id: Option<String>,
    pub provider_response: Option<Value>,
    pub status: String,
    pub last_error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Default)]
pub struct MockDatabase {
    pub users: Arc<Mutex<HashMap<Uuid, MockUser>>>,
    pub users_by_email: Arc<Mutex<HashMap<String, Uuid>>>,
    pub projects: Arc<Mutex<HashMap<Uuid, MockProject>>>,
    pub project_hackatime_links: Arc<Mutex<HashMap<Uuid, Vec<String>>>>,
    pub hackatime_connections: Arc<Mutex<HashMap<Uuid, MockHackatimeConnection>>>,
    pub attendance_registrations: Arc<Mutex<HashMap<(Uuid, Uuid), MockAttendanceRegistration>>>,
}

impl MockDatabase {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Upserts a user in the mock database.
    pub fn upsert_hackclub_user(&self, identity: &HackClubIdentity) -> Uuid {
        let mut users = self.users.lock().unwrap();
        let mut by_email = self.users_by_email.lock().unwrap();

        let email = identity.primary_email.clone().unwrap_or_default();
        let first_name = identity.first_name.clone().unwrap_or_default();
        let last_name = identity.last_name.clone().unwrap_or_default();

        if let Some(&existing_id) = by_email.get(&email) {
            if let Some(user) = users.get_mut(&existing_id) {
                user.first_name = first_name;
                user.last_name = last_name;
                user.updated_at = Utc::now();
            }
            existing_id
        } else {
            let id = Uuid::new_v4();
            let user = MockUser {
                id,
                email: email.clone(),
                first_name,
                last_name,
                role: UserRole::User,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            };
            users.insert(id, user);
            by_email.insert(email, id);
            id
        }
    }

    /// Inserts a project in the mock database.
    pub fn create_project(
        &self,
        owner_id: Uuid,
        title: String,
        description: Option<String>,
    ) -> MockProject {
        let mut projects = self.projects.lock().unwrap();
        let id = Uuid::new_v4();
        let now = Utc::now();
        let project = MockProject {
            id,
            owner_id,
            title,
            description,
            banner_url: None,
            submission_status: "draft".into(),
            submitted_at: None,
            shipped_at: None,
            created_at: now,
            updated_at: now,
        };
        projects.insert(id, project.clone());
        project
    }

    /// Links hackatime project names to a project.
    pub fn set_hackatime_projects(&self, project_id: Uuid, names: Vec<String>) {
        let mut links = self.project_hackatime_links.lock().unwrap();
        links.insert(project_id, names);
    }

    /// Saves a hackatime connection for a user.
    pub fn save_hackatime_connection(&self, user_id: Uuid, account_id: String, ciphertext: String) {
        let mut conns = self.hackatime_connections.lock().unwrap();
        let now = Utc::now();
        conns.insert(
            user_id,
            MockHackatimeConnection {
                user_id,
                account_id,
                access_token_ciphertext: ciphertext,
                connected_at: now,
                updated_at: now,
            },
        );
    }

    /// Retrieves a user by ID.
    #[must_use]
    pub fn get_user(&self, id: Uuid) -> Option<MockUser> {
        self.users.lock().unwrap().get(&id).cloned()
    }

    /// Retrieves all projects owned by a user.
    #[must_use]
    pub fn get_user_projects(&self, owner_id: Uuid) -> Vec<MockProject> {
        self.projects
            .lock()
            .unwrap()
            .values()
            .filter(|p| p.owner_id == owner_id)
            .cloned()
            .collect()
    }
}
