//! The movable-caret `TestBackend`/core suite (item 0025) — the ADR-0003 layer-2 home for the
//! shared [`TextInput`] primitive's *observable* behaviour, driven through the public two-step
//! `App` API (`handle_event` → synchronous executor → `apply_response`) against the held fake
//! client. The primitive's char-boundary / scroll math is unit-tested in its own source-owned
//! `crates/tui/src/app/text_input/tests.rs`; this suite pins the behaviour a user actually sees:
//!
//! - caret movement (Left/Right, Home/End) in a single-line field, acting mid-buffer;
//! - mid-buffer insert, Backspace, and forward Delete acting at the caret (not just end-of-buffer);
//! - the **rendered caret cell** placed via `frame.set_cursor_position`, read back through the
//!   `TestBackend` terminal cursor position — for an unmasked field, a masked password field, and
//!   the multiline note-detail Content pane;
//! - multiline Up/Down caret line-movement and **scroll-to-caret** when Content exceeds the pane;
//! - UTF-8 / multi-byte caret safety end-to-end (no panic, correct string, stable render).
//!
//! No live server and no worker thread — the only mock is the sanctioned `Client` trait (the HTTP
//! server), exactly as ADR-0003 / ADR-0006 prescribe.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

mod common;

use common::{
    FakeClient, note, notes_pane, on_tab, profile, render, render_cursor, session, submit,
};
use tui::app::{App, AuthState, Event, NotePane, NotesMode, Screen, Tab, TextInput};

const W: u16 = 80;
const H: u16 = 24;

/// Type a string into the focused field, one `Char` event per character (local edits, never
/// dispatched).
fn type_str(app: &mut App, s: &str) {
    for c in s.chars() {
        let _ = app.handle_event(Event::Char(c));
    }
}

/// A freshly-logged-in app on the `work` Tasks tab, plus the shared fake (login → profiles →
/// tasks). The active profile is `p1`/`work`.
fn logged_in() -> (FakeClient, App) {
    let client = FakeClient::new();
    client.push_login(Ok(session("jwt")));
    client.push_profiles(Ok(vec![profile("p1", "work")]));
    client.push_tasks(Ok(vec![]));
    let mut app = App::new();
    submit(&mut app, &client, Event::Submit);
    assert!(
        matches!(app.screen(), Screen::Main(_)),
        "reached the main view"
    );
    (client, app)
}

/// Log in and switch to the Notes tab (`Tab` cycles Tasks→Notes), populated from the scripted list.
fn enter_notes(notes: Vec<contract::Note>) -> (FakeClient, App) {
    let (client, mut app) = logged_in();
    client.push_notes(Ok(notes));
    submit(&mut app, &client, Event::NextTab); // Tasks -> Notes
    assert!(on_tab(&app, Tab::Notes), "switched to the Notes tab");
    (client, app)
}

/// Open the create-note dialog (single-line Title focused).
fn open_create_dialog(app: &mut App) {
    let _ = app.handle_event(Event::BeginAddNote);
    assert!(
        matches!(notes_pane(app).mode, NotesMode::Creating(_)),
        "create dialog open",
    );
}

/// The create dialog's Title `TextInput`, panicking if the create dialog is not open.
fn create_title(app: &App) -> &TextInput {
    match &notes_pane(app).mode {
        NotesMode::Creating(form) => &form.title,
        other => panic!("expected the create dialog, got {other:?}"),
    }
}

/// The auth screen state, panicking if the app is not on the auth screen.
fn auth(app: &App) -> &AuthState {
    match app.screen() {
        Screen::Auth(auth) => auth,
        other => panic!("expected the auth screen, got {other:?}"),
    }
}

/// Open a note's detail view and begin editing its multiline Content pane; returns the
/// (client, app) with the Content edit buffer live (seeded from `content`).
fn editing_content(content: &str) -> (FakeClient, App) {
    let seeded = note("n1", "a note", content, "2026-06-18T10:00:00Z");
    let (client, mut app) = enter_notes(vec![seeded.clone()]);
    // Open the detail view (a fresh GetNote, #1) on the selected note.
    client.push_get_note(Ok(seeded));
    submit(&mut app, &client, Event::Submit);
    assert!(
        matches!(notes_pane(&app).mode, NotesMode::Detail(_)),
        "detail view open",
    );
    // Cycle Title -> Content (the read-only Created pane is skipped) and begin the field edit.
    let _ = app.handle_event(Event::Next);
    assert_eq!(
        detail_focused_pane(&app),
        NotePane::Content,
        "focused the multiline Content pane",
    );
    let _ = app.handle_event(Event::BeginEditNote);
    assert!(detail_editing(&app), "Content edit buffer is live");
    (client, app)
}

