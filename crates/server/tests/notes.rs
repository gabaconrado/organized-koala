//! Integration tests for the notes surface (ADR-0007): create (201), list (200 bare array
//! newest-first), get-one / update / delete with 404-for-unowned-or-missing, title validation,
//! the flat no-`updated_at` shape, profile-scoping (#4), and auth on every route — all asserted
//! as real HTTP round-trips against the `axum` app over a per-test database.

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
    Account, app, delete, delete_auth, get, get_auth, patch_json, patch_json_auth, post_json,
    post_json_auth, register, send,
};
use contract::{ErrorCode, Note};
use serde_json::{Value, json};
use sqlx::PgPool;

/// A syntactically-valid UUID that does not name any row.
const MISSING_ID: &str = "00000000-0000-0000-0000-000000000000";

/// Create a note under `profile_id` and return the parsed [`Note`], asserting the 201.
async fn create_note(account: &Account, app: &axum::Router, title: &str, content: &str) -> Note {
    let path = format!("/api/profiles/{}/notes", account.profile_id);
    let res = send(
        app,
        post_json_auth(
            &path,
            &account.token,
            &json!({ "title": title, "content": content }),
        ),
    )
    .await;
    assert_eq!(res.status, StatusCode::CREATED, "create: {:?}", res.body);
    res.parse()
}

// ---- create ----------------------------------------------------------------

/// create a note → 201 with the full flat contract shape.
#[sqlx::test]
async fn create_note_returns_201(pool: PgPool) {
    let app = app(pool);
    let account = register(&app, "ada", "ada@example.com", "hunter2-long").await;
    let path = format!("/api/profiles/{}/notes", account.profile_id);

    let res = send(
        &app,
        post_json_auth(
            &path,
            &account.token,
            &json!({ "title": "Groceries", "content": "milk, eggs" }),
        ),
    )
    .await;
    assert_eq!(res.status, StatusCode::CREATED);
    let note: Note = res.parse();
    assert_eq!(note.title, "Groceries");
    assert_eq!(note.content, "milk, eggs");
    assert!(!note.id.is_empty());
}

/// create with empty content is allowed (content may be empty).
#[sqlx::test]
async fn create_note_allows_empty_content(pool: PgPool) {
    let app = app(pool);
    let account = register(&app, "ada", "ada@example.com", "hunter2-long").await;
    let path = format!("/api/profiles/{}/notes", account.profile_id);

    let res = send(
        &app,
        post_json_auth(
            &path,
            &account.token,
            &json!({ "title": "title only", "content": "" }),
        ),
    )
    .await;
    assert_eq!(res.status, StatusCode::CREATED);
    let note: Note = res.parse();
    assert_eq!(note.content, "");
}

/// the title is trimmed before storage.
#[sqlx::test]
async fn create_note_trims_title(pool: PgPool) {
    let app = app(pool);
    let account = register(&app, "ada", "ada@example.com", "hunter2-long").await;

    let note = create_note(&account, &app, "  spaced  ", "body").await;
    assert_eq!(note.title, "spaced", "title is trimmed");
}

/// an empty title → 400 `validation_failed`.
#[sqlx::test]
async fn create_note_empty_title_is_400(pool: PgPool) {
    let app = app(pool);
    let account = register(&app, "ada", "ada@example.com", "hunter2-long").await;
    let path = format!("/api/profiles/{}/notes", account.profile_id);

    let res = send(
        &app,
        post_json_auth(
            &path,
            &account.token,
            &json!({ "title": "", "content": "x" }),
        ),
    )
    .await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST);
    res.expect_error(ErrorCode::ValidationFailed);
}

/// a whitespace-only title → 400 `validation_failed`.
#[sqlx::test]
async fn create_note_blank_title_is_400(pool: PgPool) {
    let app = app(pool);
    let account = register(&app, "ada", "ada@example.com", "hunter2-long").await;
    let path = format!("/api/profiles/{}/notes", account.profile_id);

    let res = send(
        &app,
        post_json_auth(
            &path,
            &account.token,
            &json!({ "title": "   ", "content": "x" }),
        ),
    )
    .await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST);
    res.expect_error(ErrorCode::ValidationFailed);
}

