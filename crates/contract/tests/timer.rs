//! Wire-format and round-trip tests for the Pomodoro timer DTOs (`TimerConfig`,
//! `UpdateTimerConfigRequest`, `TimerSession`), locking the ADR-0002 conventions: the
//! account-global duration config, and the `#[serde(tag = "state")]` session enum
//! (`idle`/`running`/`completed`) carrying `started_at`, `ends_at`, `duration_minutes`, and
//! `server_now` as RFC 3339 UTC (`Z`-offset) timestamps, exactly as `Task::created_at`.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

use chrono::{DateTime, Utc};
use contract::{TimerConfig, TimerSession, UpdateTimerConfigRequest};
use serde_json::{Value, json};

const STARTED_AT: &str = "2026-06-11T12:00:00Z";
const ENDS_AT: &str = "2026-06-11T12:30:00Z";
const RUNNING_NOW: &str = "2026-06-11T12:05:00Z";
const COMPLETED_NOW: &str = "2026-06-11T12:31:00Z";
const DURATION: u32 = 30;

/// Parse a canonical RFC 3339 const into a typed timestamp for struct construction.
/// (`DateTime` has no `const` parse, so the typed values live in helper bindings.)
fn ts(s: &str) -> DateTime<Utc> {
    s.parse().unwrap()
}

// --- TimerConfig: the account-global duration config. ---

#[test]
fn timer_config_serializes_duration_minutes() {
    let config = TimerConfig {
        duration_minutes: 25,
    };
    let json = serde_json::to_value(&config).unwrap();
    assert_eq!(json, json!({ "duration_minutes": 25 }));
}

#[test]
fn timer_config_round_trips_losslessly() {
    let config = TimerConfig {
        duration_minutes: DURATION,
    };
    let wire = serde_json::to_string(&config).unwrap();
    let back: TimerConfig = serde_json::from_str(&wire).unwrap();
    assert_eq!(back, config);
}

#[test]
fn timer_config_deserializes_from_known_good_literal() {
    // Wire-compatibility guard: the exact shape the server emits from `GET /api/timer/config`.
    let wire = r#"{"duration_minutes":30}"#;
    let config: TimerConfig = serde_json::from_str(wire).unwrap();
    assert_eq!(config.duration_minutes, 30);
}

// --- UpdateTimerConfigRequest: the only-knob update body. ---

#[test]
fn update_timer_config_request_serializes_duration_minutes() {
    let req = UpdateTimerConfigRequest {
        duration_minutes: 45,
    };
    let json = serde_json::to_value(&req).unwrap();
    assert_eq!(json, json!({ "duration_minutes": 45 }));
}

#[test]
fn update_timer_config_request_round_trips_losslessly() {
    let req = UpdateTimerConfigRequest {
        duration_minutes: 45,
    };
    let wire = serde_json::to_string(&req).unwrap();
    let back: UpdateTimerConfigRequest = serde_json::from_str(&wire).unwrap();
    assert_eq!(back, req);
}

#[test]
fn update_timer_config_request_deserializes_from_known_good_literal() {
    // Wire-compatibility guard: the exact body the TUI sends to `PUT /api/timer/config`.
    let wire = r#"{"duration_minutes":45}"#;
    let req: UpdateTimerConfigRequest = serde_json::from_str(wire).unwrap();
    assert_eq!(req.duration_minutes, 45);
}

// --- TimerSession: tagged enum on `state` (idle / running / completed). ---

#[test]
fn idle_session_serializes_to_just_the_tag() {
    // Idle carries no payload — only the `state` tag is on the wire.
    let json = serde_json::to_value(&TimerSession::Idle).unwrap();
    assert_eq!(json, json!({ "state": "idle" }));
    // The key set is exactly `{ "state" }` — no stray null fields leak.
    let object = json.as_object().unwrap();
    assert_eq!(object.len(), 1);
    assert_eq!(object.get("state").unwrap(), "idle");
}

#[test]
fn running_session_serializes_with_the_tag_and_all_four_fields() {
    let session = TimerSession::Running {
        started_at: ts(STARTED_AT),
        ends_at: ts(ENDS_AT),
        duration_minutes: DURATION,
        server_now: ts(RUNNING_NOW),
    };
    let json = serde_json::to_value(&session).unwrap();
    assert_eq!(
        json,
        json!({
            "state": "running",
            "started_at": STARTED_AT,
            "ends_at": ENDS_AT,
            "duration_minutes": DURATION,
            "server_now": RUNNING_NOW,
        })
    );
    // Timestamps travel as RFC 3339 UTC strings; the absolute end-instant + server-now are the
    // render contract (ADR-0002 §2–3).
    let object = json.as_object().unwrap();
    assert!(object.get("ends_at").unwrap().is_string());
    assert!(object.get("server_now").unwrap().is_string());
}

#[test]
fn completed_session_serializes_with_the_tag_and_all_four_fields() {
    let session = TimerSession::Completed {
        started_at: ts(STARTED_AT),
        ends_at: ts(ENDS_AT),
        duration_minutes: DURATION,
        server_now: ts(COMPLETED_NOW),
    };
    let json = serde_json::to_value(&session).unwrap();
    assert_eq!(
        json,
        json!({
            "state": "completed",
            "started_at": STARTED_AT,
            "ends_at": ENDS_AT,
            "duration_minutes": DURATION,
            "server_now": COMPLETED_NOW,
        })
    );
}

#[test]
fn idle_session_round_trips_losslessly() {
    let wire = serde_json::to_string(&TimerSession::Idle).unwrap();
    let back: TimerSession = serde_json::from_str(&wire).unwrap();
    assert_eq!(back, TimerSession::Idle);
}

