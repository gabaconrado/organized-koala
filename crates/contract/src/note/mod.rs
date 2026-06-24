//! Note wire types: the flat note shape and its create / update requests.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A note, the flat shape of hard-constraint #3.
///
/// `id` is a UUID string and `created_at` is a UTC timestamp that serializes to (and parses
/// from) RFC 3339 with a `Z` offset, e.g. `"2026-06-11T12:00:00Z"`. The shape is deliberately
/// flat: exactly `{ id, title, content, created_at }` — no `updated_at`, status, or lifecycle.
///
/// # Examples
///
/// ```
/// use chrono::{DateTime, Utc};
/// use contract::Note;
///
/// let raw = r#"{
///     "id": "5f9a2c1e-0b3d-4e6f-8a1b-2c3d4e5f6a7b",
///     "title": "Groceries",
///     "content": "milk, eggs, bread",
///     "created_at": "2026-06-11T12:00:00Z"
/// }"#;
/// let note = serde_json::from_str::<Note>(raw).unwrap();
/// assert_eq!(note.title, "Groceries");
/// assert_eq!(
///     note.created_at,
///     "2026-06-11T12:00:00Z".parse::<DateTime<Utc>>().unwrap(),
/// );
///
/// // It re-serializes with the `Z` offset.
/// let json = serde_json::to_value(&note).unwrap();
/// assert_eq!(json["created_at"], "2026-06-11T12:00:00Z");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Note {
    /// Server-generated note id (UUID string).
    pub id: String,
    /// Note title; non-empty after trimming (enforced server-side).
    pub title: String,
    /// Free-form note content; may be empty.
    pub content: String,
    /// Creation timestamp; serializes as RFC 3339 UTC (e.g. `"2026-06-11T12:00:00Z"`).
    pub created_at: DateTime<Utc>,
}

/// Request body for `POST /api/profiles/{profile_id}/notes`.
///
/// `title` must be non-empty after trimming (else `400 validation_failed`); `content` may be
/// empty. On success the server returns `201` with the created [`Note`].
///
/// # Examples
///
/// ```
/// use contract::CreateNoteRequest;
///
/// let req = CreateNoteRequest {
///     title: "Groceries".to_owned(),
///     content: "milk, eggs, bread".to_owned(),
/// };
/// let json = serde_json::to_value(&req).unwrap();
/// assert_eq!(json["title"], "Groceries");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateNoteRequest {
    /// Note title; must be non-empty after trimming (enforced server-side).
    pub title: String,
    /// Free-form note content; may be empty.
    pub content: String,
}

/// Request body for `PATCH /api/profiles/{profile_id}/notes/{note_id}`.
///
/// A **full replace** of the two editable fields: `title` must be non-empty after trimming
/// (else `400 validation_failed`); `content` may be empty. Editing mutates in place; no
/// timestamp is touched. On success the server returns `200` with the updated [`Note`].
///
/// # Examples
///
/// ```
/// use contract::UpdateNoteRequest;
///
/// let req = UpdateNoteRequest {
///     title: "Groceries (updated)".to_owned(),
///     content: "milk, eggs, bread, butter".to_owned(),
/// };
/// let json = serde_json::to_value(&req).unwrap();
/// assert_eq!(json["title"], "Groceries (updated)");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateNoteRequest {
    /// Note title; must be non-empty after trimming (enforced server-side).
    pub title: String,
    /// Free-form note content; may be empty.
    pub content: String,
}
