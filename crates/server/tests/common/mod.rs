//! Shared helpers for the server's HTTP integration tests.
//!
//! Each test drives the real `axum` app in-process via [`tower::ServiceExt::oneshot`] over a
//! per-test database supplied by `#[sqlx::test]` (a fresh DB with the embedded migrations
//! applied, dropped on completion). Only the public HTTP surface is exercised; the DB is the
//! one real external service, the rest is the server's own router.

#![allow(
    dead_code,
    reason = "helpers are shared across multiple test binaries; not every file uses every one"
)]

use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, Response, StatusCode, header};
use contract::{ErrorBody, ErrorCode};
use http_body_util::BodyExt as _;
use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
use secrecy::SecretString;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt as _;
use uuid::Uuid;

use server::{AppState, JwtConfig, router};

/// A dev-only signing secret for the test app. Never a real secret.
const TEST_JWT_SECRET: &str = "test-only-jwt-secret-do-not-use-in-production";

/// Build the real router over `pool` with the default 24 h token TTL.
pub fn app(pool: PgPool) -> Router {
    app_with_ttl(pool, Duration::from_secs(86_400))
}

/// Build the real router over `pool` with an explicit token TTL. A near-zero TTL yields
/// tokens that are already expired by the time they are presented, exercising the
/// expired-token branch of the auth extractor.
pub fn app_with_ttl(pool: PgPool, ttl: Duration) -> Router {
    let jwt = JwtConfig {
        secret: SecretString::from(TEST_JWT_SECRET),
        ttl,
    };
    router(AppState::new(pool, jwt))
}

/// The session-token claim shape as seen on the wire (subject + issued-at + expiry). Mirrored
/// here so a test can mint a token with an arbitrary `exp`, exercising the verifier's expiry
/// branch with a genuinely-elapsed token rather than one inside `jsonwebtoken`'s `exp` leeway.
#[derive(Serialize, Deserialize)]
struct TestClaims {
    sub: String,
    iat: i64,
    exp: i64,
}

/// Mint an HS256 token for `user_id` signed with [`TEST_JWT_SECRET`] whose `exp` is
/// `exp_secs_from_now` seconds from now (negative = already expired). A value well below
/// `jsonwebtoken`'s default 60 s `exp` leeway yields a token the verifier must reject as
/// expired. Mirrors the production claim shape as an external wire input — no source seam.
pub fn mint_token(user_id: Uuid, exp_secs_from_now: i64) -> String {
    let now = chrono::Utc::now().timestamp();
    let claims = TestClaims {
        sub: user_id.to_string(),
        iat: now,
        exp: now + exp_secs_from_now,
    };
    let key = EncodingKey::from_secret(TEST_JWT_SECRET.as_bytes());
    encode(&Header::new(Algorithm::HS256), &claims, &key).expect("token should sign")
}

/// Build a router whose JWT signing secret differs from [`TEST_JWT_SECRET`], so a token
/// issued by the default app fails signature verification here.
pub fn app_with_foreign_secret(pool: PgPool) -> Router {
    let jwt = JwtConfig {
        secret: SecretString::from("a-different-secret-entirely"),
        ttl: Duration::from_secs(86_400),
    };
    router(AppState::new(pool, jwt))
}

/// A parsed HTTP response: status plus the body decoded as JSON (or `Null` if empty).
pub struct Json {
    /// HTTP status code.
    pub status: StatusCode,
    /// Decoded JSON body (`Value::Null` when the body is empty).
    pub body: Value,
}

impl Json {
    /// Deserialize the body into a concrete type.
    pub fn parse<T: DeserializeOwned>(&self) -> T {
        serde_json::from_value(self.body.clone()).expect("body should deserialize")
    }

    /// Assert the body is the standard error contract with the given code and a non-empty
    /// human-readable message.
    pub fn expect_error(&self, code: ErrorCode) {
        let body: ErrorBody = self.parse();
        assert_eq!(
            body.code,
            Some(code),
            "error code mismatch in {:?}",
            self.body
        );
        assert!(!body.message.is_empty(), "error message must be non-empty");
    }
}

