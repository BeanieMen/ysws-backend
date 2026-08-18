use crate::{
    adapters::http::{AppState, helpers::current_session_user},
    domain::{
        PublicShopItem, PurchaseResponse, PurchaseSummary, ShopAccountResponse, ShopItem,
        minutes_as_hours,
    },
    error::{ApiError, ApiResult},
};
use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
};
use std::sync::Arc;
use tracing::warn;
use uuid::Uuid;

/// Lists currently claimable shop items. This endpoint is intentionally public:
/// it exposes catalogue data only, never balances, purchases, or inventory
/// controls.
pub async fn list_items(
    State(state): State<Arc<AppState>>,
) -> ApiResult<Json<Vec<PublicShopItem>>> {
    let items = sqlx::query_as::<_, ShopItem>(
        "SELECT id, slug, name, description, price_minutes FROM shop_items WHERE is_active = true ORDER BY created_at",
    )
    .fetch_all(&state.db)
    .await?
    .into_iter()
    .map(PublicShopItem::from)
    .collect();
    Ok(Json(items))
}

/// Returns the signed-in user's approved-hours balance and previous claims.
pub async fn shop_account(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> ApiResult<Json<ShopAccountResponse>> {
    let user = current_session_user(&state, &headers).await?;
    let minutes: Option<i64> =
        sqlx::query_scalar("SELECT available_minutes FROM user_wallets WHERE user_id = $1")
            .bind(user.id)
            .fetch_optional(&state.db)
            .await?;
    let available_minutes = minutes.unwrap_or(0);
    let purchases = sqlx::query_as::<_, PurchaseSummary>(
        "SELECT p.id, p.item_id, i.name AS item_name, p.created_at FROM shop_purchases p \
         JOIN shop_items i ON i.id = p.item_id WHERE p.user_id = $1 ORDER BY p.created_at DESC",
    )
    .bind(user.id)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(ShopAccountResponse {
        available_minutes,
        available_hours: minutes_as_hours(available_minutes),
        purchases,
    }))
}

