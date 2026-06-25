//! Integration tests for the task surface (ADR-0005 §5, ADR-0008) and the health endpoint:
//! add, list (profile-scoped, newest-first), partial update via `PATCH` (title/description/
//! status, the done↔reopen `closed_at` coupling, empty-patch no-op), delete, and title
//! validation — asserted as real HTTP round-trips against the `axum` app over a per-test
//! database.

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
use common::{
    app, delete, delete_auth, get, get_auth, patch_json, patch_json_auth, post_json_auth, register,
    send,
};
use contract::{ErrorCode, Task, TaskStatus};
use serde_json::json;
use sqlx::PgPool;

/// `GET /healthz` → 200, unauthenticated.
#[sqlx::test]
async fn healthz_is_200(pool: PgPool) {
    let app = app(pool);
    let res = send(&app, get("/healthz")).await;
    assert_eq!(res.status, StatusCode::OK);
}

/// add a task → 201 with the full contract shape (open, no closed_at).
#[sqlx::test]
async fn create_task_returns_201_open(pool: PgPool) {
    let app = app(pool);
    let account = register(&app, "ada", "ada@example.com", "hunter2-long").await;
    let path = format!("/api/profiles/{}/tasks", account.profile_id);

    let res = send(
        &app,
        post_json_auth(
            &path,
            &account.token,
            &json!({ "title": "write tests", "description": "cover the API" }),
        ),
    )
    .await;
    assert_eq!(res.status, StatusCode::CREATED);
    let task: Task = res.parse();
    assert_eq!(task.title, "write tests");
    assert_eq!(task.description, "cover the API");
    assert_eq!(task.status, TaskStatus::Open);
    assert!(task.closed_at.is_none(), "an open task has no closed_at");
    assert!(!task.id.is_empty());
}

/// the title is trimmed before storage.
#[sqlx::test]
async fn create_task_trims_title(pool: PgPool) {
    let app = app(pool);
    let account = register(&app, "ada", "ada@example.com", "hunter2-long").await;
    let path = format!("/api/profiles/{}/tasks", account.profile_id);

    let res = send(
        &app,
        post_json_auth(
            &path,
            &account.token,
            &json!({ "title": "  spaced  ", "description": "" }),
        ),
    )
    .await;
    assert_eq!(res.status, StatusCode::CREATED);
    let task: Task = res.parse();
    assert_eq!(task.title, "spaced", "title is trimmed");
}

/// a whitespace-only title → 400 `validation_failed`.
#[sqlx::test]
async fn create_task_blank_title_is_400(pool: PgPool) {
    let app = app(pool);
    let account = register(&app, "ada", "ada@example.com", "hunter2-long").await;
    let path = format!("/api/profiles/{}/tasks", account.profile_id);

    let res = send(
        &app,
        post_json_auth(
            &path,
            &account.token,
            &json!({ "title": "   ", "description": "x" }),
        ),
    )
    .await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST);
    res.expect_error(ErrorCode::ValidationFailed);
}

/// list returns the profile's tasks newest-first.
#[sqlx::test]
async fn list_tasks_newest_first(pool: PgPool) {
    let app = app(pool);
    let account = register(&app, "ada", "ada@example.com", "hunter2-long").await;
    let path = format!("/api/profiles/{}/tasks", account.profile_id);

    for title in ["first", "second", "third"] {
        let res = send(
            &app,
            post_json_auth(
                &path,
                &account.token,
                &json!({ "title": title, "description": "" }),
            ),
        )
        .await;
        assert_eq!(res.status, StatusCode::CREATED);
    }

    let res = send(&app, get_auth(&path, &account.token)).await;
    assert_eq!(res.status, StatusCode::OK);
    let tasks: Vec<Task> = res.parse();
    let titles: Vec<&str> = tasks.iter().map(|t| t.title.as_str()).collect();
    assert_eq!(titles, vec!["third", "second", "first"], "newest-first");
}

/// list on a brand-new profile is an empty array.
#[sqlx::test]
async fn list_tasks_empty(pool: PgPool) {
    let app = app(pool);
    let account = register(&app, "ada", "ada@example.com", "hunter2-long").await;
    let path = format!("/api/profiles/{}/tasks", account.profile_id);

    let res = send(&app, get_auth(&path, &account.token)).await;
    assert_eq!(res.status, StatusCode::OK);
    let tasks: Vec<Task> = res.parse();
    assert!(tasks.is_empty());
}

