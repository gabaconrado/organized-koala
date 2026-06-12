//! The server's boundary error type and its mapping to the HTTP error contract.
//!
//! Handlers return [`ApiError`]; its [`IntoResponse`] impl renders the ADR-0005 error body
//! (`contract::ErrorBody`, a `{ code?, message }` JSON) with the matching HTTP status. The
//! `Internal` variant carries an `anyhow::Error` for logging but never leaks it to the
//! client — the wire message is always generic.

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use contract::{ErrorBody, ErrorCode};

/// A handler error mapped to the HTTP error contract at the boundary.
#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    /// Request body failed validation (400 `validation_failed`).
    #[error("{0}")]
    Validation(String),
    /// Login identifier/password mismatch (401 `invalid_credentials`).
    #[error("invalid credentials")]
    InvalidCredentials,
    /// Missing, malformed, or expired token (401 `unauthenticated`).
    #[error("unauthenticated")]
    Unauthenticated,
    /// Resource absent or not owned by the caller (404 `not_found`).
    #[error("not found")]
    NotFound,
    /// Registration username already exists (409 `username_taken`).
    #[error("username already taken")]
    UsernameTaken,
    /// Registration email already exists (409 `email_taken`).
    #[error("email already taken")]
    EmailTaken,
    /// Unexpected server error (500 `internal`). The cause is logged, never sent to clients.
    #[error("internal error")]
    Internal(#[source] anyhow::Error),
}

impl ApiError {
    /// HTTP status + stable code + the public, client-safe message for this error.
    fn parts(&self) -> (StatusCode, ErrorCode, String) {
        match self {
            Self::Validation(message) => (
                StatusCode::BAD_REQUEST,
                ErrorCode::ValidationFailed,
                message.clone(),
            ),
            Self::InvalidCredentials => (
                StatusCode::UNAUTHORIZED,
                ErrorCode::InvalidCredentials,
                "invalid credentials".to_owned(),
            ),
            Self::Unauthenticated => (
                StatusCode::UNAUTHORIZED,
                ErrorCode::Unauthenticated,
                "authentication required".to_owned(),
            ),
            Self::NotFound => (
                StatusCode::NOT_FOUND,
                ErrorCode::NotFound,
                "not found".to_owned(),
            ),
            Self::UsernameTaken => (
                StatusCode::CONFLICT,
                ErrorCode::UsernameTaken,
                "username already taken".to_owned(),
            ),
            Self::EmailTaken => (
                StatusCode::CONFLICT,
                ErrorCode::EmailTaken,
                "email already taken".to_owned(),
            ),
            Self::Internal(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                ErrorCode::Internal,
                "an unexpected error occurred".to_owned(),
            ),
        }
    }
}

/// Any unclassified failure (DB, hashing, JWT signing) becomes a logged `Internal`.
impl From<anyhow::Error> for ApiError {
    fn from(error: anyhow::Error) -> Self {
        Self::Internal(error)
    }
}

impl From<sqlx::Error> for ApiError {
    fn from(error: sqlx::Error) -> Self {
        Self::Internal(error.into())
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, code, message) = self.parts();
        // Record the full cause server-side; the client only ever sees the generic message.
        if let Self::Internal(cause) = &self {
            tracing::error!(error = %cause, "request failed with an internal error");
        }
        let body = ErrorBody {
            code: Some(code),
            message,
        };
        (status, Json(body)).into_response()
    }
}

/// Convenience alias for handler results.
pub type ApiResult<T> = Result<T, ApiError>;
