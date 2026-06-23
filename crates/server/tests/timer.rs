//! Integration tests for the account-global Pomodoro timer surface (ADR-0002 §5,
//! hard-constraint #4): the global duration config and the focus-session lifecycle, asserted
//! as real HTTP round-trips against the `axum` app over a per-test database.
//!
//! The timer is keyed on the authenticated user, **not** on a profile — none of its routes
//! carry a `profile_id`. The tests below exercise: config default + persistence + validation
//! bounds; the session idle → running → idle lifecycle (with `ends_at`/`server_now` invariants);
//! that a freshly-started session reads `running` (the read-time completion verdict, with the
//! reachable-coverage note recorded inline); account-global isolation between users; and that
//! every route requires a bearer token.

#![cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        reason = "panics are the failure channel in test code (rust-standards)"
    )
)]

mod common;

use axum::http::StatusCode;
use common::{app, get, get_auth, post, post_auth, put_json, put_json_auth, register, send};
use contract::{ErrorCode, TimerConfig, TimerSession};
use serde_json::json;
use sqlx::PgPool;

const CONFIG_PATH: &str = "/api/timer/config";
const SESSION_PATH: &str = "/api/timer/session";
const START_PATH: &str = "/api/timer/session/start";
const STOP_PATH: &str = "/api/timer/session/stop";

// ───────────────────────────── config ─────────────────────────────

/// Default read: a user who never set a config reads 30 minutes, with no row written.
#[sqlx::test]
async fn config_defaults_to_30(pool: PgPool) {
    let app = app(pool);
    let account = register(&app, "ada", "ada@example.com", "hunter2-long").await;

    let res = send(&app, get_auth(CONFIG_PATH, &account.token)).await;
    assert_eq!(res.status, StatusCode::OK);
    let config: TimerConfig = res.parse();
    assert_eq!(
        config.duration_minutes, 30,
        "default duration is 30 minutes"
    );
}

/// PUT then GET round-trips the new duration: the update persists and is read back.
#[sqlx::test]
async fn config_put_then_get_round_trips(pool: PgPool) {
    let app = app(pool);
    let account = register(&app, "ada", "ada@example.com", "hunter2-long").await;

    let put = send(
        &app,
        put_json_auth(
            CONFIG_PATH,
            &account.token,
            &json!({ "duration_minutes": 45 }),
        ),
    )
    .await;
    assert_eq!(put.status, StatusCode::OK);
    let updated: TimerConfig = put.parse();
    assert_eq!(updated.duration_minutes, 45, "PUT echoes the new duration");

    let get = send(&app, get_auth(CONFIG_PATH, &account.token)).await;
    assert_eq!(get.status, StatusCode::OK);
    let read: TimerConfig = get.parse();
    assert_eq!(read.duration_minutes, 45, "the new duration persists");
}

/// A second PUT overwrites the first (the config is a single upserted row).
#[sqlx::test]
async fn config_put_overwrites(pool: PgPool) {
    let app = app(pool);
    let account = register(&app, "ada", "ada@example.com", "hunter2-long").await;

    for minutes in [45_u32, 15] {
        let res = send(
            &app,
            put_json_auth(
                CONFIG_PATH,
                &account.token,
                &json!({ "duration_minutes": minutes }),
            ),
        )
        .await;
        assert_eq!(res.status, StatusCode::OK);
    }

    let get = send(&app, get_auth(CONFIG_PATH, &account.token)).await;
    let read: TimerConfig = get.parse();
    assert_eq!(read.duration_minutes, 15, "the latest PUT wins");
}

/// PUT with `duration_minutes` of 0 → 400 `validation_failed` (below the lower bound).
#[sqlx::test]
async fn config_put_zero_is_400(pool: PgPool) {
    let app = app(pool);
    let account = register(&app, "ada", "ada@example.com", "hunter2-long").await;

    let res = send(
        &app,
        put_json_auth(
            CONFIG_PATH,
            &account.token,
            &json!({ "duration_minutes": 0 }),
        ),
    )
    .await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST);
    res.expect_error(ErrorCode::ValidationFailed);
}

