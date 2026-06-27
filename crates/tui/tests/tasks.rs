//! Task-mutation flows (item 0011): toggle-done / reopen / edit / delete, driven through the
//! public two-step `App` API (`handle_event` → synchronous executor → `apply_response`) against
//! the held fake client (the sole external-service mock, ADR-0003 layer 2).
//!
//! Every mutation is a server round-trip whose success chains a `ListTasks` refresh, so the
//! rendered view always derives from a server response (hard-constraint #1) — never fabricated or
//! cached locally. These assert the *issued request* (what crossed the mocked wire) and the
//! *observable outcome* (`App::screen()` and the rendered buffer), not internals:
//!
//! - toggle-done issues `UpdateTask { status: done }` and the row renders done;
//! - reopen issues `UpdateTask { status: open }` and the done render clears (the toggle
//!   round-trip — the highest-value TUI test per the plan's Risks);
//! - edit issues `UpdateTask { title, description }` and the row reflects the new title/desc;
//! - an empty-title edit is rejected inline (local validation, no request issued);
//! - delete is a two-step confirm: the first `x` shows the affordance and issues no request, the
//!   second `x` issues `DeleteTask` and the row is removed after the refresh.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

mod common;

use common::{
    Call, FakeClient, done_task, open_task, profile, render, session, submit, tasks_pane,
};
use contract::TaskStatus;
use tui::app::{App, Event, Screen};

const W: u16 = 80;
const H: u16 = 24;

/// A handle to a fake plus a freshly-logged-in app sharing it, on the `work` task list with the
/// given tasks. The handle scripts later responses; the app starts settled on the task list.
fn logged_in(tasks: Vec<contract::Task>) -> (FakeClient, App) {
    let client = FakeClient::new();
    client.push_login(Ok(session("jwt")));
    client.push_profiles(Ok(vec![profile("p1", "work")]));
    client.push_tasks(Ok(tasks));
    let mut app = App::new();
    submit(&mut app, &client, Event::Submit);
    assert!(
        matches!(app.screen(), Screen::Main(_)),
        "precondition: logged in to the Tasks tab",
    );
    (client, app)
}

/// Type a string into the focused field (local edits never dispatch).
fn type_str(app: &mut App, s: &str) {
    for c in s.chars() {
        let _ = app.handle_event(Event::Char(c));
    }
}

/// The captured fields of the single `UpdateTask` call in `calls`, panicking if none was made.
fn update_call(
    calls: &[Call],
) -> (
    String,
    String,
    String,
    Option<String>,
    Option<String>,
    Option<TaskStatus>,
) {
    calls
        .iter()
        .find_map(|c| match c {
            Call::UpdateTask {
                token,
                profile_id,
                task_id,
                title,
                description,
                status,
            } => Some((
                token.clone(),
                profile_id.clone(),
                task_id.clone(),
                title.clone(),
                description.clone(),
                *status,
            )),
            _ => None,
        })
        .expect("an update_task call was made")
}

// ---- toggle-done ----

#[test]
fn toggle_done_issues_status_done_patch_and_row_renders_done() {
    let (client, mut app) = logged_in(vec![open_task("t1", "Write tests", "2026-06-18T10:00:00Z")]);
    let before = render(&app, W, H);
    assert!(before.contains("[ ] Write tests"), "starts open:\n{before}");

    // The update returns the done task; the success chains a refresh list.
    let done = done_task(
        "t1",
        "Write tests",
        "2026-06-18T10:00:00Z",
        "2026-06-18T14:00:00Z",
    );
    client.push_update(Ok(done.clone()));
    client.push_tasks(Ok(vec![done]));

    submit(&mut app, &client, Event::ToggleDone);

    // The issued patch is status-only (title/description absent) and targets the selected task.
    let calls = client.calls();
    assert_eq!(
        update_call(&calls),
        (
            "jwt".to_owned(),
            "p1".to_owned(),
            "t1".to_owned(),
            None,
            None,
            Some(TaskStatus::Done),
        ),
        "toggle-done sends only status=done: {calls:?}",
    );

    let after = render(&app, W, H);
    assert!(after.contains("[x] Write tests"), "now done:\n{after}");
}

#[test]
fn reopen_a_done_task_issues_status_open_patch_and_done_render_clears() {
    // The toggle round-trip — the highest-value TUI test (plan Risks): a done task toggled with
    // `c` reopens, issuing `status: open`, and the done marker clears once the refresh lands.
    let (client, mut app) = logged_in(vec![done_task(
        "t1",
        "Write tests",
        "2026-06-18T10:00:00Z",
        "2026-06-18T14:00:00Z",
    )]);
    let before = render(&app, W, H);
    assert!(before.contains("[x] Write tests"), "starts done:\n{before}");

    // Reopen returns the task open again (closed_at cleared server-side); refresh shows it open.
    let reopened = open_task("t1", "Write tests", "2026-06-18T10:00:00Z");
    client.push_update(Ok(reopened.clone()));
    client.push_tasks(Ok(vec![reopened]));

    submit(&mut app, &client, Event::ToggleDone);

    let calls = client.calls();
    assert_eq!(
        update_call(&calls),
        (
            "jwt".to_owned(),
            "p1".to_owned(),
            "t1".to_owned(),
            None,
            None,
            Some(TaskStatus::Open),
        ),
        "reopen sends only status=open: {calls:?}",
    );

    let row = tasks_pane(&app).tasks.first().expect("one task").clone();
    assert_eq!(row.status, TaskStatus::Open);
    assert!(row.closed_at.is_none(), "closed_at cleared on reopen");

    let after = render(&app, W, H);
    assert!(
        after.contains("[ ] Write tests"),
        "the done marker cleared:\n{after}",
    );
}

