//! Pins the keybinding contract: `terminal::map_key(screen, editing_duration, key) -> Option<Event>`
//! is pure, so these tests lock every binding `tui-dev` chose and the context-sensitivity that lets
//! a printable key be a command on the task list but typed text in a form (slice-3 acceptance 1).
//!
//! The timer is no longer a screen (ADR-0006 §8): its controls (`p` toggle, `d` edit) are global on
//! every post-auth screen, and the duration-edit sub-flow is a global text-entry mode signalled by
//! the `editing_duration` bool — not a `Screen` variant. These tests pin that new signature and the
//! global-timer bindings, and guard the absence of the old dedicated-timer navigation.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

mod common;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tui::app::Event;
use tui::terminal::map_key;

use common::{
    auth_screen, auth_screen_pending, offline_screen, offline_screen_pending, task_list_screen,
    task_list_screen_adding, task_list_screen_editing, task_list_screen_pending,
};

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn ctrl(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
}

/// `map_key` with the duration-edit sub-flow closed — the common case for non-edit bindings.
fn map(screen: &tui::app::Screen, k: KeyEvent) -> Option<Event> {
    map_key(screen, false, k)
}

/// `map_key` with the global duration-edit sub-flow active (the text-entry overlay).
fn map_editing(screen: &tui::app::Screen, k: KeyEvent) -> Option<Event> {
    map_key(screen, true, k)
}

// ---- Global / cross-screen ----

#[test]
fn ctrl_c_quits_on_every_screen() {
    for screen in [
        auth_screen(),
        task_list_screen(),
        task_list_screen_adding(),
        offline_screen(),
    ] {
        assert_eq!(
            map(&screen, ctrl('c')),
            Some(Event::Quit),
            "Ctrl+C must quit on {screen:?}",
        );
    }
}

#[test]
fn esc_quits_except_in_add_task_where_it_cancels() {
    assert_eq!(map(&auth_screen(), key(KeyCode::Esc)), Some(Event::Quit));
    assert_eq!(
        map(&task_list_screen(), key(KeyCode::Esc)),
        Some(Event::Quit),
    );
    assert_eq!(map(&offline_screen(), key(KeyCode::Esc)), Some(Event::Quit),);
    // In the add-task sub-flow, Esc cancels the flow instead of quitting the app.
    assert_eq!(
        map(&task_list_screen_adding(), key(KeyCode::Esc)),
        Some(Event::Cancel),
    );
}

#[test]
fn esc_cancels_while_a_request_is_in_flight() {
    // While a request is outstanding, Esc must map to Cancel (abandon the request), not Quit, so
    // the cancel affordance stays live in flight (0005 acceptance). This holds on every screen
    // that can be pending.
    for screen in [
        auth_screen_pending(),
        task_list_screen_pending(),
        offline_screen_pending(),
    ] {
        assert_eq!(
            map(&screen, key(KeyCode::Esc)),
            Some(Event::Cancel),
            "Esc must cancel (not quit) while pending on {screen:?}",
        );
    }
}

#[test]
fn ctrl_c_still_quits_while_pending() {
    // Ctrl+C is the hard quit and stays Quit even while a request is in flight.
    for screen in [
        auth_screen_pending(),
        task_list_screen_pending(),
        offline_screen_pending(),
    ] {
        assert_eq!(
            map(&screen, ctrl('c')),
            Some(Event::Quit),
            "Ctrl+C must quit even while pending on {screen:?}",
        );
    }
}

#[test]
fn enter_submits_everywhere() {
    for screen in [
        auth_screen(),
        task_list_screen(),
        task_list_screen_adding(),
        offline_screen(),
    ] {
        assert_eq!(map(&screen, key(KeyCode::Enter)), Some(Event::Submit));
    }
}

#[test]
fn tab_down_is_next_and_backtab_up_is_prev() {
    for screen in [auth_screen(), task_list_screen(), task_list_screen_adding()] {
        assert_eq!(map(&screen, key(KeyCode::Tab)), Some(Event::Next));
        assert_eq!(map(&screen, key(KeyCode::Down)), Some(Event::Next));
        assert_eq!(map(&screen, key(KeyCode::BackTab)), Some(Event::Prev));
        assert_eq!(map(&screen, key(KeyCode::Up)), Some(Event::Prev));
    }
}

