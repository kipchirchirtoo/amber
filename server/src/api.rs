use crate::auth::{self, AuthUser};
use crate::calc::{self, CalcRequest, View};
use crate::error::{ApiError, AppJson};
use crate::store::{NewRevision, NewStockItem, StockItem, OrderDetail, OrderSummary, Revision, RulesVersion, Store};
use amber_engine::Rules;
use axum::extract::{Path, Query, State};
use axum::middleware;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone)]
pub struct AppState {
    pub store: Arc<dyn Store>,
    /// The language model behind the assistant. `None` switches the assistant off.
    pub llm: Option<Arc<dyn crate::assistant::Llm>>,
    pub sessions: Arc<crate::assistant::Sessions>,
    pub auth: Arc<auth::AuthState>,
}

impl AppState {
    pub fn new(store: Arc<dyn Store>) -> Self {
        AppState { store, llm: None, sessions: Arc::default(), auth: Arc::default() }
    }

    pub fn with_llm(mut self, llm: Option<Arc<dyn crate::assistant::Llm>>) -> Self {
        self.llm = llm;
        self
    }
}

pub fn router(state: AppState) -> Router {
    // Everything except /health and login needs a signed-in user.
    let protected = Router::new()
        .route("/api/v1/calculate", post(calculate))
        .route("/api/v1/orders", post(create_order).get(list_orders))
        .route("/api/v1/orders/{id}", get(get_order).put(update_order))
        .route("/api/v1/orders/{id}/revisions/{revision}", get(get_revision))
        .route("/api/v1/orders/{id}/quote", get(get_quote))
        .route("/api/v1/orders/{id}/quote.pdf", get(get_quote_pdf))
        .route("/api/v1/assistant/status", get(assistant_status))
        .route("/api/v1/assistant/chat", post(assistant_chat))
        .route("/api/v1/stock", get(list_stock).post(create_stock))
        .route("/api/v1/stock/{id}", axum::routing::put(update_stock).delete(delete_stock))
        .route("/api/v1/settings/rules", get(current_rules).put(update_rules))
        .route("/api/v1/settings/rules/history", get(rules_history))
        .route("/api/v1/settings/business", get(get_business).put(put_business))
        .route("/api/v1/settings/business/logo", get(get_logo).put(put_logo).delete(delete_logo))
        .route("/api/v1/auth/logout", post(auth::logout))
        .route("/api/v1/auth/me", get(auth::me))
        .route("/api/v1/auth/ticket", post(auth::ticket))
        .route("/api/v1/auth/password", post(auth::change_password))
        .route("/api/v1/users", get(auth::list_users).post(auth::create_user))
        .route("/api/v1/users/{id}", axum::routing::put(auth::update_user))
        .route_layer(middleware::from_fn_with_state(state.clone(), auth::require_login));
    Router::new()
        .route("/health", get(health))
        .route("/api/v1/auth/login", post(auth::login))
        .merge(protected)
        .with_state(state)
}

#[derive(Serialize)]
struct CalcResponse {
    rules_version_no: i32,
    view: View,
    result: Value,
}

