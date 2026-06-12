//! Registration and login (ADR-0005 §2–3). Registration creates the user and their default
//! profile in one transaction; both endpoints return a session JWT.

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use contract::{LoginRequest, RegisterRequest, SessionResponse};
use uuid::Uuid;

use crate::app::AppState;
use crate::auth::{hash_password, verify_password};
use crate::error::{ApiError, ApiResult};

/// Max length for the free-text identity fields, surfaced as `validation_failed` past it.
const MAX_FIELD_LEN: usize = 256;

/// `POST /api/auth/register` → `201 SessionResponse`.
///
/// Validates the credentials, hashes the password with argon2, and creates the user plus a
/// default profile named `profile_name` in a single transaction (a user without a profile
/// cannot exist). A unique-violation on username/email maps to `409 username_taken` /
/// `email_taken`. On success the caller is logged in (same body as login).
#[tracing::instrument(skip_all, fields(username = %request.username))]
pub async fn register(
    State(state): State<AppState>,
    Json(request): Json<RegisterRequest>,
) -> ApiResult<(StatusCode, Json<SessionResponse>)> {
    let username = request.username.trim();
    let email = request.email.trim();
    let profile_name = request.profile_name.trim();

    validate_non_empty(username, "username")?;
    validate_len(username, "username")?;
    if username.contains('@') {
        return Err(ApiError::Validation(
            "username must not contain '@'".to_owned(),
        ));
    }
    validate_non_empty(email, "email")?;
    validate_len(email, "email")?;
    if !is_email(email) {
        return Err(ApiError::Validation("email is not valid".to_owned()));
    }
    validate_non_empty(profile_name, "profile_name")?;
    validate_len(profile_name, "profile_name")?;
    if request.password.expose().is_empty() {
        return Err(ApiError::Validation(
            "password must not be empty".to_owned(),
        ));
    }

    let password_hash = hash_password(&request.password).map_err(ApiError::Internal)?;

    let mut tx = state.pool().begin().await?;

    let existing = sqlx::query!(
        "SELECT username, email FROM users WHERE username = $1 OR email = $2",
        username,
        email,
    )
    .fetch_optional(&mut *tx)
    .await?;
    if let Some(row) = existing {
        return Err(if row.username == username {
            ApiError::UsernameTaken
        } else {
            ApiError::EmailTaken
        });
    }

    let user_id = sqlx::query_scalar!(
        "INSERT INTO users (username, email, password_hash) VALUES ($1, $2, $3) RETURNING id",
        username,
        email,
        password_hash,
    )
    .fetch_one(&mut *tx)
    .await?;

    let _inserted = sqlx::query!(
        "INSERT INTO profiles (user_id, name) VALUES ($1, $2)",
        user_id,
        profile_name,
    )
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    let token = state.jwt().issue(user_id).map_err(ApiError::Internal)?;
    tracing::info!(user_id = %user_id, "registered user with default profile");
    Ok((StatusCode::CREATED, Json(SessionResponse { token })))
}

/// `POST /api/auth/login` → `200 SessionResponse`.
///
/// `identifier` matches username or email. A missing user or password mismatch is uniformly
/// `401 invalid_credentials` — the two cases are indistinguishable to the caller. The argon2
/// verification still runs against a decoy hash for absent users so timing does not leak
/// existence.
#[tracing::instrument(skip_all)]
pub async fn login(
    State(state): State<AppState>,
    Json(request): Json<LoginRequest>,
) -> ApiResult<Json<SessionResponse>> {
    let identifier = request.identifier.trim();

    let row = sqlx::query!(
        "SELECT id, password_hash FROM users WHERE username = $1 OR email = $1",
        identifier,
    )
    .fetch_optional(state.pool())
    .await?;

    let (user_id, stored_hash): (Uuid, String) = match row {
        Some(row) => (row.id, row.password_hash),
        None => {
            // Verify against a decoy so a missing user costs the same as a wrong password.
            let _ = verify_password(&request.password, DECOY_HASH);
            return Err(ApiError::InvalidCredentials);
        }
    };

    if verify_password(&request.password, &stored_hash).map_err(ApiError::Internal)? {
        let token = state.jwt().issue(user_id).map_err(ApiError::Internal)?;
        Ok(Json(SessionResponse { token }))
    } else {
        Err(ApiError::InvalidCredentials)
    }
}

/// A valid argon2 PHC hash of a throwaway value, used to equalize login timing for absent
/// users (no real password ever hashes to this).
const DECOY_HASH: &str = "$argon2id$v=19$m=19456,t=2,p=1$PJFneJDMj6eX+DoxLlmp2Q$CmuP1bxqyUV2RK18TI/NEtEDqJ7n4hp9qHK5GKtY9jc";

/// A minimal email check: exactly one `@`, with non-empty local and domain parts.
fn is_email(value: &str) -> bool {
    let mut parts = value.split('@');
    match (parts.next(), parts.next(), parts.next()) {
        (Some(local), Some(domain), None) => !local.is_empty() && !domain.is_empty(),
        _ => false,
    }
}

/// Reject an empty trimmed field as `validation_failed`.
fn validate_non_empty(value: &str, field: &str) -> ApiResult<()> {
    if value.is_empty() {
        Err(ApiError::Validation(format!("{field} must not be empty")))
    } else {
        Ok(())
    }
}

/// Reject an over-long field as `validation_failed`.
fn validate_len(value: &str, field: &str) -> ApiResult<()> {
    if value.chars().count() > MAX_FIELD_LEN {
        Err(ApiError::Validation(format!(
            "{field} must be at most {MAX_FIELD_LEN} characters"
        )))
    } else {
        Ok(())
    }
}
