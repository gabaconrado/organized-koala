//! The notes `TestBackend`/core suite (item 0010, slice 4t) — the ADR-0003 layer-2 home for
//! interactive notes behaviour, driven through the public two-step `App` API
//! (`handle_event` → synchronous executor → `apply_response`) against the held fake client:
//!
//! - list render: the notes view mirrors exactly the server's list, newest-first as returned;
//! - create: a `CreateNote` request is issued and the list reflects it after the chained refresh;
//! - edit: an `UpdateNote` request is issued and the change reflects in place after the refresh;
//! - delete: a `DeleteNote` request is issued and the note is removed from the list;
//! - empty-title validation: a `400 validation_failed` is routed inline to the open create/edit
//!   form, the sub-flow stays open, and the list is untouched;
//! - in-flight spinner / pending state while a request is outstanding;
//! - cancel / stale-RequestId drop: a late outcome for a superseded request is ignored;
//! - profile-scoping (#4): every note request the view issues carries the active `profile_id`.
//!
//! These exercise the notes surface with no live server and no worker thread — the only mock is
//! the sanctioned `Client` trait (the HTTP server), exactly as ADR-0003 / ADR-0006 prescribe.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

mod common;

use common::{
    Call, FakeClient, api_err, drive, execute, note, offline_err, profile, render, session, submit,
};
use contract::ErrorCode;
use tui::app::{App, Event, NotesMode, Screen};

const W: u16 = 80;
const H: u16 = 24;

/// Type a string into the focused field (local edits never dispatch).
fn type_str(app: &mut App, s: &str) {
    for c in s.chars() {
        let _ = app.handle_event(Event::Char(c));
    }
}

/// A freshly-logged-in app on the `work` task list, plus the shared fake. The login chain
/// (login → profiles → tasks) is scripted; the active profile is `p1`/`work`.
fn logged_in() -> (FakeClient, App) {
    let client = FakeClient::new();
    client.push_login(Ok(session("jwt")));
    client.push_profiles(Ok(vec![profile("p1", "work")]));
    client.push_tasks(Ok(vec![]));
    let mut app = App::new();
    submit(&mut app, &client, Event::Submit);
    assert!(matches!(app.screen(), Screen::TaskList(_)));
    (client, app)
}

/// Log in and navigate into the notes view, populated from the scripted `notes` list response.
fn enter_notes(notes: Vec<contract::Note>) -> (FakeClient, App) {
    let (client, mut app) = logged_in();
    client.push_notes(Ok(notes));
    submit(&mut app, &client, Event::OpenNotes);
    assert!(
        matches!(app.screen(), Screen::Notes(_)),
        "navigated into notes",
    );
    (client, app)
}

// ---- list render ----

#[test]
fn notes_list_view_mirrors_exactly_the_server_response() {
    // The rendered list equals what the server returned — order and count — newest-first as
    // returned, with no fabricated or cached entries (hard-constraint #1).
    let server_notes = vec![
        note("n2", "newer", "later body", "2026-06-18T12:00:00Z"),
        note("n1", "older", "earlier body", "2026-06-18T10:00:00Z"),
    ];
    let (_client, app) = enter_notes(server_notes.clone());

    let Screen::Notes(state) = app.screen() else {
        panic!("notes screen");
    };
    assert_eq!(
        state.notes, server_notes,
        "view is exactly the server's list"
    );
    assert_eq!(state.selected, Some(0), "first (newest) note selected");

    // Rendered newest-first: "newer" appears before "older" in the buffer text.
    let text = render(&app, W, H);
    let newer = text.find("newer").expect("newer rendered");
    let older = text.find("older").expect("older rendered");
    assert!(newer < older, "newest-first ordering preserved:\n{text}");
}

#[test]
fn opening_notes_lists_under_the_active_profile() {
    // Navigating into notes issues exactly a `ListNotes` for the active profile (#4).
    let (client, _app) = enter_notes(vec![note("n1", "a note", "body", "2026-06-18T10:00:00Z")]);
    let calls = client.calls();
    assert!(
        matches!(calls.last(), Some(Call::ListNotes { token, profile_id })
            if token == "jwt" && profile_id == "p1"),
        "notes listed for the active profile: {calls:?}",
    );
}

// ---- create ----