// ---- list ------------------------------------------------------------------

/// list returns the profile's notes newest-first as a bare JSON array.
#[sqlx::test]
async fn list_notes_newest_first(pool: PgPool) {
    let app = app(pool);
    let account = register(&app, "ada", "ada@example.com", "hunter2-long").await;
    let path = format!("/api/profiles/{}/notes", account.profile_id);

    for title in ["first", "second", "third"] {
        let _ = create_note(&account, &app, title, "").await;
    }

    let res = send(&app, get_auth(&path, &account.token)).await;
    assert_eq!(res.status, StatusCode::OK);
    assert!(res.body.is_array(), "list is a bare JSON array");
    let notes: Vec<Note> = res.parse();
    let titles: Vec<&str> = notes.iter().map(|n| n.title.as_str()).collect();
    assert_eq!(titles, vec!["third", "second", "first"], "newest-first");
}

/// list on a brand-new profile is an empty array.
#[sqlx::test]
async fn list_notes_empty(pool: PgPool) {
    let app = app(pool);
    let account = register(&app, "ada", "ada@example.com", "hunter2-long").await;
    let path = format!("/api/profiles/{}/notes", account.profile_id);

    let res = send(&app, get_auth(&path, &account.token)).await;
    assert_eq!(res.status, StatusCode::OK);
    let notes: Vec<Note> = res.parse();
    assert!(notes.is_empty());
}

// ---- get-one ---------------------------------------------------------------

/// get-one returns the stored note → 200.
#[sqlx::test]
async fn get_note_returns_200(pool: PgPool) {
    let app = app(pool);
    let account = register(&app, "ada", "ada@example.com", "hunter2-long").await;
    let created = create_note(&account, &app, "fetch me", "the body").await;

    let path = format!("/api/profiles/{}/notes/{}", account.profile_id, created.id);
    let res = send(&app, get_auth(&path, &account.token)).await;
    assert_eq!(res.status, StatusCode::OK);
    let note: Note = res.parse();
    assert_eq!(note, created, "get-one round-trips the created note");
}

/// get-one of a nonexistent note id → 404 `not_found`.
#[sqlx::test]
async fn get_nonexistent_note_is_404(pool: PgPool) {
    let app = app(pool);
    let account = register(&app, "ada", "ada@example.com", "hunter2-long").await;

    let path = format!("/api/profiles/{}/notes/{}", account.profile_id, MISSING_ID);
    let res = send(&app, get_auth(&path, &account.token)).await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);
    res.expect_error(ErrorCode::NotFound);
}

// ---- update (PATCH) --------------------------------------------------------

/// update replaces title + content in place → 200, with created_at unchanged (no second
/// timestamp ever appears: the flat shape has no `updated_at`).
#[sqlx::test]
async fn update_note_replaces_in_place(pool: PgPool) {
    let app = app(pool);
    let account = register(&app, "ada", "ada@example.com", "hunter2-long").await;
    let created = create_note(&account, &app, "old title", "old body").await;

    let path = format!("/api/profiles/{}/notes/{}", account.profile_id, created.id);
    let res = send(
        &app,
        patch_json_auth(
            &path,
            &account.token,
            &json!({ "title": "new title", "content": "new body" }),
        ),
    )
    .await;
    assert_eq!(res.status, StatusCode::OK);
    let updated: Note = res.parse();
    assert_eq!(updated.id, created.id, "same note");
    assert_eq!(updated.title, "new title");
    assert_eq!(updated.content, "new body");
    assert_eq!(
        updated.created_at, created.created_at,
        "created_at is unchanged by an update"
    );

    // The wire body carries exactly the flat key set — no `updated_at` or other timestamp.
    let keys: Vec<&str> = res
        .body
        .as_object()
        .expect("note is a JSON object")
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(keys.len(), 4, "exactly four fields: {keys:?}");
    for key in ["id", "title", "content", "created_at"] {
        assert!(keys.contains(&key), "missing field {key}");
    }
    assert!(
        !keys.contains(&"updated_at"),
        "no second timestamp appears: {keys:?}"
    );
}

