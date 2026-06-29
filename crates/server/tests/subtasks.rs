//! Integration tests for the sub-task surface (ADR-0012/ADR-0013): create / list (per-task and
//! per-profile, creation order) / edit-title / toggle / delete, the blank-title `400`, the empty
//! patch no-op, **profile-scoping** (a sub-task under another profile's task → `404`),
//! **parent-scoping** (a wrong `{tid}` → `404`), and the **cascade** guarantee (deleting a parent
//! task removes its sub-tasks; deleting a profile removes them transitively) — asserted as real
//! HTTP round-trips against the `axum` app over a per-test database.

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
    Account, app, delete_auth, get_auth, patch_json, patch_json_auth, post_json_auth, register,
    send,
};
use contract::{ErrorCode, Subtask, Task, TaskStatus};
use serde_json::json;
use sqlx::PgPool;

const MISSING_ID: &str = "00000000-0000-0000-0000-000000000000";

/// Create a task in `account`'s default profile and return it. Asserts the create→201.
async fn create_task(app: &axum::Router, account: &Account, title: &str) -> Task {
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
    assert_eq!(
        res.status,
        StatusCode::CREATED,
        "create task: {:?}",
        res.body
    );
    res.parse()
}

/// Create a sub-task under `task_id` and return it. Asserts the create→201.
async fn create_subtask(
    app: &axum::Router,
    account: &Account,
    task_id: &str,
    title: &str,
) -> Subtask {
    let path = format!(
        "/api/profiles/{}/tasks/{}/subtasks",
        account.profile_id, task_id
    );
    let res = send(
        app,
        post_json_auth(&path, &account.token, &json!({ "title": title })),
    )
    .await;
    assert_eq!(
        res.status,
        StatusCode::CREATED,
        "create subtask: {:?}",
        res.body
    );
    res.parse()
}

/// Create a second profile for `account` and return its id. Asserts the create→201.
async fn second_profile(app: &axum::Router, account: &Account) -> String {
    let res = send(
        app,
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
    res.body
        .get("id")
        .and_then(serde_json::Value::as_str)
        .expect("created profile id")
        .to_owned()
}

// --- create ---

#[sqlx::test]
async fn create_subtask_returns_201_open(pool: PgPool) {
    let app = app(pool);
    let account = register(&app, "ada", "ada@example.com", "hunter2-long").await;
    let task = create_task(&app, &account, "parent").await;

    let path = format!(
        "/api/profiles/{}/tasks/{}/subtasks",
        account.profile_id, task.id
    );
    let res = send(
        &app,
        post_json_auth(&path, &account.token, &json!({ "title": "child" })),
    )
    .await;
    assert_eq!(res.status, StatusCode::CREATED);
    let subtask: Subtask = res.parse();
    assert_eq!(subtask.title, "child");
    assert_eq!(
        subtask.status,
        TaskStatus::Open,
        "a new sub-task starts open"
    );
    assert_eq!(subtask.task_id, task.id, "linked to its parent task");
    assert!(!subtask.id.is_empty());
}

#[sqlx::test]
async fn create_subtask_trims_title(pool: PgPool) {
    let app = app(pool);
    let account = register(&app, "ada", "ada@example.com", "hunter2-long").await;
    let task = create_task(&app, &account, "parent").await;

    let path = format!(
        "/api/profiles/{}/tasks/{}/subtasks",
        account.profile_id, task.id
    );
    let res = send(
        &app,
        post_json_auth(&path, &account.token, &json!({ "title": "  spaced  " })),
    )
    .await;
    assert_eq!(res.status, StatusCode::CREATED);
    let subtask: Subtask = res.parse();
    assert_eq!(subtask.title, "spaced", "title is trimmed");
}

#[sqlx::test]
async fn create_subtask_blank_title_is_400(pool: PgPool) {
    let app = app(pool);
    let account = register(&app, "ada", "ada@example.com", "hunter2-long").await;
    let task = create_task(&app, &account, "parent").await;

    let path = format!(
        "/api/profiles/{}/tasks/{}/subtasks",
        account.profile_id, task.id
    );
    let res = send(
        &app,
        post_json_auth(&path, &account.token, &json!({ "title": "   " })),
    )
    .await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST);
    res.expect_error(ErrorCode::ValidationFailed);
}

