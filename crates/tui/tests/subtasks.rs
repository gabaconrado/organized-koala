//! Sub-task interaction + rendering suite (item 0019, ADR-0012/0013), driven through the public
//! two-step `App` API (`handle_event` → synchronous executor → `apply_response`) against the held
//! fake client (the sole external-service mock, ADR-0003 layer 2). Covers:
//!
//! - the two-call Tasks-tab tree load (tasks + the profile's sub-tasks) and indented render;
//! - `A` (`BeginAddSubtask`) opens the add-sub-task form and a submit issues `CreateSubtask` under
//!   the selection's parent, then refreshes the tree;
//! - `e` (`BeginEditTask`) on a selected **sub-task** row edits its title (`UpdateSubtask { title }`);
//! - `Space` (`ToggleDone`) on a selected sub-task row toggles its status (`UpdateSubtask { status }`);
//! - `x` (`ToggleCollapse`) records an in-session override flipping the resolved collapse state;
//! - the `+` (collapsed-with-children) vs `>` (expanded / no children) list indicator;
//! - collapse defaults derived from parent status each render (open → expanded, done → collapsed);
//! - the Task Detail read-only "Sub-tasks" section (populated from the chained `ListTaskSubtasks`);
//! - selection traversal over interleaved task + sub-task rows, including across a collapsed parent
//!   (Risk R2): a collapsed parent's hidden children are never landed on.
//!
//! These exercise the surface with no live server and no real terminal — the only mock is the
//! sanctioned `Client` trait (the HTTP server), exactly as ADR-0003 / ADR-0006 prescribe.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

mod common;

use common::{
    Call, FakeClient, done_subtask, open_subtask, profile, render, session, submit, tasks_pane,
    today_done_task, today_open_task,
};
use contract::{Subtask, TaskStatus};
use tui::app::{App, Event, Screen, VisibleRow, current_day_number};

const W: u16 = 80;
const H: u16 = 24;

/// A handle to a fake plus a freshly-logged-in app sharing it, on the `work` Tasks tab with the
/// given tasks and the profile's sub-tasks scripted for the two-call tree load. The handle scripts
/// later responses; the app starts settled on the Tasks tab with the tree loaded.
fn logged_in(tasks: Vec<contract::Task>, subtasks: Vec<Subtask>) -> (FakeClient, App) {
    let client = FakeClient::new();
    client.push_login(Ok(session("jwt")));
    client.push_profiles(Ok(vec![profile("p1", "work")]));
    client.push_tasks(Ok(tasks)); // first tree-load call
    client.push_list_subtasks(Ok(subtasks)); // chained second call
    let mut app = App::new();
    submit(&mut app, &client, Event::Submit);
    assert!(
        matches!(app.screen(), Screen::Main(_)),
        "precondition: logged in to the Tasks tab with the tree loaded",
    );
    (client, app)
}

/// Type a string into the focused field (local edits never dispatch).
fn type_str(app: &mut App, s: &str) {
    for c in s.chars() {
        let _ = app.handle_event(Event::Char(c));
    }
}

/// The single `UpdateSubtask` call's captured fields, panicking if none was made.
fn update_subtask_call(calls: &[Call]) -> (String, String, Option<String>, Option<TaskStatus>) {
    calls
        .iter()
        .find_map(|c| match c {
            Call::UpdateSubtask {
                task_id,
                subtask_id,
                title,
                status,
                ..
            } => Some((task_id.clone(), subtask_id.clone(), title.clone(), *status)),
            _ => None,
        })
        .expect("an update_subtask call was made")
}

// ---- two-call tree load + indented render ----

#[test]
fn tree_load_renders_subtasks_indented_under_their_parent() {
    let (_client, app) = logged_in(
        vec![today_open_task("t1", "Parent", "10:00:00")],
        vec![
            open_subtask("s1", "t1", "child one"),
            done_subtask("s2", "t1", "child two"),
        ],
    );

    // Both tree-load calls landed: the pane holds the task and its two sub-tasks.
    let pane = tasks_pane(&app);
    assert_eq!(pane.tasks.len(), 1);
    assert_eq!(pane.subtasks.len(), 2);

    let text = render(&app, W, H);
    // The open parent is expanded by default, so both children render, indented (4 leading spaces).
    assert!(text.contains("> [ ] Parent"), "parent row:\n{text}");
    assert!(
        text.contains("    [ ] child one"),
        "child one indented:\n{text}"
    );
    assert!(
        text.contains("    [x] child two"),
        "done child indented:\n{text}"
    );
}