/// PUT with `duration_minutes` of 1441 → 400 `validation_failed` (above the 1440 cap).
#[sqlx::test]
async fn config_put_over_cap_is_400(pool: PgPool) {
    let app = app(pool);
    let account = register(&app, "ada", "ada@example.com", "hunter2-long").await;

    let res = send(
        &app,
        put_json_auth(
            CONFIG_PATH,
            &account.token,
            &json!({ "duration_minutes": 1441 }),
        ),
    )
    .await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST);
    res.expect_error(ErrorCode::ValidationFailed);
}

/// The boundary value 1 (the lower bound) is accepted.
#[sqlx::test]
async fn config_put_lower_bound_1_accepted(pool: PgPool) {
    let app = app(pool);
    let account = register(&app, "ada", "ada@example.com", "hunter2-long").await;

    let res = send(
        &app,
        put_json_auth(
            CONFIG_PATH,
            &account.token,
            &json!({ "duration_minutes": 1 }),
        ),
    )
    .await;
    assert_eq!(res.status, StatusCode::OK);
    let config: TimerConfig = res.parse();
    assert_eq!(config.duration_minutes, 1);
}

/// The boundary value 1440 (the upper bound, 24 h) is accepted.
#[sqlx::test]
async fn config_put_upper_bound_1440_accepted(pool: PgPool) {
    let app = app(pool);
    let account = register(&app, "ada", "ada@example.com", "hunter2-long").await;

    let res = send(
        &app,
        put_json_auth(
            CONFIG_PATH,
            &account.token,
            &json!({ "duration_minutes": 1440 }),
        ),
    )
    .await;
    assert_eq!(res.status, StatusCode::OK);
    let config: TimerConfig = res.parse();
    assert_eq!(config.duration_minutes, 1440);
}

// ───────────────────────────── session lifecycle ─────────────────────────────

/// From idle (no session ever started): GET session → `{ state: "idle" }`.
#[sqlx::test]
async fn session_starts_idle(pool: PgPool) {
    let app = app(pool);
    let account = register(&app, "ada", "ada@example.com", "hunter2-long").await;

    let res = send(&app, get_auth(SESSION_PATH, &account.token)).await;
    assert_eq!(res.status, StatusCode::OK);
    let session: TimerSession = res.parse();
    assert_eq!(session, TimerSession::Idle, "no session yet ⇒ idle");
}

/// Start → running, carrying `started_at`/`ends_at`/`duration_minutes`/`server_now`, with
/// `ends_at == started_at + duration` and `server_now < ends_at` (freshly started). Uses the
/// default 30-minute duration when no config was set.
#[sqlx::test]
async fn start_returns_running_with_consistent_instants(pool: PgPool) {
    let app = app(pool);
    let account = register(&app, "ada", "ada@example.com", "hunter2-long").await;

    let res = send(&app, post_auth(START_PATH, &account.token)).await;
    assert_eq!(res.status, StatusCode::OK);
    let session: TimerSession = res.parse();

    match session {
        TimerSession::Running {
            started_at,
            ends_at,
            duration_minutes,
            server_now,
        } => {
            assert_eq!(duration_minutes, 30, "start snapshots the default duration");
            let expected_end = started_at + chrono::Duration::minutes(30);
            assert_eq!(
                ends_at, expected_end,
                "ends_at == started_at + duration_minutes"
            );
            assert!(
                server_now < ends_at,
                "a freshly started session has not yet reached its end-instant"
            );
            // server_now is at/after the start instant (the row's started_at is `now` at start).
            assert!(
                server_now >= started_at,
                "server_now is at or after started_at"
            );
        }
        other => panic!("expected running, got {other:?}"),
    }
}