#[sqlx::test]
async fn create_subtask_under_missing_parent_is_404(pool: PgPool) {
    let app = app(pool);
    let account = register(&app, "ada", "ada@example.com", "hunter2-long").await;

    let path = format!(
        "/api/profiles/{}/tasks/{}/subtasks",
        account.profile_id, MISSING_ID
    );
    let res = send(
        &app,
        post_json_auth(&path, &account.token, &json!({ "title": "orphan" })),
    )
    .await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);
    res.expect_error(ErrorCode::NotFound);
}

// --- list (per-task + per-profile, creation order) ---

#[sqlx::test]
async fn list_task_subtasks_in_creation_order(pool: PgPool) {
    let app = app(pool);
    let account = register(&app, "ada", "ada@example.com", "hunter2-long").await;
    let task = create_task(&app, &account, "parent").await;

    for title in ["first", "second", "third"] {
        let _ = create_subtask(&app, &account, &task.id, title).await;
    }

    let path = format!(
        "/api/profiles/{}/tasks/{}/subtasks",
        account.profile_id, task.id
    );
    let res = send(&app, get_auth(&path, &account.token)).await;
    assert_eq!(res.status, StatusCode::OK);
    let subtasks: Vec<Subtask> = res.parse();
    let titles: Vec<&str> = subtasks.iter().map(|s| s.title.as_str()).collect();
    assert_eq!(
        titles,
        vec!["first", "second", "third"],
        "creation order (created_at ASC)",
    );
}

#[sqlx::test]
async fn list_task_subtasks_empty_for_a_task_with_none(pool: PgPool) {
    let app = app(pool);
    let account = register(&app, "ada", "ada@example.com", "hunter2-long").await;
    let task = create_task(&app, &account, "parent").await;

    let path = format!(
        "/api/profiles/{}/tasks/{}/subtasks",
        account.profile_id, task.id
    );
    let res = send(&app, get_auth(&path, &account.token)).await;
    assert_eq!(res.status, StatusCode::OK);
    let subtasks: Vec<Subtask> = res.parse();
    assert!(subtasks.is_empty());
}

#[sqlx::test]
async fn list_profile_subtasks_returns_all_grouped_under_their_parents(pool: PgPool) {
    let app = app(pool);
    let account = register(&app, "ada", "ada@example.com", "hunter2-long").await;
    let task_a = create_task(&app, &account, "task A").await;
    let task_b = create_task(&app, &account, "task B").await;

    let a1 = create_subtask(&app, &account, &task_a.id, "a1").await;
    let a2 = create_subtask(&app, &account, &task_a.id, "a2").await;
    let b1 = create_subtask(&app, &account, &task_b.id, "b1").await;

    let path = format!("/api/profiles/{}/subtasks", account.profile_id);
    let res = send(&app, get_auth(&path, &account.token)).await;
    assert_eq!(res.status, StatusCode::OK);
    let subtasks: Vec<Subtask> = res.parse();
    assert_eq!(subtasks.len(), 3, "all the profile's sub-tasks");

    // Every returned sub-task carries its parent linkage; the ids cover exactly what we created.
    let ids: Vec<&str> = subtasks.iter().map(|s| s.id.as_str()).collect();
    for created in [&a1, &a2, &b1] {
        assert!(
            ids.contains(&created.id.as_str()),
            "{} present",
            created.title
        );
    }
    // Within a parent, creation order holds (a1 before a2).
    let a_titles: Vec<&str> = subtasks
        .iter()
        .filter(|s| s.task_id == task_a.id)
        .map(|s| s.title.as_str())
        .collect();
    assert_eq!(a_titles, vec!["a1", "a2"], "creation order within task A");
}

// --- edit-title + toggle (PATCH partial) ---

