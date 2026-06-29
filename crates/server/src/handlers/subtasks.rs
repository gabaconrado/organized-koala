//! Profile-scoped sub-task handlers (ADR-0012/ADR-0013). A sub-task is the bounded
//! title+status-only child of a task; it is reached only through its parent task, which in
//! turn is reached only through the caller's owned profile. Every query passes the
//! `assert_owned(profile_id)` gate first, then joins `subtasks → tasks` filtering
//! `tasks.profile_id = $pid`, so neither a cross-profile profile nor a wrong parent task is
//! reachable — both surface as `404 not_found` (never 403), indistinguishable from absent.
//! The no-orphans guarantee on task/profile delete is the FK `ON DELETE CASCADE`, not handler
//! code (hard-constraint #4 / ADR-0012 §4).

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use contract::{CreateSubtaskRequest, Subtask, TaskStatus, UpdateSubtaskRequest};
use uuid::Uuid;

use crate::app::AppState;
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};

/// A sub-task row as stored, before mapping to the wire [`Subtask`]. `created_at` is selected
/// only to drive creation-order sorting and is never carried onto the wire (ADR-0012 §1).
struct SubtaskRow {
    id: Uuid,
    task_id: Uuid,
    title: String,
    status: String,
}

impl SubtaskRow {
    /// Map a stored row to the wire DTO. An unrecognized status defaults to `open`; the DB
    /// `CHECK` constraint makes that branch unreachable in practice.
    fn into_subtask(self) -> Subtask {
        let status = match self.status.as_str() {
            "done" => TaskStatus::Done,
            _ => TaskStatus::Open,
        };
        Subtask {
            id: self.id.to_string(),
            task_id: self.task_id.to_string(),
            title: self.title,
            status,
        }
    }
}

/// Confirm the caller owns `profile_id`, returning `404 not_found` otherwise. This is the
/// single ownership gate every sub-task route passes through before its parent-scoped query.
async fn assert_owned(state: &AppState, user_id: Uuid, profile_id: Uuid) -> ApiResult<()> {
    let owned = sqlx::query_scalar!(
        "SELECT EXISTS(SELECT 1 FROM profiles WHERE id = $1 AND user_id = $2)",
        profile_id,
        user_id,
    )
    .fetch_one(state.pool())
    .await?
    .unwrap_or(false);

    if owned {
        Ok(())
    } else {
        Err(ApiError::NotFound)
    }
}

/// Map a wire [`TaskStatus`] to its stored string form (`open` / `done`).
fn status_str(status: TaskStatus) -> &'static str {
    match status {
        TaskStatus::Open => "open",
        TaskStatus::Done => "done",
    }
}