/// Start snapshots the *current* configured duration (not the default) into the session.
#[sqlx::test]
async fn start_snapshots_configured_duration(pool: PgPool) {
    let app = app(pool);
    let account = register(&app, "ada", "ada@example.com", "hunter2-long").await;

    let put = send(
        &app,
        put_json_auth(
            CONFIG_PATH,
            &account.token,
            &json!({ "duration_minutes": 50 }),
        ),
    )
    .await;
    assert_eq!(put.status, StatusCode::OK);

    let res = send(&app, post_auth(START_PATH, &account.token)).await;
    let session: TimerSession = res.parse();
    match session {
        TimerSession::Running {
            started_at,
            ends_at,
            duration_minutes,
            ..
        } => {
            assert_eq!(
                duration_minutes, 50,
                "the configured duration is snapshotted"
            );
            assert_eq!(ends_at, started_at + chrono::Duration::minutes(50));
        }
        other => panic!("expected running, got {other:?}"),
    }
}

/// After start, GET session also reports `running` with the same `started_at`/`ends_at`
/// (the session persists; a coarse re-read reflects the active row).
#[sqlx::test]
async fn get_session_after_start_is_running(pool: PgPool) {
    let app = app(pool);
    let account = register(&app, "ada", "ada@example.com", "hunter2-long").await;

    let started = send(&app, post_auth(START_PATH, &account.token)).await;
    let started: TimerSession = started.parse();
    let (start_started_at, start_ends_at) = match started {
        TimerSession::Running {
            started_at,
            ends_at,
            ..
        } => (started_at, ends_at),
        other => panic!("expected running, got {other:?}"),
    };

    let read = send(&app, get_auth(SESSION_PATH, &account.token)).await;
    assert_eq!(read.status, StatusCode::OK);
    let read: TimerSession = read.parse();
    match read {
        TimerSession::Running {
            started_at,
            ends_at,
            ..
        } => {
            assert_eq!(started_at, start_started_at, "same session row");
            assert_eq!(ends_at, start_ends_at, "same derived end-instant");
        }
        other => panic!("expected running on re-read, got {other:?}"),
    }
}

/// A freshly-started session reads as `running`, never `completed`.
///
/// Coverage note (plan A5/A6): completion is a read-time verdict (`server_now >= ends_at`). The
/// minimum configurable duration is 1 minute, and the public API offers no way to set the
/// server's clock or to author a past `started_at`; forcing `now >= ends_at` would require a
/// real ~60 s sleep, which this suite deliberately does not do. So the reachable assertion here
/// is the *negative*: with the shortest allowed duration, an immediately-read session is still
/// `running` (not completed). The positive `completed` transition at `ends_at` is exercised by
/// the live `verifier` (DoD clause 4), not here.
#[sqlx::test]
async fn shortest_session_reads_running_not_completed(pool: PgPool) {
    let app = app(pool);
    let account = register(&app, "ada", "ada@example.com", "hunter2-long").await;

    let put = send(
        &app,
        put_json_auth(
            CONFIG_PATH,
            &account.token,
            &json!({ "duration_minutes": 1 }),
        ),
    )
    .await;
    assert_eq!(put.status, StatusCode::OK);

    let started = send(&app, post_auth(START_PATH, &account.token)).await;
    let started: TimerSession = started.parse();
    assert!(
        matches!(started, TimerSession::Running { .. }),
        "a just-started 1-minute session is running, not completed: {started:?}"
    );

    let read = send(&app, get_auth(SESSION_PATH, &account.token)).await;
    let read: TimerSession = read.parse();
    assert!(
        matches!(read, TimerSession::Running { .. }),
        "re-read within the minute is still running: {read:?}"
    );
}

/// Stop → idle: stopping a running session clears it.
#[sqlx::test]
async fn stop_clears_running_session(pool: PgPool) {
    let app = app(pool);
    let account = register(&app, "ada", "ada@example.com", "hunter2-long").await;

    let started = send(&app, post_auth(START_PATH, &account.token)).await;
    assert!(matches!(
        started.parse::<TimerSession>(),
        TimerSession::Running { .. }
    ));

    let stopped = send(&app, post_auth(STOP_PATH, &account.token)).await;
    assert_eq!(stopped.status, StatusCode::OK);
    assert_eq!(stopped.parse::<TimerSession>(), TimerSession::Idle);

    // And a subsequent read confirms the row was cleared (no paused state).
    let read = send(&app, get_auth(SESSION_PATH, &account.token)).await;
    assert_eq!(read.parse::<TimerSession>(), TimerSession::Idle);
}