#[sqlx::test]
async fn patch_title_only_updates_title_trimmed(pool: PgPool) {
    let app = app(pool);
    let account = register(&app, "ada", "ada@example.com", "hunter2-long").await;
    let task = create_task(&app, &account, "parent").await;
    let subtask = create_subtask(&app, &account, &task.id, "old").await;

    let path = format!(
        "/api/profiles/{}/tasks/{}/subtasks/{}",
        account.profile_id, task.id, subtask.id
    );
    let res = send(
        &app,
        patch_json_auth(&path, &account.token, &json!({ "title": "  new  " })),
    )
    .await;
    assert_eq!(res.status, StatusCode::OK);
    let updated: Subtask = res.parse();
    assert_eq!(updated.title, "new", "title updated and trimmed");
    assert_eq!(updated.status, TaskStatus::Open, "status untouched");
    assert_eq!(updated.id, subtask.id);
}

#[sqlx::test]
async fn patch_status_toggles_done_then_reopen(pool: PgPool) {
    let app = app(pool);
    let account = register(&app, "ada", "ada@example.com", "hunter2-long").await;
    let task = create_task(&app, &account, "parent").await;
    let subtask = create_subtask(&app, &account, &task.id, "child").await;
    let path = format!(
        "/api/profiles/{}/tasks/{}/subtasks/{}",
        account.profile_id, task.id, subtask.id
    );

    // open -> done (a plain status flip; a sub-task has no closed_at).
    let done = send(
        &app,
        patch_json_auth(&path, &account.token, &json!({ "status": "done" })),
    )
    .await;
    assert_eq!(done.status, StatusCode::OK);
    assert_eq!(done.parse::<Subtask>().status, TaskStatus::Done);

    // done -> open (reopen).
    let open = send(
        &app,
        patch_json_auth(&path, &account.token, &json!({ "status": "open" })),
    )
    .await;
    assert_eq!(open.status, StatusCode::OK);
    let reopened: Subtask = open.parse();
    assert_eq!(reopened.status, TaskStatus::Open);
    assert_eq!(reopened.title, "child", "title preserved across the toggle");
}

#[sqlx::test]
async fn patch_empty_is_noop(pool: PgPool) {
    let app = app(pool);
    let account = register(&app, "ada", "ada@example.com", "hunter2-long").await;
    let task = create_task(&app, &account, "parent").await;
    let subtask = create_subtask(&app, &account, &task.id, "unchanged").await;

    let path = format!(
        "/api/profiles/{}/tasks/{}/subtasks/{}",
        account.profile_id, task.id, subtask.id
    );
    let res = send(&app, patch_json_auth(&path, &account.token, &json!({}))).await;
    assert_eq!(res.status, StatusCode::OK);
    let updated: Subtask = res.parse();
    assert_eq!(updated.title, "unchanged");
    assert_eq!(updated.status, TaskStatus::Open);
    assert_eq!(updated.id, subtask.id);
}

#[sqlx::test]
async fn patch_blank_title_is_400(pool: PgPool) {
    let app = app(pool);
    let account = register(&app, "ada", "ada@example.com", "hunter2-long").await;
    let task = create_task(&app, &account, "parent").await;
    let subtask = create_subtask(&app, &account, &task.id, "valid").await;

    let path = format!(
        "/api/profiles/{}/tasks/{}/subtasks/{}",
        account.profile_id, task.id, subtask.id
    );
    let res = send(
        &app,
        patch_json_auth(&path, &account.token, &json!({ "title": "   " })),
    )
    .await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST);
    res.expect_error(ErrorCode::ValidationFailed);
}

#[sqlx::test]
async fn patch_nonexistent_subtask_is_404(pool: PgPool) {
    let app = app(pool);
    let account = register(&app, "ada", "ada@example.com", "hunter2-long").await;
    let task = create_task(&app, &account, "parent").await;

    let path = format!(
        "/api/profiles/{}/tasks/{}/subtasks/{}",
        account.profile_id, task.id, MISSING_ID
    );
    let res = send(
        &app,
        patch_json_auth(&path, &account.token, &json!({ "status": "done" })),
    )
    .await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);
    res.expect_error(ErrorCode::NotFound);
}

