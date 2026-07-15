//! Profile-scoped task handlers (ADR-0005 §5). Every query is ownership-joined on the
//! authenticated user's profile, so a profile the caller does not own is `404 not_found`
//! (never 403). The task domain is flat (hard-constraint #3).

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use chrono::{DateTime, Utc};
use contract::{
    CreateTaskRequest, MAX_TASK_LIST_LIMIT, Task, TaskListQuery, TaskStatus, UpdateTaskRequest,
};
use uuid::Uuid;

use crate::app::AppState;
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::extract::ValidatedQuery;

/// A task row as stored, before mapping to the wire [`Task`].
struct TaskRow {
    id: Uuid,
    title: String,
    description: String,
    status: String,
    created_at: DateTime<Utc>,
    closed_at: Option<DateTime<Utc>>,
}

impl TaskRow {
    /// Map a stored row to the wire DTO. An unrecognized status defaults to `open`; the DB
    /// `CHECK` constraint makes that branch unreachable in practice.
    fn into_task(self) -> Task {
        let status = match self.status.as_str() {
            "done" => TaskStatus::Done,
            _ => TaskStatus::Open,
        };
        Task {
            id: self.id.to_string(),
            title: self.title,
            description: self.description,
            status,
            created_at: self.created_at,
            closed_at: self.closed_at,
        }
    }
}

/// Confirm the caller owns `profile_id`, returning `404 not_found` otherwise. This is the
/// single ownership gate every task route passes through.
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

/// `GET /api/profiles/{pid}/tasks` → `200` bare array, newest-first (ADR-0014). Optional
/// `?limit=&offset=` query params bound the window: an absent `limit` falls back to the
/// ceiling [`MAX_TASK_LIST_LIMIT`] (preserving the whole-list default); a `limit` strictly
/// above the ceiling is a `400 validation_failed` (an explicit over-ceiling value is a client
/// error, never silently clamped); an absent `offset` is `0`. Completed-last ordering is a
/// TUI-side render concern (ADR-0014 §4) — the server keeps `ORDER BY created_at DESC`.
///
/// Optional `?created_from=&created_until=` bound `created_at` by a plain `timestamptz` range
/// (ADR-0015): when present, `created_from` is an **inclusive** lower bound and `created_until`
/// an **exclusive** upper bound, both UTC epoch **seconds**. The bounds are independent and
/// optional (an absent bound omits its side), so absent-both is byte-identical to the pre-window
/// whole-list behaviour. The server does **no** civil-day arithmetic — day granularity is a TUI
/// convention. If both bounds are present and `created_from > created_until` the window is
/// inverted (a client bug) → `400 validation_failed`; `created_from == created_until` is a valid
/// empty window (upper is exclusive) → `200 []`. Unowned profile → 404.
#[tracing::instrument(skip_all, fields(user_id = %user.user_id, profile_id = %profile_id))]
pub async fn list_tasks(
    State(state): State<AppState>,
    user: AuthUser,
    Path(profile_id): Path<Uuid>,
    ValidatedQuery(query): ValidatedQuery<TaskListQuery>,
) -> ApiResult<Json<Vec<Task>>> {
    assert_owned(&state, user.user_id, profile_id).await?;

    let limit = match query.limit {
        Some(limit) if limit > MAX_TASK_LIST_LIMIT => {
            return Err(ApiError::Validation(format!(
                "limit must not exceed {MAX_TASK_LIST_LIMIT}"
            )));
        }
        Some(limit) => limit,
        None => MAX_TASK_LIST_LIMIT,
    };
    let offset = query.offset.unwrap_or(0);

    // An inverted window (from strictly after until) can only be a client bug; reject it. An
    // equal-bounds window is valid and returns the empty list (upper bound is exclusive).
    if let (Some(from), Some(until)) = (query.created_from, query.created_until)
        && from > until
    {
        return Err(ApiError::Validation(
            "created_from must not be after created_until".to_owned(),
        ));
    }

    // A single static query: each bound is a NULL-guarded `to_timestamp` range predicate, so an
    // absent bound (NULL parameter) drops out and absent-both matches the pre-window behaviour.
    let rows = sqlx::query_as!(
        TaskRow,
        "SELECT id, title, description, status, created_at, closed_at \
         FROM tasks WHERE profile_id = $1 \
             AND ($4::bigint IS NULL OR created_at >= to_timestamp($4::bigint)) \
             AND ($5::bigint IS NULL OR created_at < to_timestamp($5::bigint)) \
         ORDER BY created_at DESC LIMIT $2 OFFSET $3",
        profile_id,
        i64::from(limit),
        i64::from(offset),
        query.created_from,
        query.created_until,
    )
    .fetch_all(state.pool())
    .await?;

    Ok(Json(rows.into_iter().map(TaskRow::into_task).collect()))
}

