//! The 0015 dialog-system `TestBackend`/core suite — the ADR-0003 layer-2 home for the modal
//! framework introduced in item 0015 (slice-5 acceptance). Driven through the public two-step `App`
//! API (`handle_event` → synchronous executor → `apply_response`) against the held fake client,
//! and through the pure `ui::draw` path onto a `TestBackend`; nothing internal is mocked.
//!
//! Covers (per the slice-5 plan):
//! - **Modal rendering:** every add/edit/delete/timer/help flow renders a *centred* dialog (title +
//!   border present, centred), and the sub-flow text is no longer in the 2-row message band.
//! - **Trimmed footer:** the footer shows only movement + tab-switch + `?` + `q` (+ spinner when
//!   pending), never the per-pane action keys.
//! - **Global-hotkey suppression:** with a dialog open, a typed character lands in the focused
//!   field (`overlay_capturing_input()` exercised end-to-end through `map_key`).
//! - **Two-tiered `Esc`:** Esc with a dialog open cancels it (no Quit); Esc with no overlay on a
//!   post-auth screen still quits; Esc with a request in flight still cancels the request.
//! - **`?` help modal:** opens on an idle post-auth screen, lists the full reference, and closes on
//!   `Esc` *or* a second `?` (the keymap's `?` arm fires on `globals_live || help_open`, so the
//!   advertised `?/Esc: close` affordance works from the keyboard); inert while a *non-help* dialog
//!   captures input (A3 — both `globals_live` and `help_open` are false there).
//! - **Purple focus border:** the focused field's border row carries the magenta fg (auth form +
//!   a dialog); a non-focused field's does not.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

mod common;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::style::Color;

use common::{
    FakeClient, note, on_tab, profile, render, render_buffer, row_fg_count, screen_name, session,
    submit,
};
use tui::app::{App, Event, Screen, Tab};
use tui::terminal::map_key;

const W: u16 = 80;
const H: u16 = 24;

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

/// A freshly-logged-in app on the `work` Tasks tab, plus the shared fake (login → profiles →
/// tasks chain scripted; active profile `p1`/`work`).
fn logged_in(tasks: Vec<contract::Task>) -> (FakeClient, App) {
    let client = FakeClient::new();
    client.push_login(Ok(session("jwt")));
    client.push_profiles(Ok(vec![profile("p1", "work")]));
    client.push_tasks(Ok(tasks));
    let mut app = App::new();
    submit(&mut app, &client, Event::Submit);
    assert!(
        matches!(app.screen(), Screen::Main(_)),
        "on the tabbed view"
    );
    (client, app)
}

/// Feed a key through the *real* keymap (`map_key`, using the app's own overlay/editing predicates,
/// exactly as the poll loop does) and then through `handle_event`, driving any dispatch to
/// completion. This is the end-to-end path: it proves the suppression rule (`map_key`) and the
/// update folding agree, with no shortcut around the keymap.
fn press(app: &mut App, client: &FakeClient, code: KeyCode) {
    if let Some(event) = map_key(
        app.screen(),
        app.overlay_capturing_input(),
        app.help_open(),
        app.is_editing_duration(),
        key(code),
    ) {
        submit(app, client, event);
    }
}

// ---- Modal rendering: each flow renders a centred dialog, not message-band text ----