#[sqlx::test]
async fn patch_requires_auth(pool: PgPool) {
    let app = app(pool);
    let account = register(&app, "ada", "ada@example.com", "hunter2-long").await;
    let task = create_task(&app, &account, "parent").await;
    let subtask = create_subtask(&app, &account, &task.id, "guarded").await;
    let path = format!(
        "/api/profiles/{}/tasks/{}/subtasks/{}",
        account.profile_id, task.id, subtask.id
    );

    let res = send(&app, patch_json(&path, &json!({ "status": "done" }))).await;
    assert_eq!(res.status, StatusCode::UNAUTHORIZED);
}

// --- delete ---

#[sqlx::test]
async fn delete_subtask_then_second_delete_is_404(pool: PgPool) {
    let app = app(pool);
    let account = register(&app, "ada", "ada@example.com", "hunter2-long").await;
    let task = create_task(&app, &account, "parent").await;
    let subtask = create_subtask(&app, &account, &task.id, "doomed").await;
    let path = format!(
        "/api/profiles/{}/tasks/{}/subtasks/{}",
        account.profile_id, task.id, subtask.id
    );

    let first = send(&app, delete_auth(&path, &account.token)).await;
    assert_eq!(first.status, StatusCode::NO_CONTENT);

    // It is gone from the per-task list.
    let list = send(
        &app,
        get_auth(
            &format!(
                "/api/profiles/{}/tasks/{}/subtasks",
                account.profile_id, task.id
            ),
            &account.token,
        ),
    )
    .await;
    assert!(list.parse::<Vec<Subtask>>().is_empty(), "sub-task removed");

    // A second delete of the same id → 404.
    let second = send(&app, delete_auth(&path, &account.token)).await;
    assert_eq!(second.status, StatusCode::NOT_FOUND);
    second.expect_error(ErrorCode::NotFound);
}

// --- parent-scoping: a wrong {tid} cannot reach a sub-task ---

#[sqlx::test]
async fn patch_subtask_under_wrong_parent_is_404(pool: PgPool) {
    // The sub-task exists, but addressed via a DIFFERENT (sibling) parent task in the SAME profile
    // → 404. Proves the query is joined on `task_id`, not merely sub-task existence (parent-scope).
    let app = app(pool);
    let account = register(&app, "ada", "ada@example.com", "hunter2-long").await;
    let parent = create_task(&app, &account, "real parent").await;
    let other = create_task(&app, &account, "sibling").await;
    let subtask = create_subtask(&app, &account, &parent.id, "child").await;

    // Address the real sub-task under the WRONG parent task id.
    let wrong = format!(
        "/api/profiles/{}/tasks/{}/subtasks/{}",
        account.profile_id, other.id, subtask.id
    );
    let res = send(
        &app,
        patch_json_auth(&wrong, &account.token, &json!({ "status": "done" })),
    )
    .await;
    assert_eq!(res.status, StatusCode::NOT_FOUND, "wrong parent → 404");
    res.expect_error(ErrorCode::NotFound);

    // And it is untouched under its REAL parent.
    let real = format!(
        "/api/profiles/{}/tasks/{}/subtasks",
        account.profile_id, parent.id
    );
    let still = send(&app, get_auth(&real, &account.token)).await;
    let subtasks: Vec<Subtask> = still.parse();
    assert_eq!(subtasks.len(), 1);
    assert_eq!(
        subtasks.first().unwrap().status,
        TaskStatus::Open,
        "the cross-parent patch did not land",
    );
}

