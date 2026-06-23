//! Account-global Pomodoro timer handlers (ADR-0002 §5). Every route keys on the
//! authenticated `user_id` — the timer is NOT profile-scoped (#4 namespaces TODOs and Notes
//! only). The only knob is the session duration (hard-constraint #3); there is no pause, and
//! stopping clears the active session.
//!
//! A session is an absolute end-instant: the server snapshots `duration_minutes` at start and
//! derives `ends_at = started_at + duration_minutes`. Completion is decided at read time
//! (`server_now >= ends_at`); the row is kept until an explicit stop so a reconnecting client
//! still sees the completed verdict (ADR-0002 §4).

use axum::Json;
use axum::extract::State;
use chrono::{DateTime, Duration, Utc};
use contract::{TimerConfig, TimerSession, UpdateTimerConfigRequest};

use crate::app::AppState;
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};

/// The default focus-session duration (minutes) for a user who has never set a config.
const DEFAULT_DURATION_MINUTES: u32 = 30;

/// Inclusive duration bounds in minutes (1 min .. 24 h); outside → `400 validation_failed`.
const MIN_DURATION_MINUTES: u32 = 1;
const MAX_DURATION_MINUTES: u32 = 1440;

/// Map a stored `INT` duration to the wire `u32`. The `CHECK (duration_minutes >= 1)` keeps
/// values positive and well within `u32`; a negative value would be a corrupt row.
fn duration_to_u32(stored: i32) -> ApiResult<u32> {
    u32::try_from(stored)
        .map_err(|_| ApiError::Internal(anyhow::anyhow!("stored duration_minutes out of range")))
}

/// Map a validated wire `u32` duration to the stored `INT`. Bounded by validation, so this
/// only fails on a logic error.
fn duration_to_i32(value: u32) -> ApiResult<i32> {
    i32::try_from(value)
        .map_err(|_| ApiError::Internal(anyhow::anyhow!("duration_minutes out of range")))
}

/// Build the wire [`TimerSession`] from a stored session row at the given instant. The
/// running/completed split is the server's verdict (`server_now >= ends_at`).
fn session_from_row(
    started_at: DateTime<Utc>,
    duration_minutes: u32,
    server_now: DateTime<Utc>,
) -> ApiResult<TimerSession> {
    let minutes = i64::from(duration_minutes);
    let ends_at = started_at
        .checked_add_signed(Duration::minutes(minutes))
        .ok_or_else(|| ApiError::Internal(anyhow::anyhow!("session end-instant out of range")))?;

    if server_now >= ends_at {
        Ok(TimerSession::Completed {
            started_at,
            ends_at,
            duration_minutes,
            server_now,
        })
    } else {
        Ok(TimerSession::Running {
            started_at,
            ends_at,
            duration_minutes,
            server_now,
        })
    }
}

/// `GET /api/timer/config` → `200 TimerConfig`. Defaults to 30 minutes when the user has no
/// config row (lazily; no row is written on read).
#[tracing::instrument(skip_all, fields(user_id = %user.user_id))]
pub async fn get_config(
    State(state): State<AppState>,
    user: AuthUser,
) -> ApiResult<Json<TimerConfig>> {
    let stored = sqlx::query_scalar!(
        "SELECT duration_minutes FROM timer_configs WHERE user_id = $1",
        user.user_id,
    )
    .fetch_optional(state.pool())
    .await?;

    let duration_minutes = match stored {
        Some(value) => duration_to_u32(value)?,
        None => DEFAULT_DURATION_MINUTES,
    };

    Ok(Json(TimerConfig { duration_minutes }))
}