#[test]
fn add_task_renders_a_centred_dialog_not_message_band_text() {
    let (_client, mut app) = logged_in(vec![]);
    let _ = app.handle_event(Event::BeginAddTask);
    let buffer = render_buffer(&app, W, H);
    let text = render(&app, W, H);

    // The dialog title + its fields render.
    assert!(text.contains("Add task"), "dialog title present:\n{text}");
    assert!(text.contains("Title"), "title field present:\n{text}");
    assert!(text.contains("Description"), "description field:\n{text}");

    // Centred: the dialog title sits well below the top and is horizontally indented (a centred
    // floating box, not a flush message-band line just above the footer).
    let rows: Vec<&str> = text.lines().collect();
    let title_row = rows
        .iter()
        .position(|r| r.contains("Add task"))
        .expect("dialog title row");
    assert!(
        title_row > 2 && title_row < rows.len().saturating_sub(2),
        "dialog is vertically centred (row {title_row} of {}):\n{text}",
        rows.len(),
    );
    let title_line = rows.get(title_row).copied().unwrap_or("");
    let indent = title_line.len() - title_line.trim_start().len();
    assert!(indent > 0, "dialog is horizontally indented:\n{text}");

    // The dialog box border is magenta (the floating-modal chrome): its title row carries magenta.
    assert!(
        row_fg_count(&buffer, "Add task", Color::Magenta) > 0,
        "the dialog box has a (magenta) border:\n{text}",
    );

    // The 2-row message band (the band directly above the footer) does NOT carry the dialog text —
    // the sub-flow no longer renders inline there.
    let footer_row = rows
        .iter()
        .position(|r| r.contains("switch tab"))
        .expect("footer caption row");
    let band: String = rows
        .iter()
        .skip(footer_row.saturating_sub(2))
        .take(2)
        .copied()
        .collect();
    assert!(
        !band.contains("Add task") && !band.contains("Title"),
        "the add-task sub-flow is NOT in the message band:\n{band:?}",
    );
}

#[test]
fn edit_task_renders_a_centred_dialog() {
    let (_client, mut app) = logged_in(vec![common::today_open_task("t1", "buy milk", "10:00:00")]);
    let _ = app.handle_event(Event::BeginEditTask);
    let text = render(&app, W, H);
    assert!(text.contains("Edit task"), "edit dialog title:\n{text}");
    assert!(text.contains("buy milk"), "prefilled title echoes:\n{text}");
}

#[test]
fn task_delete_renders_a_confirmation_dialog_when_armed() {
    let (_client, mut app) = logged_in(vec![common::today_open_task("t1", "doomed", "10:00:00")]);
    // First `x` arms the confirmation (no dispatch); it renders as a dialog.
    assert!(app.handle_event(Event::DeleteSelected).is_none());
    let text = render(&app, W, H);
    assert!(text.contains("Delete task"), "delete dialog title:\n{text}");
    assert!(
        text.contains("Delete this task?"),
        "confirmation prompt:\n{text}",
    );
}

#[test]
fn add_note_renders_a_centred_dialog() {
    let (client, mut app) = logged_in(vec![]);
    client.push_notes(Ok(vec![]));
    submit(&mut app, &client, Event::NextTab); // -> Notes
    assert!(on_tab(&app, Tab::Notes));
    let _ = app.handle_event(Event::BeginAddNote);
    let text = render(&app, W, H);
    assert!(text.contains("New note"), "note dialog title:\n{text}");
    assert!(text.contains("Content"), "content field present:\n{text}");
}

#[test]
fn note_delete_renders_a_confirmation_dialog() {
    let (client, mut app) = logged_in(vec![]);
    client.push_notes(Ok(vec![note(
        "n1",
        "keep me",
        "body",
        "2026-06-18T10:00:00Z",
    )]));
    submit(&mut app, &client, Event::NextTab); // -> Notes
    let _ = app.handle_event(Event::BeginDeleteNote);
    let text = render(&app, W, H);
    assert!(text.contains("Delete note"), "delete dialog title:\n{text}");
    assert!(
        text.contains("keep me"),
        "the named note is in the prompt:\n{text}",
    );
}

#[test]
fn add_profile_renders_a_centred_dialog() {
    let (client, mut app) = logged_in(vec![]);
    client.push_profiles(Ok(vec![profile("p1", "work")]));
    submit(&mut app, &client, Event::PrevTab); // Tasks -> Profiles (reverse cycle)
    assert!(on_tab(&app, Tab::Profiles));
    let _ = app.handle_event(Event::BeginAddProfile);
    let text = render(&app, W, H);
    assert!(
        text.contains("New profile"),
        "profile dialog title:\n{text}"
    );
    assert!(text.contains("Name"), "name field present:\n{text}");
}

#[test]
fn timer_duration_renders_a_centred_dialog_not_message_band() {
    let (_client, mut app) = logged_in(vec![]);
    let _ = app.handle_event(Event::BeginEditDuration);
    assert!(app.is_editing_duration(), "duration edit open");
    let text = render(&app, W, H);
    assert!(
        text.contains("Timer duration"),
        "timer-duration dialog title:\n{text}",
    );
    assert!(
        text.contains("Duration (minutes)"),
        "duration field present:\n{text}",
    );
}

