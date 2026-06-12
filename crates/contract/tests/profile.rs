//! Wire-format and round-trip tests for `Profile`, locking the ADR-0005 conventions:
//! snake_case fields, a UUID-string id, and an RFC 3339 UTC `created_at`.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

use chrono::{DateTime, Utc};
use contract::Profile;
use serde_json::json;

const PROFILE_ID: &str = "5f9a2c1e-0b3d-4e6f-8a1b-2c3d4e5f6a7b";
const CREATED_AT: &str = "2026-06-11T12:00:00Z";

/// Parse the canonical `created_at` const into a typed timestamp for struct construction.
fn created_at() -> DateTime<Utc> {
    CREATED_AT.parse().unwrap()
}

#[test]
fn profile_deserializes_from_the_wire_shape() {
    let wire = json!({
        "id": PROFILE_ID,
        "name": "work",
        "created_at": CREATED_AT,
    });
    let profile: Profile = serde_json::from_value(wire).unwrap();
    assert_eq!(profile.id, PROFILE_ID);
    assert_eq!(profile.name, "work");
    assert_eq!(profile.created_at, created_at());
}

#[test]
fn profile_serializes_snake_case_uuid_and_rfc3339() {
    let profile = Profile {
        id: PROFILE_ID.to_owned(),
        name: "work".to_owned(),
        created_at: created_at(),
    };
    let json = serde_json::to_value(&profile).unwrap();
    assert_eq!(
        json,
        json!({
            "id": PROFILE_ID,
            "name": "work",
            "created_at": CREATED_AT,
        })
    );
    // The timestamp travels as a string (RFC 3339 UTC), the id as a UUID string.
    assert!(json.get("created_at").unwrap().is_string());
    assert!(json.get("id").unwrap().is_string());
}

#[test]
fn profile_round_trips_losslessly() {
    let profile = Profile {
        id: PROFILE_ID.to_owned(),
        name: "personal".to_owned(),
        created_at: created_at(),
    };
    let wire = serde_json::to_string(&profile).unwrap();
    let back: Profile = serde_json::from_str(&wire).unwrap();
    assert_eq!(back, profile);
}

#[test]
fn profile_rejects_a_malformed_created_at() {
    // The typed `created_at` rejects a non-RFC-3339 string at deserialize time — behaviour the
    // old `String` field could not give us.
    let wire = json!({
        "id": PROFILE_ID,
        "name": "work",
        "created_at": "not-a-date",
    });
    assert!(serde_json::from_value::<Profile>(wire).is_err());
}
