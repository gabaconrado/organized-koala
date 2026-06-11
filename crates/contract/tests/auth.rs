//! Wire-format and round-trip tests for the auth DTOs (`RegisterRequest`,
//! `LoginRequest`, `SessionResponse`, `Password`), locking the ADR-0005 conventions.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

use contract::{LoginRequest, Password, RegisterRequest, SessionResponse};
use serde_json::{Value, json};

#[test]
fn register_request_serializes_snake_case_fields() {
    let req = RegisterRequest {
        username: "ada".to_owned(),
        email: "ada@example.com".to_owned(),
        password: Password::new("hunter2".to_owned()),
        profile_name: "work".to_owned(),
    };
    let json = serde_json::to_value(&req).unwrap();
    assert_eq!(
        json,
        json!({
            "username": "ada",
            "email": "ada@example.com",
            "password": "hunter2",
            "profile_name": "work",
        })
    );
}

#[test]
fn register_request_round_trips() {
    let wire = r#"{"username":"ada","email":"ada@example.com","password":"hunter2","profile_name":"work"}"#;
    let req: RegisterRequest = serde_json::from_str(wire).unwrap();
    assert_eq!(req.username, "ada");
    assert_eq!(req.email, "ada@example.com");
    assert_eq!(req.password.expose(), "hunter2");
    assert_eq!(req.profile_name, "work");

    // Re-serialize and confirm the value is identical (frozen seam).
    let reserialized: Value = serde_json::from_str(&serde_json::to_string(&req).unwrap()).unwrap();
    let original: Value = serde_json::from_str(wire).unwrap();
    assert_eq!(reserialized, original);
}

#[test]
fn login_request_serializes_snake_case_fields() {
    let req = LoginRequest {
        identifier: "ada@example.com".to_owned(),
        password: Password::new("hunter2".to_owned()),
    };
    let json = serde_json::to_value(&req).unwrap();
    assert_eq!(
        json,
        json!({
            "identifier": "ada@example.com",
            "password": "hunter2",
        })
    );
}

#[test]
fn login_request_round_trips() {
    let wire = r#"{"identifier":"ada","password":"hunter2"}"#;
    let req: LoginRequest = serde_json::from_str(wire).unwrap();
    assert_eq!(req.identifier, "ada");
    assert_eq!(req.password.expose(), "hunter2");
    assert_eq!(serde_json::to_string(&req).unwrap(), wire);
}

#[test]
fn session_response_round_trips() {
    let wire = r#"{"token":"jwt.abc.123"}"#;
    let session: SessionResponse = serde_json::from_str(wire).unwrap();
    assert_eq!(session.token, "jwt.abc.123");
    assert_eq!(serde_json::to_string(&session).unwrap(), wire);
}

// --- Password newtype: transparent string on the wire (the TUI depends on this). ---

#[test]
fn password_serializes_as_a_plain_string() {
    let pw = Password::new("hunter2".to_owned());
    let json = serde_json::to_value(&pw).unwrap();
    // Not an object/array — a bare JSON string.
    assert_eq!(json, Value::String("hunter2".to_owned()));
    assert_eq!(serde_json::to_string(&pw).unwrap(), r#""hunter2""#);
}

#[test]
fn password_deserializes_from_a_plain_string() {
    let pw: Password = serde_json::from_str(r#""hunter2""#).unwrap();
    assert_eq!(pw.expose(), "hunter2");
}

#[test]
fn password_round_trips_through_the_wire() {
    let pw = Password::new("p@ss w0rd with spaces".to_owned());
    let wire = serde_json::to_string(&pw).unwrap();
    let back: Password = serde_json::from_str(&wire).unwrap();
    assert_eq!(back.expose(), pw.expose());
}

#[test]
fn password_debug_is_redacted_so_it_cannot_leak() {
    let pw = Password::new("hunter2".to_owned());
    assert_eq!(format!("{pw:?}"), "[REDACTED]");
    // The redaction must hold even when the password is nested in a request struct.
    let req = LoginRequest {
        identifier: "ada".to_owned(),
        password: pw,
    };
    let dbg = format!("{req:?}");
    assert!(dbg.contains("[REDACTED]"), "debug was: {dbg}");
    assert!(!dbg.contains("hunter2"), "debug leaked the secret: {dbg}");
}
