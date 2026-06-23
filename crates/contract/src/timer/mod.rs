//! Pomodoro timer wire types: the account-global duration config and the focus-session state.
//!
//! The timer is account-global (ADR-0002 §5), not profile-scoped: its only knob is the session
//! duration. A running session is an absolute end-instant plus the server's current instant
//! (ADR-0002 §2–3), so the TUI computes `remaining = ends_at − server_now` once and ticks it
//! down locally — no per-second polling, no tick stream.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// The account-global Pomodoro config (ADR-0002 §5). The only knob is the session duration.
///
/// Returned by `GET /api/timer/config`. `duration_minutes` defaults to 30 and is enforced
/// `>= 1` server-side.
///
/// # Examples
///
/// ```
/// use contract::TimerConfig;
///
/// let json = r#"{ "duration_minutes": 25 }"#;
/// let config = serde_json::from_str::<TimerConfig>(json).unwrap();
/// assert_eq!(config.duration_minutes, 25);
///
/// let value = serde_json::to_value(&TimerConfig { duration_minutes: 30 }).unwrap();
/// assert_eq!(value["duration_minutes"], 30);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimerConfig {
    /// Configured focus-session duration in whole minutes (default 30; enforced `>= 1`).
    pub duration_minutes: u32,
}

/// Request body for `PUT /api/timer/config`. Duration is the only adjustable parameter (#3).
///
/// The server validates the value (`>= 1` and within a sane cap; otherwise `400`
/// `validation_failed`) and, on success, returns the updated [`TimerConfig`].
///
/// # Examples
///
/// ```
/// use contract::UpdateTimerConfigRequest;
///
/// let req = UpdateTimerConfigRequest { duration_minutes: 45 };
/// let value = serde_json::to_value(&req).unwrap();
/// assert_eq!(value["duration_minutes"], 45);
///
/// let parsed = serde_json::from_str::<UpdateTimerConfigRequest>(
///     r#"{ "duration_minutes": 45 }"#,
/// )
/// .unwrap();
/// assert_eq!(parsed, req);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateTimerConfigRequest {
    /// Requested focus-session duration in whole minutes; validated server-side (`>= 1`).
    pub duration_minutes: u32,
}

/// The current focus-session state, as returned by the session endpoints (ADR-0002 §2–3).
///
/// This is a tagged enum on the `state` field (`idle` / `running` / `completed`), making the
/// trichotomy illegal-states-unrepresentable. The `running` and `completed` variants carry the
/// absolute `ends_at`, the `duration_minutes` snapshot taken when the session started, and
/// `server_now` — the server's current instant, which neutralizes client clock skew
/// (ADR-0002 §3). Whether a session is `running` or `completed` is the server's verdict
/// (`server_now >= ends_at`). All timestamps serialize to (and parse from) RFC 3339 with a `Z`
/// offset, e.g. `"2026-06-11T12:00:00Z"`, exactly as [`Task::created_at`] does.
///
/// The wire shapes are:
///
/// ```json
/// { "state": "idle" }
/// { "state": "running", "started_at": "...", "ends_at": "...",
///   "duration_minutes": 30, "server_now": "..." }
/// { "state": "completed", "started_at": "...", "ends_at": "...",
///   "duration_minutes": 30, "server_now": "..." }
/// ```
///
/// [`Task::created_at`]: crate::Task::created_at
///
/// # Examples
///
/// ```
/// use chrono::{DateTime, Utc};
/// use contract::TimerSession;
///
/// // Idle is just the tag.
/// let idle = serde_json::to_value(&TimerSession::Idle).unwrap();
/// assert_eq!(idle, serde_json::json!({ "state": "idle" }));
///
/// // Running carries the absolute end-instant plus the server's current instant.
/// let json = r#"{
///     "state": "running",
///     "started_at": "2026-06-11T12:00:00Z",
///     "ends_at": "2026-06-11T12:30:00Z",
///     "duration_minutes": 30,
///     "server_now": "2026-06-11T12:05:00Z"
/// }"#;
/// let session = serde_json::from_str::<TimerSession>(json).unwrap();
/// match session {
///     TimerSession::Running { ends_at, server_now, duration_minutes, .. } => {
///         assert_eq!(duration_minutes, 30);
///         assert_eq!(ends_at, "2026-06-11T12:30:00Z".parse::<DateTime<Utc>>().unwrap());
///         assert_eq!(server_now, "2026-06-11T12:05:00Z".parse::<DateTime<Utc>>().unwrap());
///     }
///     other => panic!("expected running, got {other:?}"),
/// }
///
/// // Completed re-serializes with the `Z` offset and the `completed` tag.
/// let completed = TimerSession::Completed {
///     started_at: "2026-06-11T12:00:00Z".parse::<DateTime<Utc>>().unwrap(),
///     ends_at: "2026-06-11T12:30:00Z".parse::<DateTime<Utc>>().unwrap(),
///     duration_minutes: 30,
///     server_now: "2026-06-11T12:31:00Z".parse::<DateTime<Utc>>().unwrap(),
/// };
/// let value = serde_json::to_value(&completed).unwrap();
/// assert_eq!(value["state"], "completed");
/// assert_eq!(value["ends_at"], "2026-06-11T12:30:00Z");
/// assert_eq!(value["server_now"], "2026-06-11T12:31:00Z");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "lowercase")]
pub enum TimerSession {
    /// No active session: the timer is stopped (the reset state; ADR-0002 §5, no pause).
    Idle,
    /// A focus session is running: `server_now < ends_at` per the server.
    Running {
        /// When the session started; RFC 3339 UTC (e.g. `"2026-06-11T12:00:00Z"`).
        started_at: DateTime<Utc>,
        /// The absolute end-instant (`started_at + duration_minutes`); RFC 3339 UTC.
        ends_at: DateTime<Utc>,
        /// The duration snapshot taken when the session started, in whole minutes.
        duration_minutes: u32,
        /// The server's current instant when the response was produced; RFC 3339 UTC.
        /// Neutralizes client clock skew (ADR-0002 §3).
        server_now: DateTime<Utc>,
    },
    /// A focus session has reached its end-instant: `server_now >= ends_at` per the server.
    Completed {
        /// When the session started; RFC 3339 UTC (e.g. `"2026-06-11T12:00:00Z"`).
        started_at: DateTime<Utc>,
        /// The absolute end-instant (`started_at + duration_minutes`); RFC 3339 UTC.
        ends_at: DateTime<Utc>,
        /// The duration snapshot taken when the session started, in whole minutes.
        duration_minutes: u32,
        /// The server's current instant when the response was produced; RFC 3339 UTC.
        /// Neutralizes client clock skew (ADR-0002 §3).
        server_now: DateTime<Utc>,
    },
}
