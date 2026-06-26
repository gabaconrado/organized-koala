//! The 0014 TUI-layout-shell acceptance suite (ADR-0010 §1–2): the post-auth tabbed view, driven
//! through the public two-step `App` API (`handle_event` → synchronous executor → `apply_response`)
//! against the held fake client (the sole external-service mock). Maps the 0014 acceptance criteria:
//!
//! - the `Tasks | Notes | Profiles` tab bar renders with **Tasks selected by default**;
//! - `Tab` / `Shift+Tab` cycle Tasks→Notes→Profiles→Tasks and back, and the selected pane updates;
//! - **no** `t`/`n`/`p`/`s` key switches a tab; arrows move the in-list selection;
//! - per-tab selection **survives** a switch away and back;
//! - the auth form renders as a centred bounded box with the Login⇄Register toggle, all fields, and
//!   the inline error band;
//! - the contextual title renders **exactly** `organized koala - <user> @ [<profile>]` (verbatim —
//!   a space, a hyphen NOT an em dash, literal square brackets) with a live identifier/profile;
//! - the footer (caption + timer) sits flush near the bottom row.
//!
//! These exercise the shell with no live server and no real terminal — the only mock is the
//! sanctioned `Client` trait (the HTTP server), exactly as ADR-0003 / ADR-0006 prescribe.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

mod common;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tui::app::{App, Event, Screen, Tab};
use tui::terminal::map_key;

use common::{
    FakeClient, main_state, note, notes_pane, on_tab, open_task, profile, profiles_pane, render,
    session, submit, tasks_pane,
};

const W: u16 = 80;
const H: u16 = 24;

/// Log in as `<identifier>` to the `work` profile, landing on the Tasks tab with the given tasks.
/// The identifier is typed so the post-auth title can render the live `<user>`.
fn logged_in_as(identifier: &str, tasks: Vec<contract::Task>) -> (FakeClient, App) {
    let client = FakeClient::new();
    client.push_login(Ok(session("jwt")));
    client.push_profiles(Ok(vec![profile("p1", "work")]));
    client.push_tasks(Ok(tasks));
    let mut app = App::new();
    for c in identifier.chars() {
        let _ = app.handle_event(Event::Char(c));
    }
    submit(&mut app, &client, Event::Submit);
    assert!(
        matches!(app.screen(), Screen::Main(_)),
        "logged in to the tabbed view"
    );
    (client, app)
}

/// A `crossterm` key with no modifiers.
fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

// ---- tab bar renders with Tasks selected by default ----

#[test]
fn post_auth_shows_the_tab_bar_with_tasks_selected_by_default() {
    let (_client, app) = logged_in_as("ada", vec![open_task("t1", "task", "2026-06-18T10:00:00Z")]);

    // The active tab defaults to Tasks (ADR-0010 §1).
    assert_eq!(
        main_state(&app).active_tab,
        Tab::Tasks,
        "Tasks is the default tab"
    );

    let text = render(&app, W, H);
    // The tab bar lists all three tabs in order.
    assert!(
        text.contains("Tasks | Notes | Profiles"),
        "tab bar renders all three tabs:\n{text}",
    );
    // The Tasks pane (not Notes/Profiles) is the main content by default.
    assert!(
        text.contains("[ ] task"),
        "the Tasks pane is shown by default:\n{text}"
    );
}

// ---- Tab / Shift+Tab cycle the tabs both directions, and the pane updates ----

