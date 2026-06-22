//! End-to-end app-core flows driven through the public two-step `App` API
//! (`handle_event` → executor → `apply_response`) against the held fake client:
//!
//! - register and login flows reach the auto-selected profile's task list;
//! - the add-task flow (Title + Description) posts a `CreateTaskRequest` and re-renders from the
//!   server's fresh list;
//! - mark-done sends the `…/close` request and the row re-renders from the returned `Task`;
//! - statelessness (hard-constraint #1): every view derives from a server response — the
//!   rendered list mirrors exactly what the server returned, never fabricated/cached data, and
//!   each mutation triggers a server round-trip whose response drives the next render.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

mod common;

use common::{Call, FakeClient, done_task, drive, open_task, profile, render, session, submit};
use contract::TaskStatus;
use tui::app::{App, AuthMode, Event, Screen};

const W: u16 = 80;
const H: u16 = 24;

/// Type a string into the focused field (local edits never dispatch).
fn type_str(app: &mut App, s: &str) {
    for c in s.chars() {
        let _ = app.handle_event(Event::Char(c));
    }
}

// ---- auth flows ----

#[test]
fn login_flow_fetches_profiles_and_enters_task_list() {
    let client = FakeClient::new();
    client.push_login(Ok(session("jwt-abc")));
    client.push_profiles(Ok(vec![profile("p1", "work")]));
    client.push_tasks(Ok(vec![open_task("t1", "first", "2026-06-18T10:00:00Z")]));
    let mut app = App::new();

    // Fill the login form, then submit.
    type_str(&mut app, "ada@example.com");
    let _ = app.handle_event(Event::Next); // -> Password
    type_str(&mut app, "hunter2");
    submit(&mut app, &client, Event::Submit);

    // Landed on the task list of the auto-selected profile.
    assert!(matches!(app.screen(), Screen::TaskList(_)));
    let s = app.session().expect("session set");
    assert_eq!(s.profile_id, "p1");
    assert_eq!(s.profile_name, "work");

    // The exact server call sequence: login -> profiles -> list_tasks (auto-selected profile).
    let calls = client.calls();
    assert!(
        matches!(calls.first(), Some(Call::Login { identifier }) if identifier == "ada@example.com"),
        "login carried the identifier: {calls:?}",
    );
    assert!(
        matches!(calls.get(1), Some(Call::ListProfiles { token }) if token == "jwt-abc"),
        "profiles fetched with the token: {calls:?}",
    );
    assert!(
        matches!(calls.get(2), Some(Call::ListTasks { token, profile_id })
            if token == "jwt-abc" && profile_id == "p1"),
        "tasks listed for the selected profile: {calls:?}",
    );
}

#[test]
fn register_flow_carries_all_fields_and_enters_task_list() {
    let client = FakeClient::new();
    client.push_register(Ok(session("jwt-reg")));
    client.push_profiles(Ok(vec![profile("pX", "personal")]));
    client.push_tasks(Ok(vec![]));
    let mut app = App::new();

    let _ = app.handle_event(Event::ToggleAuthMode); // switch to register
    let Screen::Auth(auth) = app.screen() else {
        panic!("auth screen");
    };
    assert_eq!(auth.mode, AuthMode::Register);

    // Fields are Username, Email, Password, Profile name in nav order.
    type_str(&mut app, "ada");
    let _ = app.handle_event(Event::Next);
    type_str(&mut app, "ada@example.com");
    let _ = app.handle_event(Event::Next);
    type_str(&mut app, "hunter2");
    let _ = app.handle_event(Event::Next);
    type_str(&mut app, "personal");
    submit(&mut app, &client, Event::Submit);

    assert!(matches!(app.screen(), Screen::TaskList(_)));
    let calls = client.calls();
    assert!(
        matches!(
            calls.first(),
            Some(Call::Register { username, email, profile_name })
                if username == "ada" && email == "ada@example.com" && profile_name == "personal"
        ),
        "register carried the form fields: {calls:?}",
    );
}