// ---- Trimmed footer ----

#[test]
fn footer_caption_is_trimmed_to_essentials_only() {
    let (_client, app) = logged_in(vec![]);
    let text = render(&app, W, H);
    // Essentials present.
    assert!(text.contains("move"), "movement in footer:\n{text}");
    assert!(text.contains("switch tab"), "tab switch in footer:\n{text}");
    assert!(text.contains("?: help"), "help in footer:\n{text}");
    assert!(text.contains("q: quit"), "quit in footer:\n{text}");
    // Per-pane action keys are NOT in the footer (they moved into the `?` modal).
    for forbidden in ["a: add", "e: edit", "c: done", "x: del", "r: refresh"] {
        assert!(
            !text.contains(forbidden),
            "{forbidden:?} must NOT be in the trimmed footer:\n{text}",
        );
    }
}

// ---- Global-hotkey suppression: a typed char lands in the focused field ----

#[test]
fn typed_chars_land_in_the_dialog_field_with_globals_suppressed() {
    let (client, mut app) = logged_in(vec![]);
    let _ = app.handle_event(Event::BeginAddTask);

    // Type a word that is ALL global-hotkey letters — proof that with the overlay capturing input
    // the keymap routes them as text, not as q/r/p/d actions. Each goes through the real `map_key`
    // (via `press`) so the suppression predicate is exercised end-to-end.
    for c in ['p', 'd', 'r', 'q'] {
        press(&mut app, &client, KeyCode::Char(c));
    }
    // Still on the add-task dialog (no quit fired despite typing 'q'), and the field holds "pdrq".
    assert!(!app.should_quit(), "typing 'q' in a field did not quit");
    let text = render(&app, W, H);
    assert!(text.contains("Add task"), "still on the dialog:\n{text}");
    assert!(
        text.contains("pdrq"),
        "typed text landed in the field:\n{text}"
    );
}

#[test]
fn r_does_not_refresh_while_a_dialog_is_open() {
    // With the add-task dialog open, pressing `r` must NOT issue a refresh (ListTasks) — it is a
    // typed char. Drive it through the real keymap and assert no new ListTasks crossed the wire.
    let (client, mut app) = logged_in(vec![]);
    let calls_before = client.calls().len();
    let _ = app.handle_event(Event::BeginAddTask);
    press(&mut app, &client, KeyCode::Char('r'));
    assert_eq!(
        client.calls().len(),
        calls_before,
        "r issued no request while the dialog was open: {:?}",
        client.calls(),
    );
}

// ---- Two-tiered Esc ----

#[test]
fn esc_cancels_an_open_dialog_without_quitting() {
    let (client, mut app) = logged_in(vec![]);
    let _ = app.handle_event(Event::BeginAddTask);
    press(&mut app, &client, KeyCode::Esc);
    assert!(!app.should_quit(), "Esc cancelled the dialog, did not quit");
    let text = render(&app, W, H);
    assert!(
        !text.contains("Add task"),
        "the add-task dialog is closed:\n{text}",
    );
}

#[test]
fn esc_on_an_idle_post_auth_screen_still_quits() {
    let (client, mut app) = logged_in(vec![]);
    assert!(!app.overlay_capturing_input(), "no overlay open");
    press(&mut app, &client, KeyCode::Esc);
    assert!(
        app.should_quit(),
        "Esc with no overlay on a post-auth screen quits",
    );
}

#[test]
fn esc_with_a_request_in_flight_cancels_the_request() {
    // Begin a toggle-done but hold the dispatch (do NOT drive it): the task pane is in flight.
    let (_client, mut app) = logged_in(vec![common::today_open_task("t1", "task", "10:00:00")]);
    let _dispatch = app
        .handle_event(Event::ToggleDone)
        .expect("toggle-done dispatches");
    assert!(app.is_pending(), "in flight after toggle");

    // Esc maps to Cancel while pending (not Quit); handle it and confirm the request was abandoned.
    let mapped = map_key(
        app.screen(),
        app.overlay_capturing_input(),
        app.help_open(),
        app.is_editing_duration(),
        key(KeyCode::Esc),
    );
    assert_eq!(mapped, Some(Event::Cancel), "Esc cancels while pending");
    let _ = app.handle_event(Event::Cancel);
    assert!(!app.should_quit(), "cancelling a request does not quit");
    assert!(!app.is_pending(), "the in-flight request was abandoned");
}

