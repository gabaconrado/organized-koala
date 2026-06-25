//! Integration tests for profile mutation (ADR-0009): create (201 / duplicate→409
//! `profile_name_taken` / empty-after-trim→400 `validation_failed`), rename (200 / duplicate→409
//! / unowned-or-missing→404), delete (204 / last-remaining→409 `last_profile` / unowned-or-
//! missing→404), the headline delete-cascade (a profile's tasks AND notes vanish with it, #4),
//! per-account name uniqueness (the same name is fine across accounts), and auth on every route
//! — all asserted as real HTTP round-trips against the `axum` app over a per-test database.

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
    Account, app, delete, delete_auth, get_auth, patch_json, patch_json_auth, post_json,
    post_json_auth, register, send,
};
use contract::{ErrorCode, Note, Profile, Task};
use serde_json::json;
use sqlx::PgPool;

/// A syntactically-valid UUID that does not name any row.
const MISSING_ID: &str = "00000000-0000-0000-0000-000000000000";

/// Create a profile named `name` for `account` and return the parsed [`Profile`], asserting 201.
async fn create_profile(app: &axum::Router, account: &Account, name: &str) -> Profile {
    let res = send(
        app,
        post_json_auth("/api/profiles", &account.token, &json!({ "name": name })),
    )
    .await;
    assert_eq!(res.status, StatusCode::CREATED, "create: {:?}", res.body);
    res.parse()
}

/// List the account's profiles (200), returning the parsed vec.
async fn list_profiles(app: &axum::Router, account: &Account) -> Vec<Profile> {
    let res = send(app, get_auth("/api/profiles", &account.token)).await;
    assert_eq!(res.status, StatusCode::OK, "list: {:?}", res.body);
    res.parse()
}

// ---- create ----------------------------------------------------------------

/// create a profile → 201 with the created profile, a fresh id, and the trimmed name.
#[sqlx::test]
async fn create_profile_returns_201(pool: PgPool) {
    let app = app(pool);
    let account = register(&app, "ada", "ada@example.com", "hunter2-long").await;

    let res = send(
        &app,
        post_json_auth(
            "/api/profiles",
            &account.token,
            &json!({ "name": "personal" }),
        ),
    )
    .await;
    assert_eq!(res.status, StatusCode::CREATED);
    let profile: Profile = res.parse();
    assert_eq!(profile.name, "personal");
    assert!(!profile.id.is_empty());
    assert_ne!(profile.id, account.profile_id);
}

/// create stores the trimmed name (leading/trailing whitespace removed).
#[sqlx::test]
async fn create_profile_trims_name(pool: PgPool) {
    let app = app(pool);
    let account = register(&app, "ada", "ada@example.com", "hunter2-long").await;

    let res = send(
        &app,
        post_json_auth(
            "/api/profiles",
            &account.token,
            &json!({ "name": "  spaced  " }),
        ),
    )
    .await;
    assert_eq!(res.status, StatusCode::CREATED);
    let profile: Profile = res.parse();
    assert_eq!(profile.name, "spaced");
}

/// create with a name the account already uses → 409 profile_name_taken.
#[sqlx::test]
async fn create_profile_duplicate_name_is_409(pool: PgPool) {
    let app = app(pool);
    let account = register(&app, "ada", "ada@example.com", "hunter2-long").await;
    // The default profile is "work"; a second "work" collides per-account.
    let res = send(
        &app,
        post_json_auth("/api/profiles", &account.token, &json!({ "name": "work" })),
    )
    .await;
    assert_eq!(res.status, StatusCode::CONFLICT);
    res.expect_error(ErrorCode::ProfileNameTaken);
}

/// create with an empty-after-trim name → 400 validation_failed.
#[sqlx::test]
async fn create_profile_blank_name_is_400(pool: PgPool) {
    let app = app(pool);
    let account = register(&app, "ada", "ada@example.com", "hunter2-long").await;
    let res = send(
        &app,
        post_json_auth("/api/profiles", &account.token, &json!({ "name": "   " })),
    )
    .await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST);
    res.expect_error(ErrorCode::ValidationFailed);
}

/// create without a token → 401 unauthenticated.
#[sqlx::test]
async fn create_profile_requires_auth(pool: PgPool) {
    let app = app(pool);
    let res = send(
        &app,
        post_json("/api/profiles", &json!({ "name": "personal" })),
    )
    .await;
    assert_eq!(res.status, StatusCode::UNAUTHORIZED);
}

// ---- rename -----------------------------------------------------------------