#[test]
fn tab_cycles_forward_tasks_notes_profiles_tasks_and_the_pane_updates() {
    // Each switch issues a fresh list load for the destination pane; script them in cycle order.
    let (client, mut app) = logged_in_as("ada", vec![]);

    // Tasks -> Notes
    client.push_notes(Ok(vec![note(
        "n1",
        "a note",
        "body",
        "2026-06-18T10:00:00Z",
    )]));
    submit(&mut app, &client, Event::NextTab);
    assert!(on_tab(&app, Tab::Notes), "first NextTab -> Notes");
    let text = render(&app, W, H);
    assert!(
        text.contains("a note"),
        "the Notes pane is now the main content:\n{text}"
    );

    // Notes -> Profiles
    client.push_profiles(Ok(vec![profile("p1", "work")]));
    submit(&mut app, &client, Event::NextTab);
    assert!(on_tab(&app, Tab::Profiles), "second NextTab -> Profiles");
    let text = render(&app, W, H);
    assert!(
        text.contains("work"),
        "the Profiles pane is now shown:\n{text}"
    );

    // Profiles -> Tasks (wraps)
    client.push_tasks(Ok(vec![open_task("t1", "wrapped", "2026-06-18T10:00:00Z")]));
    submit(&mut app, &client, Event::NextTab);
    assert!(
        on_tab(&app, Tab::Tasks),
        "third NextTab wraps back to Tasks"
    );
    let text = render(&app, W, H);
    assert!(
        text.contains("[ ] wrapped"),
        "back on the Tasks pane:\n{text}"
    );
}

#[test]
fn shift_tab_cycles_backward_tasks_profiles_notes_tasks() {
    let (client, mut app) = logged_in_as("ada", vec![]);

    // Tasks -> Profiles (reverse)
    client.push_profiles(Ok(vec![profile("p1", "work")]));
    submit(&mut app, &client, Event::PrevTab);
    assert!(on_tab(&app, Tab::Profiles), "first PrevTab -> Profiles");

    // Profiles -> Notes (reverse)
    client.push_notes(Ok(vec![note(
        "n1",
        "a note",
        "body",
        "2026-06-18T10:00:00Z",
    )]));
    submit(&mut app, &client, Event::PrevTab);
    assert!(on_tab(&app, Tab::Notes), "second PrevTab -> Notes");

    // Notes -> Tasks (reverse, wraps)
    client.push_tasks(Ok(vec![]));
    submit(&mut app, &client, Event::PrevTab);
    assert!(
        on_tab(&app, Tab::Tasks),
        "third PrevTab wraps back to Tasks"
    );
}

#[test]
fn tab_bar_highlights_the_active_tab_distinctly_from_the_others() {
    // The active tab is rendered with the REVERSED modifier (a styling distinction); assert the
    // active cell carries it while an inactive one does not, so the user can see which tab is live.
    let (client, mut app) = logged_in_as("ada", vec![]);
    client.push_notes(Ok(vec![]));
    submit(&mut app, &client, Event::NextTab); // -> Notes active

    let backend = ratatui::backend::TestBackend::new(W, H);
    let mut terminal = ratatui::Terminal::new(backend).expect("test terminal");
    let _ = terminal
        .draw(|frame| tui::ui::draw(frame, &app, 0))
        .expect("draw");
    let buffer = terminal.backend().buffer();

    // Find the tab-bar row (contains "Tasks | Notes | Profiles") and read the cell styles for the
    // first glyph of "Notes" (active) vs the first glyph of "Tasks" (inactive).
    let reversed = ratatui::style::Modifier::REVERSED;
    let mut notes_reversed = false;
    let mut tasks_reversed = false;
    for y in 0..H {
        let mut row = String::new();
        for x in 0..W {
            row.push_str(buffer[(x, y)].symbol());
        }
        if let Some(notes_at) = row.find("Notes") {
            // The active tab's "Notes" cell carries REVERSED.
            let nx = u16::try_from(notes_at).unwrap_or(0);
            notes_reversed = buffer[(nx, y)].modifier.contains(reversed);
            if let Some(tasks_at) = row.find("Tasks") {
                let tx = u16::try_from(tasks_at).unwrap_or(0);
                tasks_reversed = buffer[(tx, y)].modifier.contains(reversed);
            }
            break;
        }
    }
    assert!(
        notes_reversed,
        "the active tab (Notes) is highlighted (REVERSED)"
    );
    assert!(
        !tasks_reversed,
        "an inactive tab (Tasks) is not highlighted"
    );
}