// ---- add-task flow ----

#[test]
fn add_task_posts_request_then_refreshes_from_server() {
    let client = FakeClient::new();
    client.push_login(Ok(session("jwt")));
    client.push_profiles(Ok(vec![profile("p1", "work")]));
    client.push_tasks(Ok(vec![])); // initial empty list
    let mut app = App::new();
    submit(&mut app, &client, Event::Submit);
    assert!(matches!(app.screen(), Screen::TaskList(_)));

    // Script the create response and the post-create refresh list.
    let created = open_task("t-new", "Buy milk", "2026-06-18T13:00:00Z");
    client.push_create(Ok(created.clone()));
    client.push_tasks(Ok(vec![created]));

    // Drive the add-task sub-flow: open, title, switch field, description, submit.
    let _ = app.handle_event(Event::BeginAddTask);
    type_str(&mut app, "Buy milk");
    let _ = app.handle_event(Event::Next); // -> description field
    type_str(&mut app, "2% organic");
    submit(&mut app, &client, Event::Submit);

    // The add flow closed and the list now shows the server's task.
    let Screen::TaskList(list) = app.screen() else {
        panic!("task list");
    };
    assert!(list.adding.is_none(), "add flow closed after success");
    assert_eq!(list.tasks.len(), 1);
    assert_eq!(list.tasks.first().expect("one task").title, "Buy milk");

    // The create call carried both Title and Description, and was followed by a fresh list.
    let calls = client.calls();
    let create = calls
        .iter()
        .find_map(|c| match c {
            Call::CreateTask {
                title, description, ..
            } => Some((title.clone(), description.clone())),
            _ => None,
        })
        .expect("a create_task call was made");
    assert_eq!(create, ("Buy milk".to_owned(), "2% organic".to_owned()));
    assert!(
        matches!(calls.last(), Some(Call::ListTasks { .. })),
        "a fresh list fetch follows the create (statelessness): {calls:?}",
    );

    // The rendered view shows the server-provided task — not anything fabricated.
    let text = render(&app, W, H);
    assert!(
        text.contains("[ ] Buy milk"),
        "rendered from server:\n{text}"
    );
}

// ---- mark-done flow ----

#[test]
fn mark_done_sends_close_and_rerenders_from_returned_task() {
    let open = open_task("t1", "Write tests", "2026-06-18T10:00:00Z");
    let client = FakeClient::new();
    client.push_login(Ok(session("jwt")));
    client.push_profiles(Ok(vec![profile("p1", "work")]));
    client.push_tasks(Ok(vec![open]));
    let mut app = App::new();
    submit(&mut app, &client, Event::Submit);

    // Before: rendered as undone.
    let before = render(&app, W, H);
    assert!(
        before.contains("[ ] Write tests"),
        "starts undone:\n{before}"
    );

    // Script the close response (status done, closed_at set).
    let closed = done_task(
        "t1",
        "Write tests",
        "2026-06-18T10:00:00Z",
        "2026-06-18T14:00:00Z",
    );
    client.push_close(Ok(closed));

    submit(&mut app, &client, Event::CloseSelected);

    // The row was replaced in place from the server's returned Task.
    let Screen::TaskList(list) = app.screen() else {
        panic!("task list");
    };
    assert_eq!(list.tasks.len(), 1, "no extra/duplicate row");
    let row = list.tasks.first().expect("one task");
    assert_eq!(row.status, TaskStatus::Done);
    assert!(
        row.closed_at.is_some(),
        "closed_at set from the server response",
    );

    // The close call targeted the selected task id under the active profile.
    let calls = client.calls();
    assert!(
        matches!(calls.last(), Some(Call::CloseTask { token, profile_id, task_id })
            if token == "jwt" && profile_id == "p1" && task_id == "t1"),
        "close targeted the right task: {calls:?}",
    );

    // After: the rendered marker flipped to done.
    let after = render(&app, W, H);
    assert!(after.contains("[x] Write tests"), "now done:\n{after}");
}

