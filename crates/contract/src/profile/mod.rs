//! Profile wire type. A profile is a namespace that owns its own tasks and notes.

use serde::{Deserialize, Serialize};

/// A profile as returned by `GET /api/profiles`.
///
/// A profile is a namespace: every task and note is scoped to one, and all domain routes
/// nest under `/api/profiles/{id}/…`. The `id` is a UUID string and `created_at` is an
/// RFC 3339 UTC string.
///
/// # Examples
///
/// ```
/// use contract::Profile;
///
/// let json = r#"{
///     "id": "5f9a2c1e-0b3d-4e6f-8a1b-2c3d4e5f6a7b",
///     "name": "work",
///     "created_at": "2026-06-11T12:00:00Z"
/// }"#;
/// let profile = serde_json::from_str::<Profile>(json).unwrap();
/// assert_eq!(profile.name, "work");
/// assert_eq!(profile.id, "5f9a2c1e-0b3d-4e6f-8a1b-2c3d4e5f6a7b");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Profile {
    /// Server-generated profile id (UUID string).
    pub id: String,
    /// Human-chosen profile name (e.g. `work`, `personal`).
    pub name: String,
    /// Creation timestamp (RFC 3339 UTC string).
    pub created_at: String,
}
