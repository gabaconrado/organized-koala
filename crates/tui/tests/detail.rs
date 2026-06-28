//! The 0016 per-field detail-view acceptance suite (ADR-0010 §4, slice 5): the task and note
//! detail views driven through the public two-step `App` API (`handle_event` → synchronous executor
//! → `apply_response`) against the held fake client (the sole external-service mock, ADR-0003 layer
//! 2). Pins the detail-view lifecycle and the global-suppression contract:
//!
//! - `Enter` opens the task / note detail view from the selected list row;
//! - `Tab` / `Shift+Tab` cycle the panes inside the view (and do NOT switch top-level tabs);
//! - `e` enters edit mode on the focused editable pane (inert on a read-only pane, A6);
//! - `Enter` commits one field — the task payload carries ONLY the edited `Option` field, and the
//!   note payload preserves the untouched field from the snapshot (R5);
//! - `Esc` cancels an in-progress edit, reverting the value; `Esc` with no edit exits to the list
//!   (the two-tiered `Esc`, R1);
//! - the focused editable pane carries the purple focus border (buffer-snapshot, mirroring 0015);
//! - the **A7 global-suppression contract**: while a detail view is open the per-tab action keys
//!   and `t`/`T`/`r`/tab-switch are captured; `?` help is reachable while no field edit is in
//!   progress; while a field edit IS in progress everything (including `?`) is captured as text.
//!
//! These exercise the detail views with no live server and no real terminal — the only mock is the
//! sanctioned `Client` trait (the HTTP server), exactly as ADR-0003 / ADR-0006 prescribe.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

mod common;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::style::Color;
use tui::app::{App, Event, NoteDetail, NotePane, NotesMode, Screen, Tab, TaskDetail, TaskPane};
use tui::terminal::map_key;

use common::{
    Call, FakeClient, note, notes_pane, on_tab, open_task, profile, render, render_buffer,
    row_fg_count, session, submit, tasks_pane,
};

const W: u16 = 80;
const H: u16 = 24;

/// A `crossterm` key with no modifiers.
fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

/// Map a key through the real keymap against the app's current live predicates — the exact call the
/// poll loop makes. Pins that the in-context binding is what the detail view will actually receive.
fn map_live(app: &App, code: KeyCode) -> Option<Event> {
    map_key(
        app.screen(),
        app.overlay_capturing_input(),
        app.help_open(),
        app.is_editing_duration(),
        key(code),
    )
}