/// update also trims the new title.
#[sqlx::test]
async fn update_note_trims_title(pool: PgPool) {
    let app = app(pool);
    let account = register(&app, "ada", "ada@example.com", "hunter2-long").await;
    let created = create_note(&account, &app, "old", "body").await;

    let path = format!("/api/profiles/{}/notes/{}", account.profile_id, created.id);
    let res = send(
        &app,
        patch_json_auth(
            &path,
            &account.token,
            &json!({ "title": "  trimmed  ", "content": "x" }),
        ),
    )
    .await;
    assert_eq!(res.status, StatusCode::OK);
    let updated: Note = res.parse();
    assert_eq!(updated.title, "trimmed");
}

/// update with a blank title → 400 `validation_failed`.
#[sqlx::test]
async fn update_note_blank_title_is_400(pool: PgPool) {
    let app = app(pool);
    let account = register(&app, "ada", "ada@example.com", "hunter2-long").await;
    let created = create_note(&account, &app, "keep", "body").await;

    let path = format!("/api/profiles/{}/notes/{}", account.profile_id, created.id);
    let res = send(
        &app,
        patch_json_auth(
            &path,
            &account.token,
            &json!({ "title": "   ", "content": "x" }),
        ),
    )
    .await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST);
    res.expect_error(ErrorCode::ValidationFailed);
}

/// updating a nonexistent note id → 404 `not_found`.
#[sqlx::test]
async fn update_nonexistent_note_is_404(pool: PgPool) {
    let app = app(pool);
    let account = register(&app, "ada", "ada@example.com", "hunter2-long").await;

    let path = format!("/api/profiles/{}/notes/{}", account.profile_id, MISSING_ID);
    let res = send(
        &app,
        patch_json_auth(
            &path,
            &account.token,
            &json!({ "title": "ghost", "content": "" }),
        ),
    )
    .await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);
    res.expect_error(ErrorCode::NotFound);
}

// ---- delete ----------------------------------------------------------------

/// delete a note → 204 with an empty body; the note is then gone.
#[sqlx::test]
async fn delete_note_returns_204(pool: PgPool) {
    let app = app(pool);
    let account = register(&app, "ada", "ada@example.com", "hunter2-long").await;
    let created = create_note(&account, &app, "doomed", "body").await;

    let path = format!("/api/profiles/{}/notes/{}", account.profile_id, created.id);
    let res = send(&app, delete_auth(&path, &account.token)).await;
    assert_eq!(res.status, StatusCode::NO_CONTENT);
    assert_eq!(res.body, Value::Null, "204 carries an empty body");

    // The note is now absent: a re-fetch is 404.
    let refetch = send(&app, get_auth(&path, &account.token)).await;
    assert_eq!(refetch.status, StatusCode::NOT_FOUND);
}

/// a second delete of the same note → 404 `not_found`.
#[sqlx::test]
async fn delete_note_twice_is_404(pool: PgPool) {
    let app = app(pool);
    let account = register(&app, "ada", "ada@example.com", "hunter2-long").await;
    let created = create_note(&account, &app, "doomed", "body").await;

    let path = format!("/api/profiles/{}/notes/{}", account.profile_id, created.id);
    let first = send(&app, delete_auth(&path, &account.token)).await;
    assert_eq!(first.status, StatusCode::NO_CONTENT);

    let second = send(&app, delete_auth(&path, &account.token)).await;
    assert_eq!(second.status, StatusCode::NOT_FOUND);
    second.expect_error(ErrorCode::NotFound);
}

/// deleting a nonexistent note id → 404 `not_found`.
#[sqlx::test]
async fn delete_nonexistent_note_is_404(pool: PgPool) {
    let app = app(pool);
    let account = register(&app, "ada", "ada@example.com", "hunter2-long").await;

    let path = format!("/api/profiles/{}/notes/{}", account.profile_id, MISSING_ID);
    let res = send(&app, delete_auth(&path, &account.token)).await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);
    res.expect_error(ErrorCode::NotFound);
}