#[test]
fn create_posts_request_then_reflects_in_list_after_refresh() {
    let (client, mut app) = enter_notes(vec![]);

    // Script the create response and the post-create refresh list.
    let created = note("n-new", "Groceries", "milk, eggs", "2026-06-18T13:00:00Z");
    client.push_create_note(Ok(created.clone()));
    client.push_notes(Ok(vec![created]));

    // Open create, type title, switch field, type content, submit.
    let _ = app.handle_event(Event::BeginAddNote);
    type_str(&mut app, "Groceries");
    let _ = app.handle_event(Event::Next); // -> content field
    type_str(&mut app, "milk, eggs");
    submit(&mut app, &client, Event::Submit);

    // The create sub-flow closed and the list now shows the server's note.
    let Screen::Notes(state) = app.screen() else {
        panic!("notes screen");
    };
    assert!(
        matches!(state.mode, NotesMode::List),
        "create sub-flow closed after success",
    );
    assert_eq!(state.notes.len(), 1);
    assert_eq!(state.notes.first().expect("one note").title, "Groceries");

    // The create call carried Title + Content under the active profile, then a fresh list.
    let calls = client.calls();
    let create = calls
        .iter()
        .find_map(|c| match c {
            Call::CreateNote {
                profile_id,
                title,
                content,
                ..
            } => Some((profile_id.clone(), title.clone(), content.clone())),
            _ => None,
        })
        .expect("a create_note call was made");
    assert_eq!(
        create,
        (
            "p1".to_owned(),
            "Groceries".to_owned(),
            "milk, eggs".to_owned()
        ),
    );
    assert!(
        matches!(calls.last(), Some(Call::ListNotes { .. })),
        "a fresh list fetch follows the create (statelessness): {calls:?}",
    );

    // The rendered view shows the server-provided note — not anything fabricated.
    let text = render(&app, W, H);
    assert!(text.contains("Groceries"), "rendered from server:\n{text}");
}

// ---- edit ----

#[test]
fn edit_issues_update_and_reflects_change_in_place() {
    let (client, mut app) = enter_notes(vec![note(
        "n1",
        "Old title",
        "old body",
        "2026-06-18T10:00:00Z",
    )]);

    // Script the update response and the post-update refresh list (edit is in place: same id,
    // same created_at, new title/content).
    let updated = note("n1", "New title", "new body", "2026-06-18T10:00:00Z");
    client.push_update_note(Ok(updated.clone()));
    client.push_notes(Ok(vec![updated]));

    // Open the edit sub-flow on the selected note; the form prefilled from it.
    let _ = app.handle_event(Event::BeginEditNote);
    let Screen::Notes(state) = app.screen() else {
        panic!("notes screen");
    };
    let NotesMode::Editing { form, .. } = &state.mode else {
        panic!("edit sub-flow open with prefilled form");
    };
    assert_eq!(form.title, "Old title", "form prefilled from the note");
    assert_eq!(form.content, "old body");

    // Replace the title: clear it then type the new one, then submit.
    for _ in 0.."Old title".len() {
        let _ = app.handle_event(Event::Backspace);
    }
    type_str(&mut app, "New title");
    let _ = app.handle_event(Event::Next); // -> content
    for _ in 0.."old body".len() {
        let _ = app.handle_event(Event::Backspace);
    }
    type_str(&mut app, "new body");
    submit(&mut app, &client, Event::Submit);

    // Edit sub-flow closed; the row reflects the change in place (same single note, new title).
    let Screen::Notes(state) = app.screen() else {
        panic!("notes screen");
    };
    assert!(matches!(state.mode, NotesMode::List), "edit closed");
    assert_eq!(state.notes.len(), 1, "no extra/duplicate row");
    assert_eq!(state.notes.first().expect("one note").title, "New title");

    // The update call targeted the right note under the active profile, then a fresh list.
    let calls = client.calls();
    let update = calls
        .iter()
        .find_map(|c| match c {
            Call::UpdateNote {
                profile_id,
                note_id,
                title,
                content,
                ..
            } => Some((
                profile_id.clone(),
                note_id.clone(),
                title.clone(),
                content.clone(),
            )),
            _ => None,
        })
        .expect("an update_note call was made");
    assert_eq!(
        update,
        (
            "p1".to_owned(),
            "n1".to_owned(),
            "New title".to_owned(),
            "new body".to_owned(),
        ),
    );
    assert!(
        matches!(calls.last(), Some(Call::ListNotes { .. })),
        "a fresh list fetch follows the update: {calls:?}",
    );

    let text = render(&app, W, H);
    assert!(text.contains("New title"), "edited title rendered:\n{text}");
}

// ---- delete ----

