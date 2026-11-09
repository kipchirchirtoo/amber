use crate::store::StoreError;
use amber_engine::EngineError;
use axum::extract::rejection::JsonRejection;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

#[derive(Debug)]
pub enum ApiError {
    BadRequest(String),
    /// The input is well-formed but the engine refuses it (e.g. window too small).
    Unprocessable(String),
    /// No valid login.
    Unauthorized,
    /// Logged in, but not allowed (e.g. staff trying an administrator action).
    Forbidden,
    /// Wrong username or password (deliberately does not say which).
    BadLogin,
    TooManyAttempts,
    AssistantDisabled,
    NotFound,
    Internal(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, code, message) = match self {
            ApiError::BadRequest(m) => (StatusCode::BAD_REQUEST, "bad_request", m),
            ApiError::Unprocessable(m) => (StatusCode::UNPROCESSABLE_ENTITY, "unprocessable", m),
            ApiError::Unauthorized => (StatusCode::UNAUTHORIZED, "unauthorized", "please sign in".to_string()),
            ApiError::Forbidden => (StatusCode::FORBIDDEN, "forbidden", "only an administrator can do this".to_string()),
            ApiError::BadLogin => (StatusCode::UNAUTHORIZED, "bad_login", "wrong username or password".to_string()),
            ApiError::TooManyAttempts => (
                StatusCode::TOO_MANY_REQUESTS,
                "too_many_attempts",
                "too many wrong passwords: wait a minute and try again".to_string(),
            ),
            ApiError::AssistantDisabled => (
                StatusCode::SERVICE_UNAVAILABLE,
                "assistant_disabled",
                "the AI assistant is not switched on: the server needs ANTHROPIC_API_KEY, OPENAI_API_KEY, GEMINI_API_KEY or GROQ_API_KEY".to_string(),
            ),
            ApiError::NotFound => (StatusCode::NOT_FOUND, "not_found", "not found".to_string()),
            ApiError::Internal(m) => {
                tracing::error!("internal error: {m}");
                (StatusCode::INTERNAL_SERVER_ERROR, "internal", "internal error".to_string())
            }
        };
        (status, Json(json!({ "error": { "code": code, "message": message } }))).into_response()
    }
}

impl From<EngineError> for ApiError {
    fn from(e: EngineError) -> Self {
        ApiError::Unprocessable(e.to_string())
    }
}

impl From<StoreError> for ApiError {
    fn from(e: StoreError) -> Self {
        match e {
            StoreError::NotFound => ApiError::NotFound,
            StoreError::Conflict => ApiError::BadRequest("that already exists".into()),
            StoreError::Backend(m) => ApiError::Internal(m),
        }
    }
}

impl From<JsonRejection> for ApiError {
    fn from(r: JsonRejection) -> Self {
        ApiError::BadRequest(r.body_text())
    }
}

/// Json extractor that reports malformed bodies in the same JSON error shape.
#[derive(axum::extract::FromRequest)]
#[from_request(via(axum::Json), rejection(ApiError))]
pub struct AppJson<T>(pub T);