// ---- profile-scoping (#4) --------------------------------------------------

/// Register two accounts: (owner of the note, attacker).
async fn two_accounts(app: &axum::Router) -> (Account, Account) {
    let alice = register(app, "alice", "alice@example.com", "hunter2-long").await;
    let bob = register(app, "bob", "bob@example.com", "hunter2-long").await;
    (alice, bob)
}

/// a note created under profile A is invisible when listing under profile B.
#[sqlx::test]
async fn note_is_invisible_across_profiles_in_list(pool: PgPool) {
    let app = app(pool);
    let (alice, bob) = two_accounts(&app).await;
    let _ = create_note(&alice, &app, "alice's note", "secret").await;

    let bob_path = format!("/api/profiles/{}/notes", bob.profile_id);
    let res = send(&app, get_auth(&bob_path, &bob.token)).await;
    assert_eq!(res.status, StatusCode::OK);
    let notes: Vec<Note> = res.parse();
    assert!(notes.is_empty(), "bob never sees alice's note");
}

/// bob getting alice's note id under his own profile → 404 (note id is real; scoping hides it).
#[sqlx::test]
async fn get_other_profiles_note_is_404(pool: PgPool) {
    let app = app(pool);
    let (alice, bob) = two_accounts(&app).await;
    let note = create_note(&alice, &app, "alice's note", "secret").await;

    // Bob, knowing the real note id, looks it up under his own profile → 404.
    let bob_path = format!("/api/profiles/{}/notes/{}", bob.profile_id, note.id);
    let res = send(&app, get_auth(&bob_path, &bob.token)).await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);
    res.expect_error(ErrorCode::NotFound);
}

/// bob updating alice's note id under his own profile → 404, and alice's note is untouched.
#[sqlx::test]
async fn update_other_profiles_note_is_404(pool: PgPool) {
    let app = app(pool);
    let (alice, bob) = two_accounts(&app).await;
    let note = create_note(&alice, &app, "alice's note", "secret").await;

    let bob_path = format!("/api/profiles/{}/notes/{}", bob.profile_id, note.id);
    let res = send(
        &app,
        patch_json_auth(
            &bob_path,
            &bob.token,
            &json!({ "title": "hijacked", "content": "tampered" }),
        ),
    )
    .await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);
    res.expect_error(ErrorCode::NotFound);

    // Alice's note is unchanged.
    let alice_path = format!("/api/profiles/{}/notes/{}", alice.profile_id, note.id);
    let refetch = send(&app, get_auth(&alice_path, &alice.token)).await;
    let still: Note = refetch.parse();
    assert_eq!(still, note, "the cross-profile update did not land");
}

/// bob deleting alice's note id under his own profile → 404, and alice's note survives.
#[sqlx::test]
async fn delete_other_profiles_note_is_404(pool: PgPool) {
    let app = app(pool);
    let (alice, bob) = two_accounts(&app).await;
    let note = create_note(&alice, &app, "alice's note", "secret").await;

    let bob_path = format!("/api/profiles/{}/notes/{}", bob.profile_id, note.id);
    let res = send(&app, delete_auth(&bob_path, &bob.token)).await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);
    res.expect_error(ErrorCode::NotFound);

    // Alice's note still exists.
    let alice_path = format!("/api/profiles/{}/notes/{}", alice.profile_id, note.id);
    let refetch = send(&app, get_auth(&alice_path, &alice.token)).await;
    assert_eq!(refetch.status, StatusCode::OK, "alice's note survives");
}

/// listing notes under another user's profile → 404 (the profile gate, not just note existence).
#[sqlx::test]
async fn list_other_users_profile_notes_is_404(pool: PgPool) {
    let app = app(pool);
    let (alice, bob) = two_accounts(&app).await;

    let path = format!("/api/profiles/{}/notes", alice.profile_id);
    let res = send(&app, get_auth(&path, &bob.token)).await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);
    res.expect_error(ErrorCode::NotFound);
}

