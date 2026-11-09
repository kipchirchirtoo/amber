//! Individual user accounts: login, sessions, roles.
//!
//! - Passwords are stored only as Argon2id hashes.
//! - A login returns a random token. Only its SHA-256 hash is stored, so a
//!   database leak does not hand out working logins. Tokens expire after 7 days
//!   and stop working at once when a user is deactivated or their password changes.
//! - Repeated wrong passwords for one username are slowed down (5 tries, then 60 s).

use crate::api::AppState;
use crate::error::{ApiError, AppJson};
use crate::store::{NewUser, Role, StoreError, User, UserUpdate};
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use axum::extract::{FromRequestParts, Path, Request, State};
use axum::http::request::Parts;
use axum::http::{header, HeaderMap};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::Json;
use chrono::{Duration, Utc};
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Instant;
use uuid::Uuid;

const SESSION_DAYS: i64 = 7;
const MIN_PASSWORD: usize = 8;
const MAX_FAILS: u32 = 5;
const LOCK_SECS: u64 = 60;
const TICKET_SECS: u64 = 120;

/// The signed-in user, available to any handler behind the login check.
#[derive(Clone, Debug)]
pub struct AuthUser(pub User);

impl AuthUser {
    pub fn require_admin(&self) -> Result<(), ApiError> {
        if self.0.role == Role::Admin {
            Ok(())
        } else {
            Err(ApiError::Forbidden)
        }
    }
}

impl<S: Send + Sync> FromRequestParts<S> for AuthUser {
    type Rejection = ApiError;
    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, ApiError> {
        parts.extensions.get::<AuthUser>().cloned().ok_or(ApiError::Unauthorized)
    }
}

// ---------------------------------------------------------------- shared state

/// Failed-login counters and short-lived PDF download tickets (in memory).
#[derive(Default)]
pub struct AuthState {
    fails: Mutex<HashMap<String, (u32, Instant)>>,
    tickets: Mutex<HashMap<String, (Uuid, Instant)>>,
}

impl AuthState {
    fn locked(&self, name: &str) -> bool {
        let g = self.fails.lock().unwrap();
        matches!(g.get(name), Some((n, at)) if *n >= MAX_FAILS && at.elapsed().as_secs() < LOCK_SECS)
    }
    fn fail(&self, name: &str) {
        let mut g = self.fails.lock().unwrap();
        if g.len() > 10_000 {
            g.retain(|_, (_, at)| at.elapsed().as_secs() < LOCK_SECS);
        }
        let e = g.entry(name.to_string()).or_insert((0, Instant::now()));
        if e.1.elapsed().as_secs() >= LOCK_SECS {
            e.0 = 0;
        }
        *e = (e.0 + 1, Instant::now());
    }
    fn clear(&self, name: &str) {
        self.fails.lock().unwrap().remove(name);
    }
    fn new_ticket(&self, user: Uuid) -> String {
        let t = random_token();
        let mut g = self.tickets.lock().unwrap();
        g.retain(|_, (_, at)| at.elapsed().as_secs() < TICKET_SECS);
        g.insert(t.clone(), (user, Instant::now()));
        t
    }
    fn ticket_user(&self, t: &str) -> Option<Uuid> {
        let g = self.tickets.lock().unwrap();
        g.get(t).filter(|(_, at)| at.elapsed().as_secs() < TICKET_SECS).map(|(u, _)| *u)
    }
}

// --------------------------------------------------------------------- crypto

pub fn random_token() -> String {
    format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple())
}

pub fn hash_token(token: &str) -> String {
    Sha256::digest(token.as_bytes()).iter().map(|b| format!("{b:02x}")).collect()
}

fn hash_blocking(password: &str) -> Result<String, ApiError> {
    // 16 random bytes from the OS random generator (a version-4 UUID is exactly that)
    let salt = SaltString::encode_b64(Uuid::new_v4().as_bytes()).map_err(|e| ApiError::Internal(e.to_string()))?;
    Argon2::default().hash_password(password.as_bytes(), &salt).map(|h| h.to_string()).map_err(|e| ApiError::Internal(e.to_string()))
}

pub async fn hash_password(password: &str) -> Result<String, ApiError> {
    let p = password.to_string();
    tokio::task::spawn_blocking(move || hash_blocking(&p)).await.map_err(|e| ApiError::Internal(e.to_string()))?
}

