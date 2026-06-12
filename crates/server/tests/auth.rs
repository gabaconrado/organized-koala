//! Integration tests for the auth surface (ADR-0005 §2–3): register, login, and the failure
//! paths, asserted as real HTTP round-trips against the `axum` app over a per-test database.

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
use common::{app, post_json, register, send};
use contract::{ErrorCode, SessionResponse};
use serde_json::json;
use sqlx::PgPool;

/// register → 201 with a session token, and the default profile exists afterwards.
#[sqlx::test]
async fn register_creates_account_and_default_profile(pool: PgPool) {
    let app = app(pool);
    let body = json!({
        "username": "ada",
        "email": "ada@example.com",
        "password": "hunter2-long",
        "profile_name": "work",
    });

    let res = send(&app, post_json("/api/auth/register", &body)).await;
    assert_eq!(res.status, StatusCode::CREATED);
    let session: SessionResponse = res.parse();
    assert!(!session.token.is_empty(), "register returns a token");

    // The default profile was created atomically and is named `profile_name`.
    let profiles = send(&app, common::get_auth("/api/profiles", &session.token)).await;
    assert_eq!(profiles.status, StatusCode::OK);
    let arr = profiles.body.as_array().expect("profiles is an array");
    assert_eq!(arr.len(), 1, "exactly one default profile");
    let name = arr
        .first()
        .and_then(|p| p.get("name"))
        .and_then(serde_json::Value::as_str);
    assert_eq!(name, Some("work"));
}

/// login by username → 200 token.
#[sqlx::test]
async fn login_by_username(pool: PgPool) {
    let app = app(pool);
    let _account = register(&app, "ada", "ada@example.com", "hunter2-long").await;

    let res = send(
        &app,
        post_json(
            "/api/auth/login",
            &json!({ "identifier": "ada", "password": "hunter2-long" }),
        ),
    )
    .await;
    assert_eq!(res.status, StatusCode::OK);
    let session: SessionResponse = res.parse();
    assert!(!session.token.is_empty());
}

/// login by email → 200 token.
#[sqlx::test]
async fn login_by_email(pool: PgPool) {
    let app = app(pool);
    let _account = register(&app, "ada", "ada@example.com", "hunter2-long").await;

    let res = send(
        &app,
        post_json(
            "/api/auth/login",
            &json!({ "identifier": "ada@example.com", "password": "hunter2-long" }),
        ),
    )
    .await;
    assert_eq!(res.status, StatusCode::OK);
    let session: SessionResponse = res.parse();
    assert!(!session.token.is_empty());
}

/// duplicate username → 409 `username_taken`.
#[sqlx::test]
async fn duplicate_username_is_409(pool: PgPool) {
    let app = app(pool);
    let _account = register(&app, "ada", "ada@example.com", "hunter2-long").await;

    let res = send(
        &app,
        post_json(
            "/api/auth/register",
            &json!({
                "username": "ada",
                "email": "different@example.com",
                "password": "hunter2-long",
                "profile_name": "work",
            }),
        ),
    )
    .await;
    assert_eq!(res.status, StatusCode::CONFLICT);
    res.expect_error(ErrorCode::UsernameTaken);
}

/// duplicate email → 409 `email_taken`.
#[sqlx::test]
async fn duplicate_email_is_409(pool: PgPool) {
    let app = app(pool);
    let _account = register(&app, "ada", "ada@example.com", "hunter2-long").await;

    let res = send(
        &app,
        post_json(
            "/api/auth/register",
            &json!({
                "username": "grace",
                "email": "ada@example.com",
                "password": "hunter2-long",
                "profile_name": "work",
            }),
        ),
    )
    .await;
    assert_eq!(res.status, StatusCode::CONFLICT);
    res.expect_error(ErrorCode::EmailTaken);
}

/// wrong password → 401 `invalid_credentials`.
#[sqlx::test]
async fn wrong_password_is_401(pool: PgPool) {
    let app = app(pool);
    let _account = register(&app, "ada", "ada@example.com", "hunter2-long").await;

    let res = send(
        &app,
        post_json(
            "/api/auth/login",
            &json!({ "identifier": "ada", "password": "wrong-password" }),
        ),
    )
    .await;
    assert_eq!(res.status, StatusCode::UNAUTHORIZED);
    res.expect_error(ErrorCode::InvalidCredentials);
}