/// Claims a shop item. Price and entitlement are decided entirely by the
/// locked database row; a client cannot submit a price or another user's ID.
#[allow(clippy::too_many_lines)] // Transactional purchase checks are kept together for auditability.
pub async fn purchase_item(
    State(state): State<Arc<AppState>>,
    Path(item_id): Path<Uuid>,
    headers: HeaderMap,
) -> ApiResult<(StatusCode, Json<PurchaseResponse>)> {
    let user = current_session_user(&state, &headers).await?;
    let idempotency_key = validated_idempotency_key(&headers)?;
    let mut tx = state.db.begin().await?;
    let item = sqlx::query_as::<_, ShopItem>(
        "SELECT id, slug, name, description, price_minutes FROM shop_items WHERE id = $1 AND is_active = true FOR UPDATE",
    )
    .bind(item_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| ApiError::NotFound("shop item is unavailable".into()))?;

    // Repeating an already-completed request is safe, including after a client
    // timeout. Do it before the balance debit.
    let existing: Option<(Uuid, Uuid)> = sqlx::query_as(
        "SELECT id, item_id FROM shop_purchases WHERE user_id = $1 AND idempotency_key = $2",
    )
    .bind(user.id)
    .bind(idempotency_key)
    .fetch_optional(&mut *tx)
    .await?;
    if let Some((purchase_id, existing_item_id)) = existing {
        if existing_item_id != item.id {
            return Err(ApiError::Conflict(
                "Idempotency-Key was already used for another item".into(),
            ));
        }
        let balance: i64 =
            sqlx::query_scalar("SELECT available_minutes FROM user_wallets WHERE user_id = $1")
                .bind(user.id)
                .fetch_one(&mut *tx)
                .await?;
        tx.commit().await?;
        return Ok((
            StatusCode::OK,
            Json(PurchaseResponse {
                purchase_id,
                item: item.into(),
                available_minutes: balance,
                available_hours: minutes_as_hours(balance),
            }),
        ));
    }

    // The INSERT ON CONFLICT DO NOTHING below doubles as the claim guard, so
    // no separate "already claimed" read is needed. The wallet row is created
    // and debited in one statement.
    let remaining: Option<i64> = sqlx::query_scalar(
        "WITH wallet AS (INSERT INTO user_wallets (user_id) VALUES ($2) \
         ON CONFLICT (user_id) DO NOTHING) \
         UPDATE user_wallets SET available_minutes = available_minutes - $1, updated_at = now() \
         WHERE user_id = $2 AND available_minutes >= $1 RETURNING available_minutes",
    )
    .bind(item.price_minutes)
    .bind(user.id)
    .fetch_optional(&mut *tx)
    .await?;
    let Some(available_minutes) = remaining else {
        return Err(ApiError::Conflict(
            "you need more approved hours to claim this item".into(),
        ));
    };

    let purchase_id = Uuid::new_v4();
    let inserted = sqlx::query(
        "INSERT INTO shop_purchases (id, user_id, item_id, price_minutes, idempotency_key) VALUES ($1, $2, $3, $4, $5) \
         ON CONFLICT (user_id, item_id) DO NOTHING",
    )
    .bind(purchase_id)
    .bind(user.id)
    .bind(item.id)
    .bind(item.price_minutes)
    .bind(idempotency_key)
    .execute(&mut *tx)
    .await?;
    if inserted.rows_affected() == 0 {
        return Err(ApiError::Conflict(
            "you have already claimed this item".into(),
        ));
    }
    if item.slug == "event-ticket" {
        sqlx::query(
            "INSERT INTO notification_outbox (id, purchase_id, kind) VALUES ($1, $2, 'ticket_purchase_confirmation')",
        )
        .bind(Uuid::new_v4())
        .bind(purchase_id)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;

    if item.slug == "event-ticket"
        && let Err(error) =
            deliver_purchase_email(&state, purchase_id, &user.email, &user.first_name).await
    {
        warn!(%purchase_id, %error, "ticket confirmation email is queued for retry");
    }
    Ok((
        StatusCode::CREATED,
        Json(PurchaseResponse {
            purchase_id,
            item: item.into(),
            available_minutes,
            available_hours: minutes_as_hours(available_minutes),
        }),
    ))
}

async fn deliver_purchase_email(
    state: &AppState,
    purchase_id: Uuid,
    email: &str,
    first_name: &str,
) -> ApiResult<()> {
    let pending: Option<Uuid> = sqlx::query_scalar(
        "UPDATE notification_outbox SET attempts = attempts + 1, processing_at = now() \
         WHERE purchase_id = $1 AND sent_at IS NULL \
         AND (processing_at IS NULL OR processing_at < now() - interval '5 minutes') RETURNING id",
    )
    .bind(purchase_id)
    .fetch_optional(&state.db)
    .await?;
    if pending.is_none() {
        return Ok(());
    }
    match state
        .notifications
        .ticket_purchase_confirmation(email, first_name)
        .await
    {
        Ok(()) => {
            sqlx::query("UPDATE notification_outbox SET sent_at = now(), processing_at = NULL, last_error = NULL WHERE purchase_id = $1")
                .bind(purchase_id)
                .execute(&state.db)
                .await?;
            Ok(())
        }
        Err(error) => {
            sqlx::query("UPDATE notification_outbox SET processing_at = NULL, last_error = $1 WHERE purchase_id = $2")
                .bind(error.to_string())
                .bind(purchase_id)
                .execute(&state.db)
                .await?;
            Err(ApiError::Internal(error))
        }
    }
}

/// Retries durable confirmation jobs left behind by a temporary mail-provider
/// outage. It is used by the backend scheduler as well as the first purchase.
pub async fn retry_pending_ticket_emails(state: &AppState) -> ApiResult<()> {
    let pending: Vec<(Uuid, String, String)> = sqlx::query_as(
        "SELECT o.purchase_id, u.email, u.first_name FROM notification_outbox o \
         JOIN shop_purchases p ON p.id = o.purchase_id JOIN users u ON u.id = p.user_id \
         WHERE o.kind = 'ticket_purchase_confirmation' AND o.sent_at IS NULL \
         ORDER BY o.created_at LIMIT 100",
    )
    .fetch_all(&state.db)
    .await?;
    for (purchase_id, email, first_name) in pending {
        if let Err(error) = deliver_purchase_email(state, purchase_id, &email, &first_name).await {
            warn!(%purchase_id, %error, "ticket confirmation retry failed");
        }
    }
    Ok(())
}

fn validated_idempotency_key(headers: &HeaderMap) -> ApiResult<&str> {
    let key = headers
        .get("idempotency-key")
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| ApiError::BadRequest("Idempotency-Key header is required".into()))?;
    if !(16..=128).contains(&key.len())
        || !key
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
    {
        return Err(ApiError::BadRequest(
            "Idempotency-Key must be 16-128 URL-safe characters".into(),
        ));
    }
    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::validated_idempotency_key;
    use axum::http::HeaderMap;

    #[test]
    fn requires_a_safe_idempotency_key() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "idempotency-key",
            axum::http::HeaderValue::from_static("a-very-safe-key-1234"),
        );
        assert!(validated_idempotency_key(&headers).is_ok());
    }
}