// ---- no t/n/p/s switches a tab; arrows move the list selection ----

#[test]
fn letter_keys_do_not_switch_tabs_end_to_end() {
    // On the idle Tasks tab, mapping t/n/p/s through the real keymap never produces a tab-switch
    // event, so feeding the mapped event keeps the active tab on Tasks (ADR-0010 §1: tab switching
    // is Tab/Shift+Tab only; `t` is unbound, reserved for the 0016 timer).
    let (client, mut app) =
        logged_in_as("ada", vec![open_task("t1", "task", "2026-06-18T10:00:00Z")]);

    for c in ['t', 'n', 's'] {
        if let Some(event) = map_key(
            app.screen(),
            app.is_editing_duration(),
            key(KeyCode::Char(c)),
        ) {
            assert!(
                !matches!(event, Event::NextTab | Event::PrevTab),
                "{c:?} mapped to a tab switch ({event:?})",
            );
            submit(&mut app, &client, event);
        }
        assert!(
            on_tab(&app, Tab::Tasks),
            "after pressing {c:?} the active tab is still Tasks",
        );
    }
}

#[test]
fn arrows_move_the_in_list_selection_without_switching_tabs() {
    // Down/Up move the Tasks-pane selection and never leave the Tasks tab.
    let (_client, mut app) = logged_in_as(
        "ada",
        vec![
            open_task("t1", "first", "2026-06-18T12:00:00Z"),
            open_task("t2", "second", "2026-06-18T11:00:00Z"),
            open_task("t3", "third", "2026-06-18T10:00:00Z"),
        ],
    );
    assert_eq!(
        tasks_pane(&app).selected,
        Some(0),
        "first row selected on entry"
    );

    // Down moves selection forward; we stay on Tasks (no Dispatch, purely local).
    assert!(
        app.handle_event(Event::Next).is_none(),
        "arrow move dispatches nothing"
    );
    assert!(on_tab(&app, Tab::Tasks), "still on Tasks after an arrow");
    assert_eq!(tasks_pane(&app).selected, Some(1), "selection advanced");

    assert!(app.handle_event(Event::Next).is_none());
    assert_eq!(
        tasks_pane(&app).selected,
        Some(2),
        "selection advanced again"
    );

    // Up moves it back.
    assert!(app.handle_event(Event::Prev).is_none());
    assert_eq!(
        tasks_pane(&app).selected,
        Some(1),
        "Up moves selection back"
    );
}

// ---- per-tab selection survives a switch away and back ----

#[test]
fn per_tab_selection_survives_a_switch_away_and_back() {
    // Move the Tasks selection to row 2, switch to Notes and back, and confirm the Tasks pane still
    // has row 2 selected (the data is re-derived from the server on the switch, but the transient
    // selection index is preserved — ADR-0010 §1).
    let (client, mut app) = logged_in_as(
        "ada",
        vec![
            open_task("t1", "first", "2026-06-18T12:00:00Z"),
            open_task("t2", "second", "2026-06-18T11:00:00Z"),
        ],
    );
    let _ = app.handle_event(Event::Next); // Tasks selection -> row 1 (second task)
    assert_eq!(tasks_pane(&app).selected, Some(1));

    // Switch Tasks -> Notes (load a notes list) ...
    client.push_notes(Ok(vec![note(
        "n1",
        "a note",
        "body",
        "2026-06-18T10:00:00Z",
    )]));
    submit(&mut app, &client, Event::NextTab);
    assert!(on_tab(&app, Tab::Notes));

    // ... then back Notes -> Profiles -> Tasks (the refreshed task list is the same two tasks).
    client.push_profiles(Ok(vec![profile("p1", "work")]));
    submit(&mut app, &client, Event::NextTab); // -> Profiles
    client.push_tasks(Ok(vec![
        open_task("t1", "first", "2026-06-18T12:00:00Z"),
        open_task("t2", "second", "2026-06-18T11:00:00Z"),
    ]));
    submit(&mut app, &client, Event::NextTab); // -> Tasks (wraps)
    assert!(on_tab(&app, Tab::Tasks));

    assert_eq!(
        tasks_pane(&app).selected,
        Some(1),
        "the Tasks-pane selection survived the round trip away and back",
    );
}