/// rename a profile → 200 with the updated name, id and created_at preserved.
#[sqlx::test]
async fn rename_profile_returns_200(pool: PgPool) {
    let app = app(pool);
    let account = register(&app, "ada", "ada@example.com", "hunter2-long").await;
    let created = create_profile(&app, &account, "personal").await;

    let path = format!("/api/profiles/{}", created.id);
    let res = send(
        &app,
        patch_json_auth(&path, &account.token, &json!({ "name": "leisure" })),
    )
    .await;
    assert_eq!(res.status, StatusCode::OK);
    let renamed: Profile = res.parse();
    assert_eq!(renamed.name, "leisure");
    assert_eq!(renamed.id, created.id);
    assert_eq!(renamed.created_at, created.created_at);
}

/// rename trims the new name.
#[sqlx::test]
async fn rename_profile_trims_name(pool: PgPool) {
    let app = app(pool);
    let account = register(&app, "ada", "ada@example.com", "hunter2-long").await;
    let created = create_profile(&app, &account, "personal").await;

    let path = format!("/api/profiles/{}", created.id);
    let res = send(
        &app,
        patch_json_auth(&path, &account.token, &json!({ "name": "  leisure  " })),
    )
    .await;
    assert_eq!(res.status, StatusCode::OK);
    let renamed: Profile = res.parse();
    assert_eq!(renamed.name, "leisure");
}

/// rename to a name the account already uses → 409 profile_name_taken.
#[sqlx::test]
async fn rename_profile_duplicate_name_is_409(pool: PgPool) {
    let app = app(pool);
    let account = register(&app, "ada", "ada@example.com", "hunter2-long").await;
    // Default "work" exists; create "personal" then try to rename it back to "work".
    let created = create_profile(&app, &account, "personal").await;

    let path = format!("/api/profiles/{}", created.id);
    let res = send(
        &app,
        patch_json_auth(&path, &account.token, &json!({ "name": "work" })),
    )
    .await;
    assert_eq!(res.status, StatusCode::CONFLICT);
    res.expect_error(ErrorCode::ProfileNameTaken);
}

/// rename with an empty-after-trim name → 400 validation_failed.
#[sqlx::test]
async fn rename_profile_blank_name_is_400(pool: PgPool) {
    let app = app(pool);
    let account = register(&app, "ada", "ada@example.com", "hunter2-long").await;
    let created = create_profile(&app, &account, "personal").await;

    let path = format!("/api/profiles/{}", created.id);
    let res = send(
        &app,
        patch_json_auth(&path, &account.token, &json!({ "name": "   " })),
    )
    .await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST);
    res.expect_error(ErrorCode::ValidationFailed);
}

/// rename a missing profile → 404 not_found.
#[sqlx::test]
async fn rename_missing_profile_is_404(pool: PgPool) {
    let app = app(pool);
    let account = register(&app, "ada", "ada@example.com", "hunter2-long").await;
    let path = format!("/api/profiles/{MISSING_ID}");
    let res = send(
        &app,
        patch_json_auth(&path, &account.token, &json!({ "name": "x" })),
    )
    .await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);
    res.expect_error(ErrorCode::NotFound);
}

/// rename another account's profile → 404 not_found (ownership-scoped, never 403).
#[sqlx::test]
async fn rename_unowned_profile_is_404(pool: PgPool) {
    let app = app(pool);
    let owner = register(&app, "ada", "ada@example.com", "hunter2-long").await;
    let other = register(&app, "bob", "bob@example.com", "hunter2-long").await;

    let path = format!("/api/profiles/{}", owner.profile_id);
    let res = send(
        &app,
        patch_json_auth(&path, &other.token, &json!({ "name": "hijack" })),
    )
    .await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);
    res.expect_error(ErrorCode::NotFound);
}

/// rename without a token → 401 unauthenticated.
#[sqlx::test]
async fn rename_profile_requires_auth(pool: PgPool) {
    let app = app(pool);
    let account = register(&app, "ada", "ada@example.com", "hunter2-long").await;
    let path = format!("/api/profiles/{}", account.profile_id);
    let res = send(&app, patch_json(&path, &json!({ "name": "x" }))).await;
    assert_eq!(res.status, StatusCode::UNAUTHORIZED);
}

// ---- delete -----------------------------------------------------------------

