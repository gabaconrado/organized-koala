//! Wire-format and round-trip tests for the error payload (`ErrorBody`, `ErrorCode`),
//! locking the ADR-0005 §6 conventions: the stable code strings, `code` omitted when
//! `None`, and forward-compatible tolerance of unknown codes.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

use contract::{ErrorBody, ErrorCode};
use serde_json::json;

/// The ADR-0005 §6 stable code set, paired with the exact wire string each must use.
const KNOWN_CODES: &[(ErrorCode, &str)] = &[
    (ErrorCode::ValidationFailed, "validation_failed"),
    (ErrorCode::InvalidCredentials, "invalid_credentials"),
    (ErrorCode::Unauthenticated, "unauthenticated"),
    (ErrorCode::NotFound, "not_found"),
    (ErrorCode::UsernameTaken, "username_taken"),
    (ErrorCode::EmailTaken, "email_taken"),
    (ErrorCode::Internal, "internal"),
];

// --- ErrorCode: stable strings for every known variant. ---

#[test]
fn known_codes_serialize_to_their_exact_wire_string() {
    for (code, wire) in KNOWN_CODES {
        let serialized = serde_json::to_string(code).unwrap();
        assert_eq!(serialized, format!(r#""{wire}""#), "code {code:?}");
    }
}

#[test]
fn known_code_strings_deserialize_to_their_variant() {
    for (code, wire) in KNOWN_CODES {
        let parsed: ErrorCode = serde_json::from_str(&format!(r#""{wire}""#)).unwrap();
        assert_eq!(&parsed, code);
        assert!(parsed.is_known());
        assert_eq!(parsed.as_str(), *wire);
    }
}

#[test]
fn known_codes_round_trip_losslessly() {
    for (code, _) in KNOWN_CODES {
        let wire = serde_json::to_string(code).unwrap();
        let back: ErrorCode = serde_json::from_str(&wire).unwrap();
        assert_eq!(&back, code);
    }
}

// --- ErrorCode: unknown codes are tolerated (acceptance criterion). ---

#[test]
fn unknown_code_deserializes_without_error() {
    // A code this crate version does not know must NOT fail to parse — a consumer built
    // against an older code set still reads a newer server's response.
    let parsed: ErrorCode = serde_json::from_str(r#""rate_limited""#).unwrap();
    assert_eq!(parsed, ErrorCode::Unknown("rate_limited".to_owned()));
    assert!(!parsed.is_known());
    assert_eq!(parsed.as_str(), "rate_limited");
}

#[test]
fn unknown_code_round_trips_losslessly() {
    // The raw wire string is preserved across a parse-then-reserialize cycle.
    let wire = r#""some_future_code""#;
    let parsed: ErrorCode = serde_json::from_str(wire).unwrap();
    assert_eq!(serde_json::to_string(&parsed).unwrap(), wire);
}

#[test]
fn unknown_code_does_not_collide_with_a_known_one() {
    let unknown: ErrorCode = serde_json::from_str(r#""not_found_extra""#).unwrap();
    assert_ne!(unknown, ErrorCode::NotFound);
    assert!(!unknown.is_known());
}

// --- ErrorBody: optional code, always-present message. ---

#[test]
fn error_body_with_code_serializes_both_fields() {
    let body = ErrorBody {
        code: Some(ErrorCode::NotFound),
        message: "no such task".to_owned(),
    };
    let json = serde_json::to_value(&body).unwrap();
    assert_eq!(
        json,
        json!({ "code": "not_found", "message": "no such task" })
    );
}

#[test]
fn error_body_omits_code_when_none() {
    // `code` is skipped from the JSON entirely when `None` (not emitted as null).
    let body = ErrorBody {
        code: None,
        message: "something went wrong".to_owned(),
    };
    let serialized = serde_json::to_string(&body).unwrap();
    assert_eq!(serialized, r#"{"message":"something went wrong"}"#);

    let json = serde_json::to_value(&body).unwrap();
    assert!(!json.as_object().unwrap().contains_key("code"));
}

#[test]
fn error_body_deserializes_with_absent_code() {
    let body: ErrorBody = serde_json::from_str(r#"{"message":"boom"}"#).unwrap();
    assert_eq!(body.code, None);
    assert_eq!(body.message, "boom");
}

#[test]
fn error_body_deserializes_with_known_code() {
    let body: ErrorBody =
        serde_json::from_str(r#"{"code":"username_taken","message":"taken"}"#).unwrap();
    assert_eq!(body.code, Some(ErrorCode::UsernameTaken));
    assert_eq!(body.message, "taken");
}

#[test]
fn error_body_tolerates_an_unknown_code() {
    // A full error body carrying a future code must still parse (forward compat).
    let body: ErrorBody =
        serde_json::from_str(r#"{"code":"rate_limited","message":"slow down"}"#).unwrap();
    assert_eq!(
        body.code,
        Some(ErrorCode::Unknown("rate_limited".to_owned()))
    );
    assert_eq!(body.message, "slow down");
}

#[test]
fn error_body_round_trips_losslessly() {
    for body in [
        ErrorBody {
            code: Some(ErrorCode::ValidationFailed),
            message: "bad".to_owned(),
        },
        ErrorBody {
            code: None,
            message: "generic".to_owned(),
        },
        ErrorBody {
            code: Some(ErrorCode::Unknown("future".to_owned())),
            message: "newer".to_owned(),
        },
    ] {
        let wire = serde_json::to_string(&body).unwrap();
        let back: ErrorBody = serde_json::from_str(&wire).unwrap();
        assert_eq!(back, body);
    }
}