#[test]
fn an_open_parent_with_subtasks_shows_the_caret_indicator_not_plus() {
    // `+` is reserved for a parent whose sub-tasks are collapsed; an open (expanded) parent keeps
    // the `>` caret even though it has children.
    let (_client, app) = logged_in(
        vec![today_open_task("t1", "Parent", "10:00:00")],
        vec![open_subtask("s1", "t1", "child")],
    );
    let text = render(&app, W, H);
    assert!(
        text.contains("> [ ] Parent"),
        "expanded parent uses `>`:\n{text}"
    );
    assert!(
        !text.contains("+ [ ] Parent"),
        "not `+` while expanded:\n{text}"
    );
}

#[test]
fn a_done_parent_collapses_children_by_default_and_shows_plus() {
    // Collapse default derives from parent status each render: a DONE parent starts collapsed, so
    // its children are hidden and its indicator is `+` (has sub-tasks AND collapsed).
    let (_client, app) = logged_in(
        vec![today_done_task("t1", "Done parent", "10:00:00")],
        vec![open_subtask("s1", "t1", "hidden child")],
    );
    let text = render(&app, W, H);
    assert!(
        text.contains("+ [x] Done parent"),
        "a done parent with sub-tasks shows `+` (collapsed):\n{text}",
    );
    assert!(
        !text.contains("hidden child"),
        "a collapsed parent hides its children:\n{text}",
    );
}

#[test]
fn a_task_with_no_subtasks_uses_the_caret_indicator() {
    let (_client, app) = logged_in(vec![today_open_task("t1", "Lonely", "10:00:00")], vec![]);
    let text = render(&app, W, H);
    assert!(
        text.contains("> [ ] Lonely"),
        "no-children task uses `>`:\n{text}"
    );
    assert!(
        !text.contains("+ [ ] Lonely"),
        "never `+` with no children:\n{text}"
    );
}

// ---- x: collapse / expand override ----

#[test]
fn x_collapses_an_open_parent_and_hides_its_children() {
    let (_client, mut app) = logged_in(
        vec![today_open_task("t1", "Parent", "10:00:00")],
        vec![open_subtask("s1", "t1", "child")],
    );
    // Open parent → expanded by default; the child renders.
    assert!(
        render(&app, W, H).contains("child"),
        "child visible initially"
    );

    // `x` on the selected parent records an override flipping it to collapsed.
    let _ = app.handle_event(Event::ToggleCollapse);
    let text = render(&app, W, H);
    assert!(
        text.contains("+ [ ] Parent"),
        "after x the open parent is collapsed (`+`):\n{text}",
    );
    assert!(
        !text.contains("child"),
        "collapsed child is hidden:\n{text}"
    );
}

#[test]
fn x_expands_a_done_parent_overriding_the_collapsed_default() {
    // A4: a done parent defaults collapsed, but an explicit `x` override expands it (last explicit
    // user intent wins over the status-derived default).
    let (_client, mut app) = logged_in(
        vec![today_done_task("t1", "Done parent", "10:00:00")],
        vec![open_subtask("s1", "t1", "revealed child")],
    );
    assert!(
        !render(&app, W, H).contains("revealed child"),
        "done parent starts collapsed",
    );

    let _ = app.handle_event(Event::ToggleCollapse);
    let text = render(&app, W, H);
    assert!(
        text.contains("> [x] Done parent"),
        "after x the done parent is expanded (`>`):\n{text}",
    );
    assert!(
        text.contains("    [ ] revealed child"),
        "the override reveals the child indented:\n{text}",
    );
}

// ---- A: create sub-task ----