/// Send a request through the app and decode the response.
pub async fn send(app: &Router, request: Request<Body>) -> Json {
    let response: Response<Body> = app
        .clone()
        .oneshot(request)
        .await
        .expect("router should respond");
    let status = response.status();
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("body should collect")
        .to_bytes();
    let body = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or(Value::Null)
    };
    Json { status, body }
}

/// Build a JSON POST request to `path` with no Authorization header.
pub fn post_json(path: &str, body: &Value) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(path)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .expect("request should build")
}

/// Build a JSON POST request to `path` carrying a `Bearer` token.
pub fn post_json_auth(path: &str, token: &str, body: &Value) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(path)
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::from(body.to_string()))
        .expect("request should build")
}

/// Build a JSON PUT request to `path` carrying a `Bearer` token.
pub fn put_json_auth(path: &str, token: &str, body: &Value) -> Request<Body> {
    Request::builder()
        .method("PUT")
        .uri(path)
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::from(body.to_string()))
        .expect("request should build")
}

/// Build a JSON PATCH request to `path` carrying a `Bearer` token.
pub fn patch_json_auth(path: &str, token: &str, body: &Value) -> Request<Body> {
    Request::builder()
        .method("PATCH")
        .uri(path)
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::from(body.to_string()))
        .expect("request should build")
}

/// Build a JSON PATCH request to `path` with no Authorization header.
pub fn patch_json(path: &str, body: &Value) -> Request<Body> {
    Request::builder()
        .method("PATCH")
        .uri(path)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .expect("request should build")
}

/// Build a bodyless DELETE request to `path` carrying a `Bearer` token.
pub fn delete_auth(path: &str, token: &str) -> Request<Body> {
    Request::builder()
        .method("DELETE")
        .uri(path)
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())
        .expect("request should build")
}

/// Build a bodyless DELETE request to `path` with no Authorization header.
pub fn delete(path: &str) -> Request<Body> {
    Request::builder()
        .method("DELETE")
        .uri(path)
        .body(Body::empty())
        .expect("request should build")
}

/// Build a bodyless POST request to `path` carrying a `Bearer` token (for action endpoints
/// that take no request body, e.g. session start/stop).
pub fn post_auth(path: &str, token: &str) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(path)
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())
        .expect("request should build")
}

/// Build a bodyless POST request to `path` with no Authorization header.
pub fn post(path: &str) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(path)
        .body(Body::empty())
        .expect("request should build")
}

/// Build a JSON PUT request to `path` with no Authorization header.
pub fn put_json(path: &str, body: &Value) -> Request<Body> {
    Request::builder()
        .method("PUT")
        .uri(path)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .expect("request should build")
}

/// Build a GET request to `path` carrying a `Bearer` token.
pub fn get_auth(path: &str, token: &str) -> Request<Body> {
    Request::builder()
        .method("GET")
        .uri(path)
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())
        .expect("request should build")
}

/// Build a GET request to `path` with no Authorization header.
pub fn get(path: &str) -> Request<Body> {
    Request::builder()
        .method("GET")
        .uri(path)
        .body(Body::empty())
        .expect("request should build")
}

/// A registered account: the session token and the id of its default profile.
pub struct Account {
    /// Session JWT for `Authorization: Bearer`.
    pub token: String,
    /// The id of the default profile created at registration.
    pub profile_id: String,
}

/// Register a fresh account and return its token plus default-profile id. Asserts the
/// register→201 and the subsequent profiles→200 round-trips succeed.
pub async fn register(app: &Router, username: &str, email: &str, password: &str) -> Account {
    let body = json!({
        "username": username,
        "email": email,
        "password": password,
        "profile_name": "work",
    });
    let res = send(app, post_json("/api/auth/register", &body)).await;
    assert_eq!(res.status, StatusCode::CREATED, "register: {:?}", res.body);
    let token = res
        .body
        .get("token")
        .and_then(Value::as_str)
        .expect("register returns a token")
        .to_owned();

    let profiles = send(app, get_auth("/api/profiles", &token)).await;
    assert_eq!(profiles.status, StatusCode::OK);
    let profile_id = profiles
        .body
        .as_array()
        .and_then(|a| a.first())
        .and_then(|p| p.get("id"))
        .and_then(Value::as_str)
        .expect("default profile id")
        .to_owned();

    Account { token, profile_id }
}