/// delete a non-last profile → 204, and it is gone from the list.
#[sqlx::test]
async fn delete_profile_returns_204(pool: PgPool) {
    let app = app(pool);
    let account = register(&app, "ada", "ada@example.com", "hunter2-long").await;
    let created = create_profile(&app, &account, "personal").await;

    let path = format!("/api/profiles/{}", created.id);
    let res = send(&app, delete_auth(&path, &account.token)).await;
    assert_eq!(res.status, StatusCode::NO_CONTENT);

    let remaining = list_profiles(&app, &account).await;
    assert!(
        remaining.iter().all(|p| p.id != created.id),
        "deleted profile must be gone from the list"
    );
    // The default profile still stands.
    assert!(remaining.iter().any(|p| p.id == account.profile_id));
}

/// delete the account's only remaining profile → 409 last_profile (the account keeps ≥1).
#[sqlx::test]
async fn delete_last_profile_is_409(pool: PgPool) {
    let app = app(pool);
    let account = register(&app, "ada", "ada@example.com", "hunter2-long").await;
    // The default profile is the only one.
    let path = format!("/api/profiles/{}", account.profile_id);
    let res = send(&app, delete_auth(&path, &account.token)).await;
    assert_eq!(res.status, StatusCode::CONFLICT);
    res.expect_error(ErrorCode::LastProfile);

    // It was NOT deleted — still reachable.
    let remaining = list_profiles(&app, &account).await;
    assert!(remaining.iter().any(|p| p.id == account.profile_id));
}

/// delete a missing profile → 404 not_found.
#[sqlx::test]
async fn delete_missing_profile_is_404(pool: PgPool) {
    let app = app(pool);
    let account = register(&app, "ada", "ada@example.com", "hunter2-long").await;
    // A second profile exists so the guard doesn't short-circuit on last-profile.
    let _ = create_profile(&app, &account, "personal").await;

    let path = format!("/api/profiles/{MISSING_ID}");
    let res = send(&app, delete_auth(&path, &account.token)).await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);
    res.expect_error(ErrorCode::NotFound);
}

/// delete another account's profile → 404 not_found (ownership-scoped).
#[sqlx::test]
async fn delete_unowned_profile_is_404(pool: PgPool) {
    let app = app(pool);
    let owner = register(&app, "ada", "ada@example.com", "hunter2-long").await;
    let other = register(&app, "bob", "bob@example.com", "hunter2-long").await;
    // `other` has a second profile, so its own guard wouldn't trip; the target is unowned.
    let _ = create_profile(&app, &other, "extra").await;

    let path = format!("/api/profiles/{}", owner.profile_id);
    let res = send(&app, delete_auth(&path, &other.token)).await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);
    res.expect_error(ErrorCode::NotFound);

    // The owner's profile is untouched.
    let remaining = list_profiles(&app, &owner).await;
    assert!(remaining.iter().any(|p| p.id == owner.profile_id));
}

/// delete without a token → 401 unauthenticated.
#[sqlx::test]
async fn delete_profile_requires_auth(pool: PgPool) {
    let app = app(pool);
    let account = register(&app, "ada", "ada@example.com", "hunter2-long").await;
    let path = format!("/api/profiles/{}", account.profile_id);
    let res = send(&app, delete(&path)).await;
    assert_eq!(res.status, StatusCode::UNAUTHORIZED);
}

// ---- delete-cascade (the headline test, #4) ---------------------------------

