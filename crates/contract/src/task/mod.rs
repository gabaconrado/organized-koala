//! Task wire types: the flat TODO shape, its status, and the create request.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Lifecycle status of a [`Task`]. Serializes as a lowercase string (`open` / `done`).
///
/// # Examples
///
/// ```
/// use contract::TaskStatus;
///
/// assert_eq!(serde_json::to_string(&TaskStatus::Open).unwrap(), r#""open""#);
/// assert_eq!(serde_json::to_string(&TaskStatus::Done).unwrap(), r#""done""#);
/// assert_eq!(
///     serde_json::from_str::<TaskStatus>(r#""done""#).unwrap(),
///     TaskStatus::Done,
/// );
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TaskStatus {
    /// The task is outstanding; `closed_at` is `null`.
    Open,
    /// The task is closed; `closed_at` is set.
    Done,
}

/// A TODO task, the flat shape of hard-constraint #3.
///
/// `id` is a UUID string and `created_at` is a UTC timestamp. `closed_at` is `null` while the
/// task is [`TaskStatus::Open`] and a UTC timestamp once it is [`TaskStatus::Done`]. Both
/// timestamps serialize to (and parse from) RFC 3339 with a `Z` offset, e.g.
/// `"2026-06-11T12:00:00Z"`.
///
/// # Examples
///
/// ```
/// use chrono::{DateTime, Utc};
/// use contract::{Task, TaskStatus};
///
/// let open = r#"{
///     "id": "5f9a2c1e-0b3d-4e6f-8a1b-2c3d4e5f6a7b",
///     "title": "Write the contract crate",
///     "description": "ADR-0005 DTOs",
///     "status": "open",
///     "created_at": "2026-06-11T12:00:00Z",
///     "closed_at": null
/// }"#;
/// let task = serde_json::from_str::<Task>(open).unwrap();
/// assert_eq!(task.status, TaskStatus::Open);
/// assert!(task.closed_at.is_none());
/// assert_eq!(
///     task.created_at,
///     "2026-06-11T12:00:00Z".parse::<DateTime<Utc>>().unwrap(),
/// );
///
/// // A done task carries a `closed_at`; it re-serializes with the `Z` offset.
/// let done = Task {
///     status: TaskStatus::Done,
///     closed_at: Some("2026-06-11T13:30:00Z".parse::<DateTime<Utc>>().unwrap()),
///     ..task
/// };
/// let json = serde_json::to_value(&done).unwrap();
/// assert_eq!(json["created_at"], "2026-06-11T12:00:00Z");
/// assert_eq!(json["closed_at"], "2026-06-11T13:30:00Z");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Task {
    /// Server-generated task id (UUID string).
    pub id: String,
    /// Task title; non-empty after trimming (enforced server-side).
    pub title: String,
    /// Free-form task description; may be empty.
    pub description: String,
    /// Whether the task is open or done.
    pub status: TaskStatus,
    /// Creation timestamp; serializes as RFC 3339 UTC (e.g. `"2026-06-11T12:00:00Z"`).
    pub created_at: DateTime<Utc>,
    /// Close timestamp (RFC 3339 UTC), or `null` while the task is open.
    pub closed_at: Option<DateTime<Utc>>,
}

/// Request body for `POST /api/profiles/{profile_id}/tasks`.
///
/// `title` must be non-empty after trimming (else `400 validation_failed`); `description`
/// may be empty. On success the server returns `201` with the created [`Task`].
///
/// # Examples
///
/// ```
/// use contract::CreateTaskRequest;
///
/// let req = CreateTaskRequest {
///     title: "Write the contract crate".to_owned(),
///     description: "ADR-0005 DTOs".to_owned(),
/// };
/// let json = serde_json::to_value(&req).unwrap();
/// assert_eq!(json["title"], "Write the contract crate");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateTaskRequest {
    /// Task title; must be non-empty after trimming (enforced server-side).
    pub title: String,
    /// Free-form task description; may be empty.
    pub description: String,
}
