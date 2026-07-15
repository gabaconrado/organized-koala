//! Server-only wrapper extractors that map axum's built-in extractor rejections onto the
//! ADR-0005 error contract (`{ code?, message }` JSON), so a malformed request never escapes
//! the envelope the rest of the API honours.

use axum::extract::{FromRequestParts, Query};
use axum::http::request::Parts;
use serde::de::DeserializeOwned;

use crate::error::ApiError;

/// A query-string extractor that mirrors [`axum::extract::Query`] but renders a
/// deserialization failure as [`ApiError::Validation`] (`400` + `validation_failed` + JSON
/// [`contract::ErrorBody`]) instead of axum's default plain-text rejection. Handlers that take
/// this get the same `T` as `Query<T>` while malformed query params (e.g. `?limit=`,
/// `?limit=abc`) stay inside the standard error contract.
#[derive(Debug, Clone, Copy)]
pub struct ValidatedQuery<T>(
    /// The successfully deserialized query parameters.
    pub T,
);

impl<T, S> FromRequestParts<S> for ValidatedQuery<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        match Query::<T>::from_request_parts(parts, state).await {
            Ok(Query(value)) => Ok(Self(value)),
            // `body_text()` carries only the caller's own malformed-input detail (e.g. "cannot
            // parse integer from empty string") — client-safe, no server internals.
            Err(rejection) => Err(ApiError::Validation(rejection.body_text())),
        }
    }
}