#[test]
fn delete_issues_request_and_removes_from_list() {
    let (client, mut app) = enter_notes(vec![
        note("n1", "keep me", "body a", "2026-06-18T12:00:00Z"),
        note("n2", "remove me", "body b", "2026-06-18T10:00:00Z"),
    ]);

    // Select the second note, confirm delete; script the 204 and the post-delete refresh list.
    let _ = app.handle_event(Event::Next); // select "remove me"
    let _ = app.handle_event(Event::BeginDeleteNote);
    let Screen::Notes(state) = app.screen() else {
        panic!("notes screen");
    };
    assert!(
        matches!(&state.mode, NotesMode::ConfirmingDelete { note_id, .. } if note_id == "n2"),
        "delete confirmation targets the selected note",
    );

    client.push_delete_note(Ok(()));
    client.push_notes(Ok(vec![note(
        "n1",
        "keep me",
        "body a",
        "2026-06-18T12:00:00Z",
    )]));
    submit(&mut app, &client, Event::Submit); // confirm

    let Screen::Notes(state) = app.screen() else {
        panic!("notes screen");
    };
    assert!(matches!(state.mode, NotesMode::List), "delete flow closed");
    assert_eq!(state.notes.len(), 1, "deleted note removed from list");
    assert_eq!(state.notes.first().expect("one note").title, "keep me");

    // The delete call targeted the right note under the active profile, then a fresh list.
    let calls = client.calls();
    let delete = calls
        .iter()
        .find_map(|c| match c {
            Call::DeleteNote {
                profile_id,
                note_id,
                ..
            } => Some((profile_id.clone(), note_id.clone())),
            _ => None,
        })
        .expect("a delete_note call was made");
    assert_eq!(delete, ("p1".to_owned(), "n2".to_owned()));
    assert!(
        matches!(calls.last(), Some(Call::ListNotes { .. })),
        "a fresh list fetch follows the delete: {calls:?}",
    );

    let text = render(&app, W, H);
    assert!(
        !text.contains("remove me"),
        "deleted note no longer rendered:\n{text}",
    );
}

// ---- open / view a note (GetNote) ----

#[test]
fn open_selected_note_fetches_and_views_from_server() {
    // Opening a note issues a fresh `GetNote` (so the read-only view derives from a server
    // response, #1) and switches to the viewing sub-flow with the returned note.
    let (client, mut app) = enter_notes(vec![note(
        "n1",
        "Title",
        "stale body",
        "2026-06-18T10:00:00Z",
    )]);
    // The server's authoritative copy may differ from the cached list entry.
    client.push_get_note(Ok(note(
        "n1",
        "Title",
        "fresh body",
        "2026-06-18T10:00:00Z",
    )));
    submit(&mut app, &client, Event::Submit); // Enter opens the selected note

    let Screen::Notes(state) = app.screen() else {
        panic!("notes screen");
    };
    let NotesMode::Viewing(viewed) = &state.mode else {
        panic!("viewing the opened note");
    };
    assert_eq!(
        viewed.content, "fresh body",
        "view derives from the GetNote response"
    );

    let calls = client.calls();
    assert!(
        matches!(calls.last(), Some(Call::GetNote { token, profile_id, note_id })
            if token == "jwt" && profile_id == "p1" && note_id == "n1"),
        "GetNote targeted the selected note under the active profile: {calls:?}",
    );

    let text = render(&app, W, H);
    assert!(text.contains("fresh body"), "viewed note rendered:\n{text}");
}

// ---- empty-title validation routed inline ----

#[test]
fn empty_title_validation_surfaces_inline_in_create_form() {
    let (client, mut app) = enter_notes(vec![]);

    // The server rejects an empty title with 400 validation_failed; no refresh follows.
    client.push_create_note(Err(api_err(
        ErrorCode::ValidationFailed,
        "title must not be empty",
    )));

    // Open create, leave title empty, submit.
    let _ = app.handle_event(Event::BeginAddNote);
    submit(&mut app, &client, Event::Submit);

    // The create sub-flow stays open with the error surfaced inline on the form; list untouched.
    let Screen::Notes(state) = app.screen() else {
        panic!("notes screen");
    };
    let NotesMode::Creating(form) = &state.mode else {
        panic!("create sub-flow stays open after a validation error");
    };
    assert_eq!(
        form.error.as_deref(),
        Some("title must not be empty"),
        "validation error routed inline to the form",
    );
    assert!(
        state.notes.is_empty(),
        "list untouched by the rejected create"
    );
    assert!(!app.is_pending(), "settled — no request in flight");

    // Exactly one create attempt crossed the wire; no list refresh followed the rejection.
    let calls = client.calls();
    assert!(
        matches!(calls.last(), Some(Call::CreateNote { .. })),
        "no refresh follows a rejected create: {calls:?}",
    );

    // The inline error is rendered on the message line.
    let text = render(&app, W, H);
    assert!(
        text.contains("title must not be empty"),
        "inline error rendered:\n{text}",
    );
}

