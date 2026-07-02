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
//! - delete is the 0015 confirm dialog (0016 Assumption A5, retiring the old `x`-again two-step):
//!   `d` (`DeleteSelected`) arms the dialog and issues no request, `Enter` (`Submit`) issues
//!   `DeleteTask` and the row is removed after the refresh, `Esc` (`Cancel`) disarms.
//!
//! The 0020 block at the bottom pins the tasks-pane render overhaul (completed-last, today/older
//! split, `h`-hide, `limit=200`) and its 2026-07-02 amendment: the today date + "Older tasks"
//! rows are full-width in-pane separators (item 1), a sub-task row's `d` arms a
//! `DeleteTarget::Subtask` and confirms into `DeleteSubtask` (item 2 / acceptance #7), and the
//! older group defaults collapsed but is `x`-toggleable like the today group (item 3).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

mod common;

use common::{
    Call, FakeClient, done_subtask, open_subtask, open_task, profile, render, session, submit,
    tasks_pane, today_at, today_done_task, today_open_task,
};
use contract::{Subtask, TaskStatus};
use tui::app::{
    App, DeleteTarget, Event, OLDER_SEPARATOR_LABEL, Screen, TASK_LIST_LIMIT, VisibleRow,
    current_day_number,
};

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
    let (client, mut app) = logged_in(vec![today_open_task("t1", "Write tests", "10:00:00")]);
    let before = render(&app, W, H);
    assert!(before.contains("[ ] Write tests"), "starts open:\n{before}");

    // The update returns the done task; the success chains a refresh list.
    let done = today_done_task("t1", "Write tests", "10:00:00");
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
    // `Space` (0016 remap, was `c`) reopens, issuing `status: open`, and the done marker clears
    // once the refresh lands.
    let (client, mut app) = logged_in(vec![today_done_task("t1", "Write tests", "10:00:00")]);
    let before = render(&app, W, H);
    assert!(before.contains("[x] Write tests"), "starts done:\n{before}");

    // Reopen returns the task open again (closed_at cleared server-side); refresh shows it open.
    let reopened = today_open_task("t1", "Write tests", "10:00:00");
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
    let (client, mut app) = logged_in(vec![today_open_task("t1", "old title", "10:00:00")]);

    // Script the update response (renamed task) and the chained refresh list.
    let renamed = today_open_task("t1", "new title", "10:00:00");
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
    // The refresh is the two-call tree load (ListTasks → ListSubtasks, 0019); statelessness #1.
    assert!(
        matches!(calls.last(), Some(Call::ListSubtasks { .. })),
        "the two-call tree refresh ends with a ListSubtasks: {calls:?}",
    );
    assert!(
        calls.iter().any(|c| matches!(c, Call::ListTasks { .. })),
        "a fresh task list fetch follows the edit (statelessness): {calls:?}",
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
    let (client, mut app) = logged_in(vec![today_open_task("t1", "keep me", "10:00:00")]);
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

// ---- delete (0015 confirm dialog; 0016 Assumption A5) ----

#[test]
fn delete_key_arms_confirm_and_issues_no_request() {
    let (client, mut app) = logged_in(vec![today_open_task("t1", "doomed", "10:00:00")]);
    let calls_before = client.calls().len();

    // `d` (`DeleteSelected`) arms the confirmation only — no request, no Dispatch, the row present.
    assert!(
        app.handle_event(Event::DeleteSelected).is_none(),
        "the delete key arms the confirm, dispatching nothing",
    );
    assert_eq!(
        client.calls().len(),
        calls_before,
        "no delete request on the arming key: {:?}",
        client.calls(),
    );

    let list = tasks_pane(&app);
    assert!(
        matches!(
            list.confirming_delete,
            Some(DeleteTarget::Task { ref task_id }) if task_id == "t1"
        ),
        "the confirm is armed against the selected task (DeleteTarget::Task): {:?}",
        list.confirming_delete,
    );
    assert_eq!(list.tasks.len(), 1, "the row is still present while armed");

    // 0016 (Assumption A5): the armed delete renders as the 0015 centred confirmation dialog,
    // confirmed via `Enter` and cancelled via `Esc` (the old `x`-again two-step is retired).
    let text = render(&app, W, H);
    assert!(
        text.contains("Delete task") && text.contains("Delete this task?"),
        "the confirmation dialog is shown:\n{text}",
    );
    assert!(
        text.contains("Enter: confirm delete") && text.contains("Esc: cancel"),
        "the dialog hint reflects the Enter-confirms / Esc-cancels affordance:\n{text}",
    );
}

#[test]
fn enter_confirms_delete_request_and_row_is_removed_after_refresh() {
    let (client, mut app) = logged_in(vec![
        today_open_task("t1", "doomed", "12:00:00"),
        today_open_task("t2", "survivor", "11:00:00"),
    ]);

    // Arm the confirm (`d`).
    assert!(app.handle_event(Event::DeleteSelected).is_none());

    // Script the delete (204, no body) and the chained refresh (the row gone server-side).
    client.push_delete(Ok(()));
    client.push_tasks(Ok(vec![today_open_task("t2", "survivor", "11:00:00")]));

    // `Enter` (`Submit`) confirms and issues the delete.
    submit(&mut app, &client, Event::Submit);

    // The delete targeted the selected task under the active profile, followed by a refresh.
    let calls = client.calls();
    assert!(
        calls.iter().any(|c| matches!(c,
            Call::DeleteTask { token, profile_id, task_id }
                if token == "jwt" && profile_id == "p1" && task_id == "t1")),
        "delete targeted the right task: {calls:?}",
    );
    // The refresh is the two-call tree load (ListTasks → ListSubtasks, 0019); statelessness #1.
    assert!(
        matches!(calls.last(), Some(Call::ListSubtasks { .. })),
        "the two-call tree refresh ends with a ListSubtasks: {calls:?}",
    );
    assert!(
        calls.iter().any(|c| matches!(c, Call::ListTasks { .. })),
        "a fresh task list fetch follows the delete (statelessness): {calls:?}",
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
fn esc_cancels_the_delete_confirm_with_no_request() {
    let (client, mut app) = logged_in(vec![today_open_task("t1", "doomed", "10:00:00")]);

    // Arm, then `Esc` (`Cancel`) — the confirm disarms and no delete issues (the dialog captures
    // input: only `Enter` confirms and `Esc` cancels, mirroring the notes/profiles confirm dialog).
    assert!(app.handle_event(Event::DeleteSelected).is_none());
    let _ = app.handle_event(Event::Cancel);

    assert!(
        tasks_pane(&app).confirming_delete.is_none(),
        "Esc disarms the confirm",
    );

    // A subsequent `d` only re-arms (it does not delete) — proving the disarm took effect.
    assert!(
        app.handle_event(Event::DeleteSelected).is_none(),
        "after cancel, the next delete key re-arms rather than deleting",
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
        let (_client, app) = logged_in(vec![today_open_task("t1", "doomed", "10:00:00")]);
        app
    };

    // Arm (`d`) then confirm (`Enter`), holding the dispatch (never driving it) so the app sits
    // in-flight.
    assert!(app.handle_event(Event::DeleteSelected).is_none());
    let _dispatch = app
        .handle_event(Event::Submit)
        .expect("Enter on the armed confirm dispatches a delete request");
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

// ============================================================================
// 0020 — tasks-pane render overhaul (completed-last, today/older, h-hide, limit)
// ============================================================================

/// Like [`logged_in`] but also scripts the chained `ListSubtasks` tree-load response, so a test can
/// exercise the sub-task render/sort. Returns the handle + the settled app on the Tasks tab.
fn logged_in_tree(tasks: Vec<contract::Task>, subtasks: Vec<Subtask>) -> (FakeClient, App) {
    let client = FakeClient::new();
    client.push_login(Ok(session("jwt")));
    client.push_profiles(Ok(vec![profile("p1", "work")]));
    client.push_tasks(Ok(tasks));
    client.push_list_subtasks(Ok(subtasks));
    let mut app = App::new();
    submit(&mut app, &client, Event::Submit);
    assert!(
        matches!(app.screen(), Screen::Main(_)),
        "precondition: logged in to the Tasks tab with the tree loaded",
    );
    (client, app)
}

/// The byte offset of the first occurrence of `needle` in `haystack`, panicking if absent.
fn pos(haystack: &str, needle: &str) -> usize {
    haystack
        .find(needle)
        .unwrap_or_else(|| panic!("expected {needle:?} in:\n{haystack}"))
}

/// The content of a bordered list row between its outer `│` border columns (exclusive), i.e. the
/// pane's inner (content) cells for that row. The Tasks pane does not begin at column 0 (it sits in
/// a layout with a leading margin), so a separator row reads e.g. ` │──── label ────│`; this returns
/// just the `──── label ────` inner span so a test can assert it spans the full inner width.
fn bordered_inner(row: &str) -> &str {
    let start = row
        .find('│')
        .unwrap_or_else(|| panic!("no left border in row:\n{row}"));
    let end = row
        .rfind('│')
        .unwrap_or_else(|| panic!("no right border in row:\n{row}"));
    assert!(
        end > start,
        "a bordered row has two distinct borders:\n{row}"
    );
    // Slice between the border chars. `│` is 3 bytes in UTF-8; step past the left one.
    &row[start + '│'.len_utf8()..end]
}

// ---- completed-last ordering (task level) ----

#[test]
fn open_tasks_render_before_done_tasks_regardless_of_server_order() {
    // The server returns tasks newest-first (ADR-0005 §5); the TUI applies a stable completed-last
    // sort on top (ADR-0014 §4). Given a done task newest and an open task older, the open task
    // still renders ABOVE the done one — sort is by status, not created-at.
    let (_client, app) = logged_in(vec![
        today_done_task("t-done", "finished", "12:00:00"),
        today_open_task("t-open", "in progress", "10:00:00"),
    ]);
    let text = render(&app, W, H);
    assert!(
        pos(&text, "in progress") < pos(&text, "finished"),
        "the open task renders above the newer done task (completed-last):\n{text}",
    );
}

#[test]
fn completed_last_re_sorts_after_a_toggle_with_no_extra_list_fetch() {
    // Acceptance #1: the ordering re-sorts immediately on a state change with NO manual refresh.
    // The success of a toggle folds the returned DTO back in and re-derives the render; the row
    // order flips on the next frame. The only list fetch that follows is the standard post-mutation
    // refresh chain (ListTasks → ListSubtasks) — there is no *additional* re-fetch driven purely to
    // re-sort (the sort is a pure render-time derivation of the held snapshot).
    let (client, mut app) = logged_in(vec![
        today_open_task("t-a", "alpha", "12:00:00"),
        today_open_task("t-b", "bravo", "10:00:00"),
    ]);
    // Both open: alpha (newer) renders above bravo.
    let before = render(&app, W, H);
    assert!(
        pos(&before, "alpha") < pos(&before, "bravo"),
        "both open, newest-first:\n{before}",
    );

    // Toggle the selected (alpha, row 0) to done; its success chains the two-call refresh.
    let done_alpha = today_done_task("t-a", "alpha", "12:00:00");
    client.push_update(Ok(done_alpha.clone()));
    client.push_tasks(Ok(vec![
        done_alpha,
        today_open_task("t-b", "bravo", "10:00:00"),
    ]));
    let list_fetches_before = client
        .calls()
        .iter()
        .filter(|c| matches!(c, Call::ListTasks { .. }))
        .count();
    submit(&mut app, &client, Event::ToggleDone);

    // After the refresh, alpha is done and sinks BELOW the still-open bravo — completed-last.
    let after = render(&app, W, H);
    assert!(
        pos(&after, "bravo") < pos(&after, "alpha"),
        "the now-done alpha sinks below the open bravo after the toggle:\n{after}",
    );

    // Exactly one further ListTasks (the post-mutation refresh) — no extra re-sort fetch.
    let list_fetches_after = client
        .calls()
        .iter()
        .filter(|c| matches!(c, Call::ListTasks { .. }))
        .count();
    assert_eq!(
        list_fetches_after - list_fetches_before,
        1,
        "only the single post-mutation refresh fetched the list; the re-sort needs no re-fetch: {:?}",
        client.calls(),
    );
}

// ---- completed-last ordering (sub-task level) ----

#[test]
fn open_subtasks_render_before_done_subtasks_under_their_parent() {
    // Completed-last applies within a parent too (ADR-0014 §4): an open sub-task renders above a
    // done one regardless of the server's creation order. The parent is created today (expanded).
    let (_client, app) = logged_in_tree(
        vec![today_open_task("t1", "Parent", "10:00:00")],
        vec![
            done_subtask("s-done", "t1", "child done"),
            open_subtask("s-open", "t1", "child open"),
        ],
    );
    let text = render(&app, W, H);
    assert!(
        pos(&text, "child open") < pos(&text, "child done"),
        "the open sub-task renders above the done sub-task (completed-last):\n{text}",
    );
}

// ---- today / older split + "Older tasks" separator ----

#[test]
fn tasks_split_into_today_above_and_older_below_the_separator() {
    // Acceptance #3: created-today tasks render above, the "Older tasks" separator between, older
    // tasks below. "Today" is the wall-clock civil day; `today_at` builds today's timestamps and a
    // fixed past date is older.
    let (_client, app) = logged_in(vec![
        today_open_task("t-today", "fresh task", "10:00:00"),
        open_task("t-old", "stale task", "2020-01-02T10:00:00Z"),
    ]);
    let text = render(&app, W, H);
    let p_today = pos(&text, "fresh task");
    let p_sep = pos(&text, OLDER_SEPARATOR_LABEL);
    let p_old = pos(&text, "stale task");
    assert!(
        p_today < p_sep && p_sep < p_old,
        "today above, separator between, older below:\n{text}",
    );
}

#[test]
fn older_task_starts_collapsed_then_x_expands_and_x_again_collapses() {
    // Acceptance #3 (amended 2026-07-02, item 3): an older-group task with sub-tasks **defaults to
    // collapsed on load** (sub-tasks hidden, `+` indicator), but `x` (`ToggleCollapse`) now toggles
    // it just like a today task — `x` emits its `VisibleRow::Subtask` rows, `x` again re-collapses.
    // (Previously the older group was hard-forced collapsed and `x` was inert; that is retired.)
    let (_client, mut app) = logged_in_tree(
        vec![
            today_open_task("t-today", "today task", "10:00:00"),
            open_task("t-old", "older open parent", "2020-01-02T10:00:00Z"),
        ],
        vec![open_subtask("s1", "t-old", "older child")],
    );
    let today_day = current_day_number();

    // On load: the older task is collapsed — no Subtask row, and the render shows `+` and hides the
    // child.
    assert!(
        !tasks_pane(&app)
            .visible_rows(today_day)
            .iter()
            .any(|r| matches!(r, VisibleRow::Subtask { .. })),
        "older task starts collapsed: no sub-task row in visible_rows",
    );
    let collapsed = render(&app, W, H);
    assert!(
        collapsed.contains("+ [ ] older open parent"),
        "an older task starts collapsed (`+`):\n{collapsed}",
    );
    assert!(
        !collapsed.contains("older child"),
        "the older task's sub-task is hidden while collapsed:\n{collapsed}",
    );

    // Select the older task (row after the today task + the separator), then `x` to expand it. The
    // sub-task row now appears in visible_rows and the child renders.
    let _ = app.handle_event(Event::Next); // today task -> (skips separator) -> older task
    assert_eq!(
        tasks_pane(&app).selected_row(today_day),
        Some(VisibleRow::Task { task_idx: 1 }),
        "selection landed on the older task (separator skipped)",
    );
    let _ = app.handle_event(Event::ToggleCollapse);
    assert!(
        tasks_pane(&app)
            .visible_rows(today_day)
            .iter()
            .any(|r| matches!(r, VisibleRow::Subtask { .. })),
        "after x the older task expands: a sub-task row appears",
    );
    let expanded = render(&app, W, H);
    assert!(
        expanded.contains("> [ ] older open parent") && expanded.contains("older child"),
        "the older task's sub-task renders after x expands it:\n{expanded}",
    );

    // `x` again re-collapses it.
    let _ = app.handle_event(Event::ToggleCollapse);
    assert!(
        !tasks_pane(&app)
            .visible_rows(today_day)
            .iter()
            .any(|r| matches!(r, VisibleRow::Subtask { .. })),
        "a second x re-collapses the older task (no sub-task row)",
    );
}

#[test]
fn today_group_and_completed_last_ordering_are_unchanged_by_the_older_toggle() {
    // Item 3 confirmation: the today group's own collapse behaviour (open expanded, done collapsed)
    // and the completed-last ordering are unaffected by the older-group amendment. A today open
    // parent shows its sub-task expanded (`>`); a done today parent collapses; and open renders
    // before done.
    let (_client, app) = logged_in_tree(
        vec![
            today_open_task("t-open", "today open", "12:00:00"),
            today_done_task("t-done", "today done", "10:00:00"),
        ],
        vec![
            open_subtask("s-open", "t-open", "open child"),
            open_subtask("s-done", "t-done", "done-parent child"),
        ],
    );
    let text = render(&app, W, H);
    // Completed-last: the open today task renders above the done today task.
    assert!(
        pos(&text, "today open") < pos(&text, "today done"),
        "today open renders above today done (completed-last):\n{text}",
    );
    // The open today parent is expanded (child visible); the done today parent is collapsed.
    assert!(
        text.contains("> [ ] today open") && text.contains("open child"),
        "an open today task stays expanded:\n{text}",
    );
    assert!(
        text.contains("+ [x] today done") && !text.contains("done-parent child"),
        "a done today task stays collapsed:\n{text}",
    );
}

#[test]
fn x_on_an_older_task_does_not_affect_another_tasks_collapse_state() {
    // Assumption A7 (no override bleed): the older-group default is render-time-derived, not a
    // mutation of `collapse_overrides`, so pressing `x` on one older task must not change any other
    // task's collapse state. Two older parents each with a sub-task; expanding the first leaves the
    // second collapsed.
    let (_client, mut app) = logged_in_tree(
        vec![
            today_open_task("t-today", "today task", "12:00:00"),
            open_task("t-old-a", "older A", "2020-01-02T10:00:00Z"),
            open_task("t-old-b", "older B", "2020-01-01T10:00:00Z"),
        ],
        vec![
            open_subtask("sa", "t-old-a", "child A"),
            open_subtask("sb", "t-old-b", "child B"),
        ],
    );

    // Select and expand the first older task (older A): today -> older A.
    let _ = app.handle_event(Event::Next);
    let today_day = current_day_number();
    assert_eq!(
        tasks_pane(&app).selected_row(today_day),
        Some(VisibleRow::Task { task_idx: 1 }),
        "selection on older A",
    );
    let _ = app.handle_event(Event::ToggleCollapse);

    let text = render(&app, W, H);
    // Older A expanded (child A visible); older B still collapsed (child B hidden, `+`).
    assert!(
        text.contains("> [ ] older A") && text.contains("child A"),
        "older A expands on x:\n{text}",
    );
    assert!(
        text.contains("+ [ ] older B") && !text.contains("child B"),
        "older B is untouched by x on older A (no override bleed, A7):\n{text}",
    );
}

// ---- h: hide / show the older group + separator ----

#[test]
fn h_toggles_the_older_group_and_separator_visibility() {
    // Acceptance #4: default shown; `h` hides the older group AND its separator; `h` again shows
    // them. Selection stays clamped onto a selectable row across the toggle.
    let (_client, mut app) = logged_in(vec![
        today_open_task("t-today", "today task", "10:00:00"),
        open_task("t-old", "older task", "2020-01-02T10:00:00Z"),
    ]);
    let shown = render(&app, W, H);
    assert!(
        shown.contains(OLDER_SEPARATOR_LABEL) && shown.contains("older task"),
        "default: older group + separator shown:\n{shown}",
    );

    // `h` hides them.
    let _ = app.handle_event(Event::ToggleHideOlder);
    let hidden = render(&app, W, H);
    assert!(
        !hidden.contains(OLDER_SEPARATOR_LABEL) && !hidden.contains("older task"),
        "after h: the older group and its separator are hidden:\n{hidden}",
    );
    assert!(
        hidden.contains("today task"),
        "the today group is unaffected by hiding older:\n{hidden}",
    );

    // `h` again shows them.
    let _ = app.handle_event(Event::ToggleHideOlder);
    let shown_again = render(&app, W, H);
    assert!(
        shown_again.contains(OLDER_SEPARATOR_LABEL) && shown_again.contains("older task"),
        "after a second h: shown again:\n{shown_again}",
    );
}

#[test]
fn selection_and_visible_rows_skip_the_hidden_older_rows() {
    // When the older group is hidden, `visible_rows` drops the separator and older tasks, and the
    // selection cursor never lands on a hidden row.
    let (_client, mut app) = logged_in(vec![
        today_open_task("t-today", "today task", "10:00:00"),
        open_task("t-old", "older task", "2020-01-02T10:00:00Z"),
    ]);
    let today_day = current_day_number();

    // Shown: today task, separator, older task.
    assert_eq!(
        tasks_pane(&app).visible_rows(today_day),
        vec![
            VisibleRow::Task { task_idx: 0 },
            VisibleRow::OlderSeparator,
            VisibleRow::Task { task_idx: 1 },
        ],
        "shown: today row, separator, older row",
    );

    // Hide the older group; only the today task remains a visible row (no separator, no older).
    let _ = app.handle_event(Event::ToggleHideOlder);
    assert_eq!(
        tasks_pane(&app).visible_rows(today_day),
        vec![VisibleRow::Task { task_idx: 0 }],
        "hidden: only the today task is a visible row",
    );
    assert_eq!(
        tasks_pane(&app).selected_row(today_day),
        Some(VisibleRow::Task { task_idx: 0 }),
        "the selection clamps onto the sole remaining (today) row",
    );
}

// ---- today date header: Tasks pane only ----

#[test]
fn today_date_header_renders_as_a_full_width_in_pane_separator_row() {
    // Acceptance #2 (amended 2026-07-02, item 1): the today date renders as a full-width, `─`-padded
    // separator row INSIDE the bordered Tasks pane (no longer a short centered line above the
    // border). Assert (a) the header text is present, (b) the header ROW spans the pane's full inner
    // width (bordered on both sides, `─`-filled), and (c) it is the first list row inside the border.
    let (_client, app) = logged_in(vec![today_open_task("t1", "task", "10:00:00")]);
    let header = tui::ui::today_header(current_day_number());
    let text = render(&app, W, H);
    assert!(
        text.contains(&header),
        "the today date header {header:?} renders in the Tasks pane:\n{text}",
    );
    // The row carrying the header is a bordered list row; its inner span (between the │ borders) is
    // `─`-filled edge-to-edge around the centered label — the full pane inner width, not a short
    // centered line. Its width matches a plain task row's inner width (both span the same pane).
    let header_row = text
        .lines()
        .find(|l| l.contains(&header))
        .expect("a rendered row carries the header");
    let task_row = text
        .lines()
        .find(|l| l.contains("task") && !l.contains(&header))
        .expect("a rendered task row");
    let header_inner = bordered_inner(header_row);
    assert!(
        header_inner.starts_with('─') && header_inner.trim_end().ends_with('─'),
        "the header inner span is `─`-filled edge-to-edge (full-width separator), not a short line:\n{header_inner:?}",
    );
    assert_eq!(
        header_inner.chars().count(),
        bordered_inner(task_row).chars().count(),
        "the header separator row spans the same inner width as a task row (full pane width)",
    );
}

#[test]
fn today_date_header_row_is_not_a_selection_target() {
    // Acceptance #2: the date row is non-selectable — the selection highlight (REVERSED) lands on
    // the first task, never on the date row. The date row is prepended at draw and is not part of
    // `visible_rows`, so `selected` (index 0) points at the first task.
    let (_client, app) = logged_in(vec![today_open_task("t1", "first task", "10:00:00")]);
    let header = tui::ui::today_header(current_day_number());
    let buffer = common::render_buffer(&app, W, H);
    let flat = render(&app, W, H);

    // The reversed (selection) highlight row must be the first task's row, not the date row.
    let header_row_idx = flat
        .lines()
        .position(|l| l.contains(&header))
        .expect("header row present");
    let task_row_idx = flat
        .lines()
        .position(|l| l.contains("first task"))
        .expect("task row present");
    assert!(
        task_row_idx > header_row_idx,
        "the first task row is below the date row",
    );

    // Count reversed cells on the date row vs the task row: the highlight is on the task row.
    let reversed_on = |row_idx: usize| -> usize {
        let y = u16::try_from(row_idx).expect("row fits u16");
        (0..W)
            .filter(|&x| {
                buffer[(x, y)]
                    .modifier
                    .contains(ratatui::style::Modifier::REVERSED)
            })
            .count()
    };
    assert_eq!(
        reversed_on(header_row_idx),
        0,
        "the date row carries no selection highlight (non-selectable)",
    );
    assert!(
        reversed_on(task_row_idx) > 0,
        "the selection highlight lands on the first task row",
    );
}

#[test]
fn older_tasks_separator_spans_the_full_pane_inner_width() {
    // Acceptance #3: the "Older tasks" separator is a full-width `─`-padded row (like the date row),
    // not the bare short `── Older tasks ──`. Assert its rendered row spans the full pane width.
    let (_client, app) = logged_in(vec![
        today_open_task("t-today", "fresh", "10:00:00"),
        open_task("t-old", "stale", "2020-01-02T10:00:00Z"),
    ]);
    let text = render(&app, W, H);
    let sep_row = text
        .lines()
        .find(|l| l.contains(OLDER_SEPARATOR_LABEL))
        .expect("the Older tasks separator row renders");
    let task_row = text
        .lines()
        .find(|l| l.contains("fresh"))
        .expect("a rendered task row");
    let sep_inner = bordered_inner(sep_row);
    assert!(
        sep_inner.starts_with('─') && sep_inner.trim_end().ends_with('─'),
        "the Older tasks separator inner span is `─`-filled edge-to-edge (full width):\n{sep_inner:?}",
    );
    assert_eq!(
        sep_inner.chars().count(),
        bordered_inner(task_row).chars().count(),
        "the Older tasks separator spans the same inner width as a task row (full pane width)",
    );
    assert!(
        sep_inner.contains('─') && sep_inner.contains(OLDER_SEPARATOR_LABEL),
        "the separator is `─`-filled around the centered label, not the bare short form:\n{sep_inner:?}",
    );
}

#[test]
fn today_date_header_is_absent_from_notes_and_profiles_panes() {
    // The date header is a Tasks-pane concept only (acceptance #2): it must NOT render on the Notes
    // or Profiles panes.
    let header = tui::ui::today_header(current_day_number());
    let (client, mut app) = logged_in(vec![today_open_task("t1", "task", "10:00:00")]);

    // Notes tab: switch and load an empty notes list.
    client.push_notes(Ok(vec![]));
    submit(&mut app, &client, Event::NextTab); // Tasks -> Notes
    let notes_text = render(&app, W, H);
    assert!(
        !notes_text.contains(&header),
        "the date header must not render on the Notes pane:\n{notes_text}",
    );

    // Profiles tab (switching re-lists the profiles).
    client.push_profiles(Ok(vec![profile("p1", "work")]));
    submit(&mut app, &client, Event::NextTab); // Notes -> Profiles
    let profiles_text = render(&app, W, H);
    assert!(
        !profiles_text.contains(&header),
        "the date header must not render on the Profiles pane:\n{profiles_text}",
    );
}

// ---- ordinal / date helpers (R5) ----

#[test]
fn ordinal_suffix_covers_the_teens_exception_and_the_common_cases() {
    // The 11-13 → `th` exception plus the st/nd/rd/th cases (ADR-0014 R5).
    assert_eq!(tui::ui::ordinal_suffix(1), "st");
    assert_eq!(tui::ui::ordinal_suffix(2), "nd");
    assert_eq!(tui::ui::ordinal_suffix(3), "rd");
    assert_eq!(tui::ui::ordinal_suffix(4), "th");
    // The teens are all `th` despite ending in 1/2/3.
    assert_eq!(tui::ui::ordinal_suffix(11), "th");
    assert_eq!(tui::ui::ordinal_suffix(12), "th");
    assert_eq!(tui::ui::ordinal_suffix(13), "th");
    // Past the teens the last-digit rule resumes.
    assert_eq!(tui::ui::ordinal_suffix(21), "st");
    assert_eq!(tui::ui::ordinal_suffix(22), "nd");
    assert_eq!(tui::ui::ordinal_suffix(23), "rd");
}

#[test]
fn today_header_formats_a_known_day_number() {
    // 2026-07-02 is a Thursday. Its civil day number is days-since-1970-01-01. The operator's
    // example format is `Weekday, Month Nth, YYYY`.
    // 2026-07-02 = 20636 days after the epoch (verified via the pure civil_from_days round-trip).
    let day = 20_636_i64;
    // Sanity-pin the day number against the civil-date seam so the expectation is self-checking.
    assert_eq!(
        tui::ui::civil_from_days(day),
        (2026, 7, 2),
        "the chosen day number maps to 2026-07-02",
    );
    assert_eq!(tui::ui::today_header(day), "Thursday, July 2nd, 2026");
}

// ---- sub-task delete (acceptance #7, added 2026-07-02) ----

/// The captured fields of the single `DeleteSubtask` call in `calls`, panicking if none was made.
fn delete_subtask_call(calls: &[Call]) -> (String, String, String, String) {
    calls
        .iter()
        .find_map(|c| match c {
            Call::DeleteSubtask {
                token,
                profile_id,
                task_id,
                subtask_id,
            } => Some((
                token.clone(),
                profile_id.clone(),
                task_id.clone(),
                subtask_id.clone(),
            )),
            _ => None,
        })
        .expect("a delete_subtask call was made")
}

/// A logged-in app on a single expanded today task with one open sub-task selected. Returns the
/// handle + app with the selection moved onto the sub-task row.
fn selected_subtask_app() -> (FakeClient, App) {
    let (client, mut app) = logged_in_tree(
        vec![today_open_task("t1", "Parent", "10:00:00")],
        vec![open_subtask("s1", "t1", "the child")],
    );
    // Parent (row 0) is open → expanded, so the sub-task is row 1. Move onto it.
    let _ = app.handle_event(Event::Next);
    let today_day = current_day_number();
    assert_eq!(
        tasks_pane(&app).selected_row(today_day),
        Some(VisibleRow::Subtask { subtask_idx: 0 }),
        "selection landed on the sub-task row",
    );
    (client, app)
}

#[test]
fn delete_on_a_subtask_row_arms_a_subtask_target() {
    // Acceptance #7: on a sub-task row, `d` (`DeleteSelected`) arms the same two-step confirm as a
    // task, but the armed target is the SUB-TASK (carrying its parent task id, since the delete wire
    // is task-scoped). No request issues on the arming key.
    let (client, mut app) = selected_subtask_app();
    let calls_before = client.calls().len();

    assert!(
        app.handle_event(Event::DeleteSelected).is_none(),
        "the delete key arms the confirm, dispatching nothing",
    );
    assert_eq!(
        client.calls().len(),
        calls_before,
        "no request on the arming key: {:?}",
        client.calls(),
    );
    let list = tasks_pane(&app);
    assert!(
        matches!(
            list.confirming_delete,
            Some(DeleteTarget::Subtask { ref task_id, ref subtask_id })
                if task_id == "t1" && subtask_id == "s1"
        ),
        "the confirm is armed against the sub-task (DeleteTarget::Subtask): {:?}",
        list.confirming_delete,
    );
}

#[test]
fn enter_confirms_a_subtask_delete_and_issues_delete_subtask() {
    // Acceptance #7: `Enter` on the armed sub-task confirm issues `DeleteSubtask { task_id: parent,
    // subtask_id: selected }` under the active profile, then the standard refresh chain runs.
    let (client, mut app) = selected_subtask_app();
    assert!(app.handle_event(Event::DeleteSelected).is_none());

    // Script the delete (204) and the chained refresh (ListTasks → ListSubtasks; the child gone).
    client.push_delete_subtask(Ok(()));
    client.push_tasks(Ok(vec![today_open_task("t1", "Parent", "10:00:00")]));
    client.push_list_subtasks(Ok(vec![]));

    submit(&mut app, &client, Event::Submit);

    let calls = client.calls();
    assert_eq!(
        delete_subtask_call(&calls),
        (
            "jwt".to_owned(),
            "p1".to_owned(),
            "t1".to_owned(),
            "s1".to_owned(),
        ),
        "the confirmed delete issues DeleteSubtask for the parent+sub-task: {calls:?}",
    );
    // The standard post-mutation refresh follows (statelessness #1).
    assert!(
        calls.iter().any(|c| matches!(c, Call::ListTasks { .. })),
        "a fresh list fetch follows the sub-task delete: {calls:?}",
    );
    // The confirm disarmed and the child is gone after the refresh.
    let list = tasks_pane(&app);
    assert!(list.confirming_delete.is_none(), "confirm disarmed");
    assert!(list.subtasks.is_empty(), "the deleted sub-task is gone");
}

#[test]
fn a_task_row_delete_still_issues_delete_task_not_delete_subtask() {
    // Acceptance #7: a task-row `d` → `Enter` continues to issue `DeleteTask` (the sub-task path is
    // additive, not a replacement). Confirms the row-kind branch in `arm_delete`/`confirm_delete`.
    let (client, mut app) = logged_in(vec![today_open_task("t1", "doomed", "10:00:00")]);
    assert!(app.handle_event(Event::DeleteSelected).is_none());
    // The armed target is a task, not a sub-task.
    assert!(
        matches!(
            tasks_pane(&app).confirming_delete,
            Some(DeleteTarget::Task { ref task_id }) if task_id == "t1"
        ),
        "a task row arms a DeleteTarget::Task",
    );

    client.push_delete(Ok(()));
    client.push_tasks(Ok(vec![]));
    submit(&mut app, &client, Event::Submit);

    let calls = client.calls();
    assert!(
        calls.iter().any(|c| matches!(c,
            Call::DeleteTask { task_id, .. } if task_id == "t1")),
        "a task-row confirm issues DeleteTask: {calls:?}",
    );
    assert!(
        !calls
            .iter()
            .any(|c| matches!(c, Call::DeleteSubtask { .. })),
        "no DeleteSubtask is issued for a task-row delete: {calls:?}",
    );
}

#[test]
fn a_non_confirm_key_between_arm_and_confirm_issues_no_delete() {
    // Acceptance #7: the two-step confirm captures input — a navigation key (`Next`) between the
    // arming `d` and the confirming `Enter` issues NO delete request (the dialog holds; only `Enter`
    // confirms and `Esc` cancels, mirroring the task-delete confirm and the notes/profiles dialog).
    let (client, mut app) = selected_subtask_app();
    assert!(app.handle_event(Event::DeleteSelected).is_none());

    // A navigation event while armed dispatches nothing and issues no request.
    assert!(
        app.handle_event(Event::Next).is_none(),
        "a navigation key while armed dispatches nothing",
    );
    assert!(
        !client
            .calls()
            .iter()
            .any(|c| matches!(c, Call::DeleteSubtask { .. } | Call::DeleteTask { .. })),
        "no delete crossed the wire from a non-confirm key: {:?}",
        client.calls(),
    );
}

#[test]
fn delete_on_the_older_separator_row_is_a_no_op() {
    // Acceptance #7 / #3: the "Older tasks" separator is non-selectable, so `d` never arms against
    // it. Navigation skips the separator, so the selection can never rest on it; `arm_delete` also
    // guards the separator explicitly. Here we assert `d` while the older group is shown arms only a
    // real row, never the separator (confirming_delete is never armed with the selection on nothing).
    let (client, mut app) = logged_in(vec![
        today_open_task("t-today", "today task", "12:00:00"),
        open_task("t-old", "older task", "2020-01-02T10:00:00Z"),
    ]);
    let today_day = current_day_number();
    // Navigate: today -> older (the separator is skipped, never selected).
    let _ = app.handle_event(Event::Next);
    assert_eq!(
        tasks_pane(&app).selected_row(today_day),
        Some(VisibleRow::Task { task_idx: 1 }),
        "navigation lands on the older task, skipping the separator",
    );
    // `d` arms against the older task (a real row), and issues no request.
    assert!(app.handle_event(Event::DeleteSelected).is_none());
    assert!(
        matches!(
            tasks_pane(&app).confirming_delete,
            Some(DeleteTarget::Task { ref task_id }) if task_id == "t-old"
        ),
        "d arms against the older task row, never the separator: {:?}",
        tasks_pane(&app).confirming_delete,
    );
    assert!(
        !client
            .calls()
            .iter()
            .any(|c| matches!(c, Call::DeleteTask { .. })),
        "arming issues no request: {:?}",
        client.calls(),
    );
}

// ---- limit=200 on the ListTasks query ----

#[test]
fn list_tasks_requests_carry_the_tui_limit_and_zero_offset() {
    // Acceptance #5 / ADR-0014 §2: every task-list load sends the TUI's hard-coded limit (200) and
    // offset 0. The post-auth bootstrap `ListTasks` carries exactly that query.
    let (client, _app) = logged_in(vec![today_open_task("t1", "task", "10:00:00")]);
    let list_call = client
        .calls()
        .into_iter()
        .find(|c| matches!(c, Call::ListTasks { .. }))
        .expect("a ListTasks call was made on login bootstrap");
    match list_call {
        Call::ListTasks { limit, offset, .. } => {
            assert_eq!(limit, Some(TASK_LIST_LIMIT), "limit is the TUI's 200");
            assert_eq!(limit, Some(200), "TASK_LIST_LIMIT is 200 (ADR-0014 §2)");
            assert_eq!(
                offset,
                Some(0),
                "offset is 0 (no pagination in this feature)"
            );
        }
        other => panic!("expected a ListTasks call, got {other:?}"),
    }
}

/// `today_at` sanity: the helper builds a timestamp on the current civil day (used above to keep the
/// today/older split deterministic against the wall clock).
#[test]
fn today_at_builds_a_timestamp_on_the_current_civil_day() {
    let ts = today_at("09:30:00");
    // Parse the date portion back to a day number and compare to "now".
    let created = open_task("probe", "probe", &ts);
    assert_eq!(
        tui::app::task_list::day_number(created.created_at.timestamp()),
        current_day_number(),
        "today_at lands on the current civil day",
    );
}