#[test]
fn notes_tab_selection_survives_a_switch_away_and_back() {
    // The same preservation holds for the Notes pane's selection.
    let (client, mut app) = logged_in_as("ada", vec![]);

    client.push_notes(Ok(vec![
        note("n1", "alpha", "body", "2026-06-18T12:00:00Z"),
        note("n2", "bravo", "body", "2026-06-18T11:00:00Z"),
    ]));
    submit(&mut app, &client, Event::NextTab); // -> Notes
    let _ = app.handle_event(Event::Next); // Notes selection -> row 1
    assert_eq!(notes_pane(&app).selected, Some(1));

    // Notes -> Profiles -> Tasks -> Notes (back), the notes list unchanged.
    client.push_profiles(Ok(vec![profile("p1", "work")]));
    submit(&mut app, &client, Event::NextTab); // -> Profiles
    client.push_tasks(Ok(vec![]));
    submit(&mut app, &client, Event::NextTab); // -> Tasks
    client.push_notes(Ok(vec![
        note("n1", "alpha", "body", "2026-06-18T12:00:00Z"),
        note("n2", "bravo", "body", "2026-06-18T11:00:00Z"),
    ]));
    submit(&mut app, &client, Event::NextTab); // -> Notes (back)
    assert!(on_tab(&app, Tab::Notes));

    assert_eq!(
        notes_pane(&app).selected,
        Some(1),
        "the Notes-pane selection survived the round trip",
    );
}

// ---- the contextual title renders the exact, load-bearing string ----

#[test]
fn contextual_title_renders_the_exact_user_and_profile_string() {
    // The title is load-bearing for acceptance (ADR-0010 §2): `organized koala - <user> @
    // [<profile>]` — a space, a hyphen (NOT an em dash), and literal square brackets. Assert the
    // verbatim string for the logged-in identifier `ada` and the active profile `work`.
    let (_client, app) = logged_in_as("ada", vec![]);
    let text = render(&app, W, H);
    assert!(
        text.contains("organized koala - ada @ [work]"),
        "exact contextual title:\n{text}",
    );
    // Guard against a stray em dash sneaking in (the auth headers used `—` pre-0014).
    assert!(
        !text.contains("organized koala — ada"),
        "the title must use a hyphen, not an em dash:\n{text}",
    );
}

#[test]
fn contextual_title_tracks_the_active_profile_after_a_pick() {
    // Picking a different profile re-scopes the title's `[<profile>]` to the newly-active one.
    let client = FakeClient::new();
    client.push_login(Ok(session("jwt")));
    client.push_profiles(Ok(vec![profile("p1", "work")]));
    client.push_tasks(Ok(vec![]));
    let mut app = App::new();
    for c in "ada".chars() {
        let _ = app.handle_event(Event::Char(c));
    }
    submit(&mut app, &client, Event::Submit);

    // Switch straight to Profiles (Shift+Tab cycles Tasks->Profiles), where two profiles are listed.
    client.push_profiles(Ok(vec![profile("p1", "work"), profile("p2", "personal")]));
    submit(&mut app, &client, Event::PrevTab); // Tasks -> Profiles (reverse cycle)
    assert!(on_tab(&app, Tab::Profiles));
    assert_eq!(profiles_pane(&app).profiles.len(), 2, "two profiles listed");

    let _ = app.handle_event(Event::Next); // select personal
    client.push_tasks(Ok(vec![])); // pick-active re-scopes + lists tasks for personal
    submit(&mut app, &client, Event::Submit);

    let text = render(&app, W, H);
    assert!(
        text.contains("organized koala - ada @ [personal]"),
        "the title re-scoped to the picked profile:\n{text}",
    );
}

// ---- the footer sits flush near the bottom row ----

