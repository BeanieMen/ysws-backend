#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::panic,
    clippy::panic_in_result_fn,
    clippy::arithmetic_side_effects,
    clippy::unchecked_time_subtraction,
    clippy::string_slice,
    clippy::too_many_lines,
    clippy::float_cmp,
    clippy::significant_drop_tightening
)]

use crate::{
    adapters::http::{AppState, helpers, router},
    cache::Cache,
    config::Config,
    crypto::TokenCipher,
    domain::HackClubIdentity,
    notifications::Notifications,
    providers::Providers,
};
use axum::{Json, Router, routing::get, routing::post};
use serde_json::{Value, json};
use sqlx::PgPool;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::net::TcpListener;
use uuid::Uuid;

/// The stable id the 0007 migration seeds for the event ticket (2400 minutes).
const TICKET_ITEM_ID: &str = "a4bba639-934d-48f4-9e51-d6328b0a7d54";

/// Spins up a mock Hackatime API (fixed durations per linked project name) and
/// a mock Resend API that records every email it is asked to send.
async fn spawn_mock_upstreams() -> (String, String, Arc<Mutex<Vec<Value>>>) {
    let hackatime_projects = Arc::new(vec![
        json!({"name": "ten_hour_proj", "total_duration": 36_000.0, "languages": []}),
        json!({"name": "thirty_hour_proj", "total_duration": 108_000.0, "languages": []}),
    ]);
    let projects_for_route = Arc::clone(&hackatime_projects);
    let hackatime_app = Router::new().route(
        "/api/v1/authenticated/projects",
        get(move || {
            let projects = Arc::clone(&projects_for_route);
            async move { Json(projects.as_ref().clone()) }
        }),
    );
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let hackatime_base = format!("http://{addr}");
    tokio::spawn(async move {
        axum::serve(listener, hackatime_app).await.unwrap();
    });

    let emails: Arc<Mutex<Vec<Value>>> = Arc::default();
    let captured = Arc::clone(&emails);
    let resend_app = Router::new().route(
        "/emails",
        post(move |Json(body): Json<Value>| async move {
            captured.lock().unwrap().push(body);
            Json(json!({"id": "mock-email-id"}))
        }),
    );
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let resend_base = format!("http://{addr}/emails");
    tokio::spawn(async move {
        axum::serve(listener, resend_app).await.unwrap();
    });

    (hackatime_base, resend_base, emails)
}

fn test_config(
    database_url: String,
    redis_url: String,
    hackatime_base_url: String,
    resend_base_url: String,
) -> Config {
    Config {
        database_url,
        redis_url,
        app_url: "http://localhost:3000".into(),
        backend_url: "http://localhost:8000".into(),
        port: 0,
        encryption_key: [42; 32],
        hackatime_api_base_url: hackatime_base_url,
        hackclub_client_id: "test-client".into(),
        hackclub_client_secret: "test-secret".into(),
        hackclub_redirect_uri: "http://localhost:3000/auth/hackclub/callback".into(),
        hackatime_client_id: "test-client".into(),
        hackatime_client_secret: "test-secret".into(),
        hackatime_redirect_uri: "http://localhost:3000/auth/hackatime/callback".into(),
        cookie_secure: false,
        resend_api_token: Some("test-resend-token".into()),
        resend_from_email: Some("test-instance@example.com".into()),
        resend_api_base_url: resend_base_url,
        slack_bot_token: None,
        slack_channel_id: None,
        lapse_api_base_url: "https://api.lapse.hackclub.com".into(),
        lapse_api_token: None,
        airtable_api_key: None,
        airtable_base_id: None,
        airtable_participants_table: "Participants".into(),
        airtable_projects_table: "Projects".into(),
        airtable_participant_id_field: "Participant ID".into(),
        airtable_project_id_field: "Project ID".into(),
        airtable_fraud_approval_field: "Fraud Approval".into(),
        airtable_sync_interval: Duration::from_secs(30),
        provider_timeout: Duration::from_secs(5),
    }
}

fn env_or_none(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|value| !value.is_empty())
}