fn detail_focused_pane(app: &App) -> NotePane {
    match &notes_pane(app).mode {
        NotesMode::Detail(d) => d.focused_pane(),
        other => panic!("expected the detail view, got {other:?}"),
    }
}

fn detail_editing(app: &App) -> bool {
    matches!(&notes_pane(app).mode, NotesMode::Detail(d) if d.is_editing())
}

/// The live Content edit buffer, panicking if the detail view is not editing.
fn content_edit(app: &App) -> &TextInput {
    match &notes_pane(app).mode {
        NotesMode::Detail(d) => d.edit.as_ref().expect("Content edit buffer live"),
        other => panic!("expected the detail view, got {other:?}"),
    }
}

// ---- single-line caret movement + mid-buffer edit (note create dialog Title) ----

#[test]
fn left_right_move_the_caret_and_insert_lands_mid_buffer() {
    let (_client, mut app) = enter_notes(vec![]);
    open_create_dialog(&mut app);
    type_str(&mut app, "abc");
    assert_eq!(
        create_title(&app).caret(),
        3,
        "caret parks at the end after typing"
    );

    // Two lefts: caret between 'a' and 'b'.
    let _ = app.handle_event(Event::MoveLeft);
    let _ = app.handle_event(Event::MoveLeft);
    assert_eq!(
        create_title(&app).caret(),
        1,
        "two lefts move the caret to index 1"
    );

    // Insert lands at the caret, not the end.
    let _ = app.handle_event(Event::Char('X'));
    assert_eq!(
        create_title(&app).as_str(),
        "aXbc",
        "insert acted mid-buffer"
    );
    assert_eq!(
        create_title(&app).caret(),
        2,
        "caret advanced past the inserted char"
    );

    // One right, then insert again.
    let _ = app.handle_event(Event::MoveRight);
    let _ = app.handle_event(Event::Char('Y'));
    assert_eq!(
        create_title(&app).as_str(),
        "aXbYc",
        "second insert acted at the moved caret"
    );
}

#[test]
fn home_and_end_jump_to_the_line_bounds() {
    let (_client, mut app) = enter_notes(vec![]);
    open_create_dialog(&mut app);
    type_str(&mut app, "hello");

    let _ = app.handle_event(Event::MoveHome);
    assert_eq!(create_title(&app).caret(), 0, "Home jumps to the start");
    let _ = app.handle_event(Event::Char('>'));
    assert_eq!(
        create_title(&app).as_str(),
        ">hello",
        "insert at the line start"
    );

    let _ = app.handle_event(Event::MoveEnd);
    assert_eq!(
        create_title(&app).caret(),
        6,
        "End jumps past the last char"
    );
    let _ = app.handle_event(Event::Char('<'));
    assert_eq!(
        create_title(&app).as_str(),
        ">hello<",
        "insert at the line end"
    );
}

#[test]
fn backspace_and_forward_delete_act_at_the_caret() {
    let (_client, mut app) = enter_notes(vec![]);
    open_create_dialog(&mut app);
    type_str(&mut app, "abcd");

    // Caret between 'b' and 'c' (index 2).
    let _ = app.handle_event(Event::MoveLeft);
    let _ = app.handle_event(Event::MoveLeft);
    assert_eq!(create_title(&app).caret(), 2);

    // Backspace deletes the char BEFORE the caret ('b'), not the end.
    let _ = app.handle_event(Event::Backspace);
    assert_eq!(
        create_title(&app).as_str(),
        "acd",
        "backspace removed the pre-caret char"
    );
    assert_eq!(
        create_title(&app).caret(),
        1,
        "caret followed the deletion left"
    );

    // Forward Delete removes the char AT the caret ('c'); the caret does not move.
    let _ = app.handle_event(Event::Delete);
    assert_eq!(
        create_title(&app).as_str(),
        "ad",
        "forward delete removed the at-caret char"
    );
    assert_eq!(
        create_title(&app).caret(),
        1,
        "forward delete left the caret in place"
    );

    // Forward Delete at the end of the buffer is a no-op.
    let _ = app.handle_event(Event::MoveEnd);
    let _ = app.handle_event(Event::Delete);
    assert_eq!(
        create_title(&app).as_str(),
        "ad",
        "forward delete at end is inert"
    );
}