async fn calculate(State(s): State<AppState>, AppJson(req): AppJson<CalcRequest>) -> Result<Json<CalcResponse>, ApiError> {
    let v = s.store.current_rules().await?;
    let result = calc::run(&req, &v.rules)?;
    Ok(Json(CalcResponse { rules_version_no: v.version_no, view: req.view, result }))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OrderBody {
    customer_name: Option<String>,
    request: CalcRequest,
}

/// Calculate with the CURRENT rules and snapshot the request, result and rules
/// version together. Later rule changes never alter a saved revision.
pub(crate) async fn snapshot(s: &AppState, req: &CalcRequest) -> Result<NewRevision, ApiError> {
    let v = s.store.current_rules().await?;
    let result = calc::run(req, &v.rules)?;
    let request = serde_json::to_value(req).map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(NewRevision { request, result, rules_version_no: v.version_no })
}

async fn create_order(State(s): State<AppState>, user: AuthUser, AppJson(b): AppJson<OrderBody>) -> Result<(axum::http::StatusCode, Json<OrderDetail>), ApiError> {
    let rev = snapshot(&s, &b.request).await?;
    let d = s.store.create_order(b.customer_name, Some(user.0.username), rev).await?;
    Ok((axum::http::StatusCode::CREATED, Json(d)))
}

/// Editing an order recalculates it with the current rules and appends a new
/// revision. Earlier revisions stay exactly as they were.
async fn update_order(State(s): State<AppState>, Path(id): Path<Uuid>, AppJson(b): AppJson<OrderBody>) -> Result<Json<OrderDetail>, ApiError> {
    s.store.get_order(id).await?;
    let rev = snapshot(&s, &b.request).await?;
    Ok(Json(s.store.add_revision(id, b.customer_name, rev).await?))
}

async fn get_order(State(s): State<AppState>, Path(id): Path<Uuid>) -> Result<Json<OrderDetail>, ApiError> {
    Ok(Json(s.store.get_order(id).await?))
}

async fn get_revision(State(s): State<AppState>, Path((id, revision)): Path<(Uuid, i32)>) -> Result<Json<Revision>, ApiError> {
    Ok(Json(s.store.get_revision(id, revision).await?))
}

#[derive(Deserialize)]
struct ListQuery {
    limit: Option<i64>,
}

async fn list_orders(State(s): State<AppState>, Query(q): Query<ListQuery>) -> Result<Json<Vec<OrderSummary>>, ApiError> {
    Ok(Json(s.store.list_orders(q.limit.unwrap_or(50).clamp(1, 200)).await?))
}

async fn current_rules(State(s): State<AppState>) -> Result<Json<RulesVersion>, ApiError> {
    Ok(Json(s.store.current_rules().await?))
}

async fn rules_history(State(s): State<AppState>) -> Result<Json<Vec<RulesVersion>>, ApiError> {
    Ok(Json(s.store.rules_history().await?))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RulesUpdate {
    rules: Rules,
    note: String,
    /// Must be true to change aluminium cutting rules or glass panel deductions.
    #[serde(default)]
    authorise_formula_change: bool,
}

/// Price/setting change (administrators only). Appends a new rules version (history is kept). Changing
/// the measurement formulas additionally needs `authorise_formula_change: true`.
async fn update_rules(State(s): State<AppState>, user: AuthUser, AppJson(u): AppJson<RulesUpdate>) -> Result<(axum::http::StatusCode, Json<RulesVersion>), ApiError> {
    user.require_admin()?;
    if u.note.trim().is_empty() {
        return Err(ApiError::BadRequest("a note explaining the change is required".into()));
    }
    let current = s.store.current_rules().await?;
    let mut new_rules = u.rules;
    new_rules.validate()?;

    let formulas_changed = current.rules.formulas_differ(&new_rules);
    if formulas_changed && !u.authorise_formula_change {
        return Err(ApiError::BadRequest(
            "this change alters the measurement formulas (aluminium cuts or glass panel deductions); set authorise_formula_change to true to authorise it".into(),
        ));
    }
    new_rules.version = current.rules.version.clone();
    if new_rules == current.rules {
        return Err(ApiError::BadRequest("no changes: these rules are identical to the current version".into()));
    }
    new_rules.version = format!("amber-v{}", current.version_no + 1);
    let saved = s.store.append_rules(new_rules, u.note, formulas_changed, Some(user.0.username)).await?;
    Ok((axum::http::StatusCode::CREATED, Json(saved)))
}

#[derive(Deserialize)]
struct QuoteQuery {
    /// Defaults to the latest revision.
    revision: Option<i32>,
    /// consumption (default) | whole_bars | existing_plus_purchase
    basis: Option<String>,
    deposit_percent: Option<rust_decimal::Decimal>,
    /// Owner-supplied: an exact selling price, or a margin % of the selling price.
    selling_price_ksh: Option<rust_decimal::Decimal>,
    margin_percent: Option<rust_decimal::Decimal>,
    /// Owner-supplied unit selling price for each window line, comma separated,
    /// in the order of the order's window lines (e.g. "1500,2000,900").
    unit_prices: Option<String>,
    payment_terms: Option<String>,
}

fn parse_unit_prices(text: &str) -> Result<Vec<rust_decimal::Decimal>, ApiError> {
    text.split(',')
        .map(|p| p.trim().parse::<rust_decimal::Decimal>().map_err(|_| ApiError::BadRequest(format!("unit_prices: '{}' is not a number", p.trim()))))
        .collect()
}

async fn quote_parts(s: &AppState, id: Uuid, q: &QuoteQuery) -> Result<(OrderDetail, Revision, crate::quote::Quote, crate::store::BusinessProfile), ApiError> {
    let order = s.store.get_order(id).await?;
    let rev = match q.revision {
        Some(n) => s.store.get_revision(id, n).await?,
        None => order.latest.clone(),
    };
    let basis = crate::quote::Basis::parse(q.basis.as_deref().unwrap_or("consumption"))?;
    let business = s.store.get_business().await?;
    // terms typed for this quotation win; otherwise the business default applies
    let terms = q.payment_terms.clone().filter(|t| !t.trim().is_empty()).or_else(|| Some(business.default_payment_terms.clone()).filter(|t| !t.trim().is_empty()));
    let quote = crate::quote::build(
        &rev.request,
        &rev.result,
        basis,
        q.deposit_percent,
        &crate::quote::Pricing {
            selling_price_ksh: q.selling_price_ksh,
            margin_percent: q.margin_percent,
            unit_prices: q.unit_prices.as_deref().filter(|t| !t.trim().is_empty()).map(parse_unit_prices).transpose()?,
            payment_terms: terms,
        },
    )?;
    Ok((order, rev, quote, business))
}

async fn get_quote(State(s): State<AppState>, Path(id): Path<Uuid>, Query(q): Query<QuoteQuery>) -> Result<Json<crate::quote::Quote>, ApiError> {
    Ok(Json(quote_parts(&s, id, &q).await?.2))
}

async fn get_quote_pdf(State(s): State<AppState>, Path(id): Path<Uuid>, Query(q): Query<QuoteQuery>) -> Result<impl axum::response::IntoResponse, ApiError> {
    let (order, rev, quote, business) = quote_parts(&s, id, &q).await?;
    let logo_bytes = s.store.get_logo().await?;
    let logo = logo_bytes.as_deref().and_then(|d| crate::quote::jpeg_info(d).ok().map(|info| crate::quote::Logo { data: d, info }));
    let reference = format!("AMB-{}-R{}", id.simple().to_string()[..8].to_uppercase(), rev.revision);
    let date = rev.created_at.format("%Y-%m-%d").to_string();
    let pdf = crate::quote::render_pdf(
        &quote,
        &crate::quote::Header { customer: order.customer_name.as_deref(), reference: &reference, revision: rev.revision, date: &date, business: &business, logo: logo.as_ref() },
    );
    Ok((
        [
            (axum::http::header::CONTENT_TYPE, "application/pdf".to_string()),
            (axum::http::header::CONTENT_DISPOSITION, format!("inline; filename=\"{reference}.pdf\"")),
        ],
        pdf,
    ))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StockBody {
    profile: amber_engine::Profile,
    /// With a unit ("8 ft", "2438.4 mm", "2.4 m"); a bare number is refused.
    length: Option<String>,
    length_mm: Option<rust_decimal::Decimal>,
    quantity: u32,
    /// Stock is only ever used by a calculation if this is true.
    #[serde(default)]
    confirmed: bool,
    note: Option<String>,
}

fn stock_item(b: StockBody) -> Result<NewStockItem, ApiError> {
    let length_mm = match (&b.length, b.length_mm) {
        (Some(t), None) => amber_engine::parse_length(t, None)?.mm(),
        (None, Some(mm)) => mm,
        _ => return Err(ApiError::BadRequest("give either 'length' or 'length_mm'".into())),
    };
    if length_mm <= rust_decimal::Decimal::ZERO {
        return Err(ApiError::BadRequest("length must be above zero".into()));
    }
    if b.quantity == 0 || b.quantity > 10_000 {
        return Err(ApiError::BadRequest("quantity must be from 1 to 10,000".into()));
    }
    let note = b.note.map(|n| n.trim().to_string()).filter(|n| !n.is_empty());
    Ok(NewStockItem { profile: b.profile, length_mm, quantity: b.quantity, confirmed: b.confirmed, note })
}

async fn list_stock(State(s): State<AppState>) -> Result<Json<Vec<StockItem>>, ApiError> {
    Ok(Json(s.store.list_stock().await?))
}

async fn create_stock(State(s): State<AppState>, AppJson(b): AppJson<StockBody>) -> Result<(axum::http::StatusCode, Json<StockItem>), ApiError> {
    Ok((axum::http::StatusCode::CREATED, Json(s.store.save_stock(None, stock_item(b)?).await?)))
}

async fn update_stock(State(s): State<AppState>, Path(id): Path<Uuid>, AppJson(b): AppJson<StockBody>) -> Result<Json<StockItem>, ApiError> {
    Ok(Json(s.store.save_stock(Some(id), stock_item(b)?).await?))
}

async fn delete_stock(State(s): State<AppState>, Path(id): Path<Uuid>) -> Result<axum::http::StatusCode, ApiError> {
    s.store.delete_stock(id).await?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}

async fn assistant_status(State(s): State<AppState>) -> Json<Value> {
    Json(json!({ "enabled": s.llm.is_some() }))
}

async fn assistant_chat(State(s): State<AppState>, user: AuthUser, AppJson(req): AppJson<crate::assistant::ChatRequest>) -> Result<Json<crate::assistant::ChatResponse>, ApiError> {
    Ok(Json(crate::assistant::chat(&s, &user.0, req).await?))
}

// ---- business profile: letterhead, payment details and logo

const MAX_FIELD: usize = 200;
const MAX_BLOCK: usize = 1000;

fn check_profile(p: &crate::store::BusinessProfile) -> Result<(), ApiError> {
    let short = [("company name", &p.company_name), ("phone", &p.phone), ("email", &p.email), ("website", &p.website), ("tax PIN", &p.tax_pin)];
    for (name, v) in short {
        if v.chars().count() > MAX_FIELD || v.contains('\n') {
            return Err(ApiError::BadRequest(format!("{name} must be one line of at most {MAX_FIELD} characters")));
        }
    }
    let blocks = [("address", &p.address), ("payment details", &p.payment_details), ("default payment terms", &p.default_payment_terms), ("footer note", &p.footer_note)];
    for (name, v) in blocks {
        if v.chars().count() > MAX_BLOCK || v.lines().count() > 10 {
            return Err(ApiError::BadRequest(format!("{name} is too long (at most {MAX_BLOCK} characters and 10 lines)")));
        }
    }
    if matches!(p.quote_valid_days, Some(d) if d == 0 || d > 365) {
        return Err(ApiError::BadRequest("quote validity must be from 1 to 365 days".into()));
    }
    Ok(())
}

async fn get_business(State(s): State<AppState>) -> Result<Json<crate::store::BusinessProfile>, ApiError> {
    Ok(Json(s.store.get_business().await?))
}

async fn put_business(State(s): State<AppState>, user: AuthUser, AppJson(p): AppJson<crate::store::BusinessProfile>) -> Result<Json<crate::store::BusinessProfile>, ApiError> {
    user.require_admin()?;
    check_profile(&p)?;
    s.store.set_business(p.clone(), Some(user.0.username)).await?;
    Ok(Json(p))
}

async fn get_logo(State(s): State<AppState>) -> Result<impl axum::response::IntoResponse, ApiError> {
    let bytes = s.store.get_logo().await?.ok_or(ApiError::NotFound)?;
    Ok(([(axum::http::header::CONTENT_TYPE, "image/jpeg")], bytes))
}

/// Upload the logo as the raw JPEG bytes (the request body).
async fn put_logo(State(s): State<AppState>, user: AuthUser, body: axum::body::Bytes) -> Result<Json<Value>, ApiError> {
    user.require_admin()?;
    if body.len() > crate::quote::MAX_LOGO_BYTES {
        return Err(ApiError::BadRequest(format!("the logo is too big: keep it under {} KB", crate::quote::MAX_LOGO_BYTES / 1024)));
    }
    let info = crate::quote::jpeg_info(&body).map_err(ApiError::BadRequest)?;
    s.store.set_logo(Some(body.to_vec())).await?;
    Ok(Json(json!({ "width": info.width, "height": info.height, "bytes": body.len() })))
}

async fn delete_logo(State(s): State<AppState>, user: AuthUser) -> Result<axum::http::StatusCode, ApiError> {
    user.require_admin()?;
    s.store.set_logo(None).await?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}

/// Healthy only if the database answers, so Docker and uptime monitors see real problems.
async fn health(State(s): State<AppState>) -> (axum::http::StatusCode, Json<Value>) {
    match s.store.current_rules().await {
        Ok(_) => (axum::http::StatusCode::OK, Json(json!({ "status": "ok" }))),
        Err(_) => (axum::http::StatusCode::SERVICE_UNAVAILABLE, Json(json!({ "status": "database unavailable" }))),
    }
}