async fn request(
    client: &reqwest::Client,
    base: &str,
    cookie: &str,
    method: reqwest::Method,
    path: &str,
    body: Option<Value>,
    idempotency_key: Option<&str>,
) -> reqwest::Response {
    let mut req = client
        .request(method.clone(), format!("{base}{path}"))
        .header("cookie", cookie);
    // The CSRF middleware requires unsafe methods to carry a matching Origin.
    if matches!(
        method,
        reqwest::Method::POST
            | reqwest::Method::PUT
            | reqwest::Method::PATCH
            | reqwest::Method::DELETE
    ) {
        req = req.header("origin", "http://localhost:3000");
    }
    if let Some(body) = body {
        req = req.json(&body);
    }
    if let Some(key) = idempotency_key {
        req = req.header("idempotency-key", key);
    }
    req.send().await.unwrap()
}

async fn expect_status(response: reqwest::Response, expected: reqwest::StatusCode) -> Value {
    let status = response.status();

    assert_eq!(
        status,
        expected,
        "unexpected status; body: {}",
        response.text().await.unwrap()
    );
    if status == reqwest::StatusCode::NO_CONTENT {
        Value::Null
    } else {
        response.json().await.unwrap()
    }
}

fn cookie_for(token: &str) -> String {
    format!("session={token}")
}

