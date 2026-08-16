#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

#[cfg(test)]
use crate::crypto::TokenCipher;
#[cfg(test)]
use crate::domain::{HackClubIdentity, HackatimeProject};
#[cfg(test)]
use crate::test::mock_db::MockDatabase;
#[cfg(test)]
use crate::test::mock_providers::MockProviders;
#[cfg(test)]
use uuid::Uuid;

#[test]
fn test_mock_db_user_and_project_lifecycle() {
    let db = MockDatabase::new();

    let identity = HackClubIdentity {
        id: "hc_123".into(),
        primary_email: Some("alice@example.com".into()),
        first_name: Some("Alice".into()),
        last_name: Some("Smith".into()),
    };

    let user_id = db.upsert_hackclub_user(&identity);
    assert!(db.get_user(user_id).is_some());

    let user = db.get_user(user_id).unwrap();
    assert_eq!(user.email, "alice@example.com");
    assert_eq!(user.first_name, "Alice");

    // Idempotent upsert update
    let updated_identity = HackClubIdentity {
        id: "hc_123".into(),
        primary_email: Some("alice@example.com".into()),
        first_name: Some("Alice-Updated".into()),
        last_name: Some("Smith".into()),
    };
    let updated_id = db.upsert_hackclub_user(&updated_identity);
    assert_eq!(user_id, updated_id);

    let user_updated = db.get_user(user_id).unwrap();
    assert_eq!(user_updated.first_name, "Alice-Updated");

    // Create project
    let project = db.create_project(user_id, "My Hackathon Project".into(), Some("Desc".into()));
    assert_eq!(project.owner_id, user_id);
    assert_eq!(project.title, "My Hackathon Project");

    let projects = db.get_user_projects(user_id);
    assert_eq!(projects.len(), 1);
    assert_eq!(projects[0].id, project.id);
}

#[test]
fn test_mock_db_hackatime_linking() {
    let db = MockDatabase::new();
    let user_id = Uuid::new_v4();
    let project = db.create_project(user_id, "Project Alpha".into(), None);

    let names = vec!["alpha_frontend".to_string(), "alpha_backend".to_string()];
    db.set_hackatime_projects(project.id, names.clone());

    let links = db.project_hackatime_links.lock().unwrap();
    assert_eq!(links.get(&project.id), Some(&names));
    drop(links);
}

#[tokio::test]
async fn test_mock_providers_oauth_flow() {
    let providers = MockProviders::new();

    let identity = HackClubIdentity {
        id: "hc_999".into(),
        primary_email: Some("bob@example.com".into()),
        first_name: Some("Bob".into()),
        last_name: Some("Jones".into()),
    };

    providers.register_hackclub_code("valid_code", identity.clone());

    let res = providers.hackclub_identity("valid_code").await;
    assert!(res.is_ok());
    let fetched = res.unwrap();
    assert_eq!(fetched.primary_email.as_deref(), Some("bob@example.com"));

    let err_res = providers.hackclub_identity("invalid_code").await;
    assert!(err_res.is_err());
}

#[tokio::test]
async fn test_mock_providers_hackatime_projects() {
    let providers = MockProviders::new();
    let token = "token_abc";

    let proj1 = HackatimeProject {
        name: "web_app".into(),
        total_heartbeats: None,
        total_duration: Some(3600.0),
        first_heartbeat: None,
        last_heartbeat: None,
        languages: vec![],
        repo: None,
    };
    let proj2 = HackatimeProject {
        name: "cli_tool".into(),
        total_heartbeats: None,
        total_duration: Some(1800.0),
        first_heartbeat: None,
        last_heartbeat: None,
        languages: vec![],
        repo: None,
    };

    providers.set_hackatime_projects(token, vec![proj1, proj2]);

    let payload = providers.hackatime_projects(token).await.unwrap();
    assert_eq!(payload.projects.len(), 2);
    assert_eq!(payload.projects[0].name, "web_app");
    assert_eq!(payload.projects[0].total_duration, Some(3600.0));
}

#[test]
fn test_mock_db_with_token_cipher() {
    let db = MockDatabase::new();
    let cipher = TokenCipher::new([42; 32]);

    let user_id = Uuid::new_v4();
    let raw_token = "secret_hackatime_access_token_123";
    let encrypted_token = cipher.encrypt(raw_token).unwrap();

    db.save_hackatime_connection(user_id, "acc_456".into(), encrypted_token);

    let conn = db
        .hackatime_connections
        .lock()
        .unwrap()
        .get(&user_id)
        .cloned()
        .unwrap();
    assert_eq!(conn.account_id, "acc_456");

    let decrypted = cipher.decrypt(&conn.access_token_ciphertext).unwrap();
    assert_eq!(decrypted, raw_token);
}