#[sqlx::test]
async fn delete_subtask_under_wrong_parent_is_404(pool: PgPool) {
    let app = app(pool);
    let account = register(&app, "ada", "ada@example.com", "hunter2-long").await;
    let parent = create_task(&app, &account, "real parent").await;
    let other = create_task(&app, &account, "sibling").await;
    let subtask = create_subtask(&app, &account, &parent.id, "child").await;

    let wrong = format!(
        "/api/profiles/{}/tasks/{}/subtasks/{}",
        account.profile_id, other.id, subtask.id
    );
    let res = send(&app, delete_auth(&wrong, &account.token)).await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);
    res.expect_error(ErrorCode::NotFound);

    // It survives under its real parent — the cross-parent delete did not land.
    let real = format!(
        "/api/profiles/{}/tasks/{}/subtasks",
        account.profile_id, parent.id
    );
    let still = send(&app, get_auth(&real, &account.token)).await;
    assert_eq!(still.parse::<Vec<Subtask>>().len(), 1);
}

// --- profile-scoping (#4): another profile's task cannot reach a sub-task ---

#[sqlx::test]
async fn create_subtask_under_another_profiles_task_is_404(pool: PgPool) {
    // bob's parent task addressed via alice's profile path → 404 (no cross-profile write).
    let app = app(pool);
    let alice = register(&app, "alice", "alice@example.com", "hunter2-long").await;
    let bob = register(&app, "bob", "bob@example.com", "hunter2-long").await;
    let bobs_task = create_task(&app, &bob, "bob's task").await;

    // alice tries to add a sub-task under bob's task, addressed through HER profile → 404.
    let path = format!(
        "/api/profiles/{}/tasks/{}/subtasks",
        alice.profile_id, bobs_task.id
    );
    let res = send(
        &app,
        post_json_auth(&path, &alice.token, &json!({ "title": "intrusion" })),
    )
    .await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);
    res.expect_error(ErrorCode::NotFound);
}

#[sqlx::test]
async fn list_subtasks_under_another_users_profile_is_404(pool: PgPool) {
    // bob listing the sub-tasks of alice's task through alice's profile → 404 at the ownership gate.
    let app = app(pool);
    let alice = register(&app, "alice", "alice@example.com", "hunter2-long").await;
    let bob = register(&app, "bob", "bob@example.com", "hunter2-long").await;
    let alices_task = create_task(&app, &alice, "alice's task").await;
    let _ = create_subtask(&app, &alice, &alices_task.id, "hers").await;

    let path = format!(
        "/api/profiles/{}/tasks/{}/subtasks",
        alice.profile_id, alices_task.id
    );
    let res = send(&app, get_auth(&path, &bob.token)).await;
    assert_eq!(res.status, StatusCode::NOT_FOUND, "unowned profile → 404");
    res.expect_error(ErrorCode::NotFound);
}

#[sqlx::test]
async fn patch_subtask_in_another_users_profile_is_404(pool: PgPool) {
    // A real sub-task under alice's profile, addressed by bob (knowing all the ids) → 404,
    // indistinguishable from absent; and the cross-profile write must not land.
    let app = app(pool);
    let alice = register(&app, "alice", "alice@example.com", "hunter2-long").await;
    let bob = register(&app, "bob", "bob@example.com", "hunter2-long").await;
    let alices_task = create_task(&app, &alice, "alice's task").await;
    let subtask = create_subtask(&app, &alice, &alices_task.id, "hers").await;

    let path = format!(
        "/api/profiles/{}/tasks/{}/subtasks/{}",
        alice.profile_id, alices_task.id, subtask.id
    );
    let res = send(
        &app,
        patch_json_auth(&path, &bob.token, &json!({ "status": "done" })),
    )
    .await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);
    res.expect_error(ErrorCode::NotFound);

    // Still open for alice — bob's attempt had no effect (re-read via the per-task list; there is
    // no single-sub-task GET route — list, patch, and delete are the surface).
    let list_path = format!(
        "/api/profiles/{}/tasks/{}/subtasks",
        alice.profile_id, alices_task.id
    );
    let still = send(&app, get_auth(&list_path, &alice.token)).await;
    let subtasks: Vec<Subtask> = still.parse();
    assert_eq!(subtasks.len(), 1);
    assert_eq!(
        subtasks.first().unwrap().status,
        TaskStatus::Open,
        "untouched",
    );
}