#[test]
fn backspace_maps_to_backspace_in_text_contexts() {
    assert_eq!(
        map(&auth_screen(), key(KeyCode::Backspace)),
        Some(Event::Backspace),
    );
    assert_eq!(
        map(&task_list_screen_adding(), key(KeyCode::Backspace)),
        Some(Event::Backspace),
    );
    // The duration-edit overlay is a text context, so Backspace edits the buffer there too.
    assert_eq!(
        map_editing(&task_list_screen(), key(KeyCode::Backspace)),
        Some(Event::Backspace),
    );
}

// ---- Auth screen ----

#[test]
fn f2_toggles_auth_mode_only_on_auth_screen() {
    assert_eq!(
        map(&auth_screen(), key(KeyCode::F(2))),
        Some(Event::ToggleAuthMode),
    );
    // F2 is not bound off the auth screen.
    assert_eq!(map(&task_list_screen(), key(KeyCode::F(2))), None);
    assert_eq!(map(&offline_screen(), key(KeyCode::F(2))), None);
}

#[test]
fn printable_keys_are_typed_literally_in_auth_form() {
    // The auth form is a text-entry context: letters that are commands on the task list
    // ('a', 'c', 'r', 'q', 'p', 'd') must be typed as Char here, not interpreted.
    for c in ['a', 'c', 'r', 'q', 'p', 'd', 'x', 'Z', '@', '7'] {
        assert_eq!(
            map(&auth_screen(), key(KeyCode::Char(c))),
            Some(Event::Char(c)),
            "{c:?} must be a literal Char in the auth form",
        );
    }
}

// ---- Task list (not entering text) ----

#[test]
fn task_list_command_keys() {
    let screen = task_list_screen();
    assert_eq!(
        map(&screen, key(KeyCode::Char('a'))),
        Some(Event::BeginAddTask),
    );
    // `e` begins the edit sub-flow; `c` toggles done/reopen; `x` arms/confirms delete (slice 4).
    assert_eq!(
        map(&screen, key(KeyCode::Char('e'))),
        Some(Event::BeginEditTask),
    );
    assert_eq!(
        map(&screen, key(KeyCode::Char('c'))),
        Some(Event::ToggleDone),
    );
    assert_eq!(
        map(&screen, key(KeyCode::Char('x'))),
        Some(Event::DeleteSelected),
    );
    assert_eq!(map(&screen, key(KeyCode::Char('r'))), Some(Event::Refresh),);
    assert_eq!(map(&screen, key(KeyCode::Char('q'))), Some(Event::Quit));
    // An unbound printable key on the task list is ignored.
    assert_eq!(map(&screen, key(KeyCode::Char('z'))), None);
}

#[test]
fn edit_task_flow_types_command_letters_literally() {
    // Once the edit sub-flow is open the task list is a text-entry context, so the command
    // letters — including the new e/c/x mutation keys and the global timer keys p/d — are typed
    // literally rather than triggering edit/toggle/delete or the timer toggle/edit.
    let screen = task_list_screen_editing();
    for c in ['a', 'c', 'e', 'x', 'r', 'q', 'p', 'd'] {
        assert_eq!(
            map(&screen, key(KeyCode::Char(c))),
            Some(Event::Char(c)),
            "{c:?} must be literal text while editing a task",
        );
    }
    // Esc cancels the edit sub-flow (not quit); Enter submits; Tab switches field.
    assert_eq!(map(&screen, key(KeyCode::Esc)), Some(Event::Cancel));
    assert_eq!(map(&screen, key(KeyCode::Enter)), Some(Event::Submit));
    assert_eq!(map(&screen, key(KeyCode::Tab)), Some(Event::Next));
    assert_eq!(
        map(&screen, key(KeyCode::Backspace)),
        Some(Event::Backspace)
    );
}

#[test]
fn add_task_flow_types_command_letters_literally() {
    // Once the add-task sub-flow is open the task list is a text-entry context, so the command
    // letters — including the global timer keys 'p'/'d' — are typed literally rather than
    // triggering add/close/refresh/quit or the timer toggle/edit (Assumption B4).
    let screen = task_list_screen_adding();
    for c in ['a', 'c', 'r', 'q', 'p', 'd', 'b'] {
        assert_eq!(
            map(&screen, key(KeyCode::Char(c))),
            Some(Event::Char(c)),
            "{c:?} must be literal text while adding a task",
        );
    }
}

// ---- Offline screen ----

