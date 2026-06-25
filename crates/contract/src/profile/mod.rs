//! Profile wire types: the namespace shape and its create / update requests.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A profile as returned by `GET /api/profiles`.
///
/// A profile is a namespace: every task and note is scoped to one, and all domain routes
/// nest under `/api/profiles/{id}/…`. The `id` is a UUID string and `created_at` is a UTC
/// timestamp that serializes to (and parses from) RFC 3339 with a `Z` offset, e.g.
/// `"2026-06-11T12:00:00Z"`.
///
/// # Examples
///
/// ```
/// use chrono::{DateTime, Utc};
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
/// assert_eq!(
///     profile.created_at,
///     "2026-06-11T12:00:00Z".parse::<DateTime<Utc>>().unwrap(),
/// );
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Profile {
    /// Server-generated profile id (UUID string).
    pub id: String,
    /// Human-chosen profile name (e.g. `work`, `personal`).
    pub name: String,
    /// Creation timestamp; serializes as RFC 3339 UTC (e.g. `"2026-06-11T12:00:00Z"`).
    pub created_at: DateTime<Utc>,
}

/// Request body for `POST /api/profiles`.
///
/// `name` must be non-empty after trimming (else `400 validation_failed`) and unique within
/// the account (else `409 profile_name_taken`). On success the server returns `201` with the
/// created [`Profile`].
///
/// # Examples
///
/// ```
/// use contract::CreateProfileRequest;
///
/// let req = CreateProfileRequest { name: "work".to_owned() };
/// let json = serde_json::to_value(&req).unwrap();
/// assert_eq!(json["name"], "work");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateProfileRequest {
    /// Profile name; must be non-empty after trimming and unique per account (enforced
    /// server-side).
    pub name: String,
}

/// Request body for `PATCH /api/profiles/{id}`.
///
/// Renames the profile: `name` must be non-empty after trimming (else `400 validation_failed`)
/// and unique within the account (else `409 profile_name_taken`). On success the server returns
/// `200` with the updated [`Profile`].
///
/// # Examples
///
/// ```
/// use contract::UpdateProfileRequest;
///
/// let req = UpdateProfileRequest { name: "personal".to_owned() };
/// let json = serde_json::to_value(&req).unwrap();
/// assert_eq!(json["name"], "personal");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateProfileRequest {
    /// Profile name; must be non-empty after trimming and unique per account (enforced
    /// server-side).
    pub name: String,
}