async fn verify_password(password: &str, hash: &str) -> bool {
    let (p, h) = (password.to_string(), hash.to_string());
    tokio::task::spawn_blocking(move || match PasswordHash::new(&h) {
        Ok(parsed) => Argon2::default().verify_password(p.as_bytes(), &parsed).is_ok(),
        Err(_) => false,
    })
    .await
    .unwrap_or(false)
}

fn bearer(h: &HeaderMap) -> Option<&str> {
    h.get(header::AUTHORIZATION)?.to_str().ok()?.strip_prefix("Bearer ").map(str::trim).filter(|t| !t.is_empty())
}

fn query_param<'a>(q: Option<&'a str>, key: &str) -> Option<&'a str> {
    q?.split('&').find_map(|kv| kv.strip_prefix(key)?.strip_prefix('='))
}

// ----------------------------------------------------------------- middleware

/// Every route behind this needs a valid login. The one exception: a quotation
/// PDF may be opened in a browser with a short-lived `ticket` (see `ticket`),
/// because a browser tab cannot send the login header.
pub async fn require_login(State(s): State<AppState>, mut req: Request, next: Next) -> Response {
    let mut user = None;
    if let Some(t) = bearer(req.headers()) {
        match s.store.session_user(&hash_token(t)).await {
            Ok(u) => user = u,
            Err(e) => return ApiError::from(e).into_response(),
        }
    } else if req.uri().path().ends_with("/quote.pdf") {
        if let Some(uid) = query_param(req.uri().query(), "ticket").and_then(|t| s.auth.ticket_user(t)) {
            user = s.store.get_user(uid).await.ok().filter(|u| u.active);
        }
    }
    match user {
        Some(u) => {
            req.extensions_mut().insert(AuthUser(u));
            next.run(req).await
        }
        None => ApiError::Unauthorized.into_response(),
    }
}

// ------------------------------------------------------------------- handlers

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LoginBody {
    username: String,
    password: String,
}

fn user_json(u: &User) -> Value {
    json!({ "id": u.id, "username": u.username, "display_name": u.display_name, "role": u.role, "active": u.active })
}

async fn start_session(s: &AppState, u: &User) -> Result<Value, ApiError> {
    let token = random_token();
    let expires = Utc::now() + Duration::days(SESSION_DAYS);
    s.store.create_session(&hash_token(&token), u.id, expires).await?;
    Ok(json!({ "token": token, "expires_at": expires, "user": user_json(u) }))
}

pub async fn login(State(s): State<AppState>, AppJson(b): AppJson<LoginBody>) -> Result<Json<Value>, ApiError> {
    let name = b.username.trim().to_lowercase();
    if s.auth.locked(&name) {
        return Err(ApiError::TooManyAttempts);
    }
    let found = s.store.user_with_hash(&name).await?;
    // Always run one Argon2 verification so a wrong username takes as long as a wrong password.
    let ok_user = match &found {
        Some((u, hash)) => verify_password(&b.password, hash).await.then_some(u.clone()),
        None => {
            verify_password(&b.password, dummy_hash()).await;
            None
        }
    };
    match ok_user.filter(|u| u.active) {
        Some(u) => {
            s.auth.clear(&name);
            Ok(Json(start_session(&s, &u).await?))
        }
        None => {
            s.auth.fail(&name);
            Err(ApiError::BadLogin)
        }
    }
}

fn dummy_hash() -> &'static str {
    static H: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    H.get_or_init(|| hash_blocking("not-a-real-password").expect("hashing works"))
}

pub async fn logout(State(s): State<AppState>, headers: HeaderMap) -> Result<Json<Value>, ApiError> {
    if let Some(t) = bearer(&headers) {
        s.store.delete_session(&hash_token(t)).await?;
    }
    Ok(Json(json!({ "ok": true })))
}

pub async fn me(user: AuthUser) -> Json<Value> {
    Json(user_json(&user.0))
}