/// `POST /api/profiles/{pid}/tasks` → `201 Task`. Title must be non-empty after trimming
/// (else `400 validation_failed`); description may be empty. Unowned profile → 404.
#[tracing::instrument(skip_all, fields(user_id = %user.user_id, profile_id = %profile_id))]
pub async fn create_task(
    State(state): State<AppState>,
    user: AuthUser,
    Path(profile_id): Path<Uuid>,
    Json(request): Json<CreateTaskRequest>,
) -> ApiResult<(StatusCode, Json<Task>)> {
    assert_owned(&state, user.user_id, profile_id).await?;

    let title = request.title.trim();
    if title.is_empty() {
        return Err(ApiError::Validation("title must not be empty".to_owned()));
    }

    let row = sqlx::query_as!(
        TaskRow,
        "INSERT INTO tasks (profile_id, title, description) VALUES ($1, $2, $3) \
         RETURNING id, title, description, status, created_at, closed_at",
        profile_id,
        title,
        request.description,
    )
    .fetch_one(state.pool())
    .await?;

    let task = row.into_task();
    tracing::info!(task_id = %task.id, "created task");
    Ok((StatusCode::CREATED, Json(task)))
}

/// Map a wire [`TaskStatus`] to its stored string form (`open` / `done`).
fn status_str(status: TaskStatus) -> &'static str {
    match status {
        TaskStatus::Open => "open",
        TaskStatus::Done => "done",
    }
}

/// `PATCH /api/profiles/{pid}/tasks/{tid}` → `200 Task`. Applies the supplied subset of
/// `{title, description, status}` in place, leaving absent fields untouched. `status = done`
/// sets `closed_at` (preserving an existing one, matching the old idempotent close);
/// `status = open` (reopen) clears `closed_at`; an absent status leaves `closed_at` untouched.
/// An empty patch is a no-op returning the task unchanged. If `title` is present it must be
/// non-empty after trimming (else `400 validation_failed`) and is stored trimmed. Unowned
/// profile or missing task → 404.
#[tracing::instrument(skip_all, fields(user_id = %user.user_id, profile_id = %profile_id, task_id = %task_id))]
pub async fn patch_task(
    State(state): State<AppState>,
    user: AuthUser,
    Path((profile_id, task_id)): Path<(Uuid, Uuid)>,
    Json(request): Json<UpdateTaskRequest>,
) -> ApiResult<Json<Task>> {
    assert_owned(&state, user.user_id, profile_id).await?;

    // Validate + trim a supplied title; an absent title leaves the column untouched.
    let title = request.title.as_deref().map(str::trim);
    if title == Some("") {
        return Err(ApiError::Validation("title must not be empty".to_owned()));
    }
    let status = request.status.map(status_str);

    // Single static parameterized UPDATE: COALESCE leaves a NULL parameter's column untouched;
    // the CASE couples status→closed_at (done preserves/sets it, open clears it, absent keeps
    // it). No string interpolation — one sqlx-offline-checkable query.
    let row = sqlx::query_as!(
        TaskRow,
        "UPDATE tasks \
         SET title = COALESCE($3, title), \
             description = COALESCE($4, description), \
             status = COALESCE($5, status), \
             closed_at = CASE \
                 WHEN $5 = 'done' THEN COALESCE(closed_at, now()) \
                 WHEN $5 = 'open' THEN NULL \
                 ELSE closed_at \
             END \
         WHERE id = $1 AND profile_id = $2 \
         RETURNING id, title, description, status, created_at, closed_at",
        task_id,
        profile_id,
        title,
        request.description.as_deref(),
        status,
    )
    .fetch_optional(state.pool())
    .await?
    .ok_or(ApiError::NotFound)?;

    let task = row.into_task();
    tracing::info!(task_id = %task.id, "updated task");
    Ok(Json(task))
}

/// `DELETE /api/profiles/{pid}/tasks/{tid}` → `204 No Content`. Ownership-scoped; a second
/// delete or an unowned/missing task → `404 not_found`.
#[tracing::instrument(skip_all, fields(user_id = %user.user_id, profile_id = %profile_id, task_id = %task_id))]
pub async fn delete_task(
    State(state): State<AppState>,
    user: AuthUser,
    Path((profile_id, task_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<StatusCode> {
    assert_owned(&state, user.user_id, profile_id).await?;

    let deleted = sqlx::query_scalar!(
        "DELETE FROM tasks WHERE id = $1 AND profile_id = $2 RETURNING id",
        task_id,
        profile_id,
    )
    .fetch_optional(state.pool())
    .await?;

    if deleted.is_none() {
        return Err(ApiError::NotFound);
    }

    tracing::info!(%task_id, "deleted task");
    Ok(StatusCode::NO_CONTENT)
}
