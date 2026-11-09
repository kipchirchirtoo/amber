use super::*;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use std::str::FromStr;
use sqlx::types::Json;
use sqlx::{FromRow, PgPool};

#[derive(Clone)]
pub struct PgStore {
    pool: PgPool,
}

impl PgStore {
    pub async fn connect(url: &str) -> StoreResult<Self> {
        // Hosted databases sit behind a connection pooler (e.g. Supabase). Pooled
        // connections can change underneath us, so do not cache prepared statements,
        // and keep the pool small so we stay inside the host's connection limit.
        let opts = PgConnectOptions::from_str(url).map_err(|e| StoreError::Backend(format!("bad DATABASE_URL: {e}")))?.statement_cache_capacity(0);
        let max: u32 = std::env::var("AMBER_DB_POOL").ok().and_then(|v| v.parse().ok()).filter(|n| (1..=50).contains(n)).unwrap_or(5);
        let pool = PgPoolOptions::new().max_connections(max).connect_with(opts).await?;
        Ok(PgStore { pool })
    }

    pub fn from_pool(pool: PgPool) -> Self {
        PgStore { pool }
    }

    pub async fn migrate(&self) -> StoreResult<()> {
        sqlx::migrate!("./migrations")
            .run(&self.pool)
            .await
            .map_err(|e| StoreError::Backend(e.to_string()))
    }

    async fn load_detail(&self, id: Uuid) -> StoreResult<OrderDetail> {
        let o: OrderRow = sqlx::query_as("SELECT id, customer_name, created_by, created_at, updated_at FROM orders WHERE id = $1")
            .bind(id)
            .fetch_one(&self.pool)
            .await?;
        let revs: Vec<RevisionRow> = sqlx::query_as(
            "SELECT revision, request, result, rules_version_no, created_at \
             FROM order_revisions WHERE order_id = $1 ORDER BY revision",
        )
        .bind(id)
        .fetch_all(&self.pool)
        .await?;
        let revisions = revs
            .iter()
            .map(|r| RevisionSummary { revision: r.revision, rules_version_no: r.rules_version_no, created_at: r.created_at })
            .collect();
        let latest = revs.into_iter().last().ok_or(StoreError::NotFound)?.into();
        Ok(OrderDetail { id: o.id, customer_name: o.customer_name, created_by: o.created_by, created_at: o.created_at, updated_at: o.updated_at, latest, revisions })
    }
}

#[derive(FromRow)]
struct OrderRow {
    id: Uuid,
    customer_name: Option<String>,
    created_by: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(FromRow)]
struct RevisionRow {
    revision: i32,
    request: Json<Value>,
    result: Json<Value>,
    rules_version_no: i32,
    created_at: DateTime<Utc>,
}

impl From<RevisionRow> for Revision {
    fn from(r: RevisionRow) -> Self {
        Revision { revision: r.revision, request: r.request.0, result: r.result.0, rules_version_no: r.rules_version_no, created_at: r.created_at }
    }
}

#[derive(FromRow)]
struct RulesRow {
    version_no: i32,
    rules: Json<Rules>,
    note: String,
    authorised_formula_change: bool,
    changed_by: Option<String>,
    created_at: DateTime<Utc>,
}

impl From<RulesRow> for RulesVersion {
    fn from(r: RulesRow) -> Self {
        RulesVersion { version_no: r.version_no, rules: r.rules.0, note: r.note, authorised_formula_change: r.authorised_formula_change, changed_by: r.changed_by, created_at: r.created_at }
    }
}

#[async_trait]
impl Store for PgStore {
    async fn current_rules(&self) -> StoreResult<RulesVersion> {
        let r: RulesRow = sqlx::query_as(
            "SELECT version_no, rules, note, authorised_formula_change, changed_by, created_at FROM rules_versions ORDER BY version_no DESC LIMIT 1",
        )
            .fetch_one(&self.pool)
            .await?;
        Ok(r.into())
    }

