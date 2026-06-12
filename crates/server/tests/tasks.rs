//! Integration tests for the task surface (ADR-0005 §5) and the health endpoint: add, list
//! (profile-scoped, newest-first), close (idempotent), and title validation — asserted as
//! real HTTP round-trips against the `axum` app over a per-test database.

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
use common::{app, get, get_auth, post_json_auth, register, send};
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

/// close a task → 200, status done, closed_at set.
#[sqlx::test]
async fn close_task_marks_done(pool: PgPool) {
    let app = app(pool);
    let account = register(&app, "ada", "ada@example.com", "hunter2-long").await;
    let tasks_path = format!("/api/profiles/{}/tasks", account.profile_id);

    let created = send(
        &app,
        post_json_auth(
            &tasks_path,
            &account.token,
            &json!({ "title": "finish", "description": "" }),
        ),
    )
    .await;
    let task: Task = created.parse();

    let close_path = format!(
        "/api/profiles/{}/tasks/{}/close",
        account.profile_id, task.id
    );
    let res = send(
        &app,
        post_json_auth(&close_path, &account.token, &json!({})),
    )
    .await;
    assert_eq!(res.status, StatusCode::OK);
    let closed: Task = res.parse();
    assert_eq!(closed.status, TaskStatus::Done);
    assert!(closed.closed_at.is_some(), "closed_at is set");
    assert_eq!(closed.id, task.id);
}

/// re-closing an already-done task is idempotent: 200, unchanged, original closed_at preserved.
#[sqlx::test]
async fn close_task_is_idempotent(pool: PgPool) {
    let app = app(pool);
    let account = register(&app, "ada", "ada@example.com", "hunter2-long").await;
    let tasks_path = format!("/api/profiles/{}/tasks", account.profile_id);

    let created = send(
        &app,
        post_json_auth(
            &tasks_path,
            &account.token,
            &json!({ "title": "finish", "description": "" }),
        ),
    )
    .await;
    let task: Task = created.parse();
    let close_path = format!(
        "/api/profiles/{}/tasks/{}/close",
        account.profile_id, task.id
    );

    let first = send(
        &app,
        post_json_auth(&close_path, &account.token, &json!({})),
    )
    .await;
    assert_eq!(first.status, StatusCode::OK);
    let first_closed: Task = first.parse();

    let second = send(
        &app,
        post_json_auth(&close_path, &account.token, &json!({})),
    )
    .await;
    assert_eq!(second.status, StatusCode::OK, "re-close does not error");
    let second_closed: Task = second.parse();

    assert_eq!(second_closed.status, TaskStatus::Done);
    assert_eq!(
        second_closed.closed_at, first_closed.closed_at,
        "the original closed_at is preserved on re-close"
    );
}

/// closing a task that does not exist in the profile → 404 `not_found`.
#[sqlx::test]
async fn close_nonexistent_task_is_404(pool: PgPool) {
    let app = app(pool);
    let account = register(&app, "ada", "ada@example.com", "hunter2-long").await;
    let close_path = format!(
        "/api/profiles/{}/tasks/{}/close",
        account.profile_id, "00000000-0000-0000-0000-000000000000"
    );

    let res = send(
        &app,
        post_json_auth(&close_path, &account.token, &json!({})),
    )
    .await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);
    res.expect_error(ErrorCode::NotFound);
}
