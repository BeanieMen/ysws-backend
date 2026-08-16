use crate::{
    adapters::http::{
        AppState,
        helpers::{current_user, idempotency_cache_key, user_hackatime_projects},
    },
    domain::{AttendanceRegistrationResponse, HackatimeProjectsPayload, RegisterAttendanceRequest},
    error::{ApiError, ApiResult},
};
use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, HeaderName},
};
use sqlx::Row;
use std::{sync::Arc, time::Duration};
use uuid::Uuid;

pub async fn list_hackatime_projects(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> ApiResult<Json<HackatimeProjectsPayload>> {
    let user_id = current_user(&state, &headers).await?;
    Ok(Json(user_hackatime_projects(&state, user_id).await?))
}

pub async fn register_attendance(
    State(state): State<Arc<AppState>>,
    Path(event_id): Path<Uuid>,
    headers: HeaderMap,
) -> ApiResult<Json<AttendanceRegistrationResponse>> {
    let user_id = current_user(&state, &headers).await?;
    if state
        .cache
        .increment_with_ttl(
            &format!("ratelimit:attendance:{user_id}"),
            Duration::from_mins(1),
        )
        .await
        .is_some_and(|count| count > 10)
    {
        return Err(ApiError::Conflict(
            "too many attendance registration attempts; retry in a minute".into(),
        ));
    }
    let idempotency_key = headers
        .get(HeaderName::from_static("idempotency-key"))
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    if let Some(key) = idempotency_key.as_deref() {
        let cache_key = idempotency_cache_key(user_id, event_id, key);
        if let Some(cached) = state.cache.get_json(&cache_key).await {
            return Ok(Json(cached));
        }
    }
    let user = sqlx::query("SELECT email, first_name, last_name FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(|| ApiError::NotFound("user not found".into()))?;
    let attendee = RegisterAttendanceRequest {
        email: user.get("email"),
        first_name: user.get("first_name"),
        last_name: user.get("last_name"),
    };
    let lock_key = format!("lock:attendance:{event_id}:{user_id}");
    let lock_token = Uuid::new_v4().to_string();
    if !state
        .cache
        .take_lock(&lock_key, &lock_token, Duration::from_secs(30))
        .await
    {
        return Err(ApiError::Conflict(
            "attendance registration is already in progress".into(),
        ));
    }
    let result = register_attendance_locked(&state, event_id, user_id, attendee).await;
    state.cache.release_lock(&lock_key, &lock_token).await;
    let response = result?;
    if let Some(key) = idempotency_key {
        state
            .cache
            .set_json(
                &idempotency_cache_key(user_id, event_id, &key),
                &response,
                Duration::from_hours(24),
            )
            .await;
    }
    Ok(Json(response))
}

async fn register_attendance_locked(
    state: &AppState,
    event_id: Uuid,
    user_id: Uuid,
    attendee: RegisterAttendanceRequest,
) -> ApiResult<AttendanceRegistrationResponse> {
    let mut registration_id = Uuid::new_v4();
    let inserted = sqlx::query("INSERT INTO attendance_registrations (id, event_id, user_id, attendee_email, attendee_first_name, attendee_last_name, status) VALUES ($1, $2, $3, $4, $5, $6, 'pending') ON CONFLICT (event_id, user_id) DO NOTHING")
        .bind(registration_id).bind(event_id).bind(user_id).bind(&attendee.email).bind(&attendee.first_name).bind(&attendee.last_name).execute(&state.db).await?.rows_affected() == 1;
    if !inserted {
        let row = sqlx::query("SELECT id, status, attend_participant_id FROM attendance_registrations WHERE event_id = $1 AND user_id = $2").bind(event_id).bind(user_id).fetch_one(&state.db).await?;
        let status: String = row.get("status");
        if status == "registered" {
            return Ok(AttendanceRegistrationResponse {
                registration_id: row.get("id"),
                event_id,
                status,
                participant_id: row.get("attend_participant_id"),
            });
        }
        if status == "pending" {
            return Err(ApiError::Conflict(
                "attendance registration is already in progress".into(),
            ));
        }
        registration_id = row.get("id");
        sqlx::query("UPDATE attendance_registrations SET status = 'pending', last_error = NULL, updated_at = now() WHERE event_id = $1 AND user_id = $2").bind(event_id).bind(user_id).execute(&state.db).await?;
    }
    match state
        .providers
        .register_attendance(event_id, &attendee)
        .await
    {
        Ok((participant_id, provider_response)) => {
            sqlx::query("UPDATE attendance_registrations SET status = 'registered', attend_participant_id = $1, provider_response = $2, updated_at = now() WHERE event_id = $3 AND user_id = $4")
                .bind(&participant_id).bind(provider_response).bind(event_id).bind(user_id).execute(&state.db).await?;
            Ok(AttendanceRegistrationResponse {
                registration_id,
                event_id,
                status: "registered".into(),
                participant_id,
            })
        }
        Err(error) => {
            let message = error.to_string();
            sqlx::query("UPDATE attendance_registrations SET status = 'failed', last_error = $1, updated_at = now() WHERE event_id = $2 AND user_id = $3").bind(&message).bind(event_id).bind(user_id).execute(&state.db).await?;
            Err(error)
        }
    }
}
