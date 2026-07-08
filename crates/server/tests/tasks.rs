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
use uuid::Uuid;

/// Insert a task fixture directly with a controlled `created_at` (UTC epoch **seconds**), so a
/// window-boundary test can pin inclusive-lower / exclusive-upper to the exact second — which the
/// `POST` create path cannot, since it stamps `created_at = now()`. Uses an unchecked runtime query
/// (no `.sqlx/` cache entry needed); it is fixture setup, not the surface under test — the public
/// `GET …/tasks` list endpoint is what the assertions exercise.
async fn insert_task_at(pool: &PgPool, profile_id: &str, title: &str, created_at_secs: i64) {
    let pid = Uuid::parse_str(profile_id).expect("profile id is a uuid");
    let _inserted = sqlx::query(
        "INSERT INTO tasks (profile_id, title, description, status, created_at) \
         VALUES ($1, $2, '', 'open', to_timestamp($3::bigint))",
    )
    .bind(pid)
    .bind(title)
    .bind(created_at_secs)
    .execute(pool)
    .await
    .expect("insert task fixture");
}

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

// ---- 0020 / ADR-0014: task-list limit + offset query params ----

/// `limit=N` caps the returned count to the N newest tasks (ADR-0014 §1–2). With five tasks and
/// `limit=2`, only the two newest come back, still newest-first.
#[sqlx::test]
async fn list_tasks_limit_caps_the_count(pool: PgPool) {
    let app = app(pool);
    let account = register(&app, "ada", "ada@example.com", "hunter2-long").await;
    let path = format!("/api/profiles/{}/tasks", account.profile_id);

    for title in ["one", "two", "three", "four", "five"] {
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

    let res = send(&app, get_auth(&format!("{path}?limit=2"), &account.token)).await;
    assert_eq!(res.status, StatusCode::OK);
    let tasks: Vec<Task> = res.parse();
    let titles: Vec<&str> = tasks.iter().map(|t| t.title.as_str()).collect();
    assert_eq!(
        titles,
        vec!["five", "four"],
        "limit=2 returns the two newest, newest-first",
    );
}

/// `offset=K` skips the K newest tasks (offset pagination, ADR-0014 §1). With five tasks and
/// `offset=2`, the leading two newest are skipped and the remainder returned newest-first.
#[sqlx::test]
async fn list_tasks_offset_skips_the_leading_tasks(pool: PgPool) {
    let app = app(pool);
    let account = register(&app, "ada", "ada@example.com", "hunter2-long").await;
    let path = format!("/api/profiles/{}/tasks", account.profile_id);

    for title in ["one", "two", "three", "four", "five"] {
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

    let res = send(&app, get_auth(&format!("{path}?offset=2"), &account.token)).await;
    assert_eq!(res.status, StatusCode::OK);
    let tasks: Vec<Task> = res.parse();
    let titles: Vec<&str> = tasks.iter().map(|t| t.title.as_str()).collect();
    assert_eq!(
        titles,
        vec!["three", "two", "one"],
        "offset=2 skips the two newest, returning the rest newest-first",
    );
}

/// `limit` + `offset` combine to page a window (offset pagination). `limit=2&offset=1` returns the
/// second-and-third-newest tasks.
#[sqlx::test]
async fn list_tasks_limit_and_offset_page_a_window(pool: PgPool) {
    let app = app(pool);
    let account = register(&app, "ada", "ada@example.com", "hunter2-long").await;
    let path = format!("/api/profiles/{}/tasks", account.profile_id);

    for title in ["one", "two", "three", "four", "five"] {
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

    let res = send(
        &app,
        get_auth(&format!("{path}?limit=2&offset=1"), &account.token),
    )
    .await;
    assert_eq!(res.status, StatusCode::OK);
    let tasks: Vec<Task> = res.parse();
    let titles: Vec<&str> = tasks.iter().map(|t| t.title.as_str()).collect();
    assert_eq!(
        titles,
        vec!["four", "three"],
        "limit=2&offset=1 returns the second and third newest",
    );
}

/// A `limit` strictly above `MAX_TASK_LIST_LIMIT` → `400 validation_failed` (no silent clamp,
/// ADR-0014 §2 / A3).
#[sqlx::test]
async fn list_tasks_limit_above_ceiling_is_400_validation_failed(pool: PgPool) {
    let app = app(pool);
    let account = register(&app, "ada", "ada@example.com", "hunter2-long").await;
    let path = format!("/api/profiles/{}/tasks", account.profile_id);

    let over = contract::MAX_TASK_LIST_LIMIT + 1;
    let res = send(
        &app,
        get_auth(&format!("{path}?limit={over}"), &account.token),
    )
    .await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST);
    res.expect_error(ErrorCode::ValidationFailed);
}

/// A `limit` exactly at the ceiling is accepted (the boundary is inclusive).
#[sqlx::test]
async fn list_tasks_limit_at_ceiling_is_ok(pool: PgPool) {
    let app = app(pool);
    let account = register(&app, "ada", "ada@example.com", "hunter2-long").await;
    let path = format!("/api/profiles/{}/tasks", account.profile_id);

    let res = send(
        &app,
        get_auth(
            &format!("{path}?limit={}", contract::MAX_TASK_LIST_LIMIT),
            &account.token,
        ),
    )
    .await;
    assert_eq!(
        res.status,
        StatusCode::OK,
        "a limit equal to the ceiling is accepted",
    );
    let tasks: Vec<Task> = res.parse();
    assert!(tasks.is_empty(), "empty profile still lists nothing");
}

/// The default (no query params) still returns the whole list newest-first — an old no-param
/// caller is unaffected (ADR-0014 §2: absent limit → server ceiling default).
#[sqlx::test]
async fn list_tasks_default_no_params_returns_whole_list_newest_first(pool: PgPool) {
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
    assert_eq!(
        titles,
        vec!["third", "second", "first"],
        "no params: the whole list, newest-first",
    );
}

/// Profile-scoping holds under a limit/offset query: a limited list never crosses profiles (#4).
/// Two profiles each own tasks; a limited list on profile A returns only A's tasks, never B's.
#[sqlx::test]
async fn list_tasks_with_limit_stays_profile_scoped(pool: PgPool) {
    let app = app(pool);
    let account = register(&app, "ada", "ada@example.com", "hunter2-long").await;

    // A second profile for the same account.
    let res = send(
        &app,
        post_json_auth(
            "/api/profiles",
            &account.token,
            &json!({ "name": "personal" }),
        ),
    )
    .await;
    assert_eq!(
        res.status,
        StatusCode::CREATED,
        "create profile: {:?}",
        res.body
    );
    let other: contract::Profile = res.parse();

    let path_a = format!("/api/profiles/{}/tasks", account.profile_id);
    let path_b = format!("/api/profiles/{}/tasks", other.id);

    // Profile A owns two tasks; profile B owns one with a recognizable title.
    for title in ["a-one", "a-two"] {
        let res = send(
            &app,
            post_json_auth(
                &path_a,
                &account.token,
                &json!({ "title": title, "description": "" }),
            ),
        )
        .await;
        assert_eq!(res.status, StatusCode::CREATED);
    }
    let res = send(
        &app,
        post_json_auth(
            &path_b,
            &account.token,
            &json!({ "title": "b-secret", "description": "" }),
        ),
    )
    .await;
    assert_eq!(res.status, StatusCode::CREATED);

    // A generously-limited list on A must return ONLY A's tasks — never B's, even though the limit
    // exceeds A's count (the LIMIT/OFFSET never widens the profile scope).
    let res = send(
        &app,
        get_auth(&format!("{path_a}?limit=100"), &account.token),
    )
    .await;
    assert_eq!(res.status, StatusCode::OK);
    let tasks: Vec<Task> = res.parse();
    let titles: Vec<&str> = tasks.iter().map(|t| t.title.as_str()).collect();
    assert_eq!(
        titles,
        vec!["a-two", "a-one"],
        "only profile A's tasks, newest-first"
    );
    assert!(
        !titles.contains(&"b-secret"),
        "a limited list never leaks the other profile's task: {titles:?}",
    );
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

// ---- 0023 / ADR-0015: task-list created_at date-window (created_from / created_until) ----

/// The window filter returns only rows with `created_from ≤ created_at < created_until`: inclusive
/// at the lower bound, exclusive at the upper bound, asserted at the exact boundary second. Six
/// fixtures straddle the window `[from, until)`; only the three inside come back, still newest-first.
#[sqlx::test]
async fn list_tasks_window_is_inclusive_lower_exclusive_upper_at_the_boundary_second(pool: PgPool) {
    let app = app(pool.clone());
    let account = register(&app, "ada", "ada@example.com", "hunter2-long").await;
    let path = format!("/api/profiles/{}/tasks", account.profile_id);

    let from: i64 = 1_720_137_600; // 2024-07-05T00:00:00Z
    let until: i64 = 1_720_483_200; // 2024-07-09T00:00:00Z

    // Straddle both boundaries to the exact second.
    insert_task_at(&pool, &account.profile_id, "below_from", from - 1).await;
    insert_task_at(&pool, &account.profile_id, "at_from", from).await; // inclusive → in
    insert_task_at(&pool, &account.profile_id, "middle", from + 1_000).await; // in
    insert_task_at(&pool, &account.profile_id, "at_until_minus_1", until - 1).await; // in
    insert_task_at(&pool, &account.profile_id, "at_until", until).await; // exclusive → out
    insert_task_at(&pool, &account.profile_id, "above_until", until + 1).await;

    let res = send(
        &app,
        get_auth(
            &format!("{path}?created_from={from}&created_until={until}"),
            &account.token,
        ),
    )
    .await;
    assert_eq!(res.status, StatusCode::OK);
    let tasks: Vec<Task> = res.parse();
    let titles: Vec<&str> = tasks.iter().map(|t| t.title.as_str()).collect();
    assert_eq!(
        titles,
        vec!["at_until_minus_1", "middle", "at_from"],
        "only [from, until) rows come back — inclusive lower, exclusive upper — newest-first",
    );
}

/// `created_from` alone is an open-ended lower bound: rows at or after it (inclusive) come back, the
/// one strictly before is excluded.
#[sqlx::test]
async fn list_tasks_created_from_alone_is_an_inclusive_lower_bound(pool: PgPool) {
    let app = app(pool.clone());
    let account = register(&app, "ada", "ada@example.com", "hunter2-long").await;
    let path = format!("/api/profiles/{}/tasks", account.profile_id);

    let from: i64 = 1_720_137_600;
    insert_task_at(&pool, &account.profile_id, "before", from - 1).await;
    insert_task_at(&pool, &account.profile_id, "boundary", from).await;
    insert_task_at(&pool, &account.profile_id, "after", from + 100).await;

    let res = send(
        &app,
        get_auth(&format!("{path}?created_from={from}"), &account.token),
    )
    .await;
    assert_eq!(res.status, StatusCode::OK);
    let tasks: Vec<Task> = res.parse();
    let titles: Vec<&str> = tasks.iter().map(|t| t.title.as_str()).collect();
    assert_eq!(
        titles,
        vec!["after", "boundary"],
        "created_from includes its boundary second and excludes anything strictly before it",
    );
}

/// `created_until` alone is an open-ended exclusive upper bound: the boundary-second row is excluded.
#[sqlx::test]
async fn list_tasks_created_until_alone_is_an_exclusive_upper_bound(pool: PgPool) {
    let app = app(pool.clone());
    let account = register(&app, "ada", "ada@example.com", "hunter2-long").await;
    let path = format!("/api/profiles/{}/tasks", account.profile_id);

    let until: i64 = 1_720_483_200;
    insert_task_at(&pool, &account.profile_id, "before", until - 100).await;
    insert_task_at(&pool, &account.profile_id, "boundary", until).await;
    insert_task_at(&pool, &account.profile_id, "after", until + 100).await;

    let res = send(
        &app,
        get_auth(&format!("{path}?created_until={until}"), &account.token),
    )
    .await;
    assert_eq!(res.status, StatusCode::OK);
    let tasks: Vec<Task> = res.parse();
    let titles: Vec<&str> = tasks.iter().map(|t| t.title.as_str()).collect();
    assert_eq!(
        titles,
        vec!["before"],
        "created_until excludes its own boundary second (upper is exclusive)",
    );
}

/// `created_from > created_until` is an inverted (necessarily-empty) window: a client bug rejected
/// as `400 validation_failed` (ADR-0015 §3), not a silent empty result.
#[sqlx::test]
async fn list_tasks_inverted_window_is_400_validation_failed(pool: PgPool) {
    let app = app(pool);
    let account = register(&app, "ada", "ada@example.com", "hunter2-long").await;
    let path = format!("/api/profiles/{}/tasks", account.profile_id);

    let res = send(
        &app,
        get_auth(
            &format!("{path}?created_from=1000&created_until=500"),
            &account.token,
        ),
    )
    .await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST);
    res.expect_error(ErrorCode::ValidationFailed);
}

/// `created_from == created_until` is a *valid* empty window (upper is exclusive) → `200 []`, not a
/// `400` (ADR-0015 §3). A task sitting exactly on the shared boundary is excluded by the upper bound.
#[sqlx::test]
async fn list_tasks_equal_bounds_window_is_200_empty(pool: PgPool) {
    let app = app(pool.clone());
    let account = register(&app, "ada", "ada@example.com", "hunter2-long").await;
    let path = format!("/api/profiles/{}/tasks", account.profile_id);

    let boundary: i64 = 1_720_137_600;
    insert_task_at(&pool, &account.profile_id, "on_boundary", boundary).await;

    let res = send(
        &app,
        get_auth(
            &format!("{path}?created_from={boundary}&created_until={boundary}"),
            &account.token,
        ),
    )
    .await;
    assert_eq!(
        res.status,
        StatusCode::OK,
        "equal bounds is valid, not a 400"
    );
    let tasks: Vec<Task> = res.parse();
    assert!(
        tasks.is_empty(),
        "an equal-bounds window is empty (upper bound is exclusive): {tasks:?}",
    );
}

/// Absent-both bounds returns the whole list within the limit — byte-identical to pre-0023
/// behaviour (ADR-0015 §2). Tasks with a spread of `created_at` all come back newest-first.
#[sqlx::test]
async fn list_tasks_absent_window_returns_whole_list_within_limit(pool: PgPool) {
    let app = app(pool.clone());
    let account = register(&app, "ada", "ada@example.com", "hunter2-long").await;
    let path = format!("/api/profiles/{}/tasks", account.profile_id);

    insert_task_at(&pool, &account.profile_id, "ancient", 1_000_000_000).await;
    insert_task_at(&pool, &account.profile_id, "old", 1_500_000_000).await;
    insert_task_at(&pool, &account.profile_id, "recent", 1_720_000_000).await;

    let res = send(&app, get_auth(&path, &account.token)).await;
    assert_eq!(res.status, StatusCode::OK);
    let tasks: Vec<Task> = res.parse();
    let titles: Vec<&str> = tasks.iter().map(|t| t.title.as_str()).collect();
    assert_eq!(
        titles,
        vec!["recent", "old", "ancient"],
        "no window params: the whole list, newest-first (created_at DESC), unaffected by 0023",
    );
}

/// The window filter is profile-scoped (#4): two profiles hold tasks inside the SAME window; a
/// windowed list on profile A returns only A's in-window tasks, never B's.
#[sqlx::test]
async fn list_tasks_window_is_profile_scoped(pool: PgPool) {
    let app = app(pool.clone());
    let a = register(&app, "ada", "ada@example.com", "hunter2-long").await;
    let b = register(&app, "bea", "bea@example.com", "hunter2-long").await;

    let from: i64 = 1_720_137_600;
    let until: i64 = 1_720_483_200;
    let inside = from + 1_000;
    insert_task_at(&pool, &a.profile_id, "a_in_window", inside).await;
    insert_task_at(&pool, &b.profile_id, "b_in_window", inside).await;

    let path_a = format!("/api/profiles/{}/tasks", a.profile_id);
    let res = send(
        &app,
        get_auth(
            &format!("{path_a}?created_from={from}&created_until={until}"),
            &a.token,
        ),
    )
    .await;
    assert_eq!(res.status, StatusCode::OK);
    let tasks: Vec<Task> = res.parse();
    let titles: Vec<&str> = tasks.iter().map(|t| t.title.as_str()).collect();
    assert_eq!(
        titles,
        vec!["a_in_window"],
        "the windowed list stays within profile A — B's in-window task never crosses (#4)",
    );
}

/// The window filter preserves `created_at DESC` ordering: several in-window tasks come back
/// newest-first, and it composes with `limit` (the newest N within the window).
#[sqlx::test]
async fn list_tasks_window_preserves_desc_order_and_composes_with_limit(pool: PgPool) {
    let app = app(pool.clone());
    let account = register(&app, "ada", "ada@example.com", "hunter2-long").await;
    let path = format!("/api/profiles/{}/tasks", account.profile_id);

    let from: i64 = 1_720_137_600;
    let until: i64 = 1_720_483_200;
    insert_task_at(&pool, &account.profile_id, "oldest", from + 10).await;
    insert_task_at(&pool, &account.profile_id, "middle", from + 20).await;
    insert_task_at(&pool, &account.profile_id, "newest", from + 30).await;
    // Outside the window — never eligible regardless of limit.
    insert_task_at(&pool, &account.profile_id, "outside", until + 1).await;

    let res = send(
        &app,
        get_auth(
            &format!("{path}?created_from={from}&created_until={until}"),
            &account.token,
        ),
    )
    .await;
    assert_eq!(res.status, StatusCode::OK);
    let titles: Vec<String> = res
        .parse::<Vec<Task>>()
        .iter()
        .map(|t| t.title.clone())
        .collect();
    assert_eq!(
        titles,
        vec!["newest", "middle", "oldest"],
        "in-window rows preserve created_at DESC",
    );

    // With limit=2 the window returns the two newest within it.
    let res = send(
        &app,
        get_auth(
            &format!("{path}?created_from={from}&created_until={until}&limit=2"),
            &account.token,
        ),
    )
    .await;
    assert_eq!(res.status, StatusCode::OK);
    let titles: Vec<String> = res
        .parse::<Vec<Task>>()
        .iter()
        .map(|t| t.title.clone())
        .collect();
    assert_eq!(
        titles,
        vec!["newest", "middle"],
        "the window composes with limit — the two newest within it",
    );
}
