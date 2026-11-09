use super::*;
use std::collections::HashMap;
use std::sync::Mutex;

#[derive(Default)]
struct Inner {
    rules: Vec<RulesVersion>,
    orders: HashMap<Uuid, OrderRow>,
    stock: Vec<StockItem>,
    users: Vec<(User, String)>,
    sessions: HashMap<String, (Uuid, DateTime<Utc>)>,
    business: BusinessProfile,
    logo: Option<Vec<u8>>,
}

struct OrderRow {
    customer_name: Option<String>,
    created_by: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    revisions: Vec<Revision>,
}

#[derive(Default)]
pub struct MemoryStore(Mutex<Inner>);

impl MemoryStore {
    pub fn new() -> Self {
        Self::default()
    }
}

fn detail(id: Uuid, o: &OrderRow) -> OrderDetail {
    OrderDetail {
        id,
        customer_name: o.customer_name.clone(),
        created_by: o.created_by.clone(),
        created_at: o.created_at,
        updated_at: o.updated_at,
        latest: o.revisions.last().expect("an order always has a revision").clone(),
        revisions: o
            .revisions
            .iter()
            .map(|r| RevisionSummary { revision: r.revision, rules_version_no: r.rules_version_no, created_at: r.created_at })
            .collect(),
    }
}

#[async_trait]
impl Store for MemoryStore {
    async fn current_rules(&self) -> StoreResult<RulesVersion> {
        self.0.lock().unwrap().rules.last().cloned().ok_or(StoreError::NotFound)
    }

    async fn rules_history(&self) -> StoreResult<Vec<RulesVersion>> {
        Ok(self.0.lock().unwrap().rules.iter().rev().cloned().collect())
    }

    async fn append_rules(&self, rules: Rules, note: String, authorised_formula_change: bool, changed_by: Option<String>) -> StoreResult<RulesVersion> {
        let mut g = self.0.lock().unwrap();
        let v = RulesVersion {
            version_no: g.rules.len() as i32 + 1,
            rules,
            note,
            authorised_formula_change,
            changed_by,
            created_at: Utc::now(),
        };
        g.rules.push(v.clone());
        Ok(v)
    }

    async fn create_order(&self, customer_name: Option<String>, created_by: Option<String>, rev: NewRevision) -> StoreResult<OrderDetail> {
        let mut g = self.0.lock().unwrap();
        let (id, now) = (Uuid::new_v4(), Utc::now());
        let row = OrderRow {
            customer_name,
            created_by,
            created_at: now,
            updated_at: now,
            revisions: vec![Revision { revision: 1, request: rev.request, result: rev.result, rules_version_no: rev.rules_version_no, created_at: now }],
        };
        let d = detail(id, &row);
        g.orders.insert(id, row);
        Ok(d)
    }

    async fn add_revision(&self, id: Uuid, customer_name: Option<String>, rev: NewRevision) -> StoreResult<OrderDetail> {
        let mut g = self.0.lock().unwrap();
        let o = g.orders.get_mut(&id).ok_or(StoreError::NotFound)?;
        let now = Utc::now();
        if customer_name.is_some() {
            o.customer_name = customer_name;
        }
        o.updated_at = now;
        o.revisions.push(Revision {
            revision: o.revisions.len() as i32 + 1,
            request: rev.request,
            result: rev.result,
            rules_version_no: rev.rules_version_no,
            created_at: now,
        });
        Ok(detail(id, o))
    }

    async fn get_order(&self, id: Uuid) -> StoreResult<OrderDetail> {
        let g = self.0.lock().unwrap();
        g.orders.get(&id).map(|o| detail(id, o)).ok_or(StoreError::NotFound)
    }

    async fn get_revision(&self, id: Uuid, revision: i32) -> StoreResult<Revision> {
        let g = self.0.lock().unwrap();
        let o = g.orders.get(&id).ok_or(StoreError::NotFound)?;
        o.revisions.iter().find(|r| r.revision == revision).cloned().ok_or(StoreError::NotFound)
    }

    async fn list_orders(&self, limit: i64) -> StoreResult<Vec<OrderSummary>> {
        let g = self.0.lock().unwrap();
        let mut v: Vec<OrderSummary> = g
            .orders
            .iter()
            .map(|(id, o)| OrderSummary {
                id: *id,
                customer_name: o.customer_name.clone(),
                created_by: o.created_by.clone(),
                created_at: o.created_at,
                updated_at: o.updated_at,
                revision_count: o.revisions.len() as i32,
            })
            .collect();
        v.sort_by_key(|o| std::cmp::Reverse(o.updated_at));
        v.truncate(limit.max(0) as usize);
        Ok(v)
    }

    async fn count_users(&self) -> StoreResult<i64> {
        Ok(self.0.lock().unwrap().users.len() as i64)
    }