#[test]
fn capital_a_creates_a_subtask_under_the_selected_tasks_parent() {
    let (client, mut app) = logged_in(vec![today_open_task("t1", "Parent", "10:00:00")], vec![]);

    // Script the create response and the chained tree refresh (now showing the new sub-task).
    let created = open_subtask("s1", "t1", "new child");
    client.push_create_subtask(Ok(created.clone()));
    client.push_tasks(Ok(vec![today_open_task("t1", "Parent", "10:00:00")]));
    client.push_list_subtasks(Ok(vec![created]));

    // `A` opens the add-sub-task form (a text-entry sub-flow); type a title and submit.
    let _ = app.handle_event(Event::BeginAddSubtask);
    assert!(
        tasks_pane(&app).adding_subtask.is_some(),
        "A opens the add-sub-task sub-flow",
    );
    type_str(&mut app, "new child");
    submit(&mut app, &client, Event::Submit);

    // The create targeted the selected task's parent (t1) under the active profile, and carried
    // exactly the typed title.
    let calls = client.calls();
    assert!(
        calls.iter().any(|c| matches!(c,
            Call::CreateSubtask { token, profile_id, task_id, title }
                if token == "jwt" && profile_id == "p1" && task_id == "t1" && title == "new child")),
        "A-create posts under the parent task with the typed title: {calls:?}",
    );
    // The form closed and the tree refreshed (two-call: ends with ListSubtasks).
    let pane = tasks_pane(&app);
    assert!(
        pane.adding_subtask.is_none(),
        "the form closed after success"
    );
    assert_eq!(
        pane.subtasks.len(),
        1,
        "the new sub-task shows from the refresh"
    );
    assert!(
        matches!(calls.last(), Some(Call::ListSubtasks { .. })),
        "the refresh ends with the tree-load's ListSubtasks: {calls:?}",
    );
    assert!(
        render(&app, W, H).contains("    [ ] new child"),
        "the new sub-task renders indented from the server response",
    );
}

#[test]
fn a_blank_subtask_title_is_rejected_inline_with_no_request() {
    let (client, mut app) = logged_in(vec![today_open_task("t1", "Parent", "10:00:00")], vec![]);
    let calls_before = client.calls().len();

    let _ = app.handle_event(Event::BeginAddSubtask);
    // Submitting an empty title is rejected locally — no Dispatch, no request crosses the wire.
    assert!(
        app.handle_event(Event::Submit).is_none(),
        "an empty-title sub-task submit dispatches nothing",
    );
    assert_eq!(
        client.calls().len(),
        calls_before,
        "no create_subtask request for a blank title: {:?}",
        client.calls(),
    );
    let add = tasks_pane(&app)
        .adding_subtask
        .as_ref()
        .expect("the form stays open on rejection");
    assert!(
        add.error.as_deref().is_some_and(|m| m.contains("empty")),
        "an inline empty-title error is shown: {:?}",
        add.error,
    );
}

#[test]
fn a_adds_to_the_parent_when_a_subtask_row_is_selected() {
    // `A` always adds to the *parent task* of the selection — pressed on a sub-task row, it targets
    // that sub-task's parent (R2: the selection model knows task vs. sub-task).
    let (client, mut app) = logged_in(
        vec![today_open_task("t1", "Parent", "10:00:00")],
        vec![open_subtask("s1", "t1", "existing")],
    );
    // Move selection from the parent (row 0) to its sub-task (row 1).
    let _ = app.handle_event(Event::Next);
    assert_eq!(
        tasks_pane(&app).selected_row(current_day_number()),
        Some(VisibleRow::Subtask { subtask_idx: 0 }),
        "the sub-task row is selected",
    );

    let created = open_subtask("s2", "t1", "added");
    client.push_create_subtask(Ok(created.clone()));
    client.push_tasks(Ok(vec![today_open_task("t1", "Parent", "10:00:00")]));
    client.push_list_subtasks(Ok(vec![open_subtask("s1", "t1", "existing"), created]));

    let _ = app.handle_event(Event::BeginAddSubtask);
    type_str(&mut app, "added");
    submit(&mut app, &client, Event::Submit);

    let calls = client.calls();
    assert!(
        calls.iter().any(|c| matches!(c,
            Call::CreateSubtask { task_id, title, .. } if task_id == "t1" && title == "added")),
        "A on a sub-task row adds under that sub-task's parent (t1): {calls:?}",
    );
}

// ---- e: edit a sub-task's title ----

#[test]
fn e_on_a_subtask_row_edits_its_title() {
    let (client, mut app) = logged_in(
        vec![today_open_task("t1", "Parent", "10:00:00")],
        vec![open_subtask("s1", "t1", "old title")],
    );
    // Select the sub-task row.
    let _ = app.handle_event(Event::Next);

    // `e` opens the edit-sub-task form, pre-filled with the current title.
    let _ = app.handle_event(Event::BeginEditTask);
    let prefill = tasks_pane(&app)
        .editing_subtask
        .as_ref()
        .expect("the edit-sub-task sub-flow is open")
        .title
        .as_str()
        .to_owned();
    assert_eq!(prefill, "old title", "edit pre-fills the sub-task's title");

    // Script the patch response + the chained tree refresh.
    let renamed = open_subtask("s1", "t1", "new title");
    client.push_update_subtask(Ok(renamed.clone()));
    client.push_tasks(Ok(vec![today_open_task("t1", "Parent", "10:00:00")]));
    client.push_list_subtasks(Ok(vec![renamed]));

    // Clear "old title" (9 chars), type the new one, submit.
    for _ in 0.."old title".len() {
        let _ = app.handle_event(Event::Backspace);
    }
    type_str(&mut app, "new title");
    submit(&mut app, &client, Event::Submit);

    // The issued patch is title-only and targets the right sub-task under the right parent.
    let (task_id, subtask_id, title, status) = update_subtask_call(&client.calls());
    assert_eq!(task_id, "t1");
    assert_eq!(subtask_id, "s1");
    assert_eq!(
        title,
        Some("new title".to_owned()),
        "edit sends the new title"
    );
    assert_eq!(status, None, "edit-title sends no status");

    let text = render(&app, W, H);
    assert!(
        text.contains("new title"),
        "the renamed sub-task renders:\n{text}"
    );
    assert!(!text.contains("old title"), "old title gone:\n{text}");
}