/// Deleting a profile cascades BOTH its tasks and its notes (a profile is a namespace, #4).
/// After the delete, neither the profile, nor the task, nor the note under it is reachable.
#[sqlx::test]
async fn delete_profile_cascades_tasks_and_notes(pool: PgPool) {
    // Keep a pool handle for a direct row-count after the delete: the definitive cascade proof is
    // that NO `tasks` and NO `notes` rows remain for the deleted profile_id (not merely that the
    // namespace is unreachable over HTTP).
    let app = app(pool.clone());
    let account = register(&app, "ada", "ada@example.com", "hunter2-long").await;
    // A second profile so this one is not the account's last (the guard would otherwise refuse).
    let target = create_profile(&app, &account, "scratch").await;

    // Add a task AND a note under the target profile.
    let task_res = send(
        &app,
        post_json_auth(
            &format!("/api/profiles/{}/tasks", target.id),
            &account.token,
            &json!({ "title": "doomed task", "description": "" }),
        ),
    )
    .await;
    assert_eq!(task_res.status, StatusCode::CREATED);
    let task: Task = task_res.parse();

    let note_res = send(
        &app,
        post_json_auth(
            &format!("/api/profiles/{}/notes", target.id),
            &account.token,
            &json!({ "title": "doomed note", "content": "bye" }),
        ),
    )
    .await;
    assert_eq!(note_res.status, StatusCode::CREATED);
    let note: Note = note_res.parse();

    // Both children are reachable before the delete.
    let tasks_before = send(
        &app,
        get_auth(
            &format!("/api/profiles/{}/tasks", target.id),
            &account.token,
        ),
    )
    .await;
    assert_eq!(tasks_before.status, StatusCode::OK);
    assert_eq!(tasks_before.parse::<Vec<Task>>().len(), 1);
    let notes_before = send(
        &app,
        get_auth(
            &format!("/api/profiles/{}/notes", target.id),
            &account.token,
        ),
    )
    .await;
    assert_eq!(notes_before.status, StatusCode::OK);
    assert_eq!(notes_before.parse::<Vec<Note>>().len(), 1);

    // Delete the profile.
    let del = send(
        &app,
        delete_auth(&format!("/api/profiles/{}", target.id), &account.token),
    )
    .await;
    assert_eq!(del.status, StatusCode::NO_CONTENT);

    // The profile is gone from the account's list.
    let remaining = list_profiles(&app, &account).await;
    assert!(
        remaining.iter().all(|p| p.id != target.id),
        "deleted profile must be gone"
    );

    // Definitive cascade proof: query the DB directly — BOTH the task row AND the note row that
    // hung off the deleted profile are gone (#4). Runtime (non-macro) queries are used so no
    // `.sqlx/` offline-cache entry is needed for this test-only assertion.
    let profile_uuid = uuid::Uuid::parse_str(&target.id).expect("target id is a uuid");
    let task_count: i64 = sqlx::query_scalar("SELECT count(*) FROM tasks WHERE profile_id = $1")
        .bind(profile_uuid)
        .fetch_one(&pool)
        .await
        .expect("count tasks");
    assert_eq!(task_count, 0, "the profile's task cascaded away");
    let note_count: i64 = sqlx::query_scalar("SELECT count(*) FROM notes WHERE profile_id = $1")
        .bind(profile_uuid)
        .fetch_one(&pool)
        .await
        .expect("count notes");
    assert_eq!(note_count, 0, "the profile's note cascaded away");

    // And both are unreachable over HTTP too: the namespace is gone, so its task list and the note
    // (by id and via the list) all 404. (`task`/`note` ids are captured above for these reads.)
    let tasks_after = send(
        &app,
        get_auth(
            &format!("/api/profiles/{}/tasks", target.id),
            &account.token,
        ),
    )
    .await;
    assert_eq!(
        tasks_after.status,
        StatusCode::NOT_FOUND,
        "cascaded task must be unreachable via the list: {:?}",
        tasks_after.body
    );
    let _ = &task; // task id captured above; reachability asserted via the (now-404) task list.
    let note_get = send(
        &app,
        get_auth(
            &format!("/api/profiles/{}/notes/{}", target.id, note.id),
            &account.token,
        ),
    )
    .await;
    assert_eq!(
        note_get.status,
        StatusCode::NOT_FOUND,
        "cascaded note must be gone: {:?}",
        note_get.body
    );
}

// ---- per-account name uniqueness --------------------------------------------

/// The same profile name is allowed for two different accounts — uniqueness is per-account.
#[sqlx::test]
async fn same_name_allowed_across_accounts(pool: PgPool) {
    let app = app(pool);
    let ada = register(&app, "ada", "ada@example.com", "hunter2-long").await;
    let bob = register(&app, "bob", "bob@example.com", "hunter2-long").await;

    // Both accounts already have a "work" default; each can create a "personal".
    let ada_p = create_profile(&app, &ada, "personal").await;
    let bob_p = create_profile(&app, &bob, "personal").await;

    assert_eq!(ada_p.name, "personal");
    assert_eq!(bob_p.name, "personal");
    // Distinct rows despite the identical name.
    assert_ne!(ada_p.id, bob_p.id);
}

/// A name freed by deleting a profile becomes available to create again (uniqueness is on the
/// live set, not historical). Sanity-locks that the unique constraint is per-row, not a tombstone.
#[sqlx::test]
async fn name_reusable_after_delete(pool: PgPool) {
    let app = app(pool);
    let account = register(&app, "ada", "ada@example.com", "hunter2-long").await;
    let created = create_profile(&app, &account, "personal").await;

    let del = send(
        &app,
        delete_auth(&format!("/api/profiles/{}", created.id), &account.token),
    )
    .await;
    assert_eq!(del.status, StatusCode::NO_CONTENT);

    // "personal" is free again.
    let again = create_profile(&app, &account, "personal").await;
    assert_eq!(again.name, "personal");
}