// ---- `?` help modal ----

#[test]
fn question_mark_opens_the_help_modal_listing_the_full_reference() {
    let (client, mut app) = logged_in(vec![]);
    press(&mut app, &client, KeyCode::Char('?'));
    assert!(app.help_open(), "? opened the help overlay");
    let text = render(&app, W, H);
    assert!(text.contains("Help"), "help modal title:\n{text}");
    // The full reference lists the per-pane action keys that the footer no longer shows.
    assert!(text.contains("Tasks"), "tasks section:\n{text}");
    assert!(text.contains("Notes"), "notes section:\n{text}");
    assert!(text.contains("Profiles"), "profiles section:\n{text}");
    assert!(
        text.contains("start / stop the focus timer"),
        "the global `t` timer toggle is documented:\n{text}",
    );
}

#[test]
fn help_modal_documents_that_esc_cancels_an_in_flight_request() {
    // ADR-0006 §8.3 amended (operator feedback): the "Esc cancels an in-flight / loading request"
    // affordance was REMOVED from the footer caption and now lives ONLY in the `?` help modal. This
    // pins its new home — the Global section documents that `Esc` cancels a loading request.
    let (client, mut app) = logged_in(vec![]);
    press(&mut app, &client, KeyCode::Char('?'));
    assert!(app.help_open(), "? opened the help overlay");
    let text = render(&app, W, H);
    assert!(
        text.contains("Esc") && text.contains("cancel an in-flight"),
        "the help modal documents Esc cancelling an in-flight / loading request:\n{text}",
    );
}

#[test]
fn help_modal_global_block_lists_quit_and_close_help_as_separate_aligned_rows() {
    // Operator bug (fixed in 8c25b97): the Global block had `q` and `? / Esc` crammed onto one
    // malformed row. This pins the corrected two-row layout on BOTH halves of the report:
    //   1. `q … quit` is its OWN row, never jammed onto the `close help` row (and vice-versa).
    //   2. `close help` is tab-aligned: its description starts in the SAME column as the sibling
    //      Global rows (`quit`, `refresh`), per the shared `  {key:<18}{description}` layout.
    let (client, mut app) = logged_in(vec![]);
    press(&mut app, &client, KeyCode::Char('?'));
    assert!(app.help_open(), "? opened the help overlay");
    let text = render(&app, W, H);
    let rows: Vec<&str> = text.lines().collect();

    // Locate the three Global rows by their description text (the box border/padding indents every
    // row, so we match on the description substring, not a column).
    let quit_row = rows
        .iter()
        .find(|r| r.contains("quit"))
        .unwrap_or_else(|| panic!("a `quit` row in the help body:\n{text}"));
    let close_help_row = rows
        .iter()
        .find(|r| r.contains("close help"))
        .unwrap_or_else(|| panic!("a `close help` row in the help body:\n{text}"));
    let refresh_row = rows
        .iter()
        .find(|r| r.contains("refresh the current view"))
        .unwrap_or_else(|| panic!("a `refresh` row in the help body:\n{text}"));

    // (1) Separate rows: the `quit` and `close help` entries are NOT on a single shared row — the
    // strongest pin against the malformed `? / Esc  close help    q  quit` regression returning.
    assert!(
        !close_help_row.contains("quit"),
        "the `close help` row must NOT also carry `quit` (the malformed combined row):\n\
         {close_help_row:?}",
    );
    assert!(
        !quit_row.contains("close help"),
        "the `quit` row must NOT also carry `close help`:\n{quit_row:?}",
    );

    // (2) Description column alignment: the `{key:<18}` field after the 2-space indent puts every
    // Global description at the same column. Assert `close help`'s description starts at the same
    // column as its siblings' descriptions, relative to those sibling rows (not a magic constant),
    // so the test documents the invariant "descriptions align in a column".
    let desc_col = |row: &str, desc: &str| {
        row.find(desc)
            .unwrap_or_else(|| panic!("description {desc:?} in row {row:?}"))
    };
    let close_help_col = desc_col(close_help_row, "close help");
    let quit_col = desc_col(quit_row, "quit");
    let refresh_col = desc_col(refresh_row, "refresh the current view");
    assert_eq!(
        close_help_col, quit_col,
        "`close help` is tab-aligned to the `quit` row's description column \
         (close_help={close_help_col}, quit={quit_col}):\n{text}",
    );
    assert_eq!(
        close_help_col, refresh_col,
        "`close help` is tab-aligned to the `refresh` row's description column \
         (close_help={close_help_col}, refresh={refresh_col}):\n{text}",
    );
}

