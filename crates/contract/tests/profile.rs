//! Wire-format and round-trip tests for `Profile`, locking the ADR-0005 conventions:
//! snake_case fields, a UUID-string id, and an RFC 3339 UTC `created_at`.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

use contract::Profile;
use serde_json::json;

const PROFILE_ID: &str = "5f9a2c1e-0b3d-4e6f-8a1b-2c3d4e5f6a7b";
const CREATED_AT: &str = "2026-06-11T12:00:00Z";

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
    assert_eq!(profile.created_at, CREATED_AT);
}

#[test]
fn profile_serializes_snake_case_uuid_and_rfc3339() {
    let profile = Profile {
        id: PROFILE_ID.to_owned(),
        name: "work".to_owned(),
        created_at: CREATED_AT.to_owned(),
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
        created_at: CREATED_AT.to_owned(),
    };
    let wire = serde_json::to_string(&profile).unwrap();
    let back: Profile = serde_json::from_str(&wire).unwrap();
    assert_eq!(back, profile);
}