// ---- statelessness ----

#[test]
fn task_list_view_mirrors_exactly_the_server_response() {
    // The rendered list equals what the server returned — order, count, and markers — with no
    // fabricated or cached entries (hard-constraint #1).
    let server_tasks = vec![
        open_task("a", "alpha", "2026-06-18T12:00:00Z"),
        done_task("b", "bravo", "2026-06-18T11:00:00Z", "2026-06-18T11:30:00Z"),
    ];
    let client = FakeClient::new();
    client.push_login(Ok(session("jwt")));
    client.push_profiles(Ok(vec![profile("p1", "work")]));
    client.push_tasks(Ok(server_tasks.clone()));
    let mut app = App::new();
    submit(&mut app, &client, Event::Submit);

    let Screen::TaskList(list) = app.screen() else {
        panic!("task list");
    };
    assert_eq!(
        list.tasks, server_tasks,
        "view is exactly the server's list"
    );
}

#[test]
fn refresh_replaces_the_list_with_the_servers_new_response() {
    let client = FakeClient::new();
    client.push_login(Ok(session("jwt")));
    client.push_profiles(Ok(vec![profile("p1", "work")]));
    client.push_tasks(Ok(vec![open_task("old", "stale", "2026-06-18T09:00:00Z")]));
    let mut app = App::new();
    submit(&mut app, &client, Event::Submit);

    // Server's state changed; a refresh must show the new list, dropping the stale entry.
    client.push_tasks(Ok(vec![open_task(
        "fresh",
        "current",
        "2026-06-18T15:00:00Z",
    )]));
    submit(&mut app, &client, Event::Refresh);

    let Screen::TaskList(list) = app.screen() else {
        panic!("task list");
    };
    assert_eq!(list.tasks.len(), 1);
    assert_eq!(
        list.tasks.first().expect("one task").title,
        "current",
        "stale data is not cached",
    );
}

#[test]
fn new_app_holds_no_session_and_starts_on_login() {
    // No on-disk/cross-run state: a fresh app is unauthenticated on the login screen, and no
    // request is dispatched before any user action.
    let app = App::new();
    assert!(app.session().is_none());
    let Screen::Auth(auth) = app.screen() else {
        panic!("starts on auth");
    };
    assert_eq!(auth.mode, AuthMode::Login);
    assert!(!app.is_pending(), "no in-flight request before any action");
}

#[test]
fn quit_event_sets_should_quit() {
    let mut app = App::new();
    assert!(!app.should_quit());
    let dispatch = app.handle_event(Event::Quit);
    assert!(dispatch.is_none(), "quit dispatches no request");
    assert!(app.should_quit());
}

#[test]
fn at_most_one_request_chains_at_a_time_through_apply_response() {
    // The auth flow fans out to three server calls (login -> profiles -> tasks) but only ever as
    // a *chain*: each completes before the next is dispatched. Driving the single login dispatch
    // to completion yields exactly that ordered sequence with nothing concurrent.
    let client = FakeClient::new();
    client.push_login(Ok(session("jwt")));
    client.push_profiles(Ok(vec![profile("p1", "work")]));
    client.push_tasks(Ok(vec![]));
    let mut app = App::new();

    let dispatch = app
        .handle_event(Event::Submit)
        .expect("login submit dispatches");
    drive(&mut app, &client, dispatch);

    let calls = client.calls();
    assert!(
        matches!(calls.first(), Some(Call::Login { .. }))
            && matches!(calls.get(1), Some(Call::ListProfiles { .. }))
            && matches!(calls.get(2), Some(Call::ListTasks { .. }))
            && calls.len() == 3,
        "exactly the chained login->profiles->tasks sequence: {calls:?}",
    );
    assert!(!app.is_pending(), "settled with no request in flight");
}