/// Create a task in `account`'s default profile and return it. Asserts the create→201.
async fn create_task(app: &axum::Router, account: &common::Account, title: &str) -> Task {
    let path = format!("/api/profiles/{}/tasks", account.profile_id);
    let res = send(
        app,
        post_json_auth(
            &path,
            &account.token,
            &json!({ "title": title, "description": "" }),
        ),
    )
    .await;
    assert_eq!(res.status, StatusCode::CREATED, "create: {:?}", res.body);
    res.parse()
}

/// `PATCH { status: done }` (the migrated close) → 200, status done, closed_at set.
#[sqlx::test]
async fn patch_status_done_marks_done(pool: PgPool) {
    let app = app(pool);
    let account = register(&app, "ada", "ada@example.com", "hunter2-long").await;
    let task = create_task(&app, &account, "finish").await;

    let path = format!("/api/profiles/{}/tasks/{}", account.profile_id, task.id);
    let res = send(
        &app,
        patch_json_auth(&path, &account.token, &json!({ "status": "done" })),
    )
    .await;
    assert_eq!(res.status, StatusCode::OK);
    let closed: Task = res.parse();
    assert_eq!(closed.status, TaskStatus::Done);
    assert!(closed.closed_at.is_some(), "closed_at is set");
    assert_eq!(closed.id, task.id);
}

/// re-applying `PATCH { status: done }` is idempotent: 200, unchanged, original closed_at
/// preserved (COALESCE — matching the old idempotent close).
#[sqlx::test]
async fn patch_status_done_is_idempotent(pool: PgPool) {
    let app = app(pool);
    let account = register(&app, "ada", "ada@example.com", "hunter2-long").await;
    let task = create_task(&app, &account, "finish").await;
    let path = format!("/api/profiles/{}/tasks/{}", account.profile_id, task.id);

    let first = send(
        &app,
        patch_json_auth(&path, &account.token, &json!({ "status": "done" })),
    )
    .await;
    assert_eq!(first.status, StatusCode::OK);
    let first_closed: Task = first.parse();

    let second = send(
        &app,
        patch_json_auth(&path, &account.token, &json!({ "status": "done" })),
    )
    .await;
    assert_eq!(second.status, StatusCode::OK, "re-close does not error");
    let second_closed: Task = second.parse();

    assert_eq!(second_closed.status, TaskStatus::Done);
    assert_eq!(
        second_closed.closed_at, first_closed.closed_at,
        "the original closed_at is preserved on re-applying done"
    );
}

/// `PATCH` on a task that does not exist in the profile → 404 `not_found`.
#[sqlx::test]
async fn patch_nonexistent_task_is_404(pool: PgPool) {
    let app = app(pool);
    let account = register(&app, "ada", "ada@example.com", "hunter2-long").await;
    let path = format!(
        "/api/profiles/{}/tasks/{}",
        account.profile_id, "00000000-0000-0000-0000-000000000000"
    );

    let res = send(
        &app,
        patch_json_auth(&path, &account.token, &json!({ "status": "done" })),
    )
    .await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);
    res.expect_error(ErrorCode::NotFound);
}

/// `PATCH { title }` updates only the title (trimmed); description/status unchanged.
#[sqlx::test]
async fn patch_title_only_updates_title_trimmed(pool: PgPool) {
    let app = app(pool);
    let account = register(&app, "ada", "ada@example.com", "hunter2-long").await;
    let path_base = format!("/api/profiles/{}/tasks", account.profile_id);
    let created = send(
        &app,
        post_json_auth(
            &path_base,
            &account.token,
            &json!({ "title": "old title", "description": "keep me" }),
        ),
    )
    .await;
    let task: Task = created.parse();

    let path = format!("{path_base}/{}", task.id);
    let res = send(
        &app,
        patch_json_auth(&path, &account.token, &json!({ "title": "  new title  " })),
    )
    .await;
    assert_eq!(res.status, StatusCode::OK);
    let updated: Task = res.parse();
    assert_eq!(updated.title, "new title", "title updated and trimmed");
    assert_eq!(updated.description, "keep me", "description untouched");
    assert_eq!(updated.status, TaskStatus::Open, "status untouched");
    assert!(
        updated.closed_at.is_none(),
        "closed_at untouched (absent status)"
    );
}