/// End-to-end approved-hours flow against a real Postgres and Redis.
///
/// Requires `TEST_DATABASE_URL` (falls back to `DATABASE_URL`) and
/// `TEST_REDIS_URL` (falls back to `REDIS_URL`); the test skips when either is
/// missing so unit-test-only environments stay green. The project `.env` is
/// intentionally not loaded: its `DATABASE_URL` points at the container
/// network, which is unreachable from the test runner.
#[tokio::test]
async fn approved_hours_flow() -> anyhow::Result<()> {
    let Some(database_url) =
        env_or_none("TEST_DATABASE_URL").or_else(|| env_or_none("DATABASE_URL"))
    else {
        eprintln!("skipping approved_hours_flow: TEST_DATABASE_URL is not set");
        return Ok(());
    };
    let Some(redis_url) = env_or_none("TEST_REDIS_URL").or_else(|| env_or_none("REDIS_URL")) else {
        eprintln!("skipping approved_hours_flow: TEST_REDIS_URL is not set");
        return Ok(());
    };

    let db: PgPool = crate::database::connect_and_migrate(&database_url).await?;
    sqlx::query("TRUNCATE sessions, users RESTART IDENTITY CASCADE")
        .execute(&db)
        .await?;

    let (hackatime_base_url, resend_base_url, sent_emails) = spawn_mock_upstreams().await;
    let config = test_config(database_url, redis_url, hackatime_base_url, resend_base_url);
    let state = AppState {
        db,
        cache: Cache::connect(&config.redis_url).await?,
        cipher: TokenCipher::new(config.encryption_key),
        providers: Providers::new(config.clone())?,
        notifications: Notifications::new(&config)?,
        app_url: config.app_url.clone(),
        cookie_secure: config.cookie_secure,
    };
    let app = router(state.clone());
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let base = format!("http://{}", listener.local_addr()?);
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()?;

    // -- Users ---------------------------------------------------------------
    let identity = HackClubIdentity {
        id: "hc_flow_user".into(),
        primary_email: Some("alice@example.com".into()),
        first_name: Some("Alice".into()),
        last_name: Some("Smith".into()),
    };
    let user = helpers::upsert_hackclub_user(&state.db, identity).await?;
    let user_token = helpers::create_session(&state.db, user.id).await?;
    let user_cookie = cookie_for(&user_token);

    let reviewer_id = Uuid::new_v4();
    sqlx::query("INSERT INTO users (id, hca_id, email, first_name, last_name, role) VALUES ($1, $2, $3, $4, $5, 'reviewer')")
        .bind(reviewer_id)
        .bind("hc_flow_reviewer")
        .bind("reviewer@example.com")
        .bind("Reed")
        .bind("Viewer")
        .execute(&state.db)
        .await?;
    let reviewer_token = helpers::create_session(&state.db, reviewer_id).await?;
    let reviewer_cookie = cookie_for(&reviewer_token);

    let encrypted_token = state.cipher.encrypt("test-access-token")?;
    sqlx::query(
        "INSERT INTO hackatime_connections (user_id, account_id, access_token_ciphertext) VALUES ($1, 'acc_flow', $2)",
    )
    .bind(user.id)
    .bind(encrypted_token)
    .execute(&state.db)
    .await?;

    // -- Phase 1: create + link a 10-hour project ---------------------------
    let body = expect_status(
        request(
            &client,
            &base,
            &user_cookie,
            reqwest::Method::POST,
            "/api/v1/projects",
            Some(json!({"title": "Ten-hour project", "description": "built a compiler"})),
            None,
        )
        .await,
        reqwest::StatusCode::CREATED,
    )
    .await;

    let project_10h: Uuid = body["id"].as_str().unwrap().parse().unwrap();

    expect_status(
        request(
            &client,
            &base,
            &user_cookie,
            reqwest::Method::PUT,
            &format!("/api/v1/projects/{project_10h}/hackatime-projects"),
            Some(json!({"names": ["ten_hour_proj"]})),
            None,
        )
        .await,
        reqwest::StatusCode::NO_CONTENT,
    )
    .await;

    // -- Phase 2: create + link a 30-hour project ---------------------------
    let body = expect_status(
        request(
            &client,
            &base,
            &user_cookie,
            reqwest::Method::POST,
            "/api/v1/projects",
            Some(json!({"title": "Thirty-hour project", "description": "soldered a keyboard"})),
            None,
        )
        .await,
        reqwest::StatusCode::CREATED,
    )
    .await;

    let project_30h: Uuid = body["id"].as_str().unwrap().parse().unwrap();

    expect_status(
        request(
            &client,
            &base,
            &user_cookie,
            reqwest::Method::PUT,
            &format!("/api/v1/projects/{project_30h}/hackatime-projects"),
            Some(json!({"names": ["thirty_hour_proj"]})),
            None,
        )
        .await,
        reqwest::StatusCode::NO_CONTENT,
    )
    .await;

    // -- Phase 3: dashboard reflects 40h tracked ----------------------------
    let dashboard = expect_status(
        request(
            &client,
            &base,
            &user_cookie,
            reqwest::Method::GET,
            "/api/v1/projects",
            None,
            None,
        )
        .await,
        reqwest::StatusCode::OK,
    )
    .await;

    let total_seconds: f64 = dashboard["total_seconds"].as_f64().unwrap();
    assert_eq!(total_seconds, 144_000.0, "expected 40h total tracked");
    let per_project: std::collections::HashMap<String, f64> = dashboard["projects"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| {
            (
                p["id"].as_str().unwrap().to_string(),
                p["total_seconds"].as_f64().unwrap(),
            )
        })
        .collect();
    assert_eq!(per_project.get(&project_10h.to_string()), Some(&36_000.0));
    assert_eq!(per_project.get(&project_30h.to_string()), Some(&108_000.0));

    // -- Phase 4: ship both, reviewer approves both -------------------------
    for project_id in [project_10h, project_30h] {
        expect_status(
            request(
                &client,
                &base,
                &user_cookie,
                reqwest::Method::POST,
                &format!("/api/v1/projects/{project_id}/ship"),
                None,
                None,
            )
            .await,
            reqwest::StatusCode::OK,
        )
        .await;

        expect_status(
            request(
                &client,
                &base,
                &reviewer_cookie,
                reqwest::Method::POST,
                &format!("/api/v1/projects/{project_id}/reviews"),
                Some(json!({"status": "approved", "comment": null})),
                None,
            )
            .await,
            reqwest::StatusCode::CREATED,
        )
        .await;
    }

    // Reviewer approval alone must not credit anything.
    let account = expect_status(
        request(
            &client,
            &base,
            &user_cookie,
            reqwest::Method::GET,
            "/api/v1/shop/me",
            None,
            None,
        )
        .await,
        reqwest::StatusCode::OK,
    )
    .await;

    assert_eq!(
        account["available_hours"].as_f64().unwrap(),
        0.0,
        "no credit before fraud approval"
    );

    // -- Phase 5: fraud approval (what the Airtable sync applies) -----------
    for project_id in [project_10h, project_30h] {
        sqlx::query(
            "UPDATE project_shipments SET fraud_approval_status = 'approved', fraud_reviewed_at = now(), updated_at = now() WHERE project_id = $1",
        )
        .bind(project_id)
        .execute(&state.db)
        .await?;
        let credited = crate::approved_hours::award_if_fully_approved(&state, project_id).await?;
        assert!(
            credited.is_some(),
            "award must fire once both approvals are granted"
        );
    }

    let awards: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM project_credit_awards WHERE user_id = $1")
            .bind(user.id)
            .fetch_one(&state.db)
            .await?;
    assert_eq!(awards, 2, "one immutable credit per project");

    let account = expect_status(
        request(
            &client,
            &base,
            &user_cookie,
            reqwest::Method::GET,
            "/api/v1/shop/me",
            None,
            None,
        )
        .await,
        reqwest::StatusCode::OK,
    )
    .await;

    assert_eq!(
        account["available_hours"].as_f64().unwrap(),
        40.0,
        "10h + 30h credited to the wallet"
    );

    // -- Phase 6: claim the event ticket ------------------------------------
    let key = "flow-test-idempotency-key-0001";
    let purchase = expect_status(
        request(
            &client,
            &base,
            &user_cookie,
            reqwest::Method::POST,
            &format!("/api/v1/shop/items/{TICKET_ITEM_ID}/purchase"),
            None,
            Some(key),
        )
        .await,
        reqwest::StatusCode::CREATED,
    )
    .await;

    assert_eq!(purchase["available_hours"].as_f64().unwrap(), 0.0);

    let account = expect_status(
        request(
            &client,
            &base,
            &user_cookie,
            reqwest::Method::GET,
            "/api/v1/shop/me",
            None,
            None,
        )
        .await,
        reqwest::StatusCode::OK,
    )
    .await;

    assert_eq!(account["available_hours"].as_f64().unwrap(), 0.0);
    let purchases = account["purchases"].as_array().unwrap();
    assert_eq!(purchases.len(), 1, "one ticket purchase recorded");
    assert_eq!(
        purchases[0]["item_id"].as_str().unwrap(),
        TICKET_ITEM_ID,
        "the claimed item is the event ticket"
    );

    // Replaying the same idempotency key must be safe and never double-debit.
    let replay = expect_status(
        request(
            &client,
            &base,
            &user_cookie,
            reqwest::Method::POST,
            &format!("/api/v1/shop/items/{TICKET_ITEM_ID}/purchase"),
            None,
            Some(key),
        )
        .await,
        reqwest::StatusCode::OK,
    )
    .await;

    assert_eq!(replay["purchase_id"], purchase["purchase_id"]);
    let wallet_minutes: i64 =
        sqlx::query_scalar("SELECT available_minutes FROM user_wallets WHERE user_id = $1")
            .bind(user.id)
            .fetch_one(&state.db)
            .await?;
    assert_eq!(wallet_minutes, 0, "replay must not debit twice");

    // -- Phase 7: confirmation email ----------------------------------------
    let (sent_at, last_error): (Option<chrono::DateTime<chrono::Utc>>, Option<String>) = sqlx::query_as(
        "SELECT sent_at, last_error FROM notification_outbox WHERE kind = 'ticket_purchase_confirmation'",
    )
    .fetch_one(&state.db)
    .await?;
    assert!(sent_at.is_some(), "email must be marked sent in the outbox");
    assert!(
        last_error.is_none(),
        "email must not have recorded an error"
    );

    {
        let emails = sent_emails.lock().unwrap();
        assert_eq!(emails.len(), 1, "exactly one confirmation email sent");
        let email = emails.first().unwrap();
        assert_eq!(
            email["subject"], "Your event ticket is confirmed!",
            "email subject"
        );
        assert_eq!(email["to"], json!(["alice@example.com"]), "email recipient");
        assert_eq!(email["from"], "test-instance@example.com", "email sender");
    }

    // The award stays idempotent even when retried.
    let retry = crate::approved_hours::award_if_fully_approved(&state, project_10h).await?;
    assert_eq!(retry, None, "a second award attempt must not re-credit");

    Ok(())
}