// --- cascade (R4): task-delete and profile-delete remove sub-tasks, no orphans ---

#[sqlx::test]
async fn deleting_a_task_cascades_its_subtasks(pool: PgPool) {
    let app = app(pool);
    let account = register(&app, "ada", "ada@example.com", "hunter2-long").await;
    let task = create_task(&app, &account, "parent").await;
    let _ = create_subtask(&app, &account, &task.id, "c1").await;
    let _ = create_subtask(&app, &account, &task.id, "c2").await;

    // Precondition: the profile-wide list sees both sub-tasks.
    let all_path = format!("/api/profiles/{}/subtasks", account.profile_id);
    let before = send(&app, get_auth(&all_path, &account.token)).await;
    assert_eq!(before.parse::<Vec<Subtask>>().len(), 2);

    // Delete the parent task (FK ON DELETE CASCADE removes its sub-tasks — no handler code).
    let task_path = format!("/api/profiles/{}/tasks/{}", account.profile_id, task.id);
    let del = send(&app, delete_auth(&task_path, &account.token)).await;
    assert_eq!(del.status, StatusCode::NO_CONTENT);

    // No orphans remain: the profile-wide sub-task list is now empty.
    let after = send(&app, get_auth(&all_path, &account.token)).await;
    assert_eq!(after.status, StatusCode::OK);
    assert!(
        after.parse::<Vec<Subtask>>().is_empty(),
        "the parent's sub-tasks cascaded away with the task (R4)",
    );
}

#[sqlx::test]
async fn deleting_a_profile_cascades_its_tasks_and_subtasks(pool: PgPool) {
    // A profile delete cascades its tasks (existing FK), which transitively cascades their
    // sub-tasks (the subtasks→tasks FK) — no orphaned sub-task can survive (ADR-0012 §4).
    let app = app(pool);
    let account = register(&app, "ada", "ada@example.com", "hunter2-long").await;
    // A second profile so the deletion is not blocked by `last_profile`.
    let extra = second_profile(&app, &account).await;

    // Put a task + sub-tasks in the `extra` profile, then delete that profile.
    let task = {
        let path = format!("/api/profiles/{extra}/tasks");
        let res = send(
            &app,
            post_json_auth(
                &path,
                &account.token,
                &json!({ "title": "doomed", "description": "" }),
            ),
        )
        .await;
        assert_eq!(res.status, StatusCode::CREATED);
        res.parse::<Task>()
    };
    let sub_path = format!("/api/profiles/{extra}/tasks/{}/subtasks", task.id);
    let made = send(
        &app,
        post_json_auth(&sub_path, &account.token, &json!({ "title": "child" })),
    )
    .await;
    assert_eq!(made.status, StatusCode::CREATED);
    let subtask: Subtask = made.parse();

    // Delete the `extra` profile (204): cascades its tasks → their sub-tasks.
    let prof_path = format!("/api/profiles/{extra}");
    let del = send(&app, delete_auth(&prof_path, &account.token)).await;
    assert_eq!(
        del.status,
        StatusCode::NO_CONTENT,
        "profile deleted: {:?}",
        del.body
    );

    // The profile (and therefore its task + sub-task) is gone: the now-unowned profile path → 404.
    let gone = send(&app, get_auth(&sub_path, &account.token)).await;
    assert_eq!(
        gone.status,
        StatusCode::NOT_FOUND,
        "the deleted profile's sub-task list is unreachable (cascaded away)",
    );
    // And the sub-task id cannot be reached under the surviving (default) profile either — it was
    // never there, and the cascade left no orphan addressable anywhere.
    let cross = format!(
        "/api/profiles/{}/tasks/{}/subtasks/{}",
        account.profile_id, task.id, subtask.id
    );
    let orphan = send(
        &app,
        patch_json_auth(&cross, &account.token, &json!({ "status": "done" })),
    )
    .await;
    assert_eq!(
        orphan.status,
        StatusCode::NOT_FOUND,
        "no orphaned sub-task survives the profile cascade (R4)",
    );
}