/// `GET /api/profiles/{pid}/tasks/{tid}/subtasks` → `200` bare array, creation order
/// (`created_at ASC`). The join requires the parent task to exist and belong to `{pid}`; an
/// unowned profile or wrong/missing parent yields an empty array (the rows simply do not match).
/// Unowned profile → 404 (the ownership gate).
#[tracing::instrument(skip_all, fields(user_id = %user.user_id, profile_id = %profile_id, task_id = %task_id))]
pub async fn list_subtasks(
    State(state): State<AppState>,
    user: AuthUser,
    Path((profile_id, task_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<Json<Vec<Subtask>>> {
    assert_owned(&state, user.user_id, profile_id).await?;

    let rows = sqlx::query_as!(
        SubtaskRow,
        "SELECT s.id, s.task_id, s.title, s.status \
         FROM subtasks s JOIN tasks t ON t.id = s.task_id \
         WHERE s.task_id = $1 AND t.profile_id = $2 \
         ORDER BY s.created_at ASC",
        task_id,
        profile_id,
    )
    .fetch_all(state.pool())
    .await?;

    Ok(Json(
        rows.into_iter().map(SubtaskRow::into_subtask).collect(),
    ))
}

/// `POST /api/profiles/{pid}/tasks/{tid}/subtasks` → `201 Subtask`. Title must be non-empty
/// after trimming (else `400 validation_failed`); a new sub-task always starts `open`. The
/// parent task must exist and belong to `{pid}` (the insert is guarded by a profile-joined
/// `SELECT` of the parent) → else `404 not_found`. Unowned profile → 404.
#[tracing::instrument(skip_all, fields(user_id = %user.user_id, profile_id = %profile_id, task_id = %task_id))]
pub async fn create_subtask(
    State(state): State<AppState>,
    user: AuthUser,
    Path((profile_id, task_id)): Path<(Uuid, Uuid)>,
    Json(request): Json<CreateSubtaskRequest>,
) -> ApiResult<(StatusCode, Json<Subtask>)> {
    assert_owned(&state, user.user_id, profile_id).await?;

    let title = request.title.trim();
    if title.is_empty() {
        return Err(ApiError::Validation("title must not be empty".to_owned()));
    }

    // Insert only when the parent task exists within the owned profile: the SELECT in the
    // INSERT…SELECT yields no row (so no insert, fetch_optional → None → 404) unless the parent
    // task's profile_id matches. This makes a wrong/cross-profile parent indistinguishable from
    // absent, with no separate parent lookup round-trip.
    let row = sqlx::query_as!(
        SubtaskRow,
        "INSERT INTO subtasks (task_id, title) \
         SELECT t.id, $3 FROM tasks t WHERE t.id = $1 AND t.profile_id = $2 \
         RETURNING id, task_id, title, status",
        task_id,
        profile_id,
        title,
    )
    .fetch_optional(state.pool())
    .await?
    .ok_or(ApiError::NotFound)?;

    let subtask = row.into_subtask();
    tracing::info!(subtask_id = %subtask.id, %task_id, "created subtask");
    Ok((StatusCode::CREATED, Json(subtask)))
}

/// `PATCH /api/profiles/{pid}/tasks/{tid}/subtasks/{sid}` → `200 Subtask`. Applies the supplied
/// subset of `{title, status}` in place, leaving absent fields untouched (COALESCE on a NULL
/// parameter). An empty patch is a no-op returning the sub-task unchanged. A present `title`
/// must be non-empty after trimming (else `400 validation_failed`) and is stored trimmed. The
/// `UPDATE` is joined to the parent task on `task_id` and `tasks.profile_id`, so a wrong parent,
/// missing sub-task, or unowned/cross-profile reach → `404 not_found`. A sub-task has no
/// `closed_at`, so `status` is a plain column flip.
#[tracing::instrument(skip_all, fields(user_id = %user.user_id, profile_id = %profile_id, task_id = %task_id, subtask_id = %subtask_id))]
pub async fn patch_subtask(
    State(state): State<AppState>,
    user: AuthUser,
    Path((profile_id, task_id, subtask_id)): Path<(Uuid, Uuid, Uuid)>,
    Json(request): Json<UpdateSubtaskRequest>,
) -> ApiResult<Json<Subtask>> {
    assert_owned(&state, user.user_id, profile_id).await?;

    // Validate + trim a supplied title; an absent title leaves the column untouched.
    let title = request.title.as_deref().map(str::trim);
    if title == Some("") {
        return Err(ApiError::Validation("title must not be empty".to_owned()));
    }
    let status = request.status.map(status_str);

    // Single static parameterized UPDATE joined to the parent task: the row is matched only
    // when the sub-task's parent is `{tid}` AND that task belongs to `{pid}`. COALESCE leaves a
    // NULL parameter's column untouched (so an empty patch is a no-op returning the row).
    let row = sqlx::query_as!(
        SubtaskRow,
        "UPDATE subtasks s \
         SET title = COALESCE($4, s.title), \
             status = COALESCE($5, s.status) \
         FROM tasks t \
         WHERE s.id = $1 AND s.task_id = $2 AND t.id = s.task_id AND t.profile_id = $3 \
         RETURNING s.id, s.task_id, s.title, s.status",
        subtask_id,
        task_id,
        profile_id,
        title,
        status,
    )
    .fetch_optional(state.pool())
    .await?
    .ok_or(ApiError::NotFound)?;

    let subtask = row.into_subtask();
    tracing::info!(subtask_id = %subtask.id, "updated subtask");
    Ok(Json(subtask))
}

/// `DELETE /api/profiles/{pid}/tasks/{tid}/subtasks/{sid}` → `204 No Content`. Joined to the
/// parent task on `task_id` and `tasks.profile_id`; a second delete, wrong parent, or an
/// unowned/missing sub-task → `404 not_found`.
#[tracing::instrument(skip_all, fields(user_id = %user.user_id, profile_id = %profile_id, task_id = %task_id, subtask_id = %subtask_id))]
pub async fn delete_subtask(
    State(state): State<AppState>,
    user: AuthUser,
    Path((profile_id, task_id, subtask_id)): Path<(Uuid, Uuid, Uuid)>,
) -> ApiResult<StatusCode> {
    assert_owned(&state, user.user_id, profile_id).await?;

    let deleted = sqlx::query_scalar!(
        "DELETE FROM subtasks s \
         USING tasks t \
         WHERE s.id = $1 AND s.task_id = $2 AND t.id = s.task_id AND t.profile_id = $3 \
         RETURNING s.id",
        subtask_id,
        task_id,
        profile_id,
    )
    .fetch_optional(state.pool())
    .await?;

    if deleted.is_none() {
        return Err(ApiError::NotFound);
    }

    tracing::info!(%subtask_id, "deleted subtask");
    Ok(StatusCode::NO_CONTENT)
}

/// `GET /api/profiles/{pid}/subtasks` → `200` bare array of **all** the profile's sub-tasks,
/// for the Tasks-tab tree load (avoids N+1: the TUI fetches tasks + this in two calls then
/// groups by `task_id` client-side). Joined `subtasks → tasks WHERE tasks.profile_id = $pid`;
/// creation order within each parent (`task_id`, then `created_at ASC`). Unowned profile → 404.
#[tracing::instrument(skip_all, fields(user_id = %user.user_id, profile_id = %profile_id))]
pub async fn list_profile_subtasks(
    State(state): State<AppState>,
    user: AuthUser,
    Path(profile_id): Path<Uuid>,
) -> ApiResult<Json<Vec<Subtask>>> {
    assert_owned(&state, user.user_id, profile_id).await?;

    let rows = sqlx::query_as!(
        SubtaskRow,
        "SELECT s.id, s.task_id, s.title, s.status \
         FROM subtasks s JOIN tasks t ON t.id = s.task_id \
         WHERE t.profile_id = $1 \
         ORDER BY s.task_id, s.created_at ASC",
        profile_id,
    )
    .fetch_all(state.pool())
    .await?;

    Ok(Json(
        rows.into_iter().map(SubtaskRow::into_subtask).collect(),
    ))
}