// ---- Space: toggle a sub-task's status ----

#[test]
fn space_on_a_subtask_row_toggles_its_status() {
    let (client, mut app) = logged_in(
        vec![today_open_task("t1", "Parent", "10:00:00")],
        vec![open_subtask("s1", "t1", "child")],
    );
    let _ = app.handle_event(Event::Next); // select the sub-task row
    assert!(
        render(&app, W, H).contains("    [ ] child"),
        "child starts open"
    );

    // Toggle returns the done sub-task; the chained refresh shows it done.
    let done = done_subtask("s1", "t1", "child");
    client.push_update_subtask(Ok(done.clone()));
    client.push_tasks(Ok(vec![today_open_task("t1", "Parent", "10:00:00")]));
    client.push_list_subtasks(Ok(vec![done]));

    submit(&mut app, &client, Event::ToggleDone);

    // The patch is status-only (no title) and targets the sub-task.
    let (task_id, subtask_id, title, status) = update_subtask_call(&client.calls());
    assert_eq!((task_id.as_str(), subtask_id.as_str()), ("t1", "s1"));
    assert_eq!(title, None, "toggle sends no title");
    assert_eq!(status, Some(TaskStatus::Done), "toggle sends status=done");

    assert!(
        render(&app, W, H).contains("    [x] child"),
        "the sub-task renders done after the refresh",
    );
}

#[test]
fn space_on_a_task_row_still_toggles_the_task_not_a_subtask() {
    // Routing: with a TASK row selected, Space toggles the task (an UpdateTask), not a sub-task.
    let (client, mut app) = logged_in(
        vec![today_open_task("t1", "Parent", "10:00:00")],
        vec![open_subtask("s1", "t1", "child")],
    );
    // The parent task row (row 0) is selected by default.
    let done = today_done_task("t1", "Parent", "10:00:00");
    client.push_update(Ok(done.clone()));
    client.push_tasks(Ok(vec![done]));
    client.push_list_subtasks(Ok(vec![open_subtask("s1", "t1", "child")]));

    submit(&mut app, &client, Event::ToggleDone);

    let calls = client.calls();
    assert!(
        calls
            .iter()
            .any(|c| matches!(c, Call::UpdateTask { task_id, status, .. }
            if task_id == "t1" && *status == Some(TaskStatus::Done))),
        "Space on a task row toggles the TASK (UpdateTask, not UpdateSubtask): {calls:?}",
    );
    assert!(
        !calls
            .iter()
            .any(|c| matches!(c, Call::UpdateSubtask { .. })),
        "no sub-task patch issued for a task-row toggle: {calls:?}",
    );
}

// ---- selection traversal over interleaved rows, incl. across a collapsed parent (R2) ----

#[test]
fn selection_traverses_task_and_subtask_rows_in_visible_order() {
    let (_client, mut app) = logged_in(
        vec![
            today_open_task("t1", "First", "12:00:00"),
            today_open_task("t2", "Second", "11:00:00"),
        ],
        vec![
            open_subtask("s1", "t1", "first-child"),
            open_subtask("s2", "t2", "second-child"),
        ],
    );
    // Visible row order (both parents open/expanded): t1, s1, t2, s2.
    let expected = vec![
        VisibleRow::Task { task_idx: 0 },
        VisibleRow::Subtask { subtask_idx: 0 },
        VisibleRow::Task { task_idx: 1 },
        VisibleRow::Subtask { subtask_idx: 1 },
    ];
    assert_eq!(
        tasks_pane(&app).visible_rows(current_day_number()),
        expected,
        "interleaved visible rows"
    );

    // Down/Next walks task → its sub-task → next task → its sub-task.
    let first = expected.first().copied();
    assert_eq!(tasks_pane(&app).selected_row(current_day_number()), first);
    for (step, want) in expected.iter().enumerate().skip(1) {
        let _ = app.handle_event(Event::Next);
        assert_eq!(
            tasks_pane(&app).selected_row(current_day_number()),
            Some(*want),
            "Next lands on visible row {step}",
        );
    }
    // Wrap back to the first row.
    let _ = app.handle_event(Event::Next);
    assert_eq!(
        tasks_pane(&app).selected_row(current_day_number()),
        first,
        "wraps to the top"
    );
}