    async fn rules_history(&self) -> StoreResult<Vec<RulesVersion>> {
        let rows: Vec<RulesRow> = sqlx::query_as(
            "SELECT version_no, rules, note, authorised_formula_change, changed_by, created_at FROM rules_versions ORDER BY version_no DESC",
        )
            .fetch_all(&self.pool)
            .await?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn append_rules(&self, rules: Rules, note: String, authorised_formula_change: bool, changed_by: Option<String>) -> StoreResult<RulesVersion> {
        let r: RulesRow = sqlx::query_as(
            "INSERT INTO rules_versions (rules, note, authorised_formula_change, changed_by) VALUES ($1, $2, $3, $4) \
             RETURNING version_no, rules, note, authorised_formula_change, changed_by, created_at",
        )
        .bind(Json(rules))
        .bind(note)
        .bind(authorised_formula_change)
        .bind(changed_by)
        .fetch_one(&self.pool)
        .await?;
        Ok(r.into())
    }

    async fn create_order(&self, customer_name: Option<String>, created_by: Option<String>, rev: NewRevision) -> StoreResult<OrderDetail> {
        let id = Uuid::new_v4();
        let mut tx = self.pool.begin().await?;
        sqlx::query("INSERT INTO orders (id, customer_name, created_by) VALUES ($1, $2, $3)")
            .bind(id)
            .bind(customer_name)
            .bind(created_by)
            .execute(&mut *tx)
            .await?;
        sqlx::query("INSERT INTO order_revisions (order_id, revision, request, result, rules_version_no) VALUES ($1, 1, $2, $3, $4)")
            .bind(id)
            .bind(Json(rev.request))
            .bind(Json(rev.result))
            .bind(rev.rules_version_no)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        self.load_detail(id).await
    }

    async fn add_revision(&self, id: Uuid, customer_name: Option<String>, rev: NewRevision) -> StoreResult<OrderDetail> {
        let mut tx = self.pool.begin().await?;
        // Lock the order row so two concurrent edits cannot pick the same revision number.
        sqlx::query_scalar::<_, Uuid>("SELECT id FROM orders WHERE id = $1 FOR UPDATE")
            .bind(id)
            .fetch_one(&mut *tx)
            .await?;
        let next: i32 = sqlx::query_scalar("SELECT COALESCE(MAX(revision), 0) + 1 FROM order_revisions WHERE order_id = $1")
            .bind(id)
            .fetch_one(&mut *tx)
            .await?;
        sqlx::query("INSERT INTO order_revisions (order_id, revision, request, result, rules_version_no) VALUES ($1, $2, $3, $4, $5)")
            .bind(id)
            .bind(next)
            .bind(Json(rev.request))
            .bind(Json(rev.result))
            .bind(rev.rules_version_no)
            .execute(&mut *tx)
            .await?;
        sqlx::query("UPDATE orders SET updated_at = now(), customer_name = COALESCE($2, customer_name) WHERE id = $1")
            .bind(id)
            .bind(customer_name)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        self.load_detail(id).await
    }

    async fn get_order(&self, id: Uuid) -> StoreResult<OrderDetail> {
        self.load_detail(id).await
    }

    async fn get_revision(&self, id: Uuid, revision: i32) -> StoreResult<Revision> {
        let r: RevisionRow = sqlx::query_as(
            "SELECT revision, request, result, rules_version_no, created_at FROM order_revisions WHERE order_id = $1 AND revision = $2",
        )
        .bind(id)
        .bind(revision)
        .fetch_one(&self.pool)
        .await?;
        Ok(r.into())
    }

    async fn list_orders(&self, limit: i64) -> StoreResult<Vec<OrderSummary>> {
        #[derive(FromRow)]
        struct Row {
            id: Uuid,
            customer_name: Option<String>,
            created_by: Option<String>,
            created_at: DateTime<Utc>,
            updated_at: DateTime<Utc>,
            revision_count: i64,
        }
        let rows: Vec<Row> = sqlx::query_as(
            "SELECT o.id, o.customer_name, o.created_by, o.created_at, o.updated_at, COUNT(r.revision) AS revision_count \
             FROM orders o JOIN order_revisions r ON r.order_id = o.id \
             GROUP BY o.id ORDER BY o.updated_at DESC LIMIT $1",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|r| OrderSummary { id: r.id, customer_name: r.customer_name, created_by: r.created_by, created_at: r.created_at, updated_at: r.updated_at, revision_count: r.revision_count as i32 })
            .collect())
    }

    async fn count_users(&self) -> StoreResult<i64> {
        Ok(sqlx::query_scalar("SELECT COUNT(*) FROM users").fetch_one(&self.pool).await?)
    }

    async fn create_user(&self, u: NewUser) -> StoreResult<User> {
        let row: UserRow = sqlx::query_as(
            "INSERT INTO users (id, username, display_name, role, password_hash) VALUES ($1, $2, $3, $4, $5) \
             RETURNING id, username, display_name, role, active, created_at",
        )
        .bind(Uuid::new_v4())
        .bind(u.username.to_lowercase())
        .bind(u.display_name)
        .bind(u.role.as_str())
        .bind(u.password_hash)
        .fetch_one(&self.pool)
        .await?;
        row.into_user()
    }