/// `PATCH { description }` updates only the description; title/status unchanged. Description
/// may be set to empty.
#[sqlx::test]
async fn patch_description_only_updates_description(pool: PgPool) {
    let app = app(pool);
    let account = register(&app, "ada", "ada@example.com", "hunter2-long").await;
    let path_base = format!("/api/profiles/{}/tasks", account.profile_id);
    let created = send(
        &app,
        post_json_auth(
            &path_base,
            &account.token,
            &json!({ "title": "keep title", "description": "old desc" }),
        ),
    )
    .await;
    let task: Task = created.parse();

    let path = format!("{path_base}/{}", task.id);
    let res = send(
        &app,
        patch_json_auth(&path, &account.token, &json!({ "description": "" })),
    )
    .await;
    assert_eq!(res.status, StatusCode::OK);
    let updated: Task = res.parse();
    assert_eq!(updated.title, "keep title", "title untouched");
    assert_eq!(updated.description, "", "description set to empty");
    assert_eq!(updated.status, TaskStatus::Open, "status untouched");
}

/// The highest-value test (plan Risks): done → reopen round-trip clears `closed_at` to null.
#[sqlx::test]
async fn patch_reopen_clears_closed_at(pool: PgPool) {
    let app = app(pool);
    let account = register(&app, "ada", "ada@example.com", "hunter2-long").await;
    let task = create_task(&app, &account, "round-trip").await;
    let path = format!("/api/profiles/{}/tasks/{}", account.profile_id, task.id);

    // done sets closed_at.
    let done = send(
        &app,
        patch_json_auth(&path, &account.token, &json!({ "status": "done" })),
    )
    .await;
    assert_eq!(done.status, StatusCode::OK);
    let done_task: Task = done.parse();
    assert_eq!(done_task.status, TaskStatus::Done);
    assert!(done_task.closed_at.is_some(), "done sets closed_at");

    // reopen clears it back to null.
    let reopened = send(
        &app,
        patch_json_auth(&path, &account.token, &json!({ "status": "open" })),
    )
    .await;
    assert_eq!(reopened.status, StatusCode::OK);
    let reopened_task: Task = reopened.parse();
    assert_eq!(reopened_task.status, TaskStatus::Open, "reopened");
    assert!(
        reopened_task.closed_at.is_none(),
        "reopen clears closed_at to null"
    );
    assert_eq!(reopened_task.id, task.id);
}

/// `PATCH` of multiple fields at once updates exactly those fields together.
#[sqlx::test]
async fn patch_multi_field_update(pool: PgPool) {
    let app = app(pool);
    let account = register(&app, "ada", "ada@example.com", "hunter2-long").await;
    let task = create_task(&app, &account, "before").await;
    let path = format!("/api/profiles/{}/tasks/{}", account.profile_id, task.id);

    let res = send(
        &app,
        patch_json_auth(
            &path,
            &account.token,
            &json!({ "title": "after", "description": "now described", "status": "done" }),
        ),
    )
    .await;
    assert_eq!(res.status, StatusCode::OK);
    let updated: Task = res.parse();
    assert_eq!(updated.title, "after");
    assert_eq!(updated.description, "now described");
    assert_eq!(updated.status, TaskStatus::Done);
    assert!(updated.closed_at.is_some(), "done set closed_at");
}

/// An empty patch `{}` → 200 and the task is returned unchanged (no-op).
#[sqlx::test]
async fn patch_empty_is_noop(pool: PgPool) {
    let app = app(pool);
    let account = register(&app, "ada", "ada@example.com", "hunter2-long").await;
    let path_base = format!("/api/profiles/{}/tasks", account.profile_id);
    let created = send(
        &app,
        post_json_auth(
            &path_base,
            &account.token,
            &json!({ "title": "unchanged", "description": "as is" }),
        ),
    )
    .await;
    let task: Task = created.parse();

    let path = format!("{path_base}/{}", task.id);
    let res = send(&app, patch_json_auth(&path, &account.token, &json!({}))).await;
    assert_eq!(res.status, StatusCode::OK);
    let updated: Task = res.parse();
    assert_eq!(updated.title, "unchanged");
    assert_eq!(updated.description, "as is");
    assert_eq!(updated.status, TaskStatus::Open);
    assert!(updated.closed_at.is_none());
    assert_eq!(updated.id, task.id);
}

