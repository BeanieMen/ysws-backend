use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct ShopItem {
    pub id: Uuid,
    pub slug: String,
    pub name: String,
    pub description: String,
    pub price_minutes: i64,
}

#[derive(Debug, Serialize)]
pub struct PublicShopItem {
    pub id: Uuid,
    pub slug: String,
    pub name: String,
    pub description: String,
    pub price_hours: f64,
}

impl From<ShopItem> for PublicShopItem {
    fn from(item: ShopItem) -> Self {
        Self {
            id: item.id,
            slug: item.slug,
            name: item.name,
            description: item.description,
            price_hours: minutes_as_hours(item.price_minutes),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ShopAccountResponse {
    pub available_minutes: i64,
    pub available_hours: f64,
    pub purchases: Vec<PurchaseSummary>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct PurchaseSummary {
    pub id: Uuid,
    pub item_id: Uuid,
    pub item_name: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct PurchaseResponse {
    pub purchase_id: Uuid,
    pub item: PublicShopItem,
    pub available_minutes: i64,
    pub available_hours: f64,
}

#[must_use]
#[allow(
    clippy::cast_precision_loss,
    clippy::as_conversions,
    reason = "i64->f64 is exact below 2^53, far beyond any realistic balance; integer minutes stay the source of truth"
)]
pub fn minutes_as_hours(minutes: i64) -> f64 {
    minutes as f64 / 60.0
}