/// Stop when already idle is idempotent: 200 with `idle` even though nothing was running.
#[sqlx::test]
async fn stop_when_idle_is_idempotent(pool: PgPool) {
    let app = app(pool);
    let account = register(&app, "ada", "ada@example.com", "hunter2-long").await;

    // No session has ever started.
    let first = send(&app, post_auth(STOP_PATH, &account.token)).await;
    assert_eq!(first.status, StatusCode::OK);
    assert_eq!(first.parse::<TimerSession>(), TimerSession::Idle);

    // Stopping again is still fine.
    let second = send(&app, post_auth(STOP_PATH, &account.token)).await;
    assert_eq!(second.status, StatusCode::OK);
    assert_eq!(second.parse::<TimerSession>(), TimerSession::Idle);
}

/// Start while a session is already active replaces it (single active session, A5): the new
/// `started_at` advances and the duration re-snapshots from the current config.
#[sqlx::test]
async fn start_while_active_replaces(pool: PgPool) {
    let app = app(pool);
    let account = register(&app, "ada", "ada@example.com", "hunter2-long").await;

    let first = send(&app, post_auth(START_PATH, &account.token)).await;
    let first_started = match first.parse::<TimerSession>() {
        TimerSession::Running { started_at, .. } => started_at,
        other => panic!("expected running, got {other:?}"),
    };

    // Change the duration, then re-start: the new session snapshots the new duration.
    let put = send(
        &app,
        put_json_auth(
            CONFIG_PATH,
            &account.token,
            &json!({ "duration_minutes": 10 }),
        ),
    )
    .await;
    assert_eq!(put.status, StatusCode::OK);

    let second = send(&app, post_auth(START_PATH, &account.token)).await;
    match second.parse::<TimerSession>() {
        TimerSession::Running {
            started_at,
            ends_at,
            duration_minutes,
            ..
        } => {
            assert_eq!(
                duration_minutes, 10,
                "re-start re-snapshots the new duration"
            );
            assert!(
                started_at >= first_started,
                "the replacing session's start is not before the original"
            );
            assert_eq!(ends_at, started_at + chrono::Duration::minutes(10));
        }
        other => panic!("expected running, got {other:?}"),
    }

    // Still exactly one active session: a read sees the replacing one, not two.
    let read = send(&app, get_auth(SESSION_PATH, &account.token)).await;
    match read.parse::<TimerSession>() {
        TimerSession::Running {
            duration_minutes, ..
        } => assert_eq!(
            duration_minutes, 10,
            "the single active session is the latest"
        ),
        other => panic!("expected running, got {other:?}"),
    }
}

// ───────────────────────────── account-global (#4 / ADR-0002 §5) ─────────────────────────────

