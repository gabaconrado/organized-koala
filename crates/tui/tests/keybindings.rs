//! Pins the keybinding contract: `terminal::map_key(screen, key) -> Option<Event>` is pure, so
//! these tests lock every binding `tui-dev` chose and the context-sensitivity that lets a
//! printable key be a command on the task list but typed text in a form (slice-3 acceptance 1).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

mod common;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tui::app::Event;
use tui::terminal::map_key;

use common::{
    auth_screen, auth_screen_pending, offline_screen, offline_screen_pending, task_list_screen,
    task_list_screen_adding, task_list_screen_pending, timer_screen, timer_screen_editing,
    timer_screen_pending,
};

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn ctrl(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
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
            map_key(&screen, ctrl('c')),
            Some(Event::Quit),
            "Ctrl+C must quit on {screen:?}",
        );
    }
}

#[test]
fn esc_quits_except_in_add_task_where_it_cancels() {
    assert_eq!(
        map_key(&auth_screen(), key(KeyCode::Esc)),
        Some(Event::Quit)
    );
    assert_eq!(
        map_key(&task_list_screen(), key(KeyCode::Esc)),
        Some(Event::Quit),
    );
    assert_eq!(
        map_key(&offline_screen(), key(KeyCode::Esc)),
        Some(Event::Quit),
    );
    // In the add-task sub-flow, Esc cancels the flow instead of quitting the app.
    assert_eq!(
        map_key(&task_list_screen_adding(), key(KeyCode::Esc)),
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
            map_key(&screen, key(KeyCode::Esc)),
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
            map_key(&screen, ctrl('c')),
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
        assert_eq!(map_key(&screen, key(KeyCode::Enter)), Some(Event::Submit));
    }
}

#[test]
fn tab_down_is_next_and_backtab_up_is_prev() {
    for screen in [auth_screen(), task_list_screen(), task_list_screen_adding()] {
        assert_eq!(map_key(&screen, key(KeyCode::Tab)), Some(Event::Next));
        assert_eq!(map_key(&screen, key(KeyCode::Down)), Some(Event::Next));
        assert_eq!(map_key(&screen, key(KeyCode::BackTab)), Some(Event::Prev));
        assert_eq!(map_key(&screen, key(KeyCode::Up)), Some(Event::Prev));
    }
}

#[test]
fn backspace_maps_to_backspace_in_text_contexts() {
    assert_eq!(
        map_key(&auth_screen(), key(KeyCode::Backspace)),
        Some(Event::Backspace),
    );
    assert_eq!(
        map_key(&task_list_screen_adding(), key(KeyCode::Backspace)),
        Some(Event::Backspace),
    );
}

// ---- Auth screen ----

#[test]
fn f2_toggles_auth_mode_only_on_auth_screen() {
    assert_eq!(
        map_key(&auth_screen(), key(KeyCode::F(2))),
        Some(Event::ToggleAuthMode),
    );
    // F2 is not bound off the auth screen.
    assert_eq!(map_key(&task_list_screen(), key(KeyCode::F(2))), None);
    assert_eq!(map_key(&offline_screen(), key(KeyCode::F(2))), None);
}