// ---- rendered caret position via the TestBackend terminal cursor ----

#[test]
fn rendered_caret_tracks_the_edit_position_in_a_dialog_field() {
    let (_client, mut app) = enter_notes(vec![]);
    open_create_dialog(&mut app);
    type_str(&mut app, "hello");

    // The caret renders at the end of the typed text.
    let (x_end, y_end) = render_cursor(&app, W, H);

    // Two lefts move the rendered caret exactly two columns left, same row.
    let _ = app.handle_event(Event::MoveLeft);
    let _ = app.handle_event(Event::MoveLeft);
    let (x_mid, y_mid) = render_cursor(&app, W, H);
    assert_eq!(y_mid, y_end, "caret stays on the field's row");
    assert_eq!(x_end - x_mid, 2, "rendered caret moved two columns left");

    // Home snaps the rendered caret to the field's content start (leftmost cell of the value).
    let _ = app.handle_event(Event::MoveHome);
    let (x_home, y_home) = render_cursor(&app, W, H);
    assert_eq!(y_home, y_end, "Home keeps the caret on the field row");
    assert!(
        x_home < x_mid,
        "Home moved the caret to the field start (leftmost)"
    );

    // End snaps back to the end column.
    let _ = app.handle_event(Event::MoveEnd);
    let (x_back, _) = render_cursor(&app, W, H);
    assert_eq!(
        x_back, x_end,
        "End returns the rendered caret to the end column"
    );
}

#[test]
fn masked_password_caret_maps_one_to_one_over_the_asterisks() {
    // The auth screen starts on the login form; move focus to the (masked) Password field.
    let mut app = App::new();
    let _ = app.handle_event(Event::Next); // Identifier -> Password
    type_str(&mut app, "secret");

    // The value renders masked, but the caret column still maps 1:1 to the char index.
    let text = render(&app, W, H);
    assert!(
        text.contains("******"),
        "password renders as asterisks:\n{text}"
    );
    assert!(
        !text.contains("secret"),
        "the plaintext never renders:\n{text}"
    );

    let (x_end, y_end) = render_cursor(&app, W, H);
    let _ = app.handle_event(Event::MoveLeft);
    let _ = app.handle_event(Event::MoveLeft);
    let _ = app.handle_event(Event::MoveLeft);
    let (x_mid, y_mid) = render_cursor(&app, W, H);
    assert_eq!(y_mid, y_end, "masked caret stays on the password row");
    assert_eq!(
        x_end - x_mid,
        3,
        "masked caret moved three columns left, 1:1 over the mask"
    );

    // The underlying value is intact and edits still act at the caret.
    let _ = app.handle_event(Event::Char('X'));
    assert_eq!(
        auth(&app).password.as_str(),
        "secXret",
        "insert acted mid-buffer under the mask"
    );
}

// ---- multiline caret: Up/Down line-move + scroll-to-caret (note detail Content) ----

#[test]
fn up_and_down_move_the_caret_between_content_lines() {
    let (_client, mut app) = editing_content("line one\nline two\nline three");
    // begin_edit seeds the caret at the end (last line, "line three").
    let end_caret = content_edit(&app).caret();

    // Up moves the caret off the last line (fewer chars precede it).
    let _ = app.handle_event(Event::MoveUp);
    let up_once = content_edit(&app).caret();
    assert!(up_once < end_caret, "Up moved the caret to an earlier line");

    // Another Up moves it earlier still.
    let _ = app.handle_event(Event::MoveUp);
    let up_twice = content_edit(&app).caret();
    assert!(
        up_twice < up_once,
        "a second Up moved the caret up another line"
    );

    // Down brings it back toward the end.
    let _ = app.handle_event(Event::MoveDown);
    let down_once = content_edit(&app).caret();
    assert!(
        down_once > up_twice,
        "Down moved the caret back down a line"
    );

    // A mid-line insert (on the middle line) proves Up/Down land mid-buffer, not at the end.
    let _ = app.handle_event(Event::MoveHome);
    let _ = app.handle_event(Event::Char('*'));
    assert!(
        content_edit(&app).as_str().contains("\n*"),
        "insert landed at a line start mid-buffer: {:?}",
        content_edit(&app).as_str(),
    );
}