// ---- edit ----

#[test]
fn edit_issues_title_and_description_patch_and_row_reflects_it() {
    let (client, mut app) = logged_in(vec![open_task("t1", "old title", "2026-06-18T10:00:00Z")]);

    // Script the update response (renamed task) and the chained refresh list.
    let renamed = open_task("t1", "new title", "2026-06-18T10:00:00Z");
    client.push_update(Ok(renamed.clone()));
    client.push_tasks(Ok(vec![renamed]));

    // Open the edit sub-flow: it pre-fills from the task, so clear the title before retyping.
    let _ = app.handle_event(Event::BeginEditTask);
    let edit_title = tasks_pane(&app)
        .editing
        .as_ref()
        .expect("edit sub-flow open")
        .title
        .clone();
    assert_eq!(edit_title, "old title", "edit pre-fills the current title");

    // Clear "old title" (9 chars) then type the new title; switch to the description field.
    for _ in 0.."old title".len() {
        let _ = app.handle_event(Event::Backspace);
    }
    type_str(&mut app, "new title");
    let _ = app.handle_event(Event::Next); // -> description field
    type_str(&mut app, "with notes");
    submit(&mut app, &client, Event::Submit);

    // The issued patch carried both title and description (trimmed title), no status.
    let calls = client.calls();
    assert_eq!(
        update_call(&calls),
        (
            "jwt".to_owned(),
            "p1".to_owned(),
            "t1".to_owned(),
            Some("new title".to_owned()),
            Some("with notes".to_owned()),
            None,
        ),
        "edit sends title+description, no status: {calls:?}",
    );
    assert!(
        matches!(calls.last(), Some(Call::ListTasks { .. })),
        "a fresh list fetch follows the edit (statelessness): {calls:?}",
    );

    // The edit sub-flow closed and the row reflects the server's renamed task.
    let list = tasks_pane(&app);
    assert!(list.editing.is_none(), "edit flow closed after success");
    assert_eq!(list.tasks.first().expect("one task").title, "new title");

    let after = render(&app, W, H);
    assert!(after.contains("new title"), "renamed row renders:\n{after}");
    assert!(!after.contains("old title"), "old title gone:\n{after}");
}

#[test]
fn empty_title_edit_is_rejected_inline_with_no_request_issued() {
    let (client, mut app) = logged_in(vec![open_task("t1", "keep me", "2026-06-18T10:00:00Z")]);
    let calls_before = client.calls().len();

    // Open the edit sub-flow and clear the title to empty, then submit.
    let _ = app.handle_event(Event::BeginEditTask);
    for _ in 0.."keep me".len() {
        let _ = app.handle_event(Event::Backspace);
    }
    // Submitting a blank title is rejected locally; no Dispatch is produced.
    assert!(
        app.handle_event(Event::Submit).is_none(),
        "an empty-title edit submit dispatches nothing",
    );

    // No update_task crossed the wire — purely local validation.
    assert_eq!(
        client.calls().len(),
        calls_before,
        "no request issued for a blank-title edit: {:?}",
        client.calls(),
    );

    // The edit sub-flow stays open with an inline error; the task is unchanged.
    let list = tasks_pane(&app);
    let edit = list.editing.as_ref().expect("edit stays open on rejection");
    assert!(
        edit.error.as_deref().is_some_and(|m| m.contains("empty")),
        "an inline empty-title error is shown: {:?}",
        edit.error,
    );
    assert_eq!(
        list.tasks.first().expect("task present").title,
        "keep me",
        "the task is unchanged by a rejected edit",
    );

    // The inline error renders in the edit panel.
    let text = render(&app, W, H);
    assert!(text.contains("Edit task"), "edit panel shown:\n{text}");
    assert!(text.contains("empty"), "inline error rendered:\n{text}");
}

// ---- delete (two-step confirm) ----