#[test]
fn selection_skips_the_hidden_children_of_a_collapsed_parent() {
    // R2: with the first parent collapsed, its sub-task is NOT a visible row — Next jumps straight
    // from the collapsed parent to the next task, never landing on a hidden child.
    let (_client, mut app) = logged_in(
        vec![
            today_open_task("t1", "First", "12:00:00"),
            today_open_task("t2", "Second", "11:00:00"),
        ],
        vec![
            open_subtask("s1", "t1", "hidden-child"),
            open_subtask("s2", "t2", "visible-child"),
        ],
    );
    // Collapse the first parent (it is selected at row 0).
    let _ = app.handle_event(Event::ToggleCollapse);

    // Visible rows now: t1 (collapsed), t2, s2 — t1's child s1 is hidden.
    assert_eq!(
        tasks_pane(&app).visible_rows(current_day_number()),
        vec![
            VisibleRow::Task { task_idx: 0 },
            VisibleRow::Task { task_idx: 1 },
            VisibleRow::Subtask { subtask_idx: 1 },
        ],
        "the collapsed parent's child is not a visible row",
    );
    assert!(
        !render(&app, W, H).contains("hidden-child"),
        "the collapsed parent's child is not rendered",
    );

    // From the collapsed parent (row 0), Next lands on the next TASK, skipping the hidden child.
    let _ = app.handle_event(Event::Next);
    assert_eq!(
        tasks_pane(&app).selected_row(current_day_number()),
        Some(VisibleRow::Task { task_idx: 1 }),
        "Next from a collapsed parent skips its hidden child to the next task",
    );
    // The next visible row after that is t2's (visible) child.
    let _ = app.handle_event(Event::Next);
    assert_eq!(
        tasks_pane(&app).selected_row(current_day_number()),
        Some(VisibleRow::Subtask { subtask_idx: 1 }),
    );
}

// ---- Task Detail "Sub-tasks" section ----

#[test]
fn opening_task_detail_loads_and_renders_the_subtasks_section() {
    let (client, mut app) = logged_in(
        vec![today_open_task("t1", "Parent", "10:00:00")],
        vec![open_subtask("s1", "t1", "tree child")],
    );
    // The detail open chains a per-task ListTaskSubtasks for the read-only section (A6); script it.
    client.push_list_task_subtasks(Ok(vec![
        open_subtask("s1", "t1", "detail child A"),
        done_subtask("s2", "t1", "detail child B"),
    ]));

    submit(&mut app, &client, Event::Submit); // Enter opens the parent task's detail
    assert!(
        tasks_pane(&app).detail.is_some(),
        "the task detail view is open"
    );

    // The chained per-task list was issued for the selected task.
    let calls = client.calls();
    assert!(
        calls.iter().any(|c| matches!(c,
            Call::ListTaskSubtasks { task_id, profile_id, .. }
                if task_id == "t1" && profile_id == "p1")),
        "opening the detail loads the parent's sub-tasks for the section: {calls:?}",
    );

    let text = render(&app, W, H);
    assert!(
        text.contains("Sub-tasks"),
        "the detail has a Sub-tasks section:\n{text}"
    );
    assert!(
        text.contains("[ ] detail child A"),
        "open sub-task listed:\n{text}"
    );
    assert!(
        text.contains("[x] detail child B"),
        "done sub-task listed:\n{text}"
    );
}

#[test]
fn a_task_with_no_subtasks_shows_an_empty_detail_section() {
    let (client, mut app) = logged_in(vec![today_open_task("t1", "Lonely", "10:00:00")], vec![]);
    client.push_list_task_subtasks(Ok(vec![]));

    submit(&mut app, &client, Event::Submit);
    let text = render(&app, W, H);
    assert!(
        text.contains("Sub-tasks"),
        "the section is present:\n{text}"
    );
    assert!(
        text.contains("(no sub-tasks)"),
        "an empty section shows the placeholder:\n{text}",
    );
}