#[test]
fn empty_title_validation_surfaces_inline_in_edit_form() {
    let (client, mut app) = enter_notes(vec![note(
        "n1",
        "Old title",
        "body",
        "2026-06-18T10:00:00Z",
    )]);
    client.push_update_note(Err(api_err(
        ErrorCode::ValidationFailed,
        "title must not be empty",
    )));

    // Open edit, clear the title, submit.
    let _ = app.handle_event(Event::BeginEditNote);
    for _ in 0.."Old title".len() {
        let _ = app.handle_event(Event::Backspace);
    }
    submit(&mut app, &client, Event::Submit);

    let Screen::Notes(state) = app.screen() else {
        panic!("notes screen");
    };
    let NotesMode::Editing { form, .. } = &state.mode else {
        panic!("edit sub-flow stays open after a validation error");
    };
    assert_eq!(
        form.error.as_deref(),
        Some("title must not be empty"),
        "validation error routed inline to the edit form",
    );
    assert_eq!(
        state.notes.first().expect("note still present").title,
        "Old title",
        "the unedited note is unchanged",
    );
}

// ---- in-flight spinner / pending ----

#[test]
fn create_shows_pending_and_spinner_while_outstanding() {
    let (client, mut app) = enter_notes(vec![]);

    // Open create, type a title, then submit but hold the dispatch (don't drive it).
    let _ = app.handle_event(Event::BeginAddNote);
    type_str(&mut app, "Groceries");
    let dispatch = app
        .handle_event(Event::Submit)
        .expect("create submit dispatches");
    assert!(app.is_pending(), "create request is in flight");

    // The spinner glyph is appended to the caption while pending (tick 1 → "/"); the caption may
    // wrap, so assert the glyph and the cancel affordance keyword are both present.
    let text = common::render_at(&app, W, H, 1);
    assert!(
        text.contains('/') && text.contains("cancel"),
        "spinner + cancel affordance shown while pending:\n{text}",
    );

    // A request-triggering event while pending is a no-op (no new dispatch, no new call).
    let calls_before = client.calls().len();
    assert!(
        app.handle_event(Event::Refresh).is_none(),
        "refresh while pending dispatches nothing",
    );
    assert_eq!(
        client.calls().len(),
        calls_before,
        "no extra call while pending"
    );

    // Completing the held request settles the flow (create → chained list refresh).
    client.push_create_note(Ok(note("n-new", "Groceries", "", "2026-06-18T13:00:00Z")));
    client.push_notes(Ok(vec![note(
        "n-new",
        "Groceries",
        "",
        "2026-06-18T13:00:00Z",
    )]));
    drive(&mut app, &client, dispatch);
    assert!(!app.is_pending(), "settled after the request completes");
}

// ---- cancel / stale-RequestId drop ----

#[test]
fn stale_delete_response_after_cancel_is_dropped() {
    let (client, mut app) =
        enter_notes(vec![note("n1", "Original", "body", "2026-06-18T10:00:00Z")]);

    // Begin a delete; capture the dispatch the worker would run.
    let _ = app.handle_event(Event::BeginDeleteNote);
    let dispatch = app
        .handle_event(Event::Submit)
        .expect("delete confirm dispatches");
    assert!(app.is_pending(), "delete is in flight");

    // User cancels before the response arrives: the in-flight marker is cleared.
    assert!(app.handle_event(Event::Cancel).is_none());
    assert!(!app.is_pending(), "cancelled");

    // The abandoned request still ran on the (mocked) server and produces a now-stale response.
    // Applying it must be a no-op (the id mismatches): the note must NOT be removed.
    client.push_delete_note(Ok(()));
    let stale = execute(&client, dispatch);
    let follow_up = app.apply_response(stale);

    assert!(follow_up.is_none(), "a stale response yields no follow-up");
    let Screen::Notes(state) = app.screen() else {
        panic!("still on the notes screen");
    };
    assert_eq!(
        state.notes.len(),
        1,
        "the dropped stale delete left the note in place"
    );
    assert_eq!(state.notes.first().expect("note present").id, "n1");
    assert!(
        !app.is_pending(),
        "still idle after dropping the stale response"
    );
}