    async fn user_with_hash(&self, username: &str) -> StoreResult<Option<(User, String)>> {
        let row: Option<UserHashRow> = sqlx::query_as(
            "SELECT id, username, display_name, role, active, created_at, password_hash FROM users WHERE username = $1",
        )
        .bind(username.to_lowercase())
        .fetch_optional(&self.pool)
        .await?;
        row.map(|r| {
            let hash = r.password_hash.clone();
            r.into_user().map(|u| (u, hash))
        })
        .transpose()
    }

    async fn get_user(&self, id: Uuid) -> StoreResult<User> {
        let row: UserRow = sqlx::query_as("SELECT id, username, display_name, role, active, created_at FROM users WHERE id = $1")
            .bind(id)
            .fetch_one(&self.pool)
            .await?;
        row.into_user()
    }

    async fn list_users(&self) -> StoreResult<Vec<User>> {
        let rows: Vec<UserRow> = sqlx::query_as("SELECT id, username, display_name, role, active, created_at FROM users ORDER BY created_at, username")
            .fetch_all(&self.pool)
            .await?;
        rows.into_iter().map(UserRow::into_user).collect()
    }

    async fn update_user(&self, id: Uuid, up: UserUpdate) -> StoreResult<User> {
        let row: UserRow = sqlx::query_as(
            "UPDATE users SET display_name = COALESCE($2, display_name), role = COALESCE($3, role), \
             active = COALESCE($4, active), password_hash = COALESCE($5, password_hash) WHERE id = $1 \
             RETURNING id, username, display_name, role, active, created_at",
        )
        .bind(id)
        .bind(up.display_name)
        .bind(up.role.map(|r| r.as_str()))
        .bind(up.active)
        .bind(up.password_hash)
        .fetch_one(&self.pool)
        .await?;
        row.into_user()
    }

