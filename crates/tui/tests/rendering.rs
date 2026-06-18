//! Buffer-snapshot rendering via `ratatui`'s `TestBackend`: renders the auth screen and the
//! task list into an in-memory buffer and asserts the observable text — the login/register
//! field labels, the password mask, the newest-first task ordering, and the done/undone
//! markers (slice-3 acceptance 2). The app core is driven through the public `App` API with the
//! fake client; nothing internal is mocked.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

mod common;

use common::{FakeClient, done_task, open_task, profile, render, session};
use tui::app::{App, Event};

const W: u16 = 80;
const H: u16 = 24;

/// Drive a fresh app from login to its task list with the given tasks, returning the app.
fn logged_in_with(tasks: Vec<contract::Task>) -> App<FakeClient> {
    let client = FakeClient::new();
    client.push_login(Ok(session("jwt-token")));
    client.push_profiles(Ok(vec![profile("p1", "work")]));
    client.push_tasks(Ok(tasks));
    let mut app = App::new(client);
    app.handle_event(Event::Submit); // submit login -> profiles -> tasks
    app
}

#[test]
fn auth_login_screen_renders_its_fields_and_hint() {
    let app = App::new(FakeClient::new());
    let text = render(&app, W, H);
    assert!(text.contains("organized-koala — Login"), "title:\n{text}");
    assert!(text.contains("Identifier"), "identifier field:\n{text}");
    assert!(text.contains("Password"), "password field:\n{text}");
    // Login form must NOT show the register-only fields.
    assert!(!text.contains("Username"), "no username on login:\n{text}");
    assert!(!text.contains("Email"), "no email on login:\n{text}");
    assert!(
        text.contains("F2: switch to register"),
        "toggle hint:\n{text}",
    );
}

#[test]
fn auth_register_screen_renders_all_four_fields() {
    let mut app = App::new(FakeClient::new());
    app.handle_event(Event::ToggleAuthMode);
    let text = render(&app, W, H);
    assert!(
        text.contains("organized-koala — Register"),
        "title:\n{text}"
    );
    assert!(text.contains("Username"), "username:\n{text}");
    assert!(text.contains("Email"), "email:\n{text}");
    assert!(text.contains("Password"), "password:\n{text}");
    assert!(text.contains("Profile name"), "profile name:\n{text}");
}

#[test]
fn password_is_rendered_masked() {
    let mut app = App::new(FakeClient::new());
    // Focus starts on Identifier; move to Password and type.
    app.handle_event(Event::Next);
    for c in "hunter2".chars() {
        app.handle_event(Event::Char(c));
    }
    let text = render(&app, W, H);
    assert!(
        text.contains("*******"),
        "password should render as 7 stars:\n{text}",
    );
    assert!(
        !text.contains("hunter2"),
        "plaintext password must never render:\n{text}",
    );
}

#[test]
fn task_list_renders_newest_first_with_markers() {
    // The server returns tasks newest-first; the view must preserve that order. We give an
    // already-ordered list (newest at index 0) and assert the rendered rows match.
    let tasks = vec![
        open_task("t-new", "newest open task", "2026-06-18T12:00:00Z"),
        done_task(
            "t-mid",
            "older done task",
            "2026-06-18T11:00:00Z",
            "2026-06-18T11:30:00Z",
        ),
        open_task("t-old", "oldest open task", "2026-06-18T10:00:00Z"),
    ];
    let app = logged_in_with(tasks);
    let text = render(&app, W, H);

    assert!(text.contains("tasks [work]"), "profile in header:\n{text}");
    // Done/undone markers.
    assert!(
        text.contains("[ ] newest open task"),
        "open marker:\n{text}"
    );
    assert!(text.contains("[x] older done task"), "done marker:\n{text}");
    assert!(
        text.contains("[ ] oldest open task"),
        "open marker:\n{text}"
    );

    // Ordering: newest row appears before the older rows in the rendered buffer.
    let pos_new = text.find("newest open task").expect("newest present");
    let pos_mid = text.find("older done task").expect("mid present");
    let pos_old = text.find("oldest open task").expect("oldest present");
    assert!(pos_new < pos_mid, "newest before mid:\n{text}");
    assert!(pos_mid < pos_old, "mid before oldest:\n{text}");
}

#[test]
fn task_list_command_hint_and_add_flow_hint() {
    let app = logged_in_with(vec![open_task("t1", "task one", "2026-06-18T10:00:00Z")]);
    let text = render(&app, W, H);
    assert!(
        text.contains("a: add") && text.contains("c: mark done") && text.contains("r: refresh"),
        "command hint:\n{text}",
    );

    // Open the add-task flow; the hint and the in-progress fields render.
    let mut app = app;
    app.handle_event(Event::BeginAddTask);
    for c in "Buy milk".chars() {
        app.handle_event(Event::Char(c));
    }
    let text = render(&app, W, H);
    assert!(text.contains("Add task"), "add-task panel:\n{text}");
    assert!(text.contains("Buy milk"), "typed title echoes:\n{text}");
    assert!(text.contains("Esc: cancel"), "add-flow hint:\n{text}");
}

#[test]
fn offline_screen_renders_blocking_message_and_retry() {
    // Render the offline screen directly via a driven flow: a transport failure on login.
    let client = FakeClient::new();
    client.push_login(Err(common::offline_err("connection refused")));
    let mut app = App::new(client);
    app.handle_event(Event::Submit);
    let text = render(&app, W, H);
    assert!(
        text.contains("Server unreachable"),
        "offline title:\n{text}",
    );
    assert!(text.contains("connection refused"), "cause shown:\n{text}");
    assert!(text.contains("Press r to retry"), "retry hint:\n{text}");
}

#[test]
fn empty_task_list_still_renders_chrome() {
    let app = logged_in_with(Vec::new());
    let text = render(&app, W, H);
    assert!(
        text.contains("tasks [work]"),
        "header even when empty:\n{text}"
    );
    assert!(text.contains("Tasks"), "list block title:\n{text}");
}