/// `PUT /api/timer/config` → `200 TimerConfig`. Upserts the duration; a value outside
/// `[1, 1440]` is `400 validation_failed` (reusing the existing code; no new `ErrorCode`).
#[tracing::instrument(skip_all, fields(user_id = %user.user_id))]
pub async fn update_config(
    State(state): State<AppState>,
    user: AuthUser,
    Json(request): Json<UpdateTimerConfigRequest>,
) -> ApiResult<Json<TimerConfig>> {
    let duration_minutes = request.duration_minutes;
    if !(MIN_DURATION_MINUTES..=MAX_DURATION_MINUTES).contains(&duration_minutes) {
        return Err(ApiError::Validation(format!(
            "duration_minutes must be between {MIN_DURATION_MINUTES} and {MAX_DURATION_MINUTES}"
        )));
    }

    let stored = duration_to_i32(duration_minutes)?;
    let saved = sqlx::query_scalar!(
        "INSERT INTO timer_configs (user_id, duration_minutes, updated_at) \
         VALUES ($1, $2, now()) \
         ON CONFLICT (user_id) \
         DO UPDATE SET duration_minutes = EXCLUDED.duration_minutes, updated_at = now() \
         RETURNING duration_minutes",
        user.user_id,
        stored,
    )
    .fetch_one(state.pool())
    .await?;

    let duration_minutes = duration_to_u32(saved)?;
    tracing::info!(duration_minutes, "updated timer config");
    Ok(Json(TimerConfig { duration_minutes }))
}

/// `GET /api/timer/session` → `200 TimerSession`. Idle when there is no active row; otherwise
/// running or completed as decided at read time (`server_now >= ends_at`). The row is not
/// deleted on completion (ADR-0002 §4).
#[tracing::instrument(skip_all, fields(user_id = %user.user_id))]
pub async fn get_session(
    State(state): State<AppState>,
    user: AuthUser,
) -> ApiResult<Json<TimerSession>> {
    let server_now = Utc::now();
    let row = sqlx::query!(
        "SELECT started_at, duration_minutes FROM timer_sessions WHERE user_id = $1",
        user.user_id,
    )
    .fetch_optional(state.pool())
    .await?;

    let session = match row {
        Some(row) => {
            let duration_minutes = duration_to_u32(row.duration_minutes)?;
            session_from_row(row.started_at, duration_minutes, server_now)?
        }
        None => TimerSession::Idle,
    };

    Ok(Json(session))
}

/// `POST /api/timer/session/start` → `200 TimerSession::Running`. Snapshots the current
/// configured duration (default 30 if unset) and starts at `now`. Starting while a session is
/// already active replaces it (single active session, ADR-0002 §5).
#[tracing::instrument(skip_all, fields(user_id = %user.user_id))]
pub async fn start_session(
    State(state): State<AppState>,
    user: AuthUser,
) -> ApiResult<Json<TimerSession>> {
    let server_now = Utc::now();

    // Snapshot the current configured duration; default when the user has no config row.
    let configured = sqlx::query_scalar!(
        "SELECT duration_minutes FROM timer_configs WHERE user_id = $1",
        user.user_id,
    )
    .fetch_optional(state.pool())
    .await?;
    let duration_minutes = match configured {
        Some(value) => duration_to_u32(value)?,
        None => DEFAULT_DURATION_MINUTES,
    };
    let stored = duration_to_i32(duration_minutes)?;

    let row = sqlx::query!(
        "INSERT INTO timer_sessions (user_id, started_at, duration_minutes) \
         VALUES ($1, $2, $3) \
         ON CONFLICT (user_id) \
         DO UPDATE SET started_at = EXCLUDED.started_at, \
                       duration_minutes = EXCLUDED.duration_minutes \
         RETURNING started_at, duration_minutes",
        user.user_id,
        server_now,
        stored,
    )
    .fetch_one(state.pool())
    .await?;

    let duration_minutes = duration_to_u32(row.duration_minutes)?;
    let session = session_from_row(row.started_at, duration_minutes, server_now)?;
    tracing::info!(duration_minutes, "started timer session");
    Ok(Json(session))
}

/// `POST /api/timer/session/stop` → `200 TimerSession::Idle`. Clears the active session (no
/// pause; stop resets). Idempotent: stopping when already idle still returns `Idle`.
#[tracing::instrument(skip_all, fields(user_id = %user.user_id))]
pub async fn stop_session(
    State(state): State<AppState>,
    user: AuthUser,
) -> ApiResult<Json<TimerSession>> {
    let _deleted = sqlx::query!(
        "DELETE FROM timer_sessions WHERE user_id = $1",
        user.user_id,
    )
    .execute(state.pool())
    .await?;

    tracing::info!("stopped timer session");
    Ok(Json(TimerSession::Idle))
}