#[test]
fn superseded_response_after_new_request_is_dropped() {
    // Cancel a delete, then start a fresh refresh (new RequestId). The first request's late
    // response carries the old id and must be dropped, not mis-applied to the new in-flight slot.
    let (client, mut app) =
        enter_notes(vec![note("n1", "Original", "body", "2026-06-18T10:00:00Z")]);

    let _ = app.handle_event(Event::BeginDeleteNote);
    let first = app
        .handle_event(Event::Submit)
        .expect("first delete dispatches");
    assert!(app.handle_event(Event::Cancel).is_none());
    assert!(!app.is_pending(), "first delete cancelled");

    // Re-confirm the (still-open) delete after cancel — a fresh RequestId, now the awaited one.
    let second = app
        .handle_event(Event::Submit)
        .expect("re-confirm delete dispatches");
    assert!(app.is_pending());

    // The first (cancelled) delete's response arrives late: dropped; the second still awaited.
    client.push_delete_note(Ok(()));
    let stale = execute(&client, first);
    assert!(
        app.apply_response(stale).is_none(),
        "the superseded response is dropped",
    );
    assert!(app.is_pending(), "the new request is still in flight");

    // The new (re-confirm) request then completes normally — the delete + chained list refresh
    // drive the view to the post-delete list.
    client.push_delete_note(Ok(()));
    client.push_notes(Ok(vec![note(
        "n2",
        "fresh",
        "body",
        "2026-06-18T15:00:00Z",
    )]));
    drive(&mut app, &client, second);
    let Screen::Notes(state) = app.screen() else {
        panic!("notes screen");
    };
    assert_eq!(
        state.notes.first().expect("note").title,
        "fresh",
        "the new request's response drove the view, not the stale one",
    );
    assert!(!app.is_pending());
}

// ---- profile-scoping (#4) ----

#[test]
fn every_note_request_carries_the_active_profile_id() {
    // Across list, create, edit, delete and open, every note request the view issues is scoped to
    // the active profile `p1` — never cross-profile (hard-constraint #4).
    let (client, mut app) = enter_notes(vec![note("n1", "First", "body", "2026-06-18T10:00:00Z")]);

    // Create.
    client.push_create_note(Ok(note("n2", "Second", "", "2026-06-18T11:00:00Z")));
    client.push_notes(Ok(vec![
        note("n2", "Second", "", "2026-06-18T11:00:00Z"),
        note("n1", "First", "body", "2026-06-18T10:00:00Z"),
    ]));
    let _ = app.handle_event(Event::BeginAddNote);
    type_str(&mut app, "Second");
    submit(&mut app, &client, Event::Submit);

    // Open (GetNote) the selected (newest) note.
    client.push_get_note(Ok(note("n2", "Second", "", "2026-06-18T11:00:00Z")));
    submit(&mut app, &client, Event::Submit);
    // Back to the list to continue.
    let _ = app.handle_event(Event::Cancel);

    // Edit the selected note.
    client.push_update_note(Ok(note("n2", "Second!", "", "2026-06-18T11:00:00Z")));
    client.push_notes(Ok(vec![
        note("n2", "Second!", "", "2026-06-18T11:00:00Z"),
        note("n1", "First", "body", "2026-06-18T10:00:00Z"),
    ]));
    let _ = app.handle_event(Event::BeginEditNote);
    submit(&mut app, &client, Event::Submit);

    // Delete the selected note.
    client.push_delete_note(Ok(()));
    client.push_notes(Ok(vec![note(
        "n1",
        "First",
        "body",
        "2026-06-18T10:00:00Z",
    )]));
    let _ = app.handle_event(Event::BeginDeleteNote);
    submit(&mut app, &client, Event::Submit);

    // Every note-bearing call carried profile_id == "p1".
    for call in client.calls() {
        let scoped = match call {
            Call::ListNotes { profile_id, .. }
            | Call::CreateNote { profile_id, .. }
            | Call::GetNote { profile_id, .. }
            | Call::UpdateNote { profile_id, .. }
            | Call::DeleteNote { profile_id, .. } => Some(profile_id),
            _ => None,
        };
        if let Some(profile_id) = scoped {
            assert_eq!(
                profile_id, "p1",
                "note request must be scoped to the active profile"
            );
        }
    }
}

// ---- error routing: offline → blocking screen ----

#[test]
fn offline_during_list_refresh_routes_to_blocking_screen() {
    // A transport failure on a notes refresh routes to the blocking offline screen (not inline),
    // mirroring the post-auth error routing for tasks.
    let (client, mut app) = enter_notes(vec![note("n1", "a note", "body", "2026-06-18T10:00:00Z")]);
    client.push_notes(Err(offline_err("connection refused")));
    submit(&mut app, &client, Event::Refresh);

    assert!(
        matches!(app.screen(), Screen::Offline { .. }),
        "offline transport failure routes to the blocking screen",
    );
}