#[test]
fn first_delete_arms_confirm_and_issues_no_request() {
    let (client, mut app) = logged_in(vec![open_task("t1", "doomed", "2026-06-18T10:00:00Z")]);
    let calls_before = client.calls().len();

    // The first `x` arms the confirmation only — no request, no Dispatch, the row still present.
    assert!(
        app.handle_event(Event::DeleteSelected).is_none(),
        "the first delete key arms the confirm, dispatching nothing",
    );
    assert_eq!(
        client.calls().len(),
        calls_before,
        "no delete request on the first key: {:?}",
        client.calls(),
    );

    let list = tasks_pane(&app);
    assert_eq!(
        list.confirming_delete.as_deref(),
        Some("t1"),
        "the confirm is armed for the selected task",
    );
    assert_eq!(list.tasks.len(), 1, "the row is still present while armed");

    // 0015: the armed two-step affordance now renders as a centred confirmation dialog (the second
    // `x` still confirms, `Esc` cancels — behaviour preserved, only the render site moved).
    let text = render(&app, W, H);
    assert!(
        text.contains("Delete task") && text.contains("Delete this task?"),
        "the confirmation dialog is shown:\n{text}",
    );
    assert!(
        text.contains("x: confirm delete") && text.contains("Esc: cancel"),
        "the dialog hint keeps the second-`x`-confirms / Esc-cancels affordance:\n{text}",
    );
}

#[test]
fn second_delete_issues_delete_request_and_row_is_removed_after_refresh() {
    let (client, mut app) = logged_in(vec![
        open_task("t1", "doomed", "2026-06-18T12:00:00Z"),
        open_task("t2", "survivor", "2026-06-18T11:00:00Z"),
    ]);

    // Arm the confirm (first `x`).
    assert!(app.handle_event(Event::DeleteSelected).is_none());

    // Script the delete (204, no body) and the chained refresh (the row gone server-side).
    client.push_delete(Ok(()));
    client.push_tasks(Ok(vec![open_task(
        "t2",
        "survivor",
        "2026-06-18T11:00:00Z",
    )]));

    // The second `x` confirms and issues the delete.
    submit(&mut app, &client, Event::DeleteSelected);

    // The delete targeted the selected task under the active profile, followed by a refresh.
    let calls = client.calls();
    assert!(
        calls.iter().any(|c| matches!(c,
            Call::DeleteTask { token, profile_id, task_id }
                if token == "jwt" && profile_id == "p1" && task_id == "t1")),
        "delete targeted the right task: {calls:?}",
    );
    assert!(
        matches!(calls.last(), Some(Call::ListTasks { .. })),
        "a fresh list fetch follows the delete (statelessness): {calls:?}",
    );

    // The deleted row is gone and the confirm disarmed; the view is exactly the server's list.
    let list = tasks_pane(&app);
    assert!(list.confirming_delete.is_none(), "confirm disarmed");
    assert_eq!(list.tasks.len(), 1, "the deleted row is removed");
    assert_eq!(list.tasks.first().expect("survivor").id, "t2");

    let after = render(&app, W, H);
    assert!(after.contains("survivor"), "survivor remains:\n{after}");
    assert!(!after.contains("doomed"), "doomed row gone:\n{after}");
}

#[test]
fn a_non_delete_key_disarms_the_delete_confirm() {
    let (client, mut app) = logged_in(vec![open_task("t1", "doomed", "2026-06-18T10:00:00Z")]);

    // Arm, then press a different key (navigation) — the confirm disarms, no delete issues.
    assert!(app.handle_event(Event::DeleteSelected).is_none());
    let _ = app.handle_event(Event::Next);

    assert!(
        tasks_pane(&app).confirming_delete.is_none(),
        "a stray key disarms the confirm",
    );

    // A subsequent single `x` only re-arms (it does not delete) — proving the disarm took effect.
    assert!(
        app.handle_event(Event::DeleteSelected).is_none(),
        "after disarm, the next delete key re-arms rather than deleting",
    );
    assert!(
        !client
            .calls()
            .iter()
            .any(|c| matches!(c, Call::DeleteTask { .. })),
        "no delete request crossed the wire: {:?}",
        client.calls(),
    );
}

// ---- in-flight spinner during a mutation ----

#[test]
fn delete_in_flight_renders_spinner_and_keeps_caption() {
    let mut app = {
        let (_client, app) = logged_in(vec![open_task("t1", "doomed", "2026-06-18T10:00:00Z")]);
        app
    };

    // Arm then confirm, holding the dispatch (never driving it) so the app sits in-flight.
    assert!(app.handle_event(Event::DeleteSelected).is_none());
    let _dispatch = app
        .handle_event(Event::DeleteSelected)
        .expect("the second delete key dispatches a delete request");
    assert!(app.is_pending(), "task list in-flight during the delete");

    let text = render(&app, W, H);
    // The trimmed footer caption is kept (not replaced) and ONLY a trailing spinner glyph is
    // appended (ADR-0006 §8.3 amended: the cancel hint moved into the `?` help modal, not the
    // footer; the per-pane action keys live there too).
    assert!(
        text.contains("switch tab") && text.contains("q: quit"),
        "the caption is not replaced while pending:\n{text}",
    );
    // ONLY a trailing spinner glyph is appended (tick 0 → "|").
    assert!(
        text.contains(tui::ui::spinner_frame(0)),
        "a trailing spinner glyph is appended while pending:\n{text}",
    );
    // The textual cancel affordance is no longer in the footer — it moved to the `?` help modal.
    assert!(
        !text.contains("Esc to cancel"),
        "the cancel hint is no longer in the footer (it lives in the ? modal):\n{text}",
    );
}
