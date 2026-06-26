//! The bearer JWT must never be reachable from any derived/`{:?}` `Debug` in the `tui` crate
//! (0013 acceptance, criterion 1). The token is held in the redacting [`SessionToken`] newtype,
//! so every `#[derive(Debug)]` type that carries it renders `[REDACTED]` in place of the secret.
//!
//! Each test below builds a token-carrying public type with a recognizable fake token value,
//! formats it with `{:?}`, and asserts the output (a) does NOT contain the token substring and
//! (b) DOES contain `[REDACTED]` — mirroring `contract`'s `Password` doctest
//! (`assert_eq!(format!("{:?}", req.password), "[REDACTED]")`). Non-secret fields (profile id /
//! name) are asserted to still render normally, proving the redaction is scoped to the token.
//!
//! These exercise only public types (`tui::app::{Session, ClientRequest, Outcome, SessionToken}`)
//! and use an obviously-fake placeholder token; no external service is involved.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

use tui::app::{ClientRequest, Outcome, Session, SessionToken};

/// An obviously-fake placeholder bearer string — never a real or plausible JWT (the Board
/// pre-commit secret scan must pass). Used as the canary substring the redacted `Debug` must
/// not contain.
const FAKE_TOKEN: &str = "SECRET.JWT.VALUE";

/// `Session`'s derived `Debug` must redact the bearer token while still rendering the
/// (non-secret) profile id and name.
#[test]
fn session_debug_redacts_token() {
    let session = Session {
        token: SessionToken::new(FAKE_TOKEN.to_owned()),
        profile_id: "p1".to_owned(),
        profile_name: "work".to_owned(),
    };

    let rendered = format!("{session:?}");

    assert!(
        !rendered.contains(FAKE_TOKEN),
        "Session Debug leaked the bearer token: {rendered}"
    );
    assert!(
        rendered.contains("[REDACTED]"),
        "Session Debug did not redact the token: {rendered}"
    );
    // The non-secret fields still render normally.
    assert!(
        rendered.contains("p1"),
        "Session Debug dropped profile_id: {rendered}"
    );
    assert!(
        rendered.contains("work"),
        "Session Debug dropped profile_name: {rendered}"
    );
}

/// A `ClientRequest` variant carrying a token must redact it in its derived `Debug` while still
/// rendering the (non-secret) profile id.
#[test]
fn client_request_debug_redacts_token() {
    let request = ClientRequest::ListTasks {
        token: SessionToken::new(FAKE_TOKEN.to_owned()),
        profile_id: "p1".to_owned(),
    };

    let rendered = format!("{request:?}");

    assert!(
        !rendered.contains(FAKE_TOKEN),
        "ClientRequest Debug leaked the bearer token: {rendered}"
    );
    assert!(
        rendered.contains("[REDACTED]"),
        "ClientRequest Debug did not redact the token: {rendered}"
    );
    // The non-secret profile id still renders normally.
    assert!(
        rendered.contains("p1"),
        "ClientRequest Debug dropped profile_id: {rendered}"
    );
}

/// `Outcome::ListProfiles` carries the token it ran under; its derived `Debug` must redact it.
#[test]
fn outcome_list_profiles_debug_redacts_token() {
    let outcome = Outcome::ListProfiles {
        token: SessionToken::new(FAKE_TOKEN.to_owned()),
        result: Ok(Vec::new()),
    };

    let rendered = format!("{outcome:?}");

    assert!(
        !rendered.contains(FAKE_TOKEN),
        "Outcome Debug leaked the bearer token: {rendered}"
    );
    assert!(
        rendered.contains("[REDACTED]"),
        "Outcome Debug did not redact the token: {rendered}"
    );
}
