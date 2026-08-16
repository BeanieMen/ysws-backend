use crate::{
    adapters::http::AppState,
    error::{ApiError, ApiResult},
    providers::{AirtableParticipantSync, AirtableProjectSync},
};
use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::FromRow;
use tracing::{Instrument, debug, info, info_span, warn};
use uuid::Uuid;

#[derive(Debug, Serialize)]
pub struct AirtableSyncReport {
    pub participants_synced: usize,
    pub shipped_projects_synced: usize,
    pub fraud_statuses_updated: usize,
}

#[derive(FromRow)]
struct ParticipantRow {
    id: Uuid,
    email: String,
    first_name: String,
    last_name: String,
    airtable_participant_record_id: Option<String>,
}

#[derive(FromRow)]
struct ShippedProjectRow {
    id: Uuid,
    owner_id: Uuid,
    owner_email: String,
    title: String,
    description: Option<String>,
    shipped_at: DateTime<Utc>,
    project_approval_status: String,
    fraud_approval_status: String,
    airtable_project_record_id: Option<String>,
}

/// Synchronizes local participants and shipped projects to Airtable, then
/// imports Airtable's fraud decision into `project_shipments`.
///
/// Data flow:
///   - Participants:       local  →  Airtable  (upsert)
///   - Shipped projects:   local  →  Airtable  (upsert, fraud field excluded)
///   - Fraud status:       Airtable  →  local   (Airtable is authoritative)
pub async fn sync(state: &AppState) -> ApiResult<AirtableSyncReport> {
    if !state.providers.airtable_configured() {
        return Err(ApiError::BadRequest(
            "Airtable sync requires AIRTABLE_API_KEY and AIRTABLE_BASE_ID".into(),
        ));
    }

    info!("starting Airtable sync");

    // ── Phase 1: push participants ──────────────────────────────────────────
    let participant_count = sync_participants(state)
        .instrument(info_span!("sync_participants"))
        .await?;

    // ── Phase 2: push shipped projects (fraud field excluded) ───────────────
    let project_count = sync_projects(state)
        .instrument(info_span!("sync_projects"))
        .await?;

    // ── Phase 3: pull fraud decisions back from Airtable ────────────────────
    let fraud_statuses_updated = sync_fraud_statuses(state)
        .instrument(info_span!("sync_fraud_statuses"))
        .await?;

    info!(
        participants_synced = participant_count,
        shipped_projects_synced = project_count,
        fraud_statuses_updated,
        "Airtable sync complete"
    );

    Ok(AirtableSyncReport {
        participants_synced: participant_count,
        shipped_projects_synced: project_count,
        fraud_statuses_updated,
    })
}

async fn sync_participants(state: &AppState) -> ApiResult<usize> {
    let participants = sqlx::query_as::<_, ParticipantRow>(
        "SELECT id, email, first_name, last_name, airtable_participant_record_id \
         FROM users ORDER BY created_at",
    )
    .fetch_all(&state.db)
    .await?;

    let total = participants.len();
    info!(total, "fetched participants from DB");

    for participant in participants {
        let is_new = participant.airtable_participant_record_id.is_none();
        debug!(
            participant_id = %participant.id,
            email = %participant.email,
            is_new,
            "upserting participant to Airtable"
        );

        let record_id = state
            .providers
            .upsert_airtable_participant(&AirtableParticipantSync {
                id: participant.id,
                email: participant.email.clone(),
                first_name: participant.first_name.clone(),
                last_name: participant.last_name.clone(),
                record_id: participant.airtable_participant_record_id.clone(),
            })
            .await
            .map_err(|error| {
                warn!(
                    participant_id = %participant.id,
                    email = %participant.email,
                    %error,
                    "failed to upsert participant to Airtable"
                );
                error
            })?;

        sqlx::query(
            "UPDATE users SET airtable_participant_record_id = $1, updated_at = now() WHERE id = $2",
        )
        .bind(&record_id)
        .bind(participant.id)
        .execute(&state.db)
        .await?;

        debug!(
            participant_id = %participant.id,
            airtable_record_id = %record_id,
            is_new,
            "participant synced"
        );
    }

    info!(total, "participants phase complete");
    Ok(total)
}