/// creating a note in another user's profile → 404, and the write does not land.
#[sqlx::test]
async fn create_in_other_users_profile_is_404(pool: PgPool) {
    let app = app(pool);
    let (alice, bob) = two_accounts(&app).await;

    let path = format!("/api/profiles/{}/notes", alice.profile_id);
    let res = send(
        &app,
        post_json_auth(
            &path,
            &bob.token,
            &json!({ "title": "intrusion", "content": "" }),
        ),
    )
    .await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);
    res.expect_error(ErrorCode::NotFound);

    // Alice's profile is still empty.
    let alice_list = send(
        &app,
        get_auth(
            &format!("/api/profiles/{}/notes", alice.profile_id),
            &alice.token,
        ),
    )
    .await;
    let notes: Vec<Note> = alice_list.parse();
    assert!(notes.is_empty(), "the cross-profile write must not land");
}

// ---- auth required on every route ------------------------------------------

/// listing notes with no token → 401 `unauthenticated`.
#[sqlx::test]
async fn list_notes_without_token_is_401(pool: PgPool) {
    let app = app(pool);
    let account = register(&app, "ada", "ada@example.com", "hunter2-long").await;

    let path = format!("/api/profiles/{}/notes", account.profile_id);
    let res = send(&app, get(&path)).await;
    assert_eq!(res.status, StatusCode::UNAUTHORIZED);
    res.expect_error(ErrorCode::Unauthenticated);
}

/// creating a note with no token → 401 `unauthenticated`.
#[sqlx::test]
async fn create_note_without_token_is_401(pool: PgPool) {
    let app = app(pool);
    let account = register(&app, "ada", "ada@example.com", "hunter2-long").await;

    let path = format!("/api/profiles/{}/notes", account.profile_id);
    let res = send(
        &app,
        post_json(&path, &json!({ "title": "x", "content": "" })),
    )
    .await;
    assert_eq!(res.status, StatusCode::UNAUTHORIZED);
    res.expect_error(ErrorCode::Unauthenticated);
}

/// get-one with no token → 401 `unauthenticated`.
#[sqlx::test]
async fn get_note_without_token_is_401(pool: PgPool) {
    let app = app(pool);
    let account = register(&app, "ada", "ada@example.com", "hunter2-long").await;

    let path = format!("/api/profiles/{}/notes/{}", account.profile_id, MISSING_ID);
    let res = send(&app, get(&path)).await;
    assert_eq!(res.status, StatusCode::UNAUTHORIZED);
    res.expect_error(ErrorCode::Unauthenticated);
}

/// updating with no token → 401 `unauthenticated`.
#[sqlx::test]
async fn update_note_without_token_is_401(pool: PgPool) {
    let app = app(pool);
    let account = register(&app, "ada", "ada@example.com", "hunter2-long").await;

    let path = format!("/api/profiles/{}/notes/{}", account.profile_id, MISSING_ID);
    let res = send(
        &app,
        patch_json(&path, &json!({ "title": "x", "content": "" })),
    )
    .await;
    assert_eq!(res.status, StatusCode::UNAUTHORIZED);
    res.expect_error(ErrorCode::Unauthenticated);
}

/// deleting with no token → 401 `unauthenticated`.
#[sqlx::test]
async fn delete_note_without_token_is_401(pool: PgPool) {
    let app = app(pool);
    let account = register(&app, "ada", "ada@example.com", "hunter2-long").await;

    let path = format!("/api/profiles/{}/notes/{}", account.profile_id, MISSING_ID);
    let res = send(&app, delete(&path)).await;
    assert_eq!(res.status, StatusCode::UNAUTHORIZED);
    res.expect_error(ErrorCode::Unauthenticated);
}

/// an invalid (malformed) token on a notes route → 401 `unauthenticated`.
#[sqlx::test]
async fn notes_route_with_malformed_token_is_401(pool: PgPool) {
    let app = app(pool);
    let account = register(&app, "ada", "ada@example.com", "hunter2-long").await;

    let path = format!("/api/profiles/{}/notes", account.profile_id);
    let res = send(&app, get_auth(&path, "not.a.jwt")).await;
    assert_eq!(res.status, StatusCode::UNAUTHORIZED);
    res.expect_error(ErrorCode::Unauthenticated);
}