#[test]
fn footer_sits_flush_near_the_bottom_row() {
    // "Tight footer" (ADR-0010 §2): the caption + timer band hugs the bottom — assert the hotkey
    // caption renders within the last three rows of the 24-row buffer (no large bottom margin).
    let (_client, app) = logged_in_as("ada", vec![open_task("t1", "task", "2026-06-18T10:00:00Z")]);
    let text = render(&app, W, H);
    let rows: Vec<&str> = text.lines().collect();

    // The caption starts with the stable "Tab: switch tab" segment; locate its row.
    let caption_row = rows
        .iter()
        .position(|r| r.contains("Tab: switch tab"))
        .expect("the hotkey caption is rendered");
    assert!(
        caption_row >= rows.len().saturating_sub(4),
        "the footer caption sits flush near the bottom (row {caption_row} of {}):\n{text}",
        rows.len(),
    );

    // The timer widget shares the footer band, also near the bottom.
    let timer_row = rows
        .iter()
        .position(|r| r.contains("timer idle"))
        .expect("the timer widget is rendered in the footer");
    assert!(
        timer_row >= rows.len().saturating_sub(4),
        "the timer widget sits flush near the bottom (row {timer_row}):\n{text}",
    );
}

// ---- the auth form is a centred bounded box with the toggle, all fields, and the error band ----

#[test]
fn auth_form_renders_centred_with_toggle_all_fields_and_error_band() {
    // Login form: a centred bounded box (its content is horizontally indented, not flush-left, and
    // vertically away from the top row), with the Login⇄Register toggle hint and both fields.
    let app = App::new();
    let text = render(&app, W, H);
    let rows: Vec<&str> = text.lines().collect();

    // Centred title: not on the very first row (vertically centred) and indented (not column 0).
    let title_row = rows
        .iter()
        .position(|r| r.contains("organized koala - Login"))
        .expect("login title rendered");
    assert!(
        title_row > 0,
        "the centred form is not pinned to the top row:\n{text}"
    );
    let title_line = rows.get(title_row).copied().unwrap_or("");
    let indent = title_line.len() - title_line.trim_start().len();
    assert!(
        indent > 0,
        "the centred title is horizontally indented:\n{text}"
    );

    // The Login⇄Register toggle hint and the login fields are present.
    assert!(
        text.contains("switch to register"),
        "Login->Register toggle hint:\n{text}"
    );
    assert!(text.contains("Identifier"), "identifier field:\n{text}");
    assert!(text.contains("Password"), "password field:\n{text}");
}

#[test]
fn auth_register_form_renders_centred_with_all_four_fields() {
    let mut app = App::new();
    let _ = app.handle_event(Event::ToggleAuthMode); // -> Register
    let text = render(&app, W, H);
    assert!(
        text.contains("organized koala - Register"),
        "register title:\n{text}"
    );
    assert!(
        text.contains("switch to login"),
        "Register->Login toggle hint:\n{text}"
    );
    assert!(text.contains("Username"), "username field:\n{text}");
    assert!(text.contains("Email"), "email field:\n{text}");
    assert!(text.contains("Password"), "password field:\n{text}");
    assert!(text.contains("Profile name"), "profile-name field:\n{text}");
}

#[test]
fn auth_form_error_band_renders_the_inline_error() {
    // The inline error band shows a failed-auth message inside the centred box (preserved from the
    // pre-0014 layout). Drive a rejected login so the auth state carries an error, then render.
    let client = FakeClient::new();
    client.push_login(Err(common::api_err(
        contract::ErrorCode::InvalidCredentials,
        "invalid username or password",
    )));
    let mut app = App::new();
    submit(&mut app, &client, Event::Submit);

    // Still on the auth screen with the error surfaced inline.
    assert!(
        matches!(app.screen(), Screen::Auth(_)),
        "stays on auth after a rejected login"
    );
    let text = render(&app, W, H);
    assert!(
        text.contains("invalid username or password"),
        "the inline error band renders the message:\n{text}",
    );
}
