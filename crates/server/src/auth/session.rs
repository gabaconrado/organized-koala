//! The `AuthUser` extractor: turns a verified `Authorization: Bearer <jwt>` into the
//! authenticated user's id, rejecting anything missing/malformed/expired as
//! `401 unauthenticated`.

use axum::extract::FromRequestParts;
use axum::http::header::AUTHORIZATION;
use axum::http::request::Parts;
use uuid::Uuid;

use crate::app::AppState;
use crate::error::ApiError;

/// The authenticated caller, extracted from a valid session token. Handlers that take this
/// are guaranteed a verified user id; a request without a valid token never reaches them.
#[derive(Debug, Clone, Copy)]
pub struct AuthUser {
    /// The authenticated user's id.
    pub user_id: Uuid,
}

impl FromRequestParts<AppState> for AuthUser {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let token = parts
            .headers
            .get(AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.strip_prefix("Bearer "))
            .map(str::trim)
            .filter(|token| !token.is_empty())
            .ok_or(ApiError::Unauthenticated)?;

        let user_id = state.jwt().verify(token).ok_or(ApiError::Unauthenticated)?;
        Ok(Self { user_id })
    }
}
