//! Profile-scoped note handlers (ADR-0007). Every query is ownership-joined on the
//! authenticated user's profile, so a profile (or note) the caller does not own is
//! `404 not_found` (never 403). The note domain is flat (hard-constraint #3): exactly
//! `{ id, title, content, created_at }` — no `updated_at`, status, or lifecycle.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use chrono::{DateTime, Utc};
use contract::{CreateNoteRequest, Note, UpdateNoteRequest};
use uuid::Uuid;

use crate::app::AppState;
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};

/// A note row as stored, before mapping to the wire [`Note`].
struct NoteRow {
    id: Uuid,
    title: String,
    content: String,
    created_at: DateTime<Utc>,
}

impl NoteRow {
    /// Map a stored row to the wire DTO.
    fn into_note(self) -> Note {
        Note {
            id: self.id.to_string(),
            title: self.title,
            content: self.content,
            created_at: self.created_at,
        }
    }
}

/// Confirm the caller owns `profile_id`, returning `404 not_found` otherwise. This is the
/// single ownership gate every note route passes through before touching notes.
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

/// Validate and trim a note title, returning the trimmed value or `400 validation_failed`.
fn trimmed_title(title: &str) -> ApiResult<&str> {
    let title = title.trim();
    if title.is_empty() {
        return Err(ApiError::Validation("title must not be empty".to_owned()));
    }
    Ok(title)
}

/// `GET /api/profiles/{pid}/notes` → `200` bare array, newest-first. Unowned profile → 404.
#[tracing::instrument(skip_all, fields(user_id = %user.user_id, profile_id = %profile_id))]
pub async fn list_notes(
    State(state): State<AppState>,
    user: AuthUser,
    Path(profile_id): Path<Uuid>,
) -> ApiResult<Json<Vec<Note>>> {
    assert_owned(&state, user.user_id, profile_id).await?;

    let rows = sqlx::query_as!(
        NoteRow,
        "SELECT id, title, content, created_at \
         FROM notes WHERE profile_id = $1 ORDER BY created_at DESC",
        profile_id,
    )
    .fetch_all(state.pool())
    .await?;

    Ok(Json(rows.into_iter().map(NoteRow::into_note).collect()))
}

/// `POST /api/profiles/{pid}/notes` → `201 Note`. Title must be non-empty after trimming
/// (else `400 validation_failed`); the stored title is the trimmed value, content may be
/// empty. Unowned profile → 404.
#[tracing::instrument(skip_all, fields(user_id = %user.user_id, profile_id = %profile_id))]
pub async fn create_note(
    State(state): State<AppState>,
    user: AuthUser,
    Path(profile_id): Path<Uuid>,
    Json(request): Json<CreateNoteRequest>,
) -> ApiResult<(StatusCode, Json<Note>)> {
    assert_owned(&state, user.user_id, profile_id).await?;

    let title = trimmed_title(&request.title)?;

    let row = sqlx::query_as!(
        NoteRow,
        "INSERT INTO notes (profile_id, title, content) VALUES ($1, $2, $3) \
         RETURNING id, title, content, created_at",
        profile_id,
        title,
        request.content,
    )
    .fetch_one(state.pool())
    .await?;

    let note = row.into_note();
    tracing::info!(note_id = %note.id, "created note");
    Ok((StatusCode::CREATED, Json(note)))
}

/// `GET /api/profiles/{pid}/notes/{nid}` → `200 Note`. Unowned profile or missing/foreign
/// note → 404 (never 403): the query is ownership-joined, so a foreign note is indistinguishable
/// from absent (ADR-0005 §4).
#[tracing::instrument(skip_all, fields(user_id = %user.user_id, profile_id = %profile_id, note_id = %note_id))]
pub async fn get_note(
    State(state): State<AppState>,
    user: AuthUser,
    Path((profile_id, note_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<Json<Note>> {
    assert_owned(&state, user.user_id, profile_id).await?;

    let row = sqlx::query_as!(
        NoteRow,
        "SELECT id, title, content, created_at \
         FROM notes WHERE id = $1 AND profile_id = $2",
        note_id,
        profile_id,
    )
    .fetch_optional(state.pool())
    .await?
    .ok_or(ApiError::NotFound)?;

    Ok(Json(row.into_note()))
}

/// `PATCH /api/profiles/{pid}/notes/{nid}` → `200 Note`. In-place full replace of title +
/// content; no timestamp is touched (#3, no `updated_at`). Title must be non-empty after
/// trimming (else `400 validation_failed`); content may be empty. Unowned profile or
/// missing/foreign note → 404.
#[tracing::instrument(skip_all, fields(user_id = %user.user_id, profile_id = %profile_id, note_id = %note_id))]
pub async fn update_note(
    State(state): State<AppState>,
    user: AuthUser,
    Path((profile_id, note_id)): Path<(Uuid, Uuid)>,
    Json(request): Json<UpdateNoteRequest>,
) -> ApiResult<Json<Note>> {
    assert_owned(&state, user.user_id, profile_id).await?;

    let title = trimmed_title(&request.title)?;

    let row = sqlx::query_as!(
        NoteRow,
        "UPDATE notes SET title = $1, content = $2 \
         WHERE id = $3 AND profile_id = $4 \
         RETURNING id, title, content, created_at",
        title,
        request.content,
        note_id,
        profile_id,
    )
    .fetch_optional(state.pool())
    .await?
    .ok_or(ApiError::NotFound)?;

    let note = row.into_note();
    tracing::info!(note_id = %note.id, "updated note");
    Ok(Json(note))
}

/// `DELETE /api/profiles/{pid}/notes/{nid}` → `204 No Content` (empty body). Unowned profile
/// or missing/foreign note (incl. a second delete) → 404, ownership-joined like the reads.
#[tracing::instrument(skip_all, fields(user_id = %user.user_id, profile_id = %profile_id, note_id = %note_id))]
pub async fn delete_note(
    State(state): State<AppState>,
    user: AuthUser,
    Path((profile_id, note_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<StatusCode> {
    assert_owned(&state, user.user_id, profile_id).await?;

    let result = sqlx::query!(
        "DELETE FROM notes WHERE id = $1 AND profile_id = $2",
        note_id,
        profile_id,
    )
    .execute(state.pool())
    .await?;

    if result.rows_affected() == 0 {
        return Err(ApiError::NotFound);
    }

    tracing::info!(%note_id, "deleted note");
    Ok(StatusCode::NO_CONTENT)
}