#[test]
fn running_session_round_trips_losslessly() {
    let session = TimerSession::Running {
        started_at: ts(STARTED_AT),
        ends_at: ts(ENDS_AT),
        duration_minutes: DURATION,
        server_now: ts(RUNNING_NOW),
    };
    let wire = serde_json::to_string(&session).unwrap();
    let back: TimerSession = serde_json::from_str(&wire).unwrap();
    assert_eq!(back, session);
}

#[test]
fn completed_session_round_trips_losslessly() {
    let session = TimerSession::Completed {
        started_at: ts(STARTED_AT),
        ends_at: ts(ENDS_AT),
        duration_minutes: DURATION,
        server_now: ts(COMPLETED_NOW),
    };
    let wire = serde_json::to_string(&session).unwrap();
    let back: TimerSession = serde_json::from_str(&wire).unwrap();
    assert_eq!(back, session);
}

// --- TimerSession: deserialization of known-good wire literals (compatibility guards). ---

#[test]
fn idle_session_deserializes_from_known_good_literal() {
    let session: TimerSession = serde_json::from_str(r#"{"state":"idle"}"#).unwrap();
    assert_eq!(session, TimerSession::Idle);
}

#[test]
fn running_session_deserializes_from_known_good_literal() {
    let wire = json!({
        "state": "running",
        "started_at": STARTED_AT,
        "ends_at": ENDS_AT,
        "duration_minutes": DURATION,
        "server_now": RUNNING_NOW,
    });
    let session: TimerSession = serde_json::from_value(wire).unwrap();
    match session {
        TimerSession::Running {
            started_at,
            ends_at,
            duration_minutes,
            server_now,
        } => {
            assert_eq!(started_at, ts(STARTED_AT));
            assert_eq!(ends_at, ts(ENDS_AT));
            assert_eq!(duration_minutes, DURATION);
            assert_eq!(server_now, ts(RUNNING_NOW));
        }
        other => panic!("expected running, got {other:?}"),
    }
}

#[test]
fn completed_session_deserializes_from_known_good_literal() {
    let wire = json!({
        "state": "completed",
        "started_at": STARTED_AT,
        "ends_at": ENDS_AT,
        "duration_minutes": DURATION,
        "server_now": COMPLETED_NOW,
    });
    let session: TimerSession = serde_json::from_value(wire).unwrap();
    match session {
        TimerSession::Completed {
            started_at,
            ends_at,
            duration_minutes,
            server_now,
        } => {
            assert_eq!(started_at, ts(STARTED_AT));
            assert_eq!(ends_at, ts(ENDS_AT));
            assert_eq!(duration_minutes, DURATION);
            assert_eq!(server_now, ts(COMPLETED_NOW));
        }
        other => panic!("expected completed, got {other:?}"),
    }
}

#[test]
fn session_rejects_an_unknown_state_tag() {
    // The tag enum is closed: only idle/running/completed are valid on the wire.
    let wire = json!({
        "state": "paused",
        "started_at": STARTED_AT,
        "ends_at": ENDS_AT,
        "duration_minutes": DURATION,
        "server_now": RUNNING_NOW,
    });
    assert!(serde_json::from_value::<TimerSession>(wire).is_err());
}

// --- TimerSession: typed-timestamp parsing on the wire. ---

#[test]
fn running_session_rejects_a_malformed_ends_at() {
    // The typed `DateTime<Utc>` fields reject a non-RFC-3339 string at deserialize time.
    let wire = json!({
        "state": "running",
        "started_at": STARTED_AT,
        "ends_at": "not-a-date",
        "duration_minutes": DURATION,
        "server_now": RUNNING_NOW,
    });
    assert!(serde_json::from_value::<TimerSession>(wire).is_err());
}

#[test]
fn running_session_normalizes_an_offset_bearing_instant_to_utc() {
    // An RFC 3339 input carrying a non-Z offset is accepted and normalized to UTC, so it
    // re-serializes with the canonical `Z` suffix. `13:30:00+01:00` is `12:30:00Z`.
    let wire = json!({
        "state": "running",
        "started_at": STARTED_AT,
        "ends_at": "2026-06-11T13:30:00+01:00",
        "duration_minutes": DURATION,
        "server_now": RUNNING_NOW,
    });
    let session: TimerSession = serde_json::from_value(wire).unwrap();
    let reserialized = serde_json::to_value(&session).unwrap();
    // The offset-bearing `13:30:00+01:00` lands as the canonical `Z`-suffixed `12:30:00Z`.
    assert_eq!(reserialized.get("ends_at").unwrap(), ENDS_AT);
    // And all other timestamps keep their canonical `Z` rendering.
    let object = reserialized.as_object().unwrap();
    assert_eq!(object.get("started_at").unwrap(), STARTED_AT);
    assert_eq!(object.get("server_now").unwrap(), RUNNING_NOW);
}

#[test]
fn session_array_deserializes_as_a_bare_array() {
    // Defensive: the enum parses uniformly inside a JSON array, mixing variants.
    let wire = json!([
        { "state": "idle" },
        {
            "state": "running",
            "started_at": STARTED_AT,
            "ends_at": ENDS_AT,
            "duration_minutes": DURATION,
            "server_now": RUNNING_NOW,
        }
    ]);
    let sessions: Vec<TimerSession> = serde_json::from_value(wire).unwrap();
    assert_eq!(sessions.len(), 2);
    assert_eq!(sessions.first().unwrap(), &TimerSession::Idle);
    let running: Value = serde_json::to_value(sessions.get(1).unwrap()).unwrap();
    assert_eq!(running.get("state").unwrap(), "running");
}