#[test]
fn offline_retry_key() {
    let screen = offline_screen();
    assert_eq!(map(&screen, key(KeyCode::Char('r'))), Some(Event::Refresh),);
    // Other command letters are not bound on the offline screen (it is not a post-auth screen, so
    // the global timer keys are inactive there too).
    assert_eq!(map(&screen, key(KeyCode::Char('a'))), None);
    assert_eq!(map(&screen, key(KeyCode::Char('q'))), None);
    assert_eq!(map(&screen, key(KeyCode::Char('p'))), None);
    assert_eq!(map(&screen, key(KeyCode::Char('d'))), None);
}

// ---- Global timer controls (ADR-0006 §8.2) ----

#[test]
fn p_toggles_the_timer_on_a_post_auth_screen() {
    // `p` is the global start/stop toggle, live on every post-auth screen (the task list).
    assert_eq!(
        map(&task_list_screen(), key(KeyCode::Char('p'))),
        Some(Event::ToggleTimer),
    );
}

#[test]
fn d_begins_the_duration_edit_on_a_post_auth_screen() {
    assert_eq!(
        map(&task_list_screen(), key(KeyCode::Char('d'))),
        Some(Event::BeginEditDuration),
    );
}

#[test]
fn global_timer_keys_are_inactive_off_post_auth_screens() {
    // The timer widget is only shown post-auth (auth excluded — no session yet, Assumption B3), so
    // its global keys are not bound on the auth or offline screens. There, 'p'/'d' are literal text
    // (auth, a text context) or ignored (offline, a command context with no timer binding).
    assert_eq!(
        map(&auth_screen(), key(KeyCode::Char('p'))),
        Some(Event::Char('p')),
    );
    assert_eq!(
        map(&auth_screen(), key(KeyCode::Char('d'))),
        Some(Event::Char('d')),
    );
    assert_eq!(map(&offline_screen(), key(KeyCode::Char('p'))), None);
    assert_eq!(map(&offline_screen(), key(KeyCode::Char('d'))), None);
}

#[test]
fn p_is_suppressed_while_a_text_entry_sub_flow_owns_keystrokes() {
    // Assumption B4: a literal `p` typed into a field is not hijacked by the global toggle. While
    // the add-task sub-flow owns keystrokes, OR while the duration-edit overlay is active, `p` is a
    // Char, not ToggleTimer.
    assert_eq!(
        map(&task_list_screen_adding(), key(KeyCode::Char('p'))),
        Some(Event::Char('p')),
    );
    assert_eq!(
        map_editing(&task_list_screen(), key(KeyCode::Char('p'))),
        Some(Event::Char('p')),
    );
}

#[test]
fn duration_edit_is_a_global_text_entry_context() {
    // While editing the duration (signalled by `editing_duration = true`) the active post-auth
    // screen is a text-entry context: digit keys (and the command letters p/d/r/a/c) are typed
    // literally, not interpreted as commands.
    let screen = task_list_screen();
    for c in ['2', '5', 'p', 'd', 'r', 'a', 'c'] {
        assert_eq!(
            map_editing(&screen, key(KeyCode::Char(c))),
            Some(Event::Char(c)),
            "{c:?} must be literal text while editing the duration",
        );
    }
    assert_eq!(
        map_editing(&screen, key(KeyCode::Backspace)),
        Some(Event::Backspace),
    );
    assert_eq!(
        map_editing(&screen, key(KeyCode::Enter)),
        Some(Event::Submit)
    );
    // Esc cancels the edit (the duration-edit overlay is a sub-flow, so Esc is Cancel not Quit).
    assert_eq!(map_editing(&screen, key(KeyCode::Esc)), Some(Event::Cancel));
}

// ---- No dedicated timer page (ADR-0006 §8 — regression guards) ----

#[test]
fn t_does_not_open_a_dedicated_timer_page() {
    // The dedicated timer screen and its `t`-to-open navigation are gone: `t` is not a command on
    // any screen anymore (it is literal text in the auth form, ignored elsewhere).
    assert_eq!(
        map(&auth_screen(), key(KeyCode::Char('t'))),
        Some(Event::Char('t')),
    );
    assert_eq!(map(&task_list_screen(), key(KeyCode::Char('t'))), None);
    assert_eq!(map(&offline_screen(), key(KeyCode::Char('t'))), None);
}

#[test]
fn r_is_not_a_command_on_the_auth_screen() {
    // 'r' is a command on task-list/offline but on the auth screen it is literal text.
    assert_eq!(
        map(&auth_screen(), key(KeyCode::Char('r'))),
        Some(Event::Char('r')),
    );
}
