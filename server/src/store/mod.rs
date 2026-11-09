//! Storage interface. Two implementations: `PgStore` (production) and
//! `MemoryStore` (fast API tests). Both must behave identically.

pub mod memory;
pub mod postgres;

use amber_engine::{Profile, Rules};
use rust_decimal::Decimal;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("not found")]
    NotFound,
    #[error("already exists")]
    Conflict,
    #[error("storage failure: {0}")]
    Backend(String),
}

impl From<sqlx::Error> for StoreError {
    fn from(e: sqlx::Error) -> Self {
        match e {
            sqlx::Error::RowNotFound => StoreError::NotFound,
            sqlx::Error::Database(ref d) if d.code().as_deref() == Some("23505") => StoreError::Conflict,
            other => StoreError::Backend(other.to_string()),
        }
    }
}

pub type StoreResult<T> = std::result::Result<T, StoreError>;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RulesVersion {
    pub version_no: i32,
    pub rules: Rules,
    pub note: String,
    pub authorised_formula_change: bool,
    /// Username of the administrator who made the change (None for the initial seed).
    pub changed_by: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// A calculation to persist as the next revision of an order.
#[derive(Debug, Clone)]
pub struct NewRevision {
    pub request: Value,
    pub result: Value,
    pub rules_version_no: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Revision {
    pub revision: i32,
    pub request: Value,
    pub result: Value,
    pub rules_version_no: i32,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RevisionSummary {
    pub revision: i32,
    pub rules_version_no: i32,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderDetail {
    pub id: Uuid,
    pub customer_name: Option<String>,
    pub created_by: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub latest: Revision,
    pub revisions: Vec<RevisionSummary>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderSummary {
    pub id: Uuid,
    pub customer_name: Option<String>,
    pub created_by: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub revision_count: i32,
}

/// Existing aluminium stock or an offcut. `confirmed` must be set by the owner:
/// unconfirmed stock is never counted by the cutting plan.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StockItem {
    pub id: Uuid,
    pub profile: Profile,
    pub length_mm: Decimal,
    pub quantity: u32,
    pub confirmed: bool,
    pub note: Option<String>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct NewStockItem {
    pub profile: Profile,
    pub length_mm: Decimal,
    pub quantity: u32,
    pub confirmed: bool,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    /// Can change prices, manage users and the business profile.
    Admin,
    /// Can calculate, save orders, manage stock and make quotations.
    Staff,
}

impl Role {
    pub fn as_str(self) -> &'static str {
        match self {
            Role::Admin => "admin",
            Role::Staff => "staff",
        }
    }
    pub fn parse(s: &str) -> Option<Role> {
        match s {
            "admin" => Some(Role::Admin),
            "staff" => Some(Role::Staff),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct User {
    pub id: Uuid,
    pub username: String,
    pub display_name: String,
    pub role: Role,
    pub active: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct NewUser {
    pub username: String,
    pub display_name: String,
    pub role: Role,
    pub password_hash: String,
}

#[derive(Debug, Clone, Default)]
pub struct UserUpdate {
    pub display_name: Option<String>,
    pub role: Option<Role>,
    pub active: Option<bool>,
    pub password_hash: Option<String>,
}

/// Letterhead and payment details printed on quotations. Plain text only; the
/// PDF uses a basic font, so characters outside ASCII print as "?".
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct BusinessProfile {
    pub company_name: String,
    /// Several lines allowed.
    pub address: String,
    pub phone: String,
    pub email: String,
    pub website: String,
    /// KRA PIN or similar tax number, printed if present.
    pub tax_pin: String,
    /// How to pay: M-Pesa Paybill / Till, bank account, etc. Several lines allowed.
    pub payment_details: String,
    /// Used on a quotation unless the user types specific terms.
    pub default_payment_terms: String,
    pub quote_valid_days: Option<u32>,
    pub footer_note: String,
}

#[async_trait]
pub trait Store: Send + Sync {
    /// The newest rules version. Always exists after `ensure_initial_rules`.
    async fn current_rules(&self) -> StoreResult<RulesVersion>;
    /// Newest first.
    async fn rules_history(&self) -> StoreResult<Vec<RulesVersion>>;
    /// Append a new version (never edits an old one) and return it.
    async fn append_rules(&self, rules: Rules, note: String, authorised_formula_change: bool, changed_by: Option<String>) -> StoreResult<RulesVersion>;

    async fn create_order(&self, customer_name: Option<String>, created_by: Option<String>, rev: NewRevision) -> StoreResult<OrderDetail>;
    /// Add a revision. `customer_name = Some(..)` also updates the name.
    async fn add_revision(&self, id: Uuid, customer_name: Option<String>, rev: NewRevision) -> StoreResult<OrderDetail>;
    async fn get_order(&self, id: Uuid) -> StoreResult<OrderDetail>;
    async fn get_revision(&self, id: Uuid, revision: i32) -> StoreResult<Revision>;
    /// Most recently updated first.
    async fn list_orders(&self, limit: i64) -> StoreResult<Vec<OrderSummary>>;

    async fn count_users(&self) -> StoreResult<i64>;
    /// `Conflict` if the username is taken (usernames are stored lower-case).
    async fn create_user(&self, u: NewUser) -> StoreResult<User>;
    async fn user_with_hash(&self, username: &str) -> StoreResult<Option<(User, String)>>;
    async fn get_user(&self, id: Uuid) -> StoreResult<User>;
    async fn list_users(&self) -> StoreResult<Vec<User>>;
    async fn update_user(&self, id: Uuid, u: UserUpdate) -> StoreResult<User>;
    async fn create_session(&self, token_hash: &str, user_id: Uuid, expires_at: DateTime<Utc>) -> StoreResult<()>;
    /// The user behind a live (unexpired) session, if that user is still active.
    async fn session_user(&self, token_hash: &str) -> StoreResult<Option<User>>;
    async fn delete_session(&self, token_hash: &str) -> StoreResult<()>;
    async fn delete_user_sessions(&self, user_id: Uuid) -> StoreResult<()>;

    async fn get_business(&self) -> StoreResult<BusinessProfile>;
    async fn set_business(&self, p: BusinessProfile, updated_by: Option<String>) -> StoreResult<()>;
    /// The letterhead logo (JPEG bytes), if one was uploaded.
    async fn get_logo(&self) -> StoreResult<Option<Vec<u8>>>;
    async fn set_logo(&self, jpeg: Option<Vec<u8>>) -> StoreResult<()>;

    /// Oldest first.
    async fn list_stock(&self) -> StoreResult<Vec<StockItem>>;
    /// `id = None` creates; `Some(id)` replaces that item (NotFound if absent).
    async fn save_stock(&self, id: Option<Uuid>, item: NewStockItem) -> StoreResult<StockItem>;
    async fn delete_stock(&self, id: Uuid) -> StoreResult<()>;
}

/// Seed version 1 with the brief's standard rules if the store is empty.
pub async fn ensure_initial_rules(store: &dyn Store) -> StoreResult<()> {
    match store.current_rules().await {
        Ok(_) => Ok(()),
        Err(StoreError::NotFound) => {
            let mut rules = Rules::standard();
            rules.version = "amber-v1".to_string();
            store.append_rules(rules, "Initial rules from the Amber Stores brief".into(), false, None).await?;
            Ok(())
        }
        Err(e) => Err(e),
    }
}