#[test]
fn content_pane_scrolls_to_keep_the_caret_in_view() {
    // Content taller than the detail Content pane (H = 24 leaves it well under 40 rows).
    let long: String = (1..=40)
        .map(|i| format!("row{i:02}"))
        .collect::<Vec<_>>()
        .join("\n");
    let (_client, mut app) = editing_content(&long);

    // begin_edit parks the caret at the end: the pane is scrolled to the tail, so the last row is
    // visible and the first row has scrolled off the top.
    let tail = render(&app, W, H);
    assert!(
        tail.contains("row40"),
        "the caret's (last) line is visible:\n{tail}"
    );
    assert!(
        !tail.contains("row01"),
        "the far-off first line scrolled out of view:\n{tail}"
    );
    let (_x_end, y_end) = render_cursor(&app, W, H);

    // Walk the caret to the top (more Ups than there are lines; extra Ups clamp at the start).
    for _ in 0..40 {
        let _ = app.handle_event(Event::MoveUp);
    }
    assert_eq!(
        content_edit(&app).caret(),
        0,
        "the caret reached the buffer start"
    );

    // The pane scrolled back to the top: the first row is visible, the last has scrolled off.
    let head = render(&app, W, H);
    assert!(
        head.contains("row01"),
        "the top scrolled into view:\n{head}"
    );
    assert!(
        !head.contains("row40"),
        "the tail scrolled out of view:\n{head}"
    );

    // And the rendered caret sits higher on the screen than it did at the tail.
    let (_x_top, y_top) = render_cursor(&app, W, H);
    assert!(
        y_top < y_end,
        "the rendered caret row rose as the pane scrolled to the top"
    );
}

// ---- UTF-8 / multi-byte caret safety end-to-end ----

#[test]
fn multibyte_caret_edits_are_char_safe() {
    let (_client, mut app) = enter_notes(vec![]);
    open_create_dialog(&mut app);
    type_str(&mut app, "café"); // 4 chars, 5 bytes ('é' is 2 bytes)
    assert_eq!(
        create_title(&app).caret(),
        4,
        "caret counts characters, not bytes"
    );

    // Caret just before the multi-byte 'é'.
    let _ = app.handle_event(Event::MoveLeft);
    assert_eq!(create_title(&app).caret(), 3);

    // Insert an ASCII char immediately before 'é' — no panic, no split of the multi-byte char. The
    // caret advances past the inserted 'x', leaving it just before 'é'.
    let _ = app.handle_event(Event::Char('x'));
    assert_eq!(
        create_title(&app).as_str(),
        "cafxé",
        "inserted before the multi-byte char"
    );
    assert_eq!(
        create_title(&app).caret(),
        4,
        "caret sits just before the multi-byte 'é'"
    );

    // Forward-delete removes the whole 'é' (one char, two bytes) cleanly.
    let _ = app.handle_event(Event::Delete);
    assert_eq!(
        create_title(&app).as_str(),
        "cafx",
        "forward-delete removed the multi-byte char"
    );

    // Backspace over a multi-byte char from the end is also clean.
    let (_client2, mut app2) = enter_notes(vec![]);
    open_create_dialog(&mut app2);
    type_str(&mut app2, "naïve");
    let _ = app2.handle_event(Event::MoveHome);
    let _ = app2.handle_event(Event::MoveRight);
    let _ = app2.handle_event(Event::MoveRight); // caret after 'a', before 'ï'
    let _ = app2.handle_event(Event::Delete); // remove 'ï'
    assert_eq!(
        create_title(&app2).as_str(),
        "nave",
        "multi-byte forward-delete mid-buffer is safe"
    );

    // The rendered caret is well-defined over multi-byte content (no panic in the draw path).
    let (_x, _y) = render_cursor(&app2, W, H);
}