    async fn create_user(&self, u: NewUser) -> StoreResult<User> {
        let mut g = self.0.lock().unwrap();
        let name = u.username.to_lowercase();
        if g.users.iter().any(|(x, _)| x.username == name) {
            return Err(StoreError::Conflict);
        }
        let user = User { id: Uuid::new_v4(), username: name, display_name: u.display_name, role: u.role, active: true, created_at: Utc::now() };
        g.users.push((user.clone(), u.password_hash));
        Ok(user)
    }

    async fn user_with_hash(&self, username: &str) -> StoreResult<Option<(User, String)>> {
        let name = username.to_lowercase();
        Ok(self.0.lock().unwrap().users.iter().find(|(u, _)| u.username == name).cloned())
    }

    async fn get_user(&self, id: Uuid) -> StoreResult<User> {
        self.0.lock().unwrap().users.iter().find(|(u, _)| u.id == id).map(|(u, _)| u.clone()).ok_or(StoreError::NotFound)
    }

    async fn list_users(&self) -> StoreResult<Vec<User>> {
        Ok(self.0.lock().unwrap().users.iter().map(|(u, _)| u.clone()).collect())
    }

    async fn update_user(&self, id: Uuid, up: UserUpdate) -> StoreResult<User> {
        let mut g = self.0.lock().unwrap();
        let (u, hash) = g.users.iter_mut().find(|(u, _)| u.id == id).ok_or(StoreError::NotFound)?;
        if let Some(n) = up.display_name {
            u.display_name = n;
        }
        if let Some(r) = up.role {
            u.role = r;
        }
        if let Some(a) = up.active {
            u.active = a;
        }
        if let Some(h) = up.password_hash {
            *hash = h;
        }
        Ok(u.clone())
    }

    async fn create_session(&self, token_hash: &str, user_id: Uuid, expires_at: DateTime<Utc>) -> StoreResult<()> {
        self.0.lock().unwrap().sessions.insert(token_hash.to_string(), (user_id, expires_at));
        Ok(())
    }

    async fn session_user(&self, token_hash: &str) -> StoreResult<Option<User>> {
        let g = self.0.lock().unwrap();
        let Some((uid, exp)) = g.sessions.get(token_hash) else { return Ok(None) };
        if *exp <= Utc::now() {
            return Ok(None);
        }
        Ok(g.users.iter().find(|(u, _)| u.id == *uid && u.active).map(|(u, _)| u.clone()))
    }

    async fn delete_session(&self, token_hash: &str) -> StoreResult<()> {
        self.0.lock().unwrap().sessions.remove(token_hash);
        Ok(())
    }

    async fn delete_user_sessions(&self, user_id: Uuid) -> StoreResult<()> {
        self.0.lock().unwrap().sessions.retain(|_, (u, _)| *u != user_id);
        Ok(())
    }

    async fn get_business(&self) -> StoreResult<BusinessProfile> {
        Ok(self.0.lock().unwrap().business.clone())
    }

    async fn set_business(&self, p: BusinessProfile, _updated_by: Option<String>) -> StoreResult<()> {
        self.0.lock().unwrap().business = p;
        Ok(())
    }

    async fn get_logo(&self) -> StoreResult<Option<Vec<u8>>> {
        Ok(self.0.lock().unwrap().logo.clone())
    }

    async fn set_logo(&self, jpeg: Option<Vec<u8>>) -> StoreResult<()> {
        self.0.lock().unwrap().logo = jpeg;
        Ok(())
    }

    async fn list_stock(&self) -> StoreResult<Vec<StockItem>> {
        Ok(self.0.lock().unwrap().stock.clone())
    }

    async fn save_stock(&self, id: Option<Uuid>, item: NewStockItem) -> StoreResult<StockItem> {
        let mut g = self.0.lock().unwrap();
        let make = |id| StockItem {
            id,
            profile: item.profile,
            length_mm: item.length_mm,
            quantity: item.quantity,
            confirmed: item.confirmed,
            note: item.note.clone(),
            updated_at: Utc::now(),
        };
        match id {
            None => {
                let it = make(Uuid::new_v4());
                g.stock.push(it.clone());
                Ok(it)
            }
            Some(id) => {
                let slot = g.stock.iter_mut().find(|s| s.id == id).ok_or(StoreError::NotFound)?;
                *slot = make(id);
                Ok(slot.clone())
            }
        }
    }

    async fn delete_stock(&self, id: Uuid) -> StoreResult<()> {
        let mut g = self.0.lock().unwrap();
        let n = g.stock.len();
        g.stock.retain(|s| s.id != id);
        if g.stock.len() == n {
            Err(StoreError::NotFound)
        } else {
            Ok(())
        }
    }
}