#[test]
fn printable_keys_are_typed_literally_in_auth_form() {
    // The auth form is a text-entry context: letters that are commands on the task list
    // ('a', 'c', 'r', 'q') must be typed as Char here, not interpreted.
    for c in ['a', 'c', 'r', 'q', 'x', 'Z', '@', '7'] {
        assert_eq!(
            map_key(&auth_screen(), key(KeyCode::Char(c))),
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
        map_key(&screen, key(KeyCode::Char('a'))),
        Some(Event::BeginAddTask),
    );
    assert_eq!(
        map_key(&screen, key(KeyCode::Char('c'))),
        Some(Event::CloseSelected),
    );
    assert_eq!(
        map_key(&screen, key(KeyCode::Char('r'))),
        Some(Event::Refresh),
    );
    assert_eq!(map_key(&screen, key(KeyCode::Char('q'))), Some(Event::Quit));
    // An unbound printable key on the task list is ignored.
    assert_eq!(map_key(&screen, key(KeyCode::Char('z'))), None);
}

#[test]
fn add_task_flow_types_command_letters_literally() {
    // Once the add-task sub-flow is open the task list is a text-entry context, so 'a'/'c'/'r'
    // /'q' are typed literally rather than triggering add/close/refresh/quit.
    let screen = task_list_screen_adding();
    for c in ['a', 'c', 'r', 'q', 'b'] {
        assert_eq!(
            map_key(&screen, key(KeyCode::Char(c))),
            Some(Event::Char(c)),
            "{c:?} must be literal text while adding a task",
        );
    }
}

// ---- Offline screen ----

#[test]
fn offline_retry_key() {
    let screen = offline_screen();
    assert_eq!(
        map_key(&screen, key(KeyCode::Char('r'))),
        Some(Event::Refresh),
    );
    // Other command letters are not bound on the offline screen.
    assert_eq!(map_key(&screen, key(KeyCode::Char('a'))), None);
    assert_eq!(map_key(&screen, key(KeyCode::Char('q'))), None);
}

// ---- Timer screen ----

#[test]
fn task_list_t_opens_the_timer() {
    // `t` on the task list (not entering text) opens the focus/timer view.
    assert_eq!(
        map_key(&task_list_screen(), key(KeyCode::Char('t'))),
        Some(Event::OpenTimer),
    );
    // `t` is not an OpenTimer command off the task list.
    assert_eq!(map_key(&offline_screen(), key(KeyCode::Char('t'))), None);
}

#[test]
fn timer_command_keys() {
    let screen = timer_screen();
    assert_eq!(
        map_key(&screen, key(KeyCode::Char('s'))),
        Some(Event::StartTimer),
    );
    assert_eq!(
        map_key(&screen, key(KeyCode::Char('x'))),
        Some(Event::StopTimer),
    );
    assert_eq!(
        map_key(&screen, key(KeyCode::Char('d'))),
        Some(Event::BeginEditDuration),
    );
    assert_eq!(
        map_key(&screen, key(KeyCode::Char('r'))),
        Some(Event::Refresh),
    );
    // Esc on the timer is Cancel (the core resolves it to back / abandon-edit / cancel-in-flight).
    assert_eq!(map_key(&screen, key(KeyCode::Esc)), Some(Event::Cancel));
    // Ctrl+C still hard-quits from the timer view.
    assert_eq!(map_key(&screen, ctrl('c')), Some(Event::Quit));
    // An unbound printable key on the timer view is ignored.
    assert_eq!(map_key(&screen, key(KeyCode::Char('z'))), None);
}

#[test]
fn timer_duration_edit_is_a_text_entry_context() {
    // While editing the duration the timer is a text-entry context: digit keys (and the command
    // letters s/x/d/r) are typed literally, not interpreted as commands.
    let screen = timer_screen_editing();
    for c in ['2', '5', 's', 'x', 'd', 'r'] {
        assert_eq!(
            map_key(&screen, key(KeyCode::Char(c))),
            Some(Event::Char(c)),
            "{c:?} must be literal text while editing the duration",
        );
    }
    assert_eq!(
        map_key(&screen, key(KeyCode::Backspace)),
        Some(Event::Backspace),
    );
    assert_eq!(map_key(&screen, key(KeyCode::Enter)), Some(Event::Submit));
    // Esc still cancels (the core abandons the edit).
    assert_eq!(map_key(&screen, key(KeyCode::Esc)), Some(Event::Cancel));
}

#[test]
fn timer_esc_cancels_while_pending() {
    // Esc on the timer maps to Cancel whether idle or in-flight; Ctrl+C stays Quit.
    assert_eq!(
        map_key(&timer_screen_pending(), key(KeyCode::Esc)),
        Some(Event::Cancel),
    );
    assert_eq!(
        map_key(&timer_screen_pending(), ctrl('c')),
        Some(Event::Quit)
    );
}

#[test]
fn r_is_not_a_command_on_the_auth_screen() {
    // 'r' is a command on task-list/offline but on the auth screen it is literal text.
    assert_eq!(
        map_key(&auth_screen(), key(KeyCode::Char('r'))),
        Some(Event::Char('r')),
    );
}
