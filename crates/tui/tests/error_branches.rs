//! Error-code branching (slice-3 acceptance 3), driven through the public `App` API with the
//! fake client returning the relevant `ClientError`:
//!
//! - `unauthenticated` -> back to the login screen with the in-memory session dropped;
//! - `validation_failed` / `invalid_credentials` -> inline message, staying on the screen;
//! - transport failure / server offline -> the blocking offline screen with manual retry.
//!
//! These assert the *observable outcome* (`App::screen()` / `App::session()`), not internals.
//! The fake client is the only mock; a held clone scripts each step's response (its shared
//! queues back the clone the `App` owns).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

mod common;

use common::{FakeClient, api_err, offline_err, open_task, profile, session};
use contract::ErrorCode;
use tui::app::{App, Event, Screen};

/// A handle to a fake plus a freshly-logged-in app sharing it. The handle scripts later
/// responses; the app is on the task list of the `work` profile with the given tasks.
fn logged_in(tasks: Vec<contract::Task>) -> (FakeClient, App<FakeClient>) {
    let client = FakeClient::new();
    client.push_login(Ok(session("jwt-token")));
    client.push_profiles(Ok(vec![profile("p1", "work")]));
    client.push_tasks(Ok(tasks));
    let mut app = App::new(client.clone());
    app.handle_event(Event::Submit);
    assert!(
        matches!(app.screen(), Screen::TaskList(_)),
        "precondition: logged in to task list",
    );
    assert!(app.session().is_some(), "precondition: session present");
    (client, app)
}

// ---- validation_failed / invalid_credentials -> inline message, stay put ----

#[test]
fn login_validation_failed_shows_inline_error_and_stays_on_auth() {
    let client = FakeClient::new();
    client.push_login(Err(api_err(
        ErrorCode::ValidationFailed,
        "identifier must not be empty",
    )));
    let mut app = App::new(client);
    app.handle_event(Event::Submit);

    let Screen::Auth(auth) = app.screen() else {
        panic!("expected to stay on the auth screen");
    };
    assert_eq!(auth.error.as_deref(), Some("identifier must not be empty"));
    assert!(app.session().is_none(), "no session on a failed login");
}

#[test]
fn invalid_credentials_shows_inline_error_on_auth() {
    let client = FakeClient::new();
    client.push_login(Err(api_err(
        ErrorCode::InvalidCredentials,
        "invalid username or password",
    )));
    let mut app = App::new(client);
    app.handle_event(Event::Submit);

    let Screen::Auth(auth) = app.screen() else {
        panic!("expected auth screen");
    };
    assert_eq!(auth.error.as_deref(), Some("invalid username or password"));
}

#[test]
fn add_task_validation_failed_shows_inline_error_and_keeps_session() {
    let (client, mut app) = logged_in(vec![]);
    client.push_create(Err(api_err(
        ErrorCode::ValidationFailed,
        "title must not be empty",
    )));

    app.handle_event(Event::BeginAddTask);
    app.handle_event(Event::Submit); // submit the empty-title task

    let Screen::TaskList(list) = app.screen() else {
        panic!("a validation error must keep us on the task list");
    };
    let add = list
        .adding
        .as_ref()
        .expect("the add-task flow stays open on a validation error");
    assert_eq!(add.error.as_deref(), Some("title must not be empty"));
    assert!(app.session().is_some(), "session preserved on a 400");
}

// ---- unauthenticated -> back to login, session dropped ----

#[test]
fn unauthenticated_on_refresh_returns_to_login_and_drops_session() {
    let (client, mut app) = logged_in(vec![open_task("t1", "task", "2026-06-18T10:00:00Z")]);
    client.push_tasks(Err(api_err(ErrorCode::Unauthenticated, "token expired")));

    app.handle_event(Event::Refresh);

    let Screen::Auth(auth) = app.screen() else {
        panic!("unauthenticated must return to login");
    };
    assert!(
        app.session().is_none(),
        "the in-memory session must be dropped on unauthenticated",
    );
    assert!(
        auth.error.as_deref().is_some_and(|m| m.contains("log in")),
        "the login screen should prompt re-auth: {:?}",
        auth.error,
    );
}

#[test]
fn unauthenticated_on_close_returns_to_login() {
    let (client, mut app) = logged_in(vec![open_task("t1", "task", "2026-06-18T10:00:00Z")]);
    client.push_close(Err(api_err(ErrorCode::Unauthenticated, "token expired")));

    app.handle_event(Event::CloseSelected);

    assert!(matches!(app.screen(), Screen::Auth(_)));
    assert!(app.session().is_none());
}

// ---- offline -> blocking screen + manual retry ----

#[test]
fn transport_failure_on_login_goes_to_offline_screen() {
    let client = FakeClient::new();
    client.push_login(Err(offline_err("connection refused")));
    let mut app = App::new(client);
    app.handle_event(Event::Submit);

    let Screen::Offline { message } = app.screen() else {
        panic!("expected offline screen");
    };
    assert!(
        message.contains("unreachable"),
        "offline message: {message}"
    );
}

#[test]
fn offline_during_session_then_retry_recovers_to_task_list() {
    let (client, mut app) = logged_in(vec![open_task("t1", "task", "2026-06-18T10:00:00Z")]);

    // A refresh hits a transport failure -> offline screen (session kept for retry).
    client.push_tasks(Err(offline_err("connection reset")));
    app.handle_event(Event::Refresh);
    assert!(matches!(app.screen(), Screen::Offline { .. }));
    assert!(
        app.session().is_some(),
        "offline is transient — the session is kept for retry",
    );

    // Manual retry: health probe succeeds, then the task list reloads from the server.
    client.push_health(Ok(()));
    client.push_tasks(Ok(vec![open_task("t1", "task", "2026-06-18T10:00:00Z")]));
    app.handle_event(Event::Refresh); // 'r' on the offline screen = retry
    assert!(
        matches!(app.screen(), Screen::TaskList(_)),
        "a successful retry recovers to the task list",
    );
}

#[test]
fn retry_while_still_offline_stays_offline() {
    let client = FakeClient::new();
    client.push_login(Err(offline_err("connection refused")));
    let mut app = App::new(client.clone());
    app.handle_event(Event::Submit);
    assert!(matches!(app.screen(), Screen::Offline { .. }));

    // Retry while still down (no session yet): the health probe fails and we stay offline.
    client.push_health(Err(offline_err("still down")));
    app.handle_event(Event::Refresh);
    assert!(matches!(app.screen(), Screen::Offline { .. }));
}

#[test]
fn malformed_api_error_without_code_surfaces_inline_generic_message() {
    // An API error the server returned without a machine-matchable code is not unauthenticated
    // and not offline, so it surfaces inline (no session drop, no offline screen).
    let client = FakeClient::new();
    client.push_login(Err(common::api_err_no_code(
        "server returned 500 with no error body",
    )));
    let mut app = App::new(client);
    app.handle_event(Event::Submit);

    let Screen::Auth(auth) = app.screen() else {
        panic!("a coded-less API error stays on auth");
    };
    assert_eq!(
        auth.error.as_deref(),
        Some("server returned 500 with no error body"),
    );
}
