//! Profile-isolation tests (hard-constraint #4, ADR-0005 §4): a profile the caller does not
//! own is indistinguishable from a nonexistent one — every cross-profile access is
//! `404 not_found` (never 403, never a leak), across GET, POST, and the close path.

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
use common::{app, get_auth, post_json_auth, register, send};
use contract::{ErrorCode, Task};
use serde_json::json;
use sqlx::PgPool;

/// Register two accounts and return (owner-of-the-target-profile, attacker).
async fn two_accounts(app: &axum::Router) -> (common::Account, common::Account) {
    let alice = register(app, "alice", "alice@example.com", "hunter2-long").await;
    let bob = register(app, "bob", "bob@example.com", "hunter2-long").await;
    (alice, bob)
}

/// user B listing user A's profile tasks → 404 `not_found`.
#[sqlx::test]
async fn list_other_users_profile_is_404(pool: PgPool) {
    let app = app(pool);
    let (alice, bob) = two_accounts(&app).await;

    let path = format!("/api/profiles/{}/tasks", alice.profile_id);
    let res = send(&app, get_auth(&path, &bob.token)).await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);
    res.expect_error(ErrorCode::NotFound);
}

/// user B creating a task in user A's profile → 404 `not_found` (no write leak).
#[sqlx::test]
async fn create_in_other_users_profile_is_404(pool: PgPool) {
    let app = app(pool);
    let (alice, bob) = two_accounts(&app).await;

    let path = format!("/api/profiles/{}/tasks", alice.profile_id);
    let res = send(
        &app,
        post_json_auth(
            &path,
            &bob.token,
            &json!({ "title": "intrusion", "description": "" }),
        ),
    )
    .await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);
    res.expect_error(ErrorCode::NotFound);

    // And the write did not land: alice's profile is still empty.
    let alice_list = send(
        &app,
        get_auth(
            &format!("/api/profiles/{}/tasks", alice.profile_id),
            &alice.token,
        ),
    )
    .await;
    let tasks: Vec<Task> = alice_list.parse();
    assert!(
        tasks.is_empty(),
        "the cross-profile write must not have landed"
    );
}

/// user B closing a task that exists in user A's profile → 404 `not_found`. The task id is
/// real, so this proves ownership is checked at the profile gate, not merely task existence.
#[sqlx::test]
async fn close_other_users_task_is_404(pool: PgPool) {
    let app = app(pool);
    let (alice, bob) = two_accounts(&app).await;

    // Alice creates a task in her own profile.
    let created = send(
        &app,
        post_json_auth(
            &format!("/api/profiles/{}/tasks", alice.profile_id),
            &alice.token,
            &json!({ "title": "alice's task", "description": "" }),
        ),
    )
    .await;
    let task: Task = created.parse();

    // Bob, knowing both ids, tries to close it → 404, indistinguishable from nonexistent.
    let close_path = format!("/api/profiles/{}/tasks/{}/close", alice.profile_id, task.id);
    let res = send(&app, post_json_auth(&close_path, &bob.token, &json!({}))).await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);
    res.expect_error(ErrorCode::NotFound);

    // The task is still open for alice: bob's attempt had no effect.
    let alice_list = send(
        &app,
        get_auth(
            &format!("/api/profiles/{}/tasks", alice.profile_id),
            &alice.token,
        ),
    )
    .await;
    let tasks: Vec<Task> = alice_list.parse();
    assert_eq!(tasks.len(), 1);
    let only = tasks.first().expect("one task");
    assert_eq!(only.status, contract::TaskStatus::Open, "untouched");
}

/// requesting a syntactically-valid but nonexistent profile id → 404 `not_found`.
#[sqlx::test]
async fn nonexistent_profile_is_404(pool: PgPool) {
    let app = app(pool);
    let bob = register(&app, "bob", "bob@example.com", "hunter2-long").await;

    let path = format!(
        "/api/profiles/{}/tasks",
        "11111111-1111-1111-1111-111111111111"
    );
    let res = send(&app, get_auth(&path, &bob.token)).await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);
    res.expect_error(ErrorCode::NotFound);
}

/// GET /api/profiles returns only the caller's own profiles, never another user's.
#[sqlx::test]
async fn profiles_list_is_scoped_to_caller(pool: PgPool) {
    let app = app(pool);
    let (alice, bob) = two_accounts(&app).await;

    let res = send(&app, get_auth("/api/profiles", &bob.token)).await;
    assert_eq!(res.status, StatusCode::OK);
    let profiles: Vec<contract::Profile> = res.parse();
    assert_eq!(profiles.len(), 1, "bob sees only his own profile");
    let bobs = profiles.first().expect("one profile");
    assert_ne!(
        bobs.id, alice.profile_id,
        "bob must not see alice's profile"
    );
}