/// Log in to the `work` profile on the Tasks tab with the given tasks. `ada`/`jwt`/`p1` throughout.
fn logged_in_tasks(tasks: Vec<contract::Task>) -> (FakeClient, App) {
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

/// Log in then switch to the Notes tab with the given notes loaded.
fn logged_in_notes(notes: Vec<contract::Note>) -> (FakeClient, App) {
    let (client, mut app) = logged_in_tasks(vec![]);
    client.push_notes(Ok(notes));
    submit(&mut app, &client, Event::NextTab); // Tasks -> Notes
    assert!(on_tab(&app, Tab::Notes), "precondition: on the Notes tab");
    (client, app)
}

/// Open the note detail view for the selected note: `Enter` issues `GetNote`, the response (the
/// `viewed` note) folds into `NotesMode::Detail`. Returns once the detail is open.
fn open_note_detail(app: &mut App, client: &FakeClient, viewed: contract::Note) {
    client.push_get_note(Ok(viewed));
    submit(app, client, Event::Submit); // Enter opens the selected note
    assert!(
        notes_pane(app).detail_open(),
        "precondition: the note detail view is open",
    );
}

// ============================================================================
// Detail-view lifecycle: open
// ============================================================================

#[test]
fn enter_opens_the_task_detail_view_with_per_field_panes() {
    let (_client, mut app) =
        logged_in_tasks(vec![open_task("t1", "the title", "2026-06-18T10:00:00Z")]);

    // `Enter` opens the per-field detail view (no request — the list is server-derived, A3).
    assert!(
        app.handle_event(Event::Submit).is_none(),
        "opening the task detail dispatches nothing (opens from the in-memory snapshot)",
    );
    let detail = tasks_pane(&app)
        .detail
        .as_ref()
        .expect("the task detail view is open");
    // The panes are the editable Title/Description plus the read-only Status/Created (an open task
    // has no Closed pane).
    assert_eq!(
        detail.panes,
        vec![
            TaskPane::Title,
            TaskPane::Description,
            TaskPane::Status,
            TaskPane::Created,
        ],
        "an open task exposes Title/Description/Status/Created panes",
    );
    assert_eq!(
        detail.focused, 0,
        "the first pane (Title) is focused on open"
    );
    assert!(detail.edit.is_none(), "no field edit in progress on open");

    // The detail renders as bordered per-field panes (not the list) with each field's label.
    let text = render(&app, W, H);
    assert!(text.contains("Title"), "Title pane label rendered:\n{text}");
    assert!(
        text.contains("Description"),
        "Description pane label rendered:\n{text}",
    );
    assert!(
        text.contains("Status") && text.contains("Created"),
        "read-only Status/Created panes rendered:\n{text}",
    );
    assert!(
        text.contains("the title"),
        "the task title value rendered in its pane:\n{text}",
    );
}

#[test]
fn done_task_detail_includes_the_closed_pane() {
    let (_client, mut app) = logged_in_tasks(vec![common::done_task(
        "t1",
        "shipped",
        "2026-06-18T10:00:00Z",
        "2026-06-18T14:00:00Z",
    )]);
    let _ = app.handle_event(Event::Submit); // open detail

    let detail = tasks_pane(&app).detail.as_ref().expect("detail open");
    assert!(
        detail.panes.contains(&TaskPane::Closed),
        "a done task's detail exposes the read-only Closed pane: {:?}",
        detail.panes,
    );
}

#[test]
fn enter_opens_the_note_detail_view_deriving_from_a_getnote_response() {
    let (client, mut app) = logged_in_notes(vec![note(
        "n1",
        "Title",
        "stale body",
        "2026-06-18T10:00:00Z",
    )]);
    // `Enter` opens the note detail from a FRESH `GetNote` (the view derives from a server
    // response, #1) — the server's authoritative copy may differ from the cached list entry.
    open_note_detail(
        &mut app,
        &client,
        note("n1", "Title", "fresh body", "2026-06-18T10:00:00Z"),
    );

    let NotesMode::Detail(detail) = &notes_pane(&app).mode else {
        panic!("the note detail view is open");
    };
    assert_eq!(
        detail.note.content, "fresh body",
        "the detail derives from the GetNote response, not the cached list entry",
    );
    assert_eq!(detail.focused, 0, "Title pane focused on open");
    assert!(detail.edit.is_none(), "no field edit in progress on open");

    let calls = client.calls();
    assert!(
        matches!(calls.last(), Some(Call::GetNote { token, profile_id, note_id })
            if token == "jwt" && profile_id == "p1" && note_id == "n1"),
        "GetNote targeted the selected note under the active profile: {calls:?}",
    );

    let text = render(&app, W, H);
    assert!(
        text.contains("Title") && text.contains("Content") && text.contains("Created"),
        "the note's Title/Content/Created panes render:\n{text}",
    );
    assert!(
        text.contains("fresh body"),
        "the fresh content renders:\n{text}"
    );
}

// ============================================================================
// Tab overloading (R3): Tab/Shift+Tab cycle panes inside a detail view
// ============================================================================

#[test]
fn tab_cycles_panes_inside_the_task_detail_and_does_not_switch_tabs() {
    let (_client, mut app) =
        logged_in_tasks(vec![open_task("t1", "the title", "2026-06-18T10:00:00Z")]);
    let _ = app.handle_event(Event::Submit); // open detail (Title focused; read-only panes inert)

    // The real keymap maps `Tab` to `Next` (NOT `NextTab`) while the detail view captures input,
    // and `Submit` is never folded into a tab switch — pin both forks of R3 through `map_key`.
    assert_eq!(
        map_live(&app, KeyCode::Tab),
        Some(Event::Next),
        "Tab cycles panes (Next), it does NOT switch top-level tabs, while the detail is open",
    );
    assert_eq!(
        map_live(&app, KeyCode::BackTab),
        Some(Event::Prev),
        "Shift+Tab cycles panes backward (Prev) while the detail is open",
    );

    // Cycling forward skips the read-only Status/Created panes: Title -> Description -> (wraps)
    // Title. Read-only panes stay rendered in place but are never a focus stop.
    for expected in [TaskPane::Description, TaskPane::Title] {
        let _ = app.handle_event(Event::Next);
        assert_eq!(
            tasks_pane(&app).detail.as_ref().unwrap().focused_pane(),
            Some(expected),
            "Tab advanced to the {expected:?} pane",
        );
        assert!(
            on_tab(&app, Tab::Tasks),
            "still on the Tasks tab while cycling panes"
        );
    }

    // Shift+Tab cycles backward to the previous EDITABLE pane: from Title it wraps to Description
    // (the last editable pane), NOT the read-only Created pane.
    let _ = app.handle_event(Event::Prev);
    assert_eq!(
        tasks_pane(&app).detail.as_ref().unwrap().focused_pane(),
        Some(TaskPane::Description),
        "Shift+Tab wraps backward from Title to the last editable pane (Description), \
         skipping read-only panes",
    );
}

#[test]
fn tab_cycles_panes_inside_the_note_detail() {
    let (client, mut app) =
        logged_in_notes(vec![note("n1", "Title", "body", "2026-06-18T10:00:00Z")]);
    open_note_detail(
        &mut app,
        &client,
        note("n1", "Title", "body", "2026-06-18T10:00:00Z"),
    );

    assert_eq!(
        map_live(&app, KeyCode::Tab),
        Some(Event::Next),
        "Tab cycles note panes, not top-level tabs",
    );

    let focused = |app: &App| match &notes_pane(app).mode {
        NotesMode::Detail(d) => d.focused_pane(),
        _ => panic!("detail open"),
    };
    // Cycling forward skips the read-only Created pane: Title -> Content -> (wraps) Title. Created
    // stays rendered in place but is never a focus stop.
    for expected in [NotePane::Content, NotePane::Title] {
        let _ = app.handle_event(Event::Next);
        assert_eq!(focused(&app), expected, "Tab advanced to {expected:?}");
        assert!(
            on_tab(&app, Tab::Notes),
            "still on the Notes tab while cycling panes"
        );
    }
}

#[test]
fn read_only_task_panes_are_never_focus_stops() {
    // A done task carries the longest read-only run: Status, Created, AND Closed. Cycling forward
    // and backward many times must never land focus on any of them.
    let (_client, mut app) = logged_in_tasks(vec![common::done_task(
        "t1",
        "shipped",
        "2026-06-18T10:00:00Z",
        "2026-06-18T14:00:00Z",
    )]);
    let _ = app.handle_event(Event::Submit); // open detail (Title focused)
    assert!(
        tasks_pane(&app)
            .detail
            .as_ref()
            .unwrap()
            .panes
            .contains(&TaskPane::Closed),
        "precondition: the done task's detail exposes the read-only Closed pane",
    );

    let focused = |app: &App| tasks_pane(app).detail.as_ref().unwrap().focused_pane();
    // Cycle forward well past the editable count, then backward the same: focus is always an
    // editable pane, never Status/Created/Closed.
    for step in 0..12 {
        let event = if step % 4 < 2 {
            Event::Next
        } else {
            Event::Prev
        };
        let _ = app.handle_event(event);
        let pane = focused(&app).expect("a pane is focused");
        assert!(
            matches!(pane, TaskPane::Title | TaskPane::Description),
            "focus stays on an editable pane after cycling, got {pane:?}",
        );
        assert!(
            pane.is_editable(),
            "the focused pane is editable (never read-only), got {pane:?}",
        );
    }
}

#[test]
fn read_only_note_pane_is_never_a_focus_stop() {
    let (client, mut app) =
        logged_in_notes(vec![note("n1", "Title", "body", "2026-06-18T10:00:00Z")]);
    open_note_detail(
        &mut app,
        &client,
        note("n1", "Title", "body", "2026-06-18T10:00:00Z"),
    );

    let focused = |app: &App| match &notes_pane(app).mode {
        NotesMode::Detail(d) => d.focused_pane(),
        _ => panic!("detail open"),
    };
    for step in 0..12 {
        let event = if step % 4 < 2 {
            Event::Next
        } else {
            Event::Prev
        };
        let _ = app.handle_event(event);
        let pane = focused(&app);
        assert!(
            matches!(pane, NotePane::Title | NotePane::Content),
            "note focus stays on an editable pane after cycling, got {pane:?}",
        );
        assert!(
            pane.is_editable(),
            "the focused note pane is editable (never the read-only Created), got {pane:?}",
        );
    }
}

#[test]
fn task_detail_opens_focused_on_the_first_editable_pane() {
    let (_client, mut app) =
        logged_in_tasks(vec![open_task("t1", "the title", "2026-06-18T10:00:00Z")]);
    let _ = app.handle_event(Event::Submit); // open detail

    let pane = tasks_pane(&app)
        .detail
        .as_ref()
        .unwrap()
        .focused_pane()
        .expect("a pane is focused on open");
    assert_eq!(pane, TaskPane::Title, "initial focus is Title");
    assert!(pane.is_editable(), "initial focus is an editable pane");
}

#[test]
fn note_detail_opens_focused_on_the_first_editable_pane() {
    let (client, mut app) =
        logged_in_notes(vec![note("n1", "Title", "body", "2026-06-18T10:00:00Z")]);
    open_note_detail(
        &mut app,
        &client,
        note("n1", "Title", "body", "2026-06-18T10:00:00Z"),
    );

    let pane = match &notes_pane(&app).mode {
        NotesMode::Detail(d) => d.focused_pane(),
        _ => panic!("detail open"),
    };
    assert_eq!(pane, NotePane::Title, "initial focus is Title");
    assert!(pane.is_editable(), "initial focus is an editable pane");
}

// ============================================================================
// Edit lifecycle: e enters edit; Enter commits one field
// ============================================================================

#[test]
fn task_commit_sends_only_the_edited_title_field() {
    let (client, mut app) =
        logged_in_tasks(vec![open_task("t1", "old title", "2026-06-18T10:00:00Z")]);
    let _ = app.handle_event(Event::Submit); // open detail, Title focused

    // `e` enters edit on the focused (Title) pane; the buffer seeds from the current value.
    let _ = app.handle_event(Event::BeginEditTask);
    assert!(
        tasks_pane(&app).detail.as_ref().unwrap().is_editing(),
        "e begins a field edit on the focused editable pane",
    );
    // Clear "old title" (9 chars) and type the new title.
    for _ in 0.."old title".len() {
        let _ = app.handle_event(Event::Backspace);
    }
    for c in "new title".chars() {
        let _ = app.handle_event(Event::Char(c));
    }

    // `Enter` commits THIS field; the success re-derives the detail and chains a list refresh.
    let committed = open_task("t1", "new title", "2026-06-18T10:00:00Z");
    client.push_update(Ok(committed.clone()));
    client.push_tasks(Ok(vec![committed]));
    submit(&mut app, &client, Event::Submit);

    // The issued patch carried ONLY the edited title — description and status stay `None`.
    let calls = client.calls();
    let update = calls
        .iter()
        .find_map(|c| match c {
            Call::UpdateTask {
                task_id,
                title,
                description,
                status,
                ..
            } => Some((task_id.clone(), title.clone(), description.clone(), *status)),
            _ => None,
        })
        .expect("an UpdateTask call was made");
    assert_eq!(
        update,
        ("t1".to_owned(), Some("new title".to_owned()), None, None),
        "a per-field title commit sends ONLY the title Option set: {calls:?}",
    );
    assert!(
        matches!(calls.last(), Some(Call::ListTasks { .. })),
        "the commit chains a list refresh (statelessness, #1): {calls:?}",
    );

    // The detail stays open, re-derived from the server's task, with the edit buffer cleared.
    let detail = tasks_pane(&app).detail.as_ref().expect("detail stays open");
    assert!(
        !detail.is_editing(),
        "the edit buffer cleared after a successful commit"
    );
    assert_eq!(
        detail.task.title, "new title",
        "the detail re-derived from the server task"
    );
}

#[test]
fn task_commit_of_description_sends_only_the_description_field() {
    let (client, mut app) =
        logged_in_tasks(vec![open_task("t1", "keep title", "2026-06-18T10:00:00Z")]);
    let _ = app.handle_event(Event::Submit); // open detail
    let _ = app.handle_event(Event::Next); // focus Description
    let _ = app.handle_event(Event::BeginEditTask);
    for c in "a note".chars() {
        let _ = app.handle_event(Event::Char(c));
    }

    let committed = open_task("t1", "keep title", "2026-06-18T10:00:00Z");
    client.push_update(Ok(committed.clone()));
    client.push_tasks(Ok(vec![committed]));
    submit(&mut app, &client, Event::Submit);

    let calls = client.calls();
    let update = calls
        .iter()
        .find_map(|c| match c {
            Call::UpdateTask {
                title,
                description,
                status,
                ..
            } => Some((title.clone(), description.clone(), *status)),
            _ => None,
        })
        .expect("an UpdateTask call was made");
    assert_eq!(
        update,
        (None, Some("a note".to_owned()), None),
        "a per-field description commit sends ONLY the description Option set: {calls:?}",
    );
}

#[test]
fn note_commit_preserves_the_untouched_field_from_the_snapshot() {
    // R5: `UpdateNoteRequest` has no Option fields, so committing Title must RE-SEND the snapshot's
    // current Content (and vice versa), or the other field would be blanked.
    let (client, mut app) = logged_in_notes(vec![note(
        "n1",
        "old title",
        "the content",
        "2026-06-18T10:00:00Z",
    )]);
    open_note_detail(
        &mut app,
        &client,
        note("n1", "old title", "the content", "2026-06-18T10:00:00Z"),
    );

    // Edit ONLY the Title pane (focused on open).
    let _ = app.handle_event(Event::BeginEditNote);
    for _ in 0.."old title".len() {
        let _ = app.handle_event(Event::Backspace);
    }
    for c in "new title".chars() {
        let _ = app.handle_event(Event::Char(c));
    }

    let committed = note("n1", "new title", "the content", "2026-06-18T10:00:00Z");
    client.push_update_note(Ok(committed.clone()));
    client.push_notes(Ok(vec![committed]));
    submit(&mut app, &client, Event::Submit);

    // The issued payload carried the edited title AND re-sent the untouched content from the
    // snapshot (R5).
    let calls = client.calls();
    let update = calls
        .iter()
        .find_map(|c| match c {
            Call::UpdateNote {
                note_id,
                title,
                content,
                ..
            } => Some((note_id.clone(), title.clone(), content.clone())),
            _ => None,
        })
        .expect("an UpdateNote call was made");
    assert_eq!(
        update,
        (
            "n1".to_owned(),
            "new title".to_owned(),
            "the content".to_owned()
        ),
        "the note commit preserves the untouched Content from the snapshot (R5): {calls:?}",
    );
    assert!(
        matches!(calls.last(), Some(Call::ListNotes { .. })),
        "the commit chains a list refresh (statelessness, #1): {calls:?}",
    );

    // The detail stays open, re-derived from the returned note, edit cleared.
    let NotesMode::Detail(detail) = &notes_pane(&app).mode else {
        panic!("the note detail stays open after commit");
    };
    assert!(!detail.is_editing(), "the edit buffer cleared after commit");
    assert_eq!(
        detail.note.title, "new title",
        "the detail re-derived from the server note"
    );
}

#[test]
fn note_commit_of_content_preserves_the_title_from_the_snapshot() {
    let (client, mut app) = logged_in_notes(vec![note(
        "n1",
        "the title",
        "old content",
        "2026-06-18T10:00:00Z",
    )]);
    open_note_detail(
        &mut app,
        &client,
        note("n1", "the title", "old content", "2026-06-18T10:00:00Z"),
    );

    // Focus the Content pane, then edit it.
    let _ = app.handle_event(Event::Next); // Title -> Content
    let _ = app.handle_event(Event::BeginEditNote);
    for _ in 0.."old content".len() {
        let _ = app.handle_event(Event::Backspace);
    }
    for c in "new content".chars() {
        let _ = app.handle_event(Event::Char(c));
    }

    let committed = note("n1", "the title", "new content", "2026-06-18T10:00:00Z");
    client.push_update_note(Ok(committed.clone()));
    client.push_notes(Ok(vec![committed]));
    submit(&mut app, &client, Event::Submit);

    let calls = client.calls();
    let update = calls
        .iter()
        .find_map(|c| match c {
            Call::UpdateNote { title, content, .. } => Some((title.clone(), content.clone())),
            _ => None,
        })
        .expect("an UpdateNote call was made");
    assert_eq!(
        update,
        ("the title".to_owned(), "new content".to_owned()),
        "a Content commit preserves the untouched Title from the snapshot (R5): {calls:?}",
    );
}

// ============================================================================
// e on a read-only pane is inert (A6)
// ============================================================================

// Cycling never lands focus on a read-only pane any more, so the A6 guard is reached by forcing
// focus there directly via the documented `focus_pane` test seam (ADR-0003 layer 2), then driving
// the same `begin_edit` path `BeginEditTask`/`BeginEditNote` routes through. The detail types are
// constructed directly from a snapshot — the guard is `begin_edit`'s own, exercised over the public
// API without the now-removed read-only cycling path.
#[test]
fn e_on_a_read_only_task_pane_is_inert() {
    let task = common::done_task(
        "t1",
        "shipped",
        "2026-06-18T10:00:00Z",
        "2026-06-18T14:00:00Z",
    );
    // Every read-only pane present on a done task: Status, Created, Closed.
    for pane in [TaskPane::Status, TaskPane::Created, TaskPane::Closed] {
        let mut detail = TaskDetail::new(task.clone());
        assert!(
            detail.focus_pane(pane),
            "the {pane:?} pane is present on a done task's detail",
        );
        assert_eq!(
            detail.focused_pane(),
            Some(pane),
            "focus_pane forced focus onto {pane:?}",
        );
        assert!(!pane.is_editable(), "{pane:?} is a read-only pane");

        detail.begin_edit(); // the path `BeginEditTask` routes through
        assert!(
            !detail.is_editing(),
            "e is inert on the read-only {pane:?} pane (no edit buffer opens)",
        );
    }
}

#[test]
fn e_on_the_read_only_created_note_pane_is_inert() {
    let mut detail = NoteDetail::new(note("n1", "Title", "body", "2026-06-18T10:00:00Z"));
    detail.focus_pane(NotePane::Created); // the read-only pane, forced via the test seam
    assert_eq!(
        detail.focused_pane(),
        NotePane::Created,
        "focus_pane forced focus onto the read-only Created pane",
    );
    assert!(
        !NotePane::Created.is_editable(),
        "Created is a read-only note pane",
    );

    detail.begin_edit(); // the path `BeginEditNote` routes through
    assert!(
        !detail.is_editing(),
        "e is inert on the read-only Created pane (no edit buffer opens)",
    );
}

// ============================================================================
// Two-tiered Esc (R1): cancel an edit reverting; exit to list with no edit
// ============================================================================

#[test]
fn esc_cancels_an_in_progress_task_edit_reverting_the_value() {
    let (_client, mut app) =
        logged_in_tasks(vec![open_task("t1", "original", "2026-06-18T10:00:00Z")]);
    let _ = app.handle_event(Event::Submit); // open detail
    let _ = app.handle_event(Event::BeginEditTask); // edit Title
    for c in "XYZ".chars() {
        let _ = app.handle_event(Event::Char(c));
    }
    assert_eq!(
        tasks_pane(&app).detail.as_ref().unwrap().edit.as_deref(),
        Some("originalXYZ"),
        "the edit buffer holds the in-progress value",
    );

    // First Esc tier: cancel the edit. The buffer drops; the detail stays open; the snapshot value
    // is untouched (no commit happened) — and crucially the app does NOT exit to the list or quit.
    let _ = app.handle_event(Event::Cancel);
    let detail = tasks_pane(&app)
        .detail
        .as_ref()
        .expect("detail still open after edit cancel");
    assert!(!detail.is_editing(), "the in-progress edit was cancelled");
    assert_eq!(
        detail.task.title, "original",
        "the field reverted to the snapshot value"
    );
    assert!(!app.should_quit(), "cancelling an edit does not quit");
}

#[test]
fn esc_with_no_edit_exits_the_task_detail_to_the_list() {
    let (_client, mut app) =
        logged_in_tasks(vec![open_task("t1", "the title", "2026-06-18T10:00:00Z")]);
    let _ = app.handle_event(Event::Submit); // open detail (no edit in progress)
    assert!(tasks_pane(&app).detail.is_some(), "detail open");

    // Second Esc tier: with no edit in progress, Esc exits the detail view back to the list (it does
    // NOT quit — the detail view counts as input-capturing, so map_key routes Esc to Cancel).
    assert_eq!(
        map_live(&app, KeyCode::Esc),
        Some(Event::Cancel),
        "Esc is Cancel (not Quit) while a detail view is open",
    );
    let _ = app.handle_event(Event::Cancel);
    assert!(
        tasks_pane(&app).detail.is_none(),
        "Esc with no edit exits the detail view back to the list",
    );
    assert!(!app.should_quit(), "exiting the detail view does not quit");

    // Back on the list: the task row renders again (not the detail panes).
    let text = render(&app, W, H);
    assert!(
        text.contains("[ ] the title"),
        "the task list row renders again:\n{text}"
    );
}

#[test]
fn two_tiered_esc_unwinds_one_level_at_a_time_in_the_note_detail() {
    // R1 end-to-end on the note detail: Esc from a field edit cancels THAT edit (staying in the
    // view); a second Esc (no edit) exits to the list — never both levels at once.
    let (client, mut app) =
        logged_in_notes(vec![note("n1", "Title", "body", "2026-06-18T10:00:00Z")]);
    open_note_detail(
        &mut app,
        &client,
        note("n1", "Title", "body", "2026-06-18T10:00:00Z"),
    );
    let _ = app.handle_event(Event::BeginEditNote); // edit Title
    for c in "!".chars() {
        let _ = app.handle_event(Event::Char(c));
    }

    // Tier 1: cancel the edit; still in the detail view.
    let _ = app.handle_event(Event::Cancel);
    assert!(
        notes_pane(&app).detail_open(),
        "still in the detail view after the edit cancel"
    );
    assert!(
        !matches!(&notes_pane(&app).mode, NotesMode::Detail(d) if d.is_editing()),
        "the edit was cancelled",
    );

    // Tier 2: exit the detail view to the list.
    let _ = app.handle_event(Event::Cancel);
    assert!(
        matches!(notes_pane(&app).mode, NotesMode::List),
        "a second Esc (no edit) exits the note detail to the list",
    );
    assert!(!app.should_quit(), "exiting the note detail does not quit");
}

// ============================================================================
// Global-suppression (R2 / A7): action keys captured while a detail view is open
// ============================================================================

#[test]
fn an_open_task_detail_suppresses_per_tab_action_keys_and_globals() {
    // A7: while a detail view is open (no field edit) the per-tab action keys (`a`/`d`/`Space`) and
    // the globals (`t`/`T`/`r`/`q`/tab-switch) are suppressed — none fires its action. `?` stays
    // reachable (asserted separately). Pinned through the LIVE keymap (the detail counts as
    // input-capturing via `overlay_capturing_input`, so `globals_live` is false).
    let (_client, mut app) =
        logged_in_tasks(vec![open_task("t1", "the title", "2026-06-18T10:00:00Z")]);
    let _ = app.handle_event(Event::Submit); // open detail, no edit
    assert!(
        app.overlay_capturing_input(),
        "an open detail view counts as input-capturing (A7)",
    );

    for code in [
        KeyCode::Char('a'),
        KeyCode::Char('d'),
        KeyCode::Char(' '),
        KeyCode::Char('t'),
        KeyCode::Char('T'),
        KeyCode::Char('r'),
        KeyCode::Char('q'),
    ] {
        let mapped = map_live(&app, code);
        assert!(
            !matches!(
                mapped,
                Some(
                    Event::BeginAddTask
                        | Event::DeleteSelected
                        | Event::ToggleDone
                        | Event::ToggleTimer
                        | Event::BeginEditDuration
                        | Event::Refresh
                        | Event::Quit
                )
            ),
            "{code:?} must NOT fire its action while the detail view is open (got {mapped:?})",
        );
    }
    // Tab does not switch top-level tabs (it cycles panes) — pinned in the R3 test, re-asserted here.
    assert_eq!(
        map_live(&app, KeyCode::Tab),
        Some(Event::Next),
        "Tab cycles panes, never switches tabs, while the detail is open",
    );
}

#[test]
fn question_mark_is_reachable_over_an_idle_detail_but_captured_during_a_field_edit() {
    // A7: `?` help stays reachable while a detail view is open but NO field edit is in progress;
    // once a field edit IS in progress everything (including `?` and printable chars) is captured.
    let (_client, mut app) =
        logged_in_tasks(vec![open_task("t1", "the title", "2026-06-18T10:00:00Z")]);
    let _ = app.handle_event(Event::Submit); // open detail, no edit

    // Idle detail: `?` opens help.
    assert_eq!(
        map_live(&app, KeyCode::Char('?')),
        Some(Event::ToggleHelp),
        "? is reachable over an idle detail view (A7)",
    );
    let _ = app.handle_event(Event::ToggleHelp);
    assert!(
        app.help_open(),
        "? opened the help overlay over the idle detail"
    );
    let _ = app.handle_event(Event::Cancel); // close help, back to the idle detail
    assert!(!app.help_open(), "help closed");

    // Now begin a field edit: `?` is captured as a literal Char, NOT the help toggle.
    let _ = app.handle_event(Event::BeginEditTask);
    assert!(
        tasks_pane(&app).detail.as_ref().unwrap().is_editing(),
        "a field edit is in progress",
    );
    assert_eq!(
        map_live(&app, KeyCode::Char('?')),
        Some(Event::Char('?')),
        "? is captured as text while a field edit is in progress (A7)",
    );
    // A printable char also goes to the buffer, never a command.
    assert_eq!(
        map_live(&app, KeyCode::Char('a')),
        Some(Event::Char('a')),
        "printable chars go to the edit buffer during a field edit, never an action key",
    );
}

// ============================================================================
// Purple focus border on the focused detail pane (buffer-snapshot, mirroring 0015)
// ============================================================================

#[test]
fn focused_editable_task_pane_carries_the_purple_focus_border() {
    // Mirrors the 0015 focus-border test: the focused EDITABLE pane's border row carries the magenta
    // focus cue; a non-focused pane's row does not. The detail box renders in the main content area
    // (no enclosing magenta dialog), so the contrast is stark.
    let (_client, mut app) =
        logged_in_tasks(vec![open_task("t1", "the title", "2026-06-18T10:00:00Z")]);
    let _ = app.handle_event(Event::Submit); // open detail, Title (editable) focused

    let buffer = render_buffer(&app, W, H);
    let focused = row_fg_count(&buffer, "Title", Color::Magenta);
    let unfocused = row_fg_count(&buffer, "Description", Color::Magenta);
    assert!(
        focused > unfocused,
        "the focused Title pane's border is purple, the non-focused Description pane's is not \
         (focused magenta cells {focused}, non-focused {unfocused})",
    );
    assert!(focused > 0, "the focused editable pane has a purple border");
}

#[test]
fn purple_border_follows_pane_focus_to_the_description() {
    let (_client, mut app) =
        logged_in_tasks(vec![open_task("t1", "the title", "2026-06-18T10:00:00Z")]);
    let _ = app.handle_event(Event::Submit); // open detail
    let _ = app.handle_event(Event::Next); // focus Description

    let buffer = render_buffer(&app, W, H);
    let title = row_fg_count(&buffer, "Title", Color::Magenta);
    let description = row_fg_count(&buffer, "Description", Color::Magenta);
    assert!(
        description > title,
        "the purple border moved to the now-focused Description pane \
         (Description magenta cells {description}, Title {title})",
    );
}

#[test]
fn read_only_panes_carry_no_purple_border() {
    // A6 render cue: read-only panes are bordered but NEVER purple (signalling `e` is inert there).
    // Cycling can no longer focus a read-only pane, so the purple cue stays on an editable pane;
    // the read-only Status/Created panes carry zero magenta cells regardless of which editable pane
    // holds focus.
    let (_client, mut app) =
        logged_in_tasks(vec![open_task("t1", "the title", "2026-06-18T10:00:00Z")]);
    let _ = app.handle_event(Event::Submit); // open detail (Title focused)
    let _ = app.handle_event(Event::Next); // -> Description (still an editable pane)
    assert_eq!(
        tasks_pane(&app).detail.as_ref().unwrap().focused_pane(),
        Some(TaskPane::Description),
        "cycling lands on the editable Description pane, never a read-only one",
    );

    let buffer = render_buffer(&app, W, H);
    let status = row_fg_count(&buffer, "Status", Color::Magenta);
    let created = row_fg_count(&buffer, "Created", Color::Magenta);
    assert_eq!(
        status, 0,
        "the read-only Status pane carries no purple border ({status} magenta cells)",
    );
    assert_eq!(
        created, 0,
        "the read-only Created pane carries no purple border ({created} magenta cells)",
    );
}

#[test]
fn focused_editable_note_pane_carries_the_purple_focus_border() {
    let (client, mut app) =
        logged_in_notes(vec![note("n1", "Title", "body", "2026-06-18T10:00:00Z")]);
    open_note_detail(
        &mut app,
        &client,
        note("n1", "Title", "body", "2026-06-18T10:00:00Z"),
    );

    let buffer = render_buffer(&app, W, H);
    let focused = row_fg_count(&buffer, "Title", Color::Magenta);
    let created = row_fg_count(&buffer, "Created", Color::Magenta);
    assert!(
        focused > created,
        "the focused Title pane is purple, the read-only Created pane is not \
         (focused {focused}, created {created})",
    );
    assert!(
        focused > 0,
        "the focused editable note pane has a purple border"
    );
}

// ============================================================================
// 0018: multiline Content text area in the note detail (ADR-0011)
// ============================================================================

/// The 0-based index of the first buffer row whose text contains `needle`, or a large sentinel if
/// absent (so an ordering assertion fails loudly rather than silently passing on a missing label).
fn first_row(text: &str, needle: &str) -> usize {
    text.lines()
        .position(|line| line.contains(needle))
        .unwrap_or(usize::MAX)
}

#[test]
fn note_detail_renders_panes_in_order_title_created_content() {
    // Acceptance 1: the note detail view renders panes top-to-bottom as Title → Created → Content
    // (Created moved above the multiline Content, ADR-0011 / NotePane::ALL).
    let (client, mut app) = logged_in_notes(vec![note(
        "n1",
        "the title",
        "the body",
        "2026-06-18T10:00:00Z",
    )]);
    open_note_detail(
        &mut app,
        &client,
        note("n1", "the title", "the body", "2026-06-18T10:00:00Z"),
    );

    // The pane order is fixed in NotePane::ALL — assert the data order first.
    assert_eq!(
        NotePane::ALL,
        [NotePane::Title, NotePane::Created, NotePane::Content],
        "the note detail pane order is Title → Created → Content",
    );

    // And the rendered layout reflects it: the Title pane's label row sits above the Created label
    // row, which sits above the Content label row.
    let text = render(&app, W, H);
    let title = first_row(&text, "Title");
    let created = first_row(&text, "Created");
    let content = first_row(&text, "Content");
    assert!(
        title < created && created < content,
        "panes render in order Title({title}) → Created({created}) → Content({content}):\n{text}",
    );
}

#[test]
fn content_pane_fills_the_remaining_height_and_renders_multiline_without_truncation() {
    // Acceptance 2: the Content pane takes the remaining height (taller than the fixed 3-row
    // Title/Created boxes) and a multi-line value renders across lines; Title/Created are not
    // truncated.
    let body = "line one\nline two\nline three\nline four";
    let (client, mut app) =
        logged_in_notes(vec![note("n1", "the title", body, "2026-06-18T10:00:00Z")]);
    open_note_detail(
        &mut app,
        &client,
        note("n1", "the title", body, "2026-06-18T10:00:00Z"),
    );

    let text = render(&app, W, H);

    // Title and Created are still fully rendered (not pushed out or truncated by the growing
    // Content pane).
    assert!(
        text.contains("the title"),
        "the Title value still renders, not truncated:\n{text}",
    );
    assert!(
        text.contains("2026-06-18"),
        "the read-only Created timestamp still renders, not truncated:\n{text}",
    );

    // Every line of the multi-line Content renders on its own row (the '\n' line breaks are
    // honoured, not collapsed).
    for line in ["line one", "line two", "line three", "line four"] {
        assert!(
            text.contains(line),
            "Content line {line:?} renders:\n{text}"
        );
    }
    let l1 = first_row(&text, "line one");
    let l4 = first_row(&text, "line four");
    assert!(
        l1 < l4 && l4 != usize::MAX,
        "the Content lines render top-to-bottom across distinct rows (line one @ {l1}, four @ {l4})",
    );

    // The Content box spans more rows than the fixed 3-row Title box: measure the row span between
    // the Content label and the last Content line, which exceeds the 3-row Title box.
    let content_label = first_row(&text, "Content");
    let content_span = l4.saturating_sub(content_label);
    assert!(
        content_span >= 4,
        "the Content pane fills more than a fixed 3-row box (span {content_span} rows):\n{text}",
    );
}

#[test]
fn newline_inserts_a_line_break_into_the_content_buffer_and_renders_it() {
    // Acceptance 3: while editing the Content pane, `Event::Newline` (Enter, mapped per ADR-0011 §2)
    // pushes a '\n' into the edit buffer, and the rendered Content shows the break.
    let (client, mut app) = logged_in_notes(vec![note(
        "n1",
        "the title",
        "start",
        "2026-06-18T10:00:00Z",
    )]);
    open_note_detail(
        &mut app,
        &client,
        note("n1", "the title", "start", "2026-06-18T10:00:00Z"),
    );

    // Focus the Content pane, begin editing, clear the seeded value, then type "a", newline, "b".
    let _ = app.handle_event(Event::Next); // Title -> Content (Created skipped)
    let _ = app.handle_event(Event::BeginEditNote);
    assert!(
        notes_pane(&app).editing_content_pane(),
        "precondition: editing the multiline Content pane",
    );
    for _ in 0.."start".len() {
        let _ = app.handle_event(Event::Backspace);
    }
    // Distinctive tokens so the render-row assertion is unambiguous against the rest of the chrome.
    for c in "ALPHATOKEN".chars() {
        let _ = app.handle_event(Event::Char(c));
    }
    let _ = app.handle_event(Event::Newline);
    for c in "BRAVOTOKEN".chars() {
        let _ = app.handle_event(Event::Char(c));
    }

    // The edit buffer holds the embedded newline.
    let NotesMode::Detail(detail) = &notes_pane(&app).mode else {
        panic!("detail open");
    };
    assert_eq!(
        detail.edit.as_deref(),
        Some("ALPHATOKEN\nBRAVOTOKEN"),
        "Newline inserted a '\\n' into the Content edit buffer",
    );

    // And the in-progress buffer renders across two lines (the break is visible).
    let text = render(&app, W, H);
    let row_a = first_row(&text, "ALPHATOKEN");
    let row_b = first_row(&text, "BRAVOTOKEN");
    assert!(
        row_a != usize::MAX && row_b != usize::MAX && row_a < row_b,
        "the buffered line break renders as two rows (ALPHATOKEN @ {row_a}, BRAVOTOKEN @ {row_b}):\n{text}",
    );
}

#[test]
fn ctrl_s_commits_multiline_content_via_the_update_note_path() {
    // Acceptance 4: while editing Content, `Event::Commit` (Ctrl+S) commits the field via the
    // existing UpdateNote path, and the issued UpdateNoteRequest carries the multi-line content
    // (the untouched Title preserved from the snapshot, R5).
    let (client, mut app) = logged_in_notes(vec![note(
        "n1",
        "keep title",
        "old body",
        "2026-06-18T10:00:00Z",
    )]);
    open_note_detail(
        &mut app,
        &client,
        note("n1", "keep title", "old body", "2026-06-18T10:00:00Z"),
    );

    // Edit the Content pane into a multi-line value.
    let _ = app.handle_event(Event::Next); // focus Content
    let _ = app.handle_event(Event::BeginEditNote);
    for _ in 0.."old body".len() {
        let _ = app.handle_event(Event::Backspace);
    }
    let _ = app.handle_event(Event::Char('x'));
    let _ = app.handle_event(Event::Newline);
    let _ = app.handle_event(Event::Char('y'));

    // `Ctrl+S` (Event::Commit) commits via UpdateNote; the success re-derives the detail and chains
    // a list refresh.
    let committed = note("n1", "keep title", "x\ny", "2026-06-18T10:00:00Z");
    client.push_update_note(Ok(committed.clone()));
    client.push_notes(Ok(vec![committed]));
    submit(&mut app, &client, Event::Commit);

    // The issued UpdateNoteRequest carried the multi-line content and the preserved title.
    let calls = client.calls();
    let update = calls
        .iter()
        .find_map(|c| match c {
            Call::UpdateNote {
                note_id,
                title,
                content,
                ..
            } => Some((note_id.clone(), title.clone(), content.clone())),
            _ => None,
        })
        .expect("an UpdateNote call was made on Ctrl+S commit");
    assert_eq!(
        update,
        ("n1".to_owned(), "keep title".to_owned(), "x\ny".to_owned()),
        "Ctrl+S commits the multi-line Content via UpdateNote, preserving the Title (R5): {calls:?}",
    );
    assert!(
        matches!(calls.last(), Some(Call::ListNotes { .. })),
        "the Content commit chains a list refresh (statelessness, #1): {calls:?}",
    );

    // The detail stays open, re-derived from the returned note, edit buffer cleared.
    let NotesMode::Detail(detail) = &notes_pane(&app).mode else {
        panic!("the note detail stays open after a Ctrl+S commit");
    };
    assert!(
        !detail.is_editing(),
        "the edit buffer cleared after the Ctrl+S commit",
    );
    assert_eq!(
        detail.note.content, "x\ny",
        "the detail re-derived the multi-line content from the server note",
    );
}

#[test]
fn esc_cancels_a_content_edit_reverting_the_buffer_and_stays_in_the_detail() {
    // Acceptance 5: `Esc` while editing Content reverts the buffer (no commit) and stays in the
    // detail view — the first tier of the two-tiered Esc, on the multiline pane.
    let (client, mut app) = logged_in_notes(vec![note(
        "n1",
        "the title",
        "original body",
        "2026-06-18T10:00:00Z",
    )]);
    open_note_detail(
        &mut app,
        &client,
        note("n1", "the title", "original body", "2026-06-18T10:00:00Z"),
    );

    let _ = app.handle_event(Event::Next); // focus Content
    let _ = app.handle_event(Event::BeginEditNote);
    let _ = app.handle_event(Event::Newline);
    let _ = app.handle_event(Event::Char('Z'));
    let NotesMode::Detail(detail) = &notes_pane(&app).mode else {
        panic!("detail open");
    };
    assert_eq!(
        detail.edit.as_deref(),
        Some("original body\nZ"),
        "the Content edit buffer holds the in-progress multi-line value",
    );

    // Esc cancels the edit: the buffer drops, the snapshot is untouched, the detail stays open, and
    // no UpdateNote crossed the wire (no commit).
    let before = client.calls().len();
    let _ = app.handle_event(Event::Cancel);
    assert!(
        notes_pane(&app).detail_open(),
        "still in the note detail view after cancelling the Content edit",
    );
    let NotesMode::Detail(detail) = &notes_pane(&app).mode else {
        panic!("detail still open");
    };
    assert!(
        !detail.is_editing(),
        "the in-progress Content edit was cancelled (buffer dropped)",
    );
    assert_eq!(
        detail.note.content, "original body",
        "the Content reverted to the snapshot value (no commit)",
    );
    assert!(
        !app.should_quit(),
        "cancelling a Content edit does not quit"
    );
    let calls = client.calls();
    assert_eq!(
        calls.len(),
        before,
        "no request crossed the wire on an Esc-cancelled Content edit: {calls:?}",
    );
    assert!(
        !calls.iter().any(|c| matches!(c, Call::UpdateNote { .. })),
        "Esc cancel never issues an UpdateNote: {calls:?}",
    );
}

#[test]
fn title_pane_still_commits_on_enter_in_the_note_detail() {
    // Acceptance 6 (regression fork): outside the Content edit, Enter is NOT a newline — the
    // single-line Title pane still commits on Enter (Event::Submit) via UpdateNote.
    let (client, mut app) = logged_in_notes(vec![note(
        "n1",
        "old title",
        "the body",
        "2026-06-18T10:00:00Z",
    )]);
    open_note_detail(
        &mut app,
        &client,
        note("n1", "old title", "the body", "2026-06-18T10:00:00Z"),
    );

    // Edit the Title pane (focused on open) and commit with Enter (Submit).
    let _ = app.handle_event(Event::BeginEditNote);
    assert!(
        !notes_pane(&app).editing_content_pane(),
        "editing the single-line Title pane, not Content",
    );
    for _ in 0.."old title".len() {
        let _ = app.handle_event(Event::Backspace);
    }
    for c in "new title".chars() {
        let _ = app.handle_event(Event::Char(c));
    }

    let committed = note("n1", "new title", "the body", "2026-06-18T10:00:00Z");
    client.push_update_note(Ok(committed.clone()));
    client.push_notes(Ok(vec![committed]));
    submit(&mut app, &client, Event::Submit); // Enter commits the Title

    let calls = client.calls();
    let update = calls
        .iter()
        .find_map(|c| match c {
            Call::UpdateNote { title, content, .. } => Some((title.clone(), content.clone())),
            _ => None,
        })
        .expect("an UpdateNote call was made on the Enter commit of the Title");
    assert_eq!(
        update,
        ("new title".to_owned(), "the body".to_owned()),
        "Enter commits the Title field (Submit), preserving Content (R5): {calls:?}",
    );

    let NotesMode::Detail(detail) = &notes_pane(&app).mode else {
        panic!("the note detail stays open after the Enter commit");
    };
    assert_eq!(
        detail.note.title, "new title",
        "the detail re-derived the committed title from the server note",
    );
}

// ============================================================================
// Help body documents the Detail bindings (final hotkey scheme)
// ============================================================================

#[test]
fn help_modal_documents_the_detail_view_bindings() {
    let (_client, mut app) =
        logged_in_tasks(vec![open_task("t1", "the title", "2026-06-18T10:00:00Z")]);
    // Open help from the idle list (the help body is screen-independent).
    let _ = app.handle_event(Event::ToggleHelp);
    assert!(app.help_open(), "? opened the help overlay");
    let text = render(&app, W, H);
    assert!(
        text.contains("Detail"),
        "the help body has a Detail section for the per-field view:\n{text}",
    );
    // The final scheme's task/notes action keys are documented (Space done, d delete, Enter detail).
    assert!(
        text.contains("Space done") && text.contains("Enter detail"),
        "the help body documents the final task bindings (Space done / Enter detail):\n{text}",
    );
}