#[test]
fn help_modal_tasks_line_renders_intact_without_wrapping_d_delete() {
    // Operator bug (fixed in 5fc5021): the 0019 sub-task hotkeys grew the Tasks reference line to
    // `Tasks    a add · A add sub-task · e edit · Space done · d delete` (64 chars), which overflowed
    // the 62-col inner area of the old DIALOG_WIDTH=64 help box, wrapping the trailing `d delete`
    // token to a flush-left, un-indented continuation row. Fixed by widening ONLY the help overlay to
    // HELP_DIALOG_WIDTH=72 (inner ~70) so the whole Tasks line fits on one rendered row.
    //
    // This pins the line intact: `d delete` renders on the SAME row as the rest of the Tasks line
    // (the row that also carries `a add` / `A add sub-task`), and is NOT marooned on a separate
    // flush-left continuation row. The strongest pin against the wrap regressing if a future hotkey
    // is added or the help width is reverted.
    let (client, mut app) = logged_in(vec![]);
    press(&mut app, &client, KeyCode::Char('?'));
    assert!(app.help_open(), "? opened the help overlay");
    let text = render(&app, W, H);
    let rows: Vec<&str> = text.lines().collect();

    // The Tasks line is identified by its leading-token content (`a add` + `A add sub-task`); the box
    // border/padding indents it, so we match on the action substrings, not a column.
    let tasks_row = rows
        .iter()
        .find(|r| r.contains("a add") && r.contains("A add sub-task"))
        .unwrap_or_else(|| panic!("the Tasks reference row in the help body:\n{text}"));

    // (1) `d delete` is on the SAME rendered row as the rest of the Tasks line — it did NOT wrap to a
    // continuation row.
    assert!(
        tasks_row.contains("d delete"),
        "the Tasks line must keep `d delete` on the same row as `a add` / `A add sub-task` \
         (it must not wrap):\n{tasks_row:?}\nfull help:\n{text}",
    );

    // (2) No row carries a stranded, flush-left `d delete` continuation — i.e. there is no row whose
    // trimmed-left content STARTS with `d delete` (the malformed wrap put the orphaned token at the
    // start of its own un-indented row). The intact Tasks row has `d delete` mid-line, never leading.
    assert!(
        !rows.iter().any(|r| r.trim_start().starts_with("d delete")),
        "no row may begin with a stranded `d delete` (the wrapped-continuation regression):\n{text}",
    );
}

#[test]
fn help_modal_tasks_second_line_keeps_h_hide_older_without_wrapping() {
    // 0020 (learned 0015, recurred 0019): the new `h hide older` hotkey was placed on the SECOND
    // Tasks reference line (`x collapse/expand sub-tasks · Enter detail · h hide older`) to avoid
    // lengthening the already-tight first Tasks line. Adding it can still overflow the fixed-width
    // help box (HELP_DIALOG_WIDTH=72, inner ~70) and reflow the trailing `h hide older` to a
    // flush-left, un-indented continuation row — a pure-geometry bug the build/clippy never catch.
    //
    // This pins the second Tasks line intact: `h hide older` renders on the SAME row as
    // `x collapse/expand`, and NO row is a stranded flush-left `h hide older` continuation.
    let (client, mut app) = logged_in(vec![]);
    press(&mut app, &client, KeyCode::Char('?'));
    assert!(app.help_open(), "? opened the help overlay");
    let text = render(&app, W, H);
    let rows: Vec<&str> = text.lines().collect();

    // The second Tasks line is identified by its `x collapse/expand` content (the `Enter detail`
    // continuation row), not a column — the box border/padding indents it.
    let tasks_second_row = rows
        .iter()
        .find(|r| r.contains("x collapse/expand") && r.contains("Enter detail"))
        .unwrap_or_else(|| panic!("the second Tasks reference row in the help body:\n{text}"));

    // (1) `h hide older` stays on the same rendered row as `x collapse/expand` — it did NOT wrap.
    assert!(
        tasks_second_row.contains("h hide older"),
        "the second Tasks line must keep `h hide older` on the same row as `x collapse/expand` \
         (it must not wrap):\n{tasks_second_row:?}\nfull help:\n{text}",
    );

    // (2) No row is a stranded, flush-left `h hide older` continuation (the malformed-wrap shape).
    assert!(
        !rows
            .iter()
            .any(|r| r.trim_start().starts_with("h hide older")),
        "no row may begin with a stranded `h hide older` (the wrapped-continuation regression):\n{text}",
    );
}

