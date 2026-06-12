//! Profile-scoped task handlers (ADR-0005 §5). Every query is ownership-joined on the
//! authenticated user's profile, so a profile the caller does not own is `404 not_found`
//! (never 403). The task domain is flat (hard-constraint #3).

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use chrono::{DateTime, Utc};
use contract::{CreateTaskRequest, Task, TaskStatus};
use uuid::Uuid;

use crate::app::AppState;
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};

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

/// `GET /api/profiles/{pid}/tasks` → `200` bare array, newest-first. Unowned profile → 404.
#[tracing::instrument(skip_all, fields(user_id = %user.user_id, profile_id = %profile_id))]
pub async fn list_tasks(
    State(state): State<AppState>,
    user: AuthUser,
    Path(profile_id): Path<Uuid>,
) -> ApiResult<Json<Vec<Task>>> {
    assert_owned(&state, user.user_id, profile_id).await?;

    let rows = sqlx::query_as!(
        TaskRow,
        "SELECT id, title, description, status, created_at, closed_at \
         FROM tasks WHERE profile_id = $1 ORDER BY created_at DESC",
        profile_id,
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

/// `POST /api/profiles/{pid}/tasks/{tid}/close` → `200 Task`. Sets `status = done` and
/// `closed_at = now`. Idempotent: re-closing a done task returns it unchanged (`closed_at`
/// preserved). Unowned profile or missing task → 404.
#[tracing::instrument(skip_all, fields(user_id = %user.user_id, profile_id = %profile_id, task_id = %task_id))]
pub async fn close_task(
    State(state): State<AppState>,
    user: AuthUser,
    Path((profile_id, task_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<Json<Task>> {
    assert_owned(&state, user.user_id, profile_id).await?;

    // Idempotent: only an open task is mutated; a done task keeps its original closed_at.
    let row = sqlx::query_as!(
        TaskRow,
        "UPDATE tasks \
         SET status = 'done', closed_at = COALESCE(closed_at, now()) \
         WHERE id = $1 AND profile_id = $2 \
         RETURNING id, title, description, status, created_at, closed_at",
        task_id,
        profile_id,
    )
    .fetch_optional(state.pool())
    .await?
    .ok_or(ApiError::NotFound)?;

    let task = row.into_task();
    tracing::info!(task_id = %task.id, "closed task");
    Ok(Json(task))
}