/// The highest-value boundary test: the timer is account-global, keyed on the user, NOT on a
/// profile. None of its routes carry a `profile_id` — they are reached identically regardless
/// of which profile is active — and two distinct accounts have independent timers.
///
/// The current account model creates exactly one profile per account at registration (no
/// profile-creation endpoint exists), so "the same session observed across two profiles of one
/// user" is asserted in the two reachable ways:
///   1. Structurally: the config/session paths contain no profile segment, so the *same* token
///      reaches the *same* session no matter the caller's active profile (there is no active-
///      profile notion on these routes to vary).
///   2. By isolation: a *different* account (a different user) sees its own independent timer,
///      proving the key is the user — the property #4 protects.
#[sqlx::test]
async fn timer_is_account_global_not_profile_scoped(pool: PgPool) {
    let app = app(pool);
    let ada = register(&app, "ada", "ada@example.com", "hunter2-long").await;
    let bob = register(&app, "bob", "bob@example.com", "hunter2-long").await;

    // The timer routes carry no profile id: they are the same paths for every caller.
    assert!(
        !CONFIG_PATH.contains("profile"),
        "config path has no profile segment"
    );
    assert!(
        !SESSION_PATH.contains("profile"),
        "session path has no profile segment"
    );
    assert!(
        !START_PATH.contains("profile"),
        "start path has no profile segment"
    );

    // Ada sets a 45-minute config and starts a session.
    let put = send(
        &app,
        put_json_auth(CONFIG_PATH, &ada.token, &json!({ "duration_minutes": 45 })),
    )
    .await;
    assert_eq!(put.status, StatusCode::OK);
    let ada_started = send(&app, post_auth(START_PATH, &ada.token)).await;
    let ada_session: TimerSession = ada_started.parse();
    let ada_started_at = match ada_session {
        TimerSession::Running {
            started_at,
            duration_minutes,
            ..
        } => {
            assert_eq!(duration_minutes, 45);
            started_at
        }
        other => panic!("expected running, got {other:?}"),
    };

    // Reading the session again with Ada's token — regardless of which profile she'd consider
    // "active" — observes the SAME session: the route does not vary by profile.
    let ada_reread = send(&app, get_auth(SESSION_PATH, &ada.token)).await;
    match ada_reread.parse::<TimerSession>() {
        TimerSession::Running { started_at, .. } => {
            assert_eq!(started_at, ada_started_at, "same account ⇒ same session");
        }
        other => panic!("expected running, got {other:?}"),
    }

    // Bob (a different user) has his own independent timer: still default config, still idle —
    // Ada's config and session did not bleed into Bob's account.
    let bob_config = send(&app, get_auth(CONFIG_PATH, &bob.token)).await;
    assert_eq!(
        bob_config.parse::<TimerConfig>().duration_minutes,
        30,
        "Bob's config is unaffected by Ada's 45-minute setting"
    );
    let bob_session = send(&app, get_auth(SESSION_PATH, &bob.token)).await;
    assert_eq!(
        bob_session.parse::<TimerSession>(),
        TimerSession::Idle,
        "Bob has no session; Ada's running session did not bleed across accounts"
    );
}

// ───────────────────────────── auth required ─────────────────────────────

/// GET /api/timer/config without a bearer token → 401 `unauthenticated`.
#[sqlx::test]
async fn get_config_requires_auth(pool: PgPool) {
    let app = app(pool);
    let res = send(&app, get(CONFIG_PATH)).await;
    assert_eq!(res.status, StatusCode::UNAUTHORIZED);
    res.expect_error(ErrorCode::Unauthenticated);
}

/// PUT /api/timer/config without a bearer token → 401 `unauthenticated`.
#[sqlx::test]
async fn put_config_requires_auth(pool: PgPool) {
    let app = app(pool);
    let res = send(
        &app,
        put_json(CONFIG_PATH, &json!({ "duration_minutes": 25 })),
    )
    .await;
    assert_eq!(res.status, StatusCode::UNAUTHORIZED);
    res.expect_error(ErrorCode::Unauthenticated);
}

/// GET /api/timer/session without a bearer token → 401 `unauthenticated`.
#[sqlx::test]
async fn get_session_requires_auth(pool: PgPool) {
    let app = app(pool);
    let res = send(&app, get(SESSION_PATH)).await;
    assert_eq!(res.status, StatusCode::UNAUTHORIZED);
    res.expect_error(ErrorCode::Unauthenticated);
}

/// POST /api/timer/session/start without a bearer token → 401 `unauthenticated`.
#[sqlx::test]
async fn start_session_requires_auth(pool: PgPool) {
    let app = app(pool);
    let res = send(&app, post(START_PATH)).await;
    assert_eq!(res.status, StatusCode::UNAUTHORIZED);
    res.expect_error(ErrorCode::Unauthenticated);
}

/// POST /api/timer/session/stop without a bearer token → 401 `unauthenticated`.
#[sqlx::test]
async fn stop_session_requires_auth(pool: PgPool) {
    let app = app(pool);
    let res = send(&app, post(STOP_PATH)).await;
    assert_eq!(res.status, StatusCode::UNAUTHORIZED);
    res.expect_error(ErrorCode::Unauthenticated);
}