#[test]
fn help_modal_tasks_third_line_keeps_f_filter_by_date_without_wrapping() {
    // 0023 (learned 0015, recurred 0019/0020): the two new date-window hotkeys were placed on a
    // THIRD Tasks reference line (`F window size · f filter by date`) rather than lengthening the
    // already-tight first/second Tasks lines. A newly-added reference line is only as safe as a test
    // that pins it does not overflow the fixed-width help box (HELP_DIALOG_WIDTH=72, inner ~70) and
    // reflow the trailing `f filter by date` to a flush-left, un-indented continuation row — the
    // pure-geometry bug the build/clippy never catch.
    //
    // This pins the third Tasks line intact: `f filter by date` renders on the SAME row as
    // `F window size`, and NO row is a stranded flush-left `f filter by date` continuation.
    let (client, mut app) = logged_in(vec![]);
    press(&mut app, &client, KeyCode::Char('?'));
    assert!(app.help_open(), "? opened the help overlay");
    let text = render(&app, W, H);
    let rows: Vec<&str> = text.lines().collect();

    // The third Tasks line is identified by its `F window size` content, not a column — the box
    // border/padding indents it.
    let tasks_third_row = rows
        .iter()
        .find(|r| r.contains("F window size"))
        .unwrap_or_else(|| panic!("the third Tasks reference row in the help body:\n{text}"));

    // (1) `f filter by date` stays on the same rendered row as `F window size` — it did NOT wrap.
    assert!(
        tasks_third_row.contains("f filter by date"),
        "the third Tasks line must keep `f filter by date` on the same row as `F window size` \
         (it must not wrap):\n{tasks_third_row:?}\nfull help:\n{text}",
    );

    // (2) No row is a stranded, flush-left `f filter by date` continuation (the malformed-wrap shape).
    assert!(
        !rows
            .iter()
            .any(|r| r.trim_start().starts_with("f filter by date")),
        "no row may begin with a stranded `f filter by date` (the wrapped-continuation regression):\n{text}",
    );
}

#[test]
fn help_modal_closes_with_esc() {
    // Esc closes the help modal (the two-tiered Esc: an overlay is open, so Esc → Cancel, which the
    // app core folds into closing the help overlay) — without quitting.
    let (client, mut app) = logged_in(vec![]);
    press(&mut app, &client, KeyCode::Char('?'));
    assert!(app.help_open());
    press(&mut app, &client, KeyCode::Esc);
    assert!(!app.help_open(), "Esc closes the help modal");
    assert!(!app.should_quit(), "closing help via Esc does not quit");
}

#[test]
fn question_mark_closes_help_while_open() {
    // After the fix-now correction (`map_key`'s `?` arm now fires on `globals_live || help_open`),
    // a live `?` keypress CLOSES the help overlay while it is open — the advertised `?/Esc: close`
    // affordance works from the keyboard. The whole path runs through the real keymap (`press`).
    let (client, mut app) = logged_in(vec![]);
    press(&mut app, &client, KeyCode::Char('?'));
    assert!(app.help_open(), "? opened the help overlay");
    press(&mut app, &client, KeyCode::Char('?'));
    assert!(
        !app.help_open(),
        "? closes the help overlay while it is open"
    );
    assert!(!app.should_quit(), "closing help via ? does not quit");
}

