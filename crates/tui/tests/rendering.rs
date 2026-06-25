//! Buffer-snapshot rendering via `ratatui`'s `TestBackend`: renders each screen into an
//! in-memory buffer and asserts the observable text — the login/register field labels, the
//! password mask, the newest-first task ordering, the done/undone markers, and the in-flight
//! append-spinner indicator (ADR-0006 §8.3 / Board 0008-R1: a trailing spinner + "(Esc to cancel)"
//! is APPENDED to the stable caption, never replacing it — the flicker fix). The app core is
//! driven through the public two-step `App` API (`handle_event` → executor → `apply_response`)
//! with the fake client; nothing internal is mocked.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

mod common;

use common::{
    FakeClient, done_task, drive, open_task, profile, render, render_at, session, submit,
};
use tui::app::{App, Event};

const W: u16 = 80;
const H: u16 = 24;

/// Drive a fresh app from login to its task list with the given tasks, returning the app.
fn logged_in_with(tasks: Vec<contract::Task>) -> App {
    let client = FakeClient::new();
    client.push_login(Ok(session("jwt-token")));
    client.push_profiles(Ok(vec![profile("p1", "work")]));
    client.push_tasks(Ok(tasks));
    let mut app = App::new();
    submit(&mut app, &client, Event::Submit); // login -> profiles -> tasks
    app
}

#[test]
fn auth_login_screen_renders_its_fields_and_hint() {
    let app = App::new();
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
    let mut app = App::new();
    let _ = app.handle_event(Event::ToggleAuthMode);
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
    let mut app = App::new();
    // Focus starts on Identifier; move to Password and type.
    let _ = app.handle_event(Event::Next);
    for c in "hunter2".chars() {
        let _ = app.handle_event(Event::Char(c));
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
        text.contains("a: add")
            && text.contains("e: edit")
            && text.contains("c: done")
            && text.contains("x: del")
            && text.contains("r: refresh"),
        "command hint:\n{text}",
    );

    // Open the add-task flow; the hint and the in-progress fields render.
    let mut app = app;
    let _ = app.handle_event(Event::BeginAddTask);
    for c in "Buy milk".chars() {
        let _ = app.handle_event(Event::Char(c));
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
    let mut app = App::new();
    submit(&mut app, &client, Event::Submit);
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

// ---- In-flight render (append-spinner indicator + cancel hint, ADR-0006 §8.3) ----

#[test]
fn auth_in_flight_appends_spinner_without_replacing_the_caption() {
    // After a submit, before the response is applied, the auth screen is pending. The flicker fix
    // (0008-R1): the stable caption is KEPT and a trailing spinner + "(Esc to cancel)" is appended,
    // rather than the old behaviour where the caption was substituted with a "working…" string. We
    // hold the dispatch (do NOT drive it) so the app sits in the in-flight state, and render it.
    let client = FakeClient::new();
    let mut app = App::new();
    let dispatch = app
        .handle_event(Event::Submit)
        .expect("login submit dispatches a request");
    assert!(app.is_pending(), "app is in-flight while awaiting login");

    let text = render(&app, W, H);
    // The caption text is STILL present (no flicker / no replacement) — the regression guard.
    assert!(
        text.contains("F2: switch to register"),
        "the stable caption is NOT replaced while pending:\n{text}",
    );
    // The old "working…" replacement string is gone.
    assert!(
        !text.contains("working…"),
        "the caption is no longer replaced by a working… string:\n{text}",
    );
    // The cancel affordance is appended alongside the spinner.
    assert!(
        text.contains("Esc to cancel"),
        "cancel affordance appended while pending:\n{text}",
    );

    // Sanity: the request was a real login the executor can complete (keeps the fake honest).
    client.push_login(Ok(session("jwt")));
    client.push_profiles(Ok(vec![profile("p1", "work")]));
    client.push_tasks(Ok(vec![]));
    drive(&mut app, &client, dispatch);
    assert!(!app.is_pending(), "no longer pending once the flow settles");
}

#[test]
fn spinner_glyph_advances_with_the_tick() {
    // The spinner animates: different ticks render different glyphs while pending. Pin the public
    // cadence via `ui::spinner_frame` and confirm the rendered buffer reflects it.
    let mut app = App::new();
    let _dispatch = app
        .handle_event(Event::Submit)
        .expect("login submit dispatches a request");

    let f0 = tui::ui::spinner_frame(0);
    let f1 = tui::ui::spinner_frame(1);
    assert_ne!(f0, f1, "consecutive spinner frames differ");

    let at0 = render_at(&app, W, H, 0);
    let at1 = render_at(&app, W, H, 1);
    assert!(at0.contains(f0), "tick 0 glyph {f0:?} present:\n{at0}");
    assert!(at1.contains(f1), "tick 1 glyph {f1:?} present:\n{at1}");
}

#[test]
fn task_list_in_flight_appends_spinner_without_replacing_the_caption() {
    // A close/refresh on the task list puts it in-flight; the command caption is KEPT and a
    // trailing spinner + cancel affordance is appended (ADR-0006 §8.3 — no flicker).
    let mut app = logged_in_with(vec![open_task("t1", "task", "2026-06-18T10:00:00Z")]);
    let _dispatch = app
        .handle_event(Event::ToggleDone)
        .expect("toggle-done dispatches a request");
    assert!(app.is_pending(), "task list in-flight after toggle-done");

    let text = render(&app, W, H);
    // The command caption stays present (not replaced) — the regression guard.
    assert!(
        text.contains("a: add") && text.contains("p: timer"),
        "the command caption is NOT replaced while pending:\n{text}",
    );
    assert!(
        !text.contains("working…"),
        "the caption is no longer replaced by a working… string:\n{text}",
    );
    assert!(
        text.contains("Esc to cancel"),
        "cancel affordance appended while pending:\n{text}",
    );
}

#[test]
fn offline_retry_in_flight_appends_spinner_without_replacing_the_caption() {
    // On the offline screen, pressing retry ('r') fires a health probe; while it is outstanding
    // the retry caption is KEPT and a trailing spinner + cancel affordance is appended.
    let client = FakeClient::new();
    client.push_login(Err(common::offline_err("connection refused")));
    let mut app = App::new();
    submit(&mut app, &client, Event::Submit);
    assert_eq!(common::screen_name(&app), "offline");

    let _dispatch = app
        .handle_event(Event::Refresh)
        .expect("retry dispatches a health probe");
    assert!(app.is_pending(), "offline screen in-flight while probing");

    let text = render(&app, W, H);
    assert!(
        text.contains("Press r to retry"),
        "the retry caption is NOT replaced while probing:\n{text}",
    );
    assert!(
        !text.contains("working…"),
        "the caption is no longer replaced by a working… string:\n{text}",
    );
    assert!(
        text.contains("Esc to cancel"),
        "cancel affordance appended while probing:\n{text}",
    );
}