/// unknown user → 401 `invalid_credentials` (indistinguishable from a wrong password).
#[sqlx::test]
async fn unknown_user_is_401(pool: PgPool) {
    let app = app(pool);

    let res = send(
        &app,
        post_json(
            "/api/auth/login",
            &json!({ "identifier": "nobody", "password": "whatever-long" }),
        ),
    )
    .await;
    assert_eq!(res.status, StatusCode::UNAUTHORIZED);
    res.expect_error(ErrorCode::InvalidCredentials);
}

/// register with a `@` in the username → 400 `validation_failed`.
#[sqlx::test]
async fn username_with_at_is_rejected(pool: PgPool) {
    let app = app(pool);

    let res = send(
        &app,
        post_json(
            "/api/auth/register",
            &json!({
                "username": "ada@home",
                "email": "ada@example.com",
                "password": "hunter2-long",
                "profile_name": "work",
            }),
        ),
    )
    .await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST);
    res.expect_error(ErrorCode::ValidationFailed);
}

/// register with an empty username → 400 `validation_failed`.
#[sqlx::test]
async fn empty_username_is_rejected(pool: PgPool) {
    let app = app(pool);

    let res = send(
        &app,
        post_json(
            "/api/auth/register",
            &json!({
                "username": "   ",
                "email": "ada@example.com",
                "password": "hunter2-long",
                "profile_name": "work",
            }),
        ),
    )
    .await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST);
    res.expect_error(ErrorCode::ValidationFailed);
}

/// register with an invalid email (no `@`) → 400 `validation_failed`.
#[sqlx::test]
async fn invalid_email_is_rejected(pool: PgPool) {
    let app = app(pool);

    let res = send(
        &app,
        post_json(
            "/api/auth/register",
            &json!({
                "username": "ada",
                "email": "not-an-email",
                "password": "hunter2-long",
                "profile_name": "work",
            }),
        ),
    )
    .await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST);
    res.expect_error(ErrorCode::ValidationFailed);
}

/// register with an empty password → 400 `validation_failed`.
#[sqlx::test]
async fn empty_password_is_rejected(pool: PgPool) {
    let app = app(pool);

    let res = send(
        &app,
        post_json(
            "/api/auth/register",
            &json!({
                "username": "ada",
                "email": "ada@example.com",
                "password": "",
                "profile_name": "work",
            }),
        ),
    )
    .await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST);
    res.expect_error(ErrorCode::ValidationFailed);
}

/// A protected route with no token → 401 `unauthenticated`.
#[sqlx::test]
async fn missing_token_is_401(pool: PgPool) {
    let app = app(pool);

    let res = send(&app, common::get("/api/profiles")).await;
    assert_eq!(res.status, StatusCode::UNAUTHORIZED);
    res.expect_error(ErrorCode::Unauthenticated);
}

/// A protected route with a malformed token → 401 `unauthenticated`.
#[sqlx::test]
async fn malformed_token_is_401(pool: PgPool) {
    let app = app(pool);

    let res = send(&app, common::get_auth("/api/profiles", "not.a.jwt")).await;
    assert_eq!(res.status, StatusCode::UNAUTHORIZED);
    res.expect_error(ErrorCode::Unauthenticated);
}

// NOTE: expired-token → 401 is intentionally NOT asserted here. The only public route to a
// token is `Jwt::issue`, which always sets `exp = now + ttl` for a non-negative `ttl`; even a
// zero TTL lands inside `jsonwebtoken`'s default 60 s `exp` leeway, so no genuinely-expired
// token is constructible through the public surface. Expiry enforcement lives in `Jwt::verify`
// (source-owned) and is covered by the verifier's live pass (ADR-0003); the extractor's reject
// path is exercised here via the missing/malformed/foreign-signature cases.

/// A token signed with a different secret → 401 `unauthenticated`. The token is issued by an
/// app whose secret differs from the verifying app's, so the signature fails.
#[sqlx::test]
async fn token_with_foreign_signature_is_401(pool: PgPool) {
    let foreign = common::app_with_foreign_secret(pool.clone());
    let account = register(&foreign, "ada", "ada@example.com", "hunter2-long").await;

    // The default app uses the standard secret; the foreign-signed token must not verify.
    let verifier = app(pool);
    let res = send(&verifier, common::get_auth("/api/profiles", &account.token)).await;
    assert_eq!(res.status, StatusCode::UNAUTHORIZED);
    res.expect_error(ErrorCode::Unauthenticated);
}