/// A 2-minute ticket that lets a browser tab open a quotation PDF.
pub async fn ticket(State(s): State<AppState>, user: AuthUser) -> Json<Value> {
    Json(json!({ "ticket": s.auth.new_ticket(user.0.id), "valid_seconds": TICKET_SECS }))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChangePassword {
    current_password: String,
    new_password: String,
}

fn check_password_strength(p: &str) -> Result<(), ApiError> {
    if p.chars().count() < MIN_PASSWORD || p.len() > 200 {
        return Err(ApiError::BadRequest(format!("password must be at least {MIN_PASSWORD} characters")));
    }
    Ok(())
}

/// Changes your own password, signs out every other device, and returns a fresh login.
pub async fn change_password(State(s): State<AppState>, user: AuthUser, AppJson(b): AppJson<ChangePassword>) -> Result<Json<Value>, ApiError> {
    check_password_strength(&b.new_password)?;
    let (u, hash) = s.store.user_with_hash(&user.0.username).await?.ok_or(ApiError::Unauthorized)?;
    if !verify_password(&b.current_password, &hash).await {
        return Err(ApiError::BadRequest("the current password is wrong".into()));
    }
    let new_hash = hash_password(&b.new_password).await?;
    s.store.update_user(u.id, UserUpdate { password_hash: Some(new_hash), ..Default::default() }).await?;
    s.store.delete_user_sessions(u.id).await?;
    Ok(Json(start_session(&s, &u).await?))
}

// ---- user management (administrators only)

pub async fn list_users(State(s): State<AppState>, user: AuthUser) -> Result<Json<Vec<User>>, ApiError> {
    user.require_admin()?;
    Ok(Json(s.store.list_users().await?))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateUser {
    username: String,
    display_name: String,
    role: Role,
    password: String,
}

fn valid_username(u: &str) -> bool {
    (3..=32).contains(&u.len()) && u.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"._-@".contains(&b))
}

pub async fn create_user(State(s): State<AppState>, user: AuthUser, AppJson(b): AppJson<CreateUser>) -> Result<(axum::http::StatusCode, Json<User>), ApiError> {
    user.require_admin()?;
    let username = b.username.trim().to_lowercase();
    if !valid_username(&username) {
        return Err(ApiError::BadRequest("username must be 3 to 32 characters: lower-case letters, digits, '.', '_', '-' or '@'".into()));
    }
    let display = b.display_name.trim().to_string();
    if display.is_empty() || display.chars().count() > 80 {
        return Err(ApiError::BadRequest("display name must be 1 to 80 characters".into()));
    }
    check_password_strength(&b.password)?;
    let password_hash = hash_password(&b.password).await?;
    match s.store.create_user(NewUser { username, display_name: display, role: b.role, password_hash }).await {
        Ok(u) => Ok((axum::http::StatusCode::CREATED, Json(u))),
        Err(StoreError::Conflict) => Err(ApiError::BadRequest("that username is already taken".into())),
        Err(e) => Err(e.into()),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpdateUser {
    display_name: Option<String>,
    role: Option<Role>,
    active: Option<bool>,
    new_password: Option<String>,
}

pub async fn update_user(State(s): State<AppState>, user: AuthUser, Path(id): Path<Uuid>, AppJson(b): AppJson<UpdateUser>) -> Result<Json<User>, ApiError> {
    user.require_admin()?;
    let target = s.store.get_user(id).await?;
    let loses_admin = target.role == Role::Admin && target.active && (b.role == Some(Role::Staff) || b.active == Some(false));
    if loses_admin {
        let admins = s.store.list_users().await?.iter().filter(|u| u.role == Role::Admin && u.active).count();
        if admins <= 1 {
            return Err(ApiError::BadRequest("at least one active administrator must remain".into()));
        }
    }
    let display_name = match b.display_name {
        Some(n) if n.trim().is_empty() || n.chars().count() > 80 => return Err(ApiError::BadRequest("display name must be 1 to 80 characters".into())),
        Some(n) => Some(n.trim().to_string()),
        None => None,
    };
    let password_hash = match &b.new_password {
        Some(p) => {
            check_password_strength(p)?;
            Some(hash_password(p).await?)
        }
        None => None,
    };
    let sign_out = b.new_password.is_some() || b.active == Some(false) || b.role.is_some_and(|r| r != target.role);
    let updated = s.store.update_user(id, UserUpdate { display_name, role: b.role, active: b.active, password_hash }).await?;
    if sign_out {
        s.store.delete_user_sessions(id).await?;
    }
    Ok(Json(updated))
}

/// Creates the first administrator when the user table is empty. Called at start-up.
pub async fn bootstrap_admin(s: &AppState, username: &str, password: &str) -> Result<bool, ApiError> {
    if s.store.count_users().await? > 0 {
        return Ok(false);
    }
    let username = username.trim().to_lowercase();
    if !valid_username(&username) {
        return Err(ApiError::BadRequest("bootstrap username is not valid".into()));
    }
    check_password_strength(password)?;
    let password_hash = hash_password(password).await?;
    s.store.create_user(NewUser { username, display_name: "Administrator".into(), role: Role::Admin, password_hash }).await?;
    Ok(true)
}
