use crate::{
    adapters::http::AppState,
    error::{ApiError, ApiResult},
};
use sqlx::Row;
use uuid::Uuid;

/// Credits the project's owner once, and only once, after both independent
/// approvals have succeeded. The time snapshot is deliberately immutable.
pub async fn award_if_fully_approved(state: &AppState, project_id: Uuid) -> ApiResult<Option<i64>> {
    let project = sqlx::query(
        "SELECT p.owner_id, s.project_approval_status, s.fraud_approval_status, h.access_token_ciphertext \
         FROM projects p JOIN project_shipments s ON s.project_id = p.id \
         LEFT JOIN hackatime_connections h ON h.user_id = p.owner_id WHERE p.id = $1",
    )
    .bind(project_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| ApiError::NotFound("project not found".into()))?;

    let owner_id: Uuid = project.get("owner_id");
    let project_status: String = project.get("project_approval_status");
    let fraud_status: String = project.get("fraud_approval_status");
    if project_status != "approved" || fraud_status != "approved" {
        return Ok(None);
    }

    // The credit is immutable and once-only. Bail before the upstream
    // Hackatime round trip so syncs can retry the award cheaply instead of
    // re-fetching durations for projects that were already credited.
    let already_awarded: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM project_credit_awards WHERE project_id = $1)",
    )
    .bind(project_id)
    .fetch_one(&state.db)
    .await?;
    if already_awarded {
        return Ok(None);
    }

    let linked_names: Vec<String> = sqlx::query_scalar(
        "SELECT hackatime_project_name FROM project_hackatime_projects WHERE project_id = $1",
    )
    .bind(project_id)
    .fetch_all(&state.db)
    .await?;
    let encrypted_token: Option<String> = project.get("access_token_ciphertext");
    let credited_minutes = match (encrypted_token, linked_names.is_empty()) {
        (Some(token), false) => {
            let token = state.cipher.decrypt(&token).map_err(ApiError::Internal)?;
            let linked_names: std::collections::HashSet<_> = linked_names.into_iter().collect();
            let seconds = state
                .providers
                .hackatime_projects(&token)
                .await?
                .projects
                .into_iter()
                .filter(|project| linked_names.contains(&project.name))
                .filter_map(|project| project.total_duration)
                .filter(|duration| duration.is_finite() && *duration > 0.0)
                .sum::<f64>();
            minutes_from_seconds(seconds)?
        }
        _ => 0,
    };

    let mut tx = state.db.begin().await?;
    let still_approved: Option<bool> = sqlx::query_scalar(
        "SELECT project_approval_status = 'approved' AND fraud_approval_status = 'approved' \
         FROM project_shipments WHERE project_id = $1 FOR UPDATE",
    )
    .bind(project_id)
    .fetch_optional(&mut *tx)
    .await?;
    if still_approved != Some(true) {
        tx.rollback().await?;
        return Ok(None);
    }

    let inserted: Option<Uuid> = sqlx::query_scalar(
        "INSERT INTO project_credit_awards (project_id, user_id, credited_minutes) VALUES ($1, $2, $3) \
         ON CONFLICT (project_id) DO NOTHING RETURNING project_id",
    )
    .bind(project_id)
    .bind(owner_id)
    .bind(credited_minutes)
    .fetch_optional(&mut *tx)
    .await?;
    if inserted.is_none() {
        tx.rollback().await?;
        return Ok(None);
    }
    sqlx::query(
        "INSERT INTO user_wallets (user_id, available_minutes) VALUES ($1, $2) \
         ON CONFLICT (user_id) DO UPDATE SET available_minutes = user_wallets.available_minutes + EXCLUDED.available_minutes, updated_at = now()",
    )
    .bind(owner_id)
    .bind(credited_minutes)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(Some(credited_minutes))
}

fn minutes_from_seconds(seconds: f64) -> ApiResult<i64> {
    let minutes = (seconds / 60.0).floor();
    if !minutes.is_finite() || minutes < 0.0 {
        return Err(ApiError::Upstream(
            "Hackatime returned an invalid duration".into(),
        ));
    }
    // Truncation is intentional: `minutes` was already floored to a whole
    // minute and the value is exact below 2^53.
    #[allow(
        clippy::cast_possible_truncation,
        clippy::as_conversions,
        reason = "value is a non-negative floored whole minute, exact below 2^53"
    )]
    {
        Ok(minutes as i64)
    }
}

#[cfg(test)]
mod tests {
    use super::minutes_from_seconds;

    #[test]
    fn truncates_time_to_whole_minutes() {
        assert!(matches!(minutes_from_seconds(7_259.9), Ok(120)));
        assert!(matches!(minutes_from_seconds(0.0), Ok(0)));
    }
}