/// `PATCH { title }` present-but-whitespace → 400 `validation_failed` (error contract asserted).
#[sqlx::test]
async fn patch_blank_title_is_400(pool: PgPool) {
    let app = app(pool);
    let account = register(&app, "ada", "ada@example.com", "hunter2-long").await;
    let task = create_task(&app, &account, "valid").await;
    let path = format!("/api/profiles/{}/tasks/{}", account.profile_id, task.id);

    let res = send(
        &app,
        patch_json_auth(&path, &account.token, &json!({ "title": "   " })),
    )
    .await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST);
    res.expect_error(ErrorCode::ValidationFailed);
}

/// `PATCH` with no/invalid token → 401 (auth required).
#[sqlx::test]
async fn patch_requires_auth(pool: PgPool) {
    let app = app(pool);
    let account = register(&app, "ada", "ada@example.com", "hunter2-long").await;
    let task = create_task(&app, &account, "guarded").await;
    let path = format!("/api/profiles/{}/tasks/{}", account.profile_id, task.id);

    let no_token = send(&app, patch_json(&path, &json!({ "status": "done" }))).await;
    assert_eq!(no_token.status, StatusCode::UNAUTHORIZED);

    let bad_token = send(
        &app,
        patch_json_auth(&path, "not-a-real-token", &json!({ "status": "done" })),
    )
    .await;
    assert_eq!(bad_token.status, StatusCode::UNAUTHORIZED);
}

/// `DELETE` → 204, then the task is gone and a second delete → 404 `not_found`.
#[sqlx::test]
async fn delete_task_then_second_delete_is_404(pool: PgPool) {
    let app = app(pool);
    let account = register(&app, "ada", "ada@example.com", "hunter2-long").await;
    let task = create_task(&app, &account, "doomed").await;
    let path = format!("/api/profiles/{}/tasks/{}", account.profile_id, task.id);

    let first = send(&app, delete_auth(&path, &account.token)).await;
    assert_eq!(first.status, StatusCode::NO_CONTENT);

    // It is gone from the list.
    let list = send(
        &app,
        get_auth(
            &format!("/api/profiles/{}/tasks", account.profile_id),
            &account.token,
        ),
    )
    .await;
    let tasks: Vec<Task> = list.parse();
    assert!(tasks.is_empty(), "the task was removed");

    // Second delete of the same id → 404.
    let second = send(&app, delete_auth(&path, &account.token)).await;
    assert_eq!(second.status, StatusCode::NOT_FOUND);
    second.expect_error(ErrorCode::NotFound);
}

/// `DELETE` of a task that never existed in the profile → 404 `not_found`.
#[sqlx::test]
async fn delete_nonexistent_task_is_404(pool: PgPool) {
    let app = app(pool);
    let account = register(&app, "ada", "ada@example.com", "hunter2-long").await;
    let path = format!(
        "/api/profiles/{}/tasks/{}",
        account.profile_id, "00000000-0000-0000-0000-000000000000"
    );

    let res = send(&app, delete_auth(&path, &account.token)).await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);
    res.expect_error(ErrorCode::NotFound);
}

/// `DELETE` with no token → 401 (auth required).
#[sqlx::test]
async fn delete_requires_auth(pool: PgPool) {
    let app = app(pool);
    let account = register(&app, "ada", "ada@example.com", "hunter2-long").await;
    let task = create_task(&app, &account, "guarded").await;
    let path = format!("/api/profiles/{}/tasks/{}", account.profile_id, task.id);

    let res = send(&app, delete(&path)).await;
    assert_eq!(res.status, StatusCode::UNAUTHORIZED);
}

/// The removed `POST .../close` route no longer exists: the old path is unrouted (404/405),
/// never a 200. Proves no code path still serves the breaking-removed endpoint.
#[sqlx::test]
async fn old_close_route_is_gone(pool: PgPool) {
    let app = app(pool);
    let account = register(&app, "ada", "ada@example.com", "hunter2-long").await;
    let task = create_task(&app, &account, "finish").await;

    let close_path = format!(
        "/api/profiles/{}/tasks/{}/close",
        account.profile_id, task.id
    );
    let res = send(
        &app,
        post_json_auth(&close_path, &account.token, &json!({})),
    )
    .await;
    assert!(
        matches!(
            res.status,
            StatusCode::NOT_FOUND | StatusCode::METHOD_NOT_ALLOWED
        ),
        "the removed close route must be unrouted (404/405), got {}",
        res.status
    );
    assert_ne!(res.status, StatusCode::OK, "close must not succeed");
}
