//! Profile discovery (ADR-0005 §4): the caller's own profiles, newest-first.

use axum::Json;
use axum::extract::State;
use contract::Profile;

use crate::app::AppState;
use crate::auth::AuthUser;
use crate::error::ApiResult;

/// `GET /api/profiles` → `200` array of the authenticated user's profiles, newest-first.
/// Scoped to the caller: only profiles they own are returned.
#[tracing::instrument(skip_all, fields(user_id = %user.user_id))]
pub async fn list_profiles(
    State(state): State<AppState>,
    user: AuthUser,
) -> ApiResult<Json<Vec<Profile>>> {
    let rows = sqlx::query!(
        "SELECT id, name, created_at FROM profiles WHERE user_id = $1 ORDER BY created_at DESC",
        user.user_id,
    )
    .fetch_all(state.pool())
    .await?;

    let profiles = rows
        .into_iter()
        .map(|row| Profile {
            id: row.id.to_string(),
            name: row.name,
            created_at: row.created_at,
        })
        .collect();
    Ok(Json(profiles))
}