async fn sync_projects(state: &AppState) -> ApiResult<usize> {
    let projects = sqlx::query_as::<_, ShippedProjectRow>(
        "SELECT p.id, p.owner_id, u.email AS owner_email, p.title, p.description, \
                s.shipped_at, s.project_approval_status, s.fraud_approval_status, \
                s.airtable_project_record_id \
         FROM project_shipments s \
         JOIN projects p ON p.id = s.project_id \
         JOIN users u ON u.id = p.owner_id \
         WHERE s.shipped_at IS NOT NULL \
         ORDER BY s.shipped_at",
    )
    .fetch_all(&state.db)
    .await?;

    let total = projects.len();
    info!(total, "fetched shipped projects from DB");

    for project in projects {
        let is_new = project.airtable_project_record_id.is_none();
        debug!(
            project_id = %project.id,
            title = %project.title,
            project_approval_status = %project.project_approval_status,
            // fraud_approval_status is NOT pushed to Airtable (Airtable owns it)
            is_new,
            "upserting project to Airtable"
        );

        let record_id = state
            .providers
            .upsert_airtable_project(&AirtableProjectSync {
                id: project.id,
                owner_id: project.owner_id,
                owner_email: project.owner_email.clone(),
                title: project.title.clone(),
                description: project.description.clone(),
                shipped_at: project.shipped_at,
                project_approval_status: project.project_approval_status.clone(),
                fraud_approval_status: project.fraud_approval_status.clone(),
                record_id: project.airtable_project_record_id.clone(),
            })
            .await
            .map_err(|error| {
                warn!(
                    project_id = %project.id,
                    title = %project.title,
                    %error,
                    "failed to upsert project to Airtable"
                );
                error
            })?;

        sqlx::query(
            "UPDATE project_shipments \
             SET airtable_project_record_id = $1, airtable_synced_at = now(), updated_at = now() \
             WHERE project_id = $2",
        )
        .bind(&record_id)
        .bind(project.id)
        .execute(&state.db)
        .await?;

        debug!(
            project_id = %project.id,
            airtable_record_id = %record_id,
            is_new,
            "project synced"
        );
    }

    info!(total, "projects phase complete");
    Ok(total)
}

async fn sync_fraud_statuses(state: &AppState) -> ApiResult<usize> {
    let records = state
        .providers
        .airtable_fraud_statuses()
        .await
        .map_err(|error| {
            warn!(%error, "failed to fetch fraud statuses from Airtable");
            error
        })?;

    let fetched = records.len();
    info!(fetched, "fetched fraud status records from Airtable");

    let mut updated: usize = 0;
    let mut skipped_invalid_id: usize = 0;
    let mut skipped_invalid_status: usize = 0;

    for record in records {
        let Ok(project_id) = Uuid::parse_str(&record.project_id) else {
            warn!(
                project_id = %record.project_id,
                airtable_record_id = %record.record_id,
                "skipping Airtable record: invalid project UUID"
            );
            skipped_invalid_id = skipped_invalid_id.saturating_add(1);
            continue;
        };

        let Some(status) = normalized_fraud_status(&record.status) else {
            warn!(
                %project_id,
                raw_status = %record.status,
                airtable_record_id = %record.record_id,
                "skipping Airtable record: unrecognised fraud status value"
            );
            skipped_invalid_status = skipped_invalid_status.saturating_add(1);
            continue;
        };

        debug!(
            %project_id,
            fraud_status = status,
            airtable_record_id = %record.record_id,
            "writing fraud status to DB"
        );

        let result = sqlx::query(
            "UPDATE project_shipments \
             SET fraud_approval_status = $1, \
                 fraud_reviewed_at = CASE \
                     WHEN fraud_approval_status IS DISTINCT FROM $1 THEN now() \
                     ELSE fraud_reviewed_at \
                 END, \
                 airtable_project_record_id = $2, \
                 airtable_synced_at = now(), \
                 updated_at = now() \
             WHERE project_id = $3",
        )
        .bind(status)
        .bind(&record.record_id)
        .bind(project_id)
        .execute(&state.db)
        .await?;

        let rows = result.rows_affected();
        if rows == 0 {
            warn!(
                %project_id,
                fraud_status = status,
                "fraud status update matched no rows (project not in DB?)"
            );
        } else {
            debug!(%project_id, fraud_status = status, "fraud status updated in DB");
        }
        updated = updated.saturating_add(usize::try_from(rows).unwrap_or(0));
    }

    info!(
        fetched,
        updated, skipped_invalid_id, skipped_invalid_status, "fraud statuses phase complete"
    );
    Ok(updated)
}

pub async fn run_scheduler(state: AppState, interval: std::time::Duration) {
    info!(
        interval_secs = interval.as_secs(),
        "Airtable sync scheduler started"
    );
    let mut ticker = tokio::time::interval(interval);
    loop {
        ticker.tick().await;
        info!("Airtable sync tick");
        if let Err(error) = sync(&state).await {
            warn!(%error, "Airtable sync failed");
        }
    }
}

fn normalized_fraud_status(value: &str) -> Option<&'static str> {
    match value
        .trim()
        .to_ascii_lowercase()
        .replace(['-', ' '], "_")
        .as_str()
    {
        "approved" | "approve" | "clear" | "passed" | "pass" | "true" => Some("approved"),
        "rejected" | "reject" | "failed" | "fraud" | "blocked" | "false" => Some("rejected"),
        "pending" | "in_review" | "needs_review" | "" => Some("pending"),
        _ => None,
    }
}