    async fn create_session(&self, token_hash: &str, user_id: Uuid, expires_at: DateTime<Utc>) -> StoreResult<()> {
        // expired sessions are swept as new ones are created
        sqlx::query("DELETE FROM auth_sessions WHERE expires_at <= now()").execute(&self.pool).await?;
        sqlx::query("INSERT INTO auth_sessions (token_hash, user_id, expires_at) VALUES ($1, $2, $3)")
            .bind(token_hash)
            .bind(user_id)
            .bind(expires_at)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn session_user(&self, token_hash: &str) -> StoreResult<Option<User>> {
        let row: Option<UserRow> = sqlx::query_as(
            "SELECT u.id, u.username, u.display_name, u.role, u.active, u.created_at FROM auth_sessions s \
             JOIN users u ON u.id = s.user_id WHERE s.token_hash = $1 AND s.expires_at > now() AND u.active",
        )
        .bind(token_hash)
        .fetch_optional(&self.pool)
        .await?;
        row.map(UserRow::into_user).transpose()
    }

    async fn delete_session(&self, token_hash: &str) -> StoreResult<()> {
        sqlx::query("DELETE FROM auth_sessions WHERE token_hash = $1").bind(token_hash).execute(&self.pool).await?;
        Ok(())
    }

    async fn delete_user_sessions(&self, user_id: Uuid) -> StoreResult<()> {
        sqlx::query("DELETE FROM auth_sessions WHERE user_id = $1").bind(user_id).execute(&self.pool).await?;
        Ok(())
    }

    async fn get_business(&self) -> StoreResult<BusinessProfile> {
        let v: Option<Json<BusinessProfile>> = sqlx::query_scalar("SELECT value FROM app_settings WHERE key = 'business'").fetch_optional(&self.pool).await?;
        Ok(v.map(|j| j.0).unwrap_or_default())
    }

    async fn set_business(&self, p: BusinessProfile, updated_by: Option<String>) -> StoreResult<()> {
        sqlx::query(
            "INSERT INTO app_settings (key, value, updated_by) VALUES ('business', $1, $2) \
             ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value, updated_by = EXCLUDED.updated_by, updated_at = now()",
        )
        .bind(Json(p))
        .bind(updated_by)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn get_logo(&self) -> StoreResult<Option<Vec<u8>>> {
        Ok(sqlx::query_scalar("SELECT data FROM app_assets WHERE key = 'logo'").fetch_optional(&self.pool).await?)
    }

    async fn set_logo(&self, jpeg: Option<Vec<u8>>) -> StoreResult<()> {
        match jpeg {
            Some(bytes) => {
                sqlx::query(
                    "INSERT INTO app_assets (key, content_type, data) VALUES ('logo', 'image/jpeg', $1) \
                     ON CONFLICT (key) DO UPDATE SET data = EXCLUDED.data, updated_at = now()",
                )
                .bind(bytes)
                .execute(&self.pool)
                .await?;
            }
            None => {
                sqlx::query("DELETE FROM app_assets WHERE key = 'logo'").execute(&self.pool).await?;
            }
        }
        Ok(())
    }

    async fn list_stock(&self) -> StoreResult<Vec<StockItem>> {
        let rows: Vec<StockRow> =
            sqlx::query_as("SELECT id, profile, length_mm, quantity, confirmed, note, updated_at FROM stock_items ORDER BY updated_at, id")
                .fetch_all(&self.pool)
                .await?;
        rows.into_iter().map(StockRow::into_item).collect()
    }

    async fn save_stock(&self, id: Option<Uuid>, item: NewStockItem) -> StoreResult<StockItem> {
        let profile = serde_json::to_value(item.profile)
            .ok()
            .and_then(|v| v.as_str().map(String::from))
            .ok_or_else(|| StoreError::Backend("profile".into()))?;
        let (length, qty) = (item.length_mm.to_string(), item.quantity as i32);
        let q = match id {
            None => sqlx::query_as::<_, StockRow>(
                "INSERT INTO stock_items (id, profile, length_mm, quantity, confirmed, note) VALUES ($1, $2, $3, $4, $5, $6) \
                 RETURNING id, profile, length_mm, quantity, confirmed, note, updated_at",
            )
            .bind(Uuid::new_v4()),
            Some(id) => sqlx::query_as::<_, StockRow>(
                "UPDATE stock_items SET profile = $2, length_mm = $3, quantity = $4, confirmed = $5, note = $6, updated_at = now() \
                 WHERE id = $1 RETURNING id, profile, length_mm, quantity, confirmed, note, updated_at",
            )
            .bind(id),
        };
        let row = q.bind(profile).bind(length).bind(qty).bind(item.confirmed).bind(item.note).fetch_one(&self.pool).await?;
        row.into_item()
    }

    async fn delete_stock(&self, id: Uuid) -> StoreResult<()> {
        let r = sqlx::query("DELETE FROM stock_items WHERE id = $1").bind(id).execute(&self.pool).await?;
        if r.rows_affected() == 0 {
            Err(StoreError::NotFound)
        } else {
            Ok(())
        }
    }
}

#[derive(FromRow)]
struct StockRow {
    id: Uuid,
    profile: String,
    length_mm: String,
    quantity: i32,
    confirmed: bool,
    note: Option<String>,
    updated_at: DateTime<Utc>,
}

impl StockRow {
    fn into_item(self) -> StoreResult<StockItem> {
        let bad = |what: &str| StoreError::Backend(format!("stock row {}: bad {what}", self.id));
        Ok(StockItem {
            id: self.id,
            profile: serde_json::from_value(Value::String(self.profile.clone())).map_err(|_| bad("profile"))?,
            length_mm: self.length_mm.parse().map_err(|_| bad("length"))?,
            quantity: u32::try_from(self.quantity).map_err(|_| bad("quantity"))?,
            confirmed: self.confirmed,
            note: self.note,
            updated_at: self.updated_at,
        })
    }
}

#[derive(FromRow)]
struct UserRow {
    id: Uuid,
    username: String,
    display_name: String,
    role: String,
    active: bool,
    created_at: DateTime<Utc>,
}

impl UserRow {
    fn into_user(self) -> StoreResult<User> {
        Ok(User {
            id: self.id,
            username: self.username,
            display_name: self.display_name,
            role: Role::parse(&self.role).ok_or_else(|| StoreError::Backend(format!("user {}: bad role", self.id)))?,
            active: self.active,
            created_at: self.created_at,
        })
    }
}

#[derive(FromRow)]
struct UserHashRow {
    id: Uuid,
    username: String,
    display_name: String,
    role: String,
    active: bool,
    created_at: DateTime<Utc>,
    password_hash: String,
}

impl UserHashRow {
    fn into_user(self) -> StoreResult<User> {
        UserRow { id: self.id, username: self.username, display_name: self.display_name, role: self.role, active: self.active, created_at: self.created_at }.into_user()
    }
}