#[test]
fn help_close_is_reachable_from_the_keyboard_via_question_mark() {
    // The keymap now reaches the core's `ToggleHelp` close-fold via a `?` keypress while help is
    // open: `map_key` returns `Some(Event::ToggleHelp)` (not `None`), and `handle_event` folds it
    // into closing the overlay, dispatching nothing.
    let (_client, mut app) = logged_in(vec![]);
    let _ = app.handle_event(Event::ToggleHelp);
    assert!(app.help_open(), "ToggleHelp opened help");
    let mapped = map_key(
        app.screen(),
        app.overlay_capturing_input(),
        app.help_open(),
        app.is_editing_duration(),
        key(KeyCode::Char('?')),
    );
    assert_eq!(
        mapped,
        Some(Event::ToggleHelp),
        "? maps to ToggleHelp while help is open (it is no longer suppressed)",
    );
    let closed = app.handle_event(Event::ToggleHelp);
    assert!(closed.is_none(), "closing help dispatches nothing");
    assert!(
        !app.help_open(),
        "ToggleHelp folds into closing the help overlay"
    );
}

#[test]
fn question_mark_is_inert_while_another_dialog_is_open() {
    // Assumption A3: one overlay at a time. With the add-task dialog open, `?` does NOT open help —
    // it is a typed char in the field (the dialog stays the only overlay).
    let (client, mut app) = logged_in(vec![]);
    let _ = app.handle_event(Event::BeginAddTask);
    press(&mut app, &client, KeyCode::Char('?'));
    assert!(!app.help_open(), "? did not open help over an open dialog");
    let text = render(&app, W, H);
    assert!(
        text.contains("Add task"),
        "still on the add dialog:\n{text}"
    );
    assert!(text.contains('?'), "? was typed into the field:\n{text}");
}

// ---- Purple focus border (auth form + a dialog) ----

#[test]
fn auth_form_focused_field_border_is_purple() {
    // The auth form: focus starts on Identifier. Its border row carries far more magenta cells than
    // the non-focused Password field's row (which has no purple border).
    let app = App::new();
    let buffer = render_buffer(&app, W, H);
    let focused = row_fg_count(&buffer, "Identifier", Color::Magenta);
    let unfocused = row_fg_count(&buffer, "Password", Color::Magenta);
    assert!(
        focused > unfocused,
        "the focused field's border is purple, the non-focused field's is not \
         (focused magenta cells {focused}, non-focused {unfocused})",
    );
    assert!(focused > 0, "the focused field has a purple border");
}

#[test]
fn dialog_focused_field_border_is_purple() {
    // In the add-task dialog the focus starts on the Title field; its border row carries more
    // magenta than the non-focused Description field's row. (The outer dialog box border is also
    // magenta, so the contrast — not mere presence — is the discriminator; `row_fg_count`
    // compares.)
    let (_client, mut app) = logged_in(vec![]);
    let _ = app.handle_event(Event::BeginAddTask);
    let buffer = render_buffer(&app, W, H);
    let focused = row_fg_count(&buffer, "Title", Color::Magenta);
    let unfocused = row_fg_count(&buffer, "Description", Color::Magenta);
    assert!(
        focused > unfocused,
        "the focused (Title) field border is purple, the non-focused (Description) is not \
         (focused magenta cells {focused}, non-focused {unfocused})",
    );
}

// ---- Behaviour preserved: submit still folds the chained refresh ----

#[test]
fn add_task_dialog_submit_still_creates_and_chains_refresh() {
    // Moving the render into a dialog must not change the request/response folding: typing through
    // the dialog and submitting issues CreateTask, then the chained ListTasks refresh, and the new
    // task shows from the server response (#1).
    let (client, mut app) = logged_in(vec![]);
    let _ = app.handle_event(Event::BeginAddTask);
    for c in "Groceries".chars() {
        press(&mut app, &client, KeyCode::Char(c));
    }
    let created = common::today_open_task("t-new", "Groceries", "13:00:00");
    client.push_create(Ok(created.clone()));
    client.push_tasks(Ok(vec![created]));
    press(&mut app, &client, KeyCode::Enter); // submit

    assert!(!app.is_pending(), "flow settled after create + refresh");
    let text = render(&app, W, H);
    assert!(
        !text.contains("Add task"),
        "the dialog closed after a successful create:\n{text}",
    );
    assert!(text.contains("Groceries"), "the new task shows:\n{text}");
    assert_eq!(
        screen_name(&app),
        "main:tasks",
        "still on the Tasks pane after the chained refresh",
    );
}
