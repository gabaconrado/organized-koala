//! Profile management (ADR-0005 §4, ADR-0009): the caller's own profiles plus create / rename
//! / delete. Every query is ownership-scoped on the authenticated user, so a profile the
//! caller does not own is `404 not_found` (never 403). Names are unique per account — the DB
//! `UNIQUE (user_id, name)` constraint is mapped to `409 profile_name_taken` at this boundary
//! (race-safe; no TOCTOU pre-check). Deleting a profile cascades its tasks and notes via the
//! FK `ON DELETE CASCADE` (no app-level fan-out), and the account always retains ≥1 profile
//! (deleting the last → `409 last_profile`).

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use chrono::{DateTime, Utc};
use contract::{CreateProfileRequest, Profile, UpdateProfileRequest};
use uuid::Uuid;

use crate::app::AppState;
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};

/// A profile row as stored, before mapping to the wire [`Profile`].
struct ProfileRow {
    id: Uuid,
    name: String,
    created_at: DateTime<Utc>,
}

impl ProfileRow {
    /// Map a stored row to the wire DTO.
    fn into_profile(self) -> Profile {
        Profile {
            id: self.id.to_string(),
            name: self.name,
            created_at: self.created_at,
        }
    }
}

/// Validate and trim a profile name, returning the trimmed value or `400 validation_failed`.
fn trimmed_name(name: &str) -> ApiResult<&str> {
    let name = name.trim();
    if name.is_empty() {
        return Err(ApiError::Validation("name must not be empty".to_owned()));
    }
    Ok(name)
}

/// Map a sqlx error to `409 profile_name_taken` when it is the per-account unique-name
/// violation, leaving any other error to bubble as the boundary type would otherwise map it.
fn map_name_conflict(error: sqlx::Error) -> ApiError {
    match &error {
        sqlx::Error::Database(db) if db.is_unique_violation() => ApiError::ProfileNameTaken,
        _ => ApiError::from(error),
    }
}

/// `GET /api/profiles` → `200` array of the authenticated user's profiles, oldest-first
/// (ascending insertion order). Scoped to the caller: only profiles they own are returned.
#[tracing::instrument(skip_all, fields(user_id = %user.user_id))]
pub async fn list_profiles(
    State(state): State<AppState>,
    user: AuthUser,
) -> ApiResult<Json<Vec<Profile>>> {
    let rows = sqlx::query_as!(
        ProfileRow,
        "SELECT id, name, created_at FROM profiles WHERE user_id = $1 ORDER BY created_at ASC",
        user.user_id,
    )
    .fetch_all(state.pool())
    .await?;

    Ok(Json(
        rows.into_iter().map(ProfileRow::into_profile).collect(),
    ))
}

/// `POST /api/profiles` → `201 Profile`. Name must be non-empty after trimming (else
/// `400 validation_failed`); the stored name is the trimmed value. A duplicate per-account
/// name → `409 profile_name_taken`, mapped from the DB unique-violation (no pre-check race).
#[tracing::instrument(skip_all, fields(user_id = %user.user_id))]
pub async fn create_profile(
    State(state): State<AppState>,
    user: AuthUser,
    Json(request): Json<CreateProfileRequest>,
) -> ApiResult<(StatusCode, Json<Profile>)> {
    let name = trimmed_name(&request.name)?;

    let row = sqlx::query_as!(
        ProfileRow,
        "INSERT INTO profiles (user_id, name) VALUES ($1, $2) RETURNING id, name, created_at",
        user.user_id,
        name,
    )
    .fetch_one(state.pool())
    .await
    .map_err(map_name_conflict)?;

    let profile = row.into_profile();
    tracing::info!(profile_id = %profile.id, "created profile");
    Ok((StatusCode::CREATED, Json(profile)))
}

/// `PATCH /api/profiles/{id}` → `200 Profile`. Renames in place. Name must be non-empty after
/// trimming (else `400 validation_failed`); duplicate per-account name → `409 profile_name_taken`
/// (mapped from the DB unique-violation). Unowned/missing → `404 not_found` (ownership-joined).
#[tracing::instrument(skip_all, fields(user_id = %user.user_id, profile_id = %profile_id))]
pub async fn rename_profile(
    State(state): State<AppState>,
    user: AuthUser,
    Path(profile_id): Path<Uuid>,
    Json(request): Json<UpdateProfileRequest>,
) -> ApiResult<Json<Profile>> {
    let name = trimmed_name(&request.name)?;

    let row = sqlx::query_as!(
        ProfileRow,
        "UPDATE profiles SET name = $1 WHERE id = $2 AND user_id = $3 \
         RETURNING id, name, created_at",
        name,
        profile_id,
        user.user_id,
    )
    .fetch_optional(state.pool())
    .await
    .map_err(map_name_conflict)?
    .ok_or(ApiError::NotFound)?;

    let profile = row.into_profile();
    tracing::info!(profile_id = %profile.id, "renamed profile");
    Ok(Json(profile))
}

/// `DELETE /api/profiles/{id}` → `204 No Content`. The profile's tasks and notes cascade via
/// the FK `ON DELETE CASCADE` (no app-level fan-out). The account must retain ≥1 profile, so
/// deleting the only remaining profile → `409 last_profile`; unowned/missing → `404 not_found`.
///
/// The guard is a single statement that deletes only when the account holds more than one
/// profile, so two concurrent deletes cannot both pass and empty the account. When it removes
/// nothing, a follow-up ownership check distinguishes the last-profile refusal (409) from an
/// unowned/missing profile (404).
#[tracing::instrument(skip_all, fields(user_id = %user.user_id, profile_id = %profile_id))]
pub async fn delete_profile(
    State(state): State<AppState>,
    user: AuthUser,
    Path(profile_id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    let result = sqlx::query!(
        "DELETE FROM profiles \
         WHERE id = $1 AND user_id = $2 \
         AND (SELECT count(*) FROM profiles WHERE user_id = $2) > 1",
        profile_id,
        user.user_id,
    )
    .execute(state.pool())
    .await?;

    if result.rows_affected() == 0 {
        // Either the profile is unowned/missing (404) or it is the account's last one (409).
        let owned = sqlx::query_scalar!(
            "SELECT EXISTS(SELECT 1 FROM profiles WHERE id = $1 AND user_id = $2)",
            profile_id,
            user.user_id,
        )
        .fetch_one(state.pool())
        .await?
        .unwrap_or(false);

        return Err(if owned {
            ApiError::LastProfile
        } else {
            ApiError::NotFound
        });
    }

    tracing::info!(%profile_id, "deleted profile");
    Ok(StatusCode::NO_CONTENT)
}
