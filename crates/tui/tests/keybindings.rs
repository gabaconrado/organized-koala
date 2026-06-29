//! Pins the keybinding contract:
//! `terminal::map_key(screen, overlay_capturing, help_open, editing_duration, key) -> Option<Event>`
//! is pure, so these tests lock every binding `tui-dev` chose and the context-sensitivity that lets
//! a printable key be a command on the task list but typed text in a form (slice-3 acceptance 1).
//!
//! The timer is no longer a screen (ADR-0006 §8): its controls are global on every post-auth
//! screen, and the duration-edit sub-flow is a global text-entry mode signalled by the
//! `editing_duration` bool — not a `Screen` variant. 0015 adds the unified `overlay_capturing`
//! predicate (ADR-0010 §3): while any dialog/overlay owns input — an add/edit form, a delete-confirm
//! dialog, the duration edit, or the `?` help overlay — every global hotkey (`q`/`r`/`?`/`t`/`T` and
//! tab-switch) is suppressed and `Esc` cancels. These tests pin that signature, the global-timer
//! bindings, and guard the absence of the old dedicated-timer navigation.
//!
//! **0016 final hotkey scheme (ADR-0010 §4, item table).** The canonical remap pinned here:
//! `Space` toggles task done/undone (was `c`); `d` deletes on every tab (was `x`); `t` starts/stops
//! the timer (was `p`); `T` opens the timer-config/duration dialog (was the old duration-edit `d`).
//! Per-entity action keys (`a`/`e`/`d`/`Enter`/`Space`) stay context-scoped to the active tab; the
//! task delete is armed via `d` and confirmed via `Enter` (the 0015 confirm dialog, Assumption A5 —
//! the old `x`-again two-step is retired). The detail-view bindings live in `navigation.rs`/
//! `flows.rs`.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

mod common;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tui::app::Event;
use tui::terminal::map_key;

use common::{
    auth_screen, auth_screen_pending, notes_screen, notes_screen_confirming_delete,
    notes_screen_creating, notes_screen_detail_idle, notes_screen_editing,
    notes_screen_editing_content, notes_screen_editing_title, offline_screen,
    offline_screen_pending, profiles_screen, profiles_screen_confirming_delete,
    profiles_screen_creating, profiles_screen_pending, profiles_screen_renaming,
    screen_overlay_capturing, task_list_screen, task_list_screen_adding,
    task_list_screen_confirming_delete, task_list_screen_editing, task_list_screen_pending,
};

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn ctrl(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
}

/// `map_key` with the global duration-edit sub-flow closed and no help overlay — the common case
/// for non-edit bindings. The unified `overlay_capturing` predicate is derived from the screen (the
/// production value comes from `App`; these tests build a bare `Screen`, so the screen-driven branch
/// is mirrored in [`screen_overlay_capturing`]). `help_open` is `false`: these screen builders model
/// no open help overlay (the help-overlay keypaths are exercised in the `dialogs` suite via `App`).
fn map(screen: &tui::app::Screen, k: KeyEvent) -> Option<Event> {
    map_key(screen, screen_overlay_capturing(screen), false, false, k)
}

/// `map_key` with the global duration-edit sub-flow active (the text-entry overlay). The duration
/// edit is itself an input-capturing overlay, so `overlay_capturing` is `true`; `help_open` is
/// `false` (the help overlay and the duration edit never stack — one overlay at a time, A3).
fn map_editing(screen: &tui::app::Screen, k: KeyEvent) -> Option<Event> {
    map_key(screen, true, false, true, k)
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
fn tab_switches_field_in_auth_and_in_sub_flows() {
    // On the auth form and inside a post-auth text-entry sub-flow, Tab/BackTab switch the focused
    // FIELD (Next/Prev) — tab-cycling is reserved for idle post-auth lists (ADR-0010 §1).
    for screen in [auth_screen(), task_list_screen_adding()] {
        assert_eq!(map(&screen, key(KeyCode::Tab)), Some(Event::Next));
        assert_eq!(map(&screen, key(KeyCode::BackTab)), Some(Event::Prev));
    }
}

#[test]
fn arrows_always_move_the_list_selection() {
    // Arrows move the selection (Next/Prev) on every screen — auth, the idle post-auth lists, and
    // sub-flows alike. Tab no longer owns list movement (ADR-0010 §1: arrows move, Tab cycles).
    for screen in [
        auth_screen(),
        task_list_screen(),
        notes_screen(),
        profiles_screen(),
        task_list_screen_adding(),
    ] {
        assert_eq!(
            map(&screen, key(KeyCode::Down)),
            Some(Event::Next),
            "Down moves selection on {screen:?}",
        );
        assert_eq!(
            map(&screen, key(KeyCode::Up)),
            Some(Event::Prev),
            "Up moves selection on {screen:?}",
        );
    }
}

// ---- Top-level tab cycling (ADR-0010 §1) ----

#[test]
fn tab_and_shift_tab_cycle_tabs_on_an_idle_post_auth_list() {
    // On an idle post-auth list (any tab), Tab → NextTab and Shift+Tab → PrevTab; the cycling
    // itself (Tasks→Notes→Profiles→Tasks) is exercised end-to-end in the navigation suite.
    for screen in [task_list_screen(), notes_screen(), profiles_screen()] {
        assert_eq!(
            map(&screen, key(KeyCode::Tab)),
            Some(Event::NextTab),
            "Tab cycles to the next tab on the idle list {screen:?}",
        );
        assert_eq!(
            map(&screen, key(KeyCode::BackTab)),
            Some(Event::PrevTab),
            "Shift+Tab cycles to the previous tab on the idle list {screen:?}",
        );
    }
}

#[test]
fn tab_switches_field_not_tabs_inside_a_post_auth_sub_flow() {
    // Inside any post-auth sub-flow Tab must switch the focused field (Next), NEVER cycle tabs —
    // otherwise typing into a form would jump panes (ADR-0010 §1).
    for screen in [
        task_list_screen_adding(),
        task_list_screen_editing(),
        profiles_screen_creating(),
        profiles_screen_renaming(),
    ] {
        assert_eq!(
            map(&screen, key(KeyCode::Tab)),
            Some(Event::Next),
            "Tab switches field (not tab) in the sub-flow {screen:?}",
        );
        assert_eq!(
            map(&screen, key(KeyCode::BackTab)),
            Some(Event::Prev),
            "Shift+Tab switches field (not tab) in the sub-flow {screen:?}",
        );
    }
}

#[test]
fn letter_keys_never_switch_tabs() {
    // Deliberately NO n/s/p tab-letter hotkeys (ADR-0010 §1). On the idle Tasks tab these letters
    // are either a pane/global command or unbound — crucially none yields NextTab/PrevTab.
    let screen = task_list_screen();
    for c in ['n', 's', 'p', 't', 'T'] {
        let mapped = map(&screen, key(KeyCode::Char(c)));
        assert!(
            !matches!(mapped, Some(Event::NextTab) | Some(Event::PrevTab)),
            "{c:?} must NOT switch tabs (got {mapped:?})",
        );
    }
    // `n`/`s`/`p` are unbound on the Tasks tab (the old timer toggle `p` is retired); `t` is the
    // timer toggle and `T` the timer-config dialog (neither a tab switch).
    assert_eq!(map(&screen, key(KeyCode::Char('n'))), None, "n unbound");
    assert_eq!(map(&screen, key(KeyCode::Char('s'))), None, "s unbound");
    assert_eq!(map(&screen, key(KeyCode::Char('p'))), None, "p unbound now");
    assert_eq!(
        map(&screen, key(KeyCode::Char('t'))),
        Some(Event::ToggleTimer),
        "t is the timer toggle, not a tab switch",
    );
    assert_eq!(
        map(&screen, key(KeyCode::Char('T'))),
        Some(Event::BeginEditDuration),
        "T is the timer-config dialog, not a tab switch",
    );
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
    // The auth form is a text-entry context: letters that are commands post-auth
    // ('a', 'r', 'q', 't', 'T', 'd') must be typed as Char here, not interpreted.
    for c in ['a', 'c', 'r', 'q', 't', 'T', 'd', 'x', 'Z', '@', '7'] {
        assert_eq!(
            map(&auth_screen(), key(KeyCode::Char(c))),
            Some(Event::Char(c)),
            "{c:?} must be a literal Char in the auth form",
        );
    }
}

// ---- Task list (not entering text) — 0016 final scheme ----

#[test]
fn task_list_command_keys() {
    let screen = task_list_screen();
    assert_eq!(
        map(&screen, key(KeyCode::Char('a'))),
        Some(Event::BeginAddTask),
    );
    // `e` begins the edit sub-flow; `Space` toggles done/reopen (was `c`); `d` arms delete (was
    // `x`); `Enter` opens the per-field detail view (ADR-0010 §4).
    assert_eq!(
        map(&screen, key(KeyCode::Char('e'))),
        Some(Event::BeginEditTask),
    );
    assert_eq!(
        map(&screen, key(KeyCode::Char(' '))),
        Some(Event::ToggleDone),
        "Space toggles done (the new binding)",
    );
    assert_eq!(
        map(&screen, key(KeyCode::Char('d'))),
        Some(Event::DeleteSelected),
        "d deletes (the new binding)",
    );
    assert_eq!(
        map(&screen, key(KeyCode::Enter)),
        Some(Event::Submit),
        "Enter opens the task detail view (folded by the core)",
    );
    assert_eq!(map(&screen, key(KeyCode::Char('r'))), Some(Event::Refresh),);
    assert_eq!(map(&screen, key(KeyCode::Char('q'))), Some(Event::Quit));
    // Sub-tasks (0019): `A` (Shift+a) adds a sub-task to the selection's parent; `x` toggles the
    // parent's collapse/expand. `a` stays add-task (asserted above).
    assert_eq!(
        map(&screen, key(KeyCode::Char('A'))),
        Some(Event::BeginAddSubtask),
        "A adds a sub-task (0019)",
    );
    assert_eq!(
        map(&screen, key(KeyCode::Char('x'))),
        Some(Event::ToggleCollapse),
        "x toggles collapse on the Tasks tab (0019, the freed pre-0016 delete key)",
    );
    // The old `c` (toggle-done) key no longer fires its action — `c` is unbound on the Tasks tab.
    assert_eq!(
        map(&screen, key(KeyCode::Char('c'))),
        None,
        "c no longer toggles done",
    );
    // The old `n` (open notes) / `s` (open profiles) cross-screen keys are removed (ADR-0010 §1):
    // they are now unbound on the Tasks tab.
    assert_eq!(map(&screen, key(KeyCode::Char('n'))), None, "n unbound now");
    assert_eq!(map(&screen, key(KeyCode::Char('s'))), None, "s unbound now");
    // An unbound printable key on the task list is ignored.
    assert_eq!(map(&screen, key(KeyCode::Char('z'))), None);
}

#[test]
fn notes_tab_command_keys() {
    // On the idle Notes tab: `a` create, `e` edit, `d` delete (was `x`), Enter opens the selected
    // note; the global timer/refresh/quit keys are live and `Tab` cycles tabs (covered above).
    let screen = notes_screen();
    assert_eq!(
        map(&screen, key(KeyCode::Char('a'))),
        Some(Event::BeginAddNote),
    );
    assert_eq!(
        map(&screen, key(KeyCode::Char('e'))),
        Some(Event::BeginEditNote),
    );
    assert_eq!(
        map(&screen, key(KeyCode::Char('d'))),
        Some(Event::BeginDeleteNote),
        "d deletes on the notes tab (the new binding)",
    );
    assert_eq!(map(&screen, key(KeyCode::Enter)), Some(Event::Submit));
    assert_eq!(map(&screen, key(KeyCode::Char('r'))), Some(Event::Refresh));
    assert_eq!(map(&screen, key(KeyCode::Char('q'))), Some(Event::Quit));
    // `Space` (toggle-done) is a Tasks-only command — unbound on the Notes tab.
    assert_eq!(
        map(&screen, key(KeyCode::Char(' '))),
        None,
        "Space unbound on notes"
    );
    // The old `x` delete is retired; unbound on the notes tab.
    assert_eq!(map(&screen, key(KeyCode::Char('x'))), None, "x unbound now");
    // `n`/`s` never switch tabs; `t` is the global timer toggle, not a tab switch.
    assert_eq!(map(&screen, key(KeyCode::Char('n'))), None);
    assert_eq!(
        map(&screen, key(KeyCode::Char('t'))),
        Some(Event::ToggleTimer),
    );
}

#[test]
fn edit_task_flow_types_command_letters_literally() {
    // Once the edit sub-flow is open the task list is a text-entry context, so the command
    // letters — including the e/d mutation keys, Space, and the global timer keys t/T — are typed
    // literally rather than triggering edit/toggle/delete or the timer toggle/config.
    let screen = task_list_screen_editing();
    for c in ['a', 'c', 'e', 'd', 'x', 'r', 'q', 't', 'T', ' '] {
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
    // letters — including the global timer keys 't'/'T' and the new Space/d keys — are typed
    // literally rather than triggering add/close/refresh/quit or the timer toggle/config.
    let screen = task_list_screen_adding();
    for c in ['a', 'c', 'd', 'r', 'q', 't', 'T', 'b', ' '] {
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
    assert_eq!(map(&screen, key(KeyCode::Char('t'))), None);
    assert_eq!(map(&screen, key(KeyCode::Char('T'))), None);
}

// ---- Global timer controls (ADR-0006 §8.2; 0016 remap `t`/`T`) ----

#[test]
fn t_toggles_the_timer_on_a_post_auth_screen() {
    // `t` is the global start/stop toggle, live on every post-auth screen (was `p`).
    assert_eq!(
        map(&task_list_screen(), key(KeyCode::Char('t'))),
        Some(Event::ToggleTimer),
    );
}

#[test]
fn shift_t_begins_the_timer_config_on_a_post_auth_screen() {
    // `T` opens the timer-config/duration dialog (was the old duration-edit `d`).
    assert_eq!(
        map(&task_list_screen(), key(KeyCode::Char('T'))),
        Some(Event::BeginEditDuration),
    );
}

#[test]
fn old_timer_keys_no_longer_fire() {
    // The pre-0016 timer keys are retired: `p` (old toggle) and `d` (old duration-edit) must NOT
    // produce a timer event on an idle post-auth screen — `p` is unbound, and `d` is now the delete
    // action, never the duration edit.
    let screen = task_list_screen();
    assert_eq!(
        map(&screen, key(KeyCode::Char('p'))),
        None,
        "p no longer toggles the timer",
    );
    assert_eq!(
        map(&screen, key(KeyCode::Char('d'))),
        Some(Event::DeleteSelected),
        "d is now the delete action, never the duration edit",
    );
}

#[test]
fn global_timer_keys_are_inactive_off_post_auth_screens() {
    // The timer widget is only shown post-auth (auth excluded — no session yet, Assumption B3), so
    // its global keys are not bound on the auth or offline screens. There, 't'/'T' are literal text
    // (auth, a text context) or ignored (offline, a command context with no timer binding).
    assert_eq!(
        map(&auth_screen(), key(KeyCode::Char('t'))),
        Some(Event::Char('t')),
    );
    assert_eq!(
        map(&auth_screen(), key(KeyCode::Char('T'))),
        Some(Event::Char('T')),
    );
    assert_eq!(map(&offline_screen(), key(KeyCode::Char('t'))), None);
    assert_eq!(map(&offline_screen(), key(KeyCode::Char('T'))), None);
}

#[test]
fn t_is_suppressed_while_a_text_entry_sub_flow_owns_keystrokes() {
    // Assumption B4: a literal `t` typed into a field is not hijacked by the global toggle. While
    // the add-task sub-flow owns keystrokes, OR while the duration-edit overlay is active, `t` is a
    // Char, not ToggleTimer.
    assert_eq!(
        map(&task_list_screen_adding(), key(KeyCode::Char('t'))),
        Some(Event::Char('t')),
    );
    assert_eq!(
        map_editing(&task_list_screen(), key(KeyCode::Char('t'))),
        Some(Event::Char('t')),
    );
}

#[test]
fn duration_edit_is_a_global_text_entry_context() {
    // While editing the duration (signalled by `editing_duration = true`) the active post-auth
    // screen is a text-entry context: digit keys (and the command letters t/T/r/a/d) are typed
    // literally, not interpreted as commands.
    let screen = task_list_screen();
    for c in ['2', '5', 't', 'T', 'r', 'a', 'd'] {
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
    // The dedicated timer screen is gone. Post-0016 `t` is the global start/stop toggle (NOT a
    // screen-opening navigation): on the task list it is `ToggleTimer`, never a page-open; literal
    // text in the auth form; ignored on the offline screen.
    assert_eq!(
        map(&auth_screen(), key(KeyCode::Char('t'))),
        Some(Event::Char('t')),
    );
    assert_eq!(
        map(&task_list_screen(), key(KeyCode::Char('t'))),
        Some(Event::ToggleTimer),
        "t toggles the timer in place, it does not open a timer page",
    );
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

// ---- Profiles tab (ADR-0009 §5 — a/e/d/r/Enter on the idle list, reached via Tab now) ----

#[test]
fn profiles_tab_list_command_keys() {
    // On the idle Profiles tab: `a` create, `e` rename, `d` delete (was `x`), `r` refresh, `q`
    // quit, Enter picks the active profile (Submit), Up/Down navigate. The old idle-`Esc`-back is
    // removed (ADR-0010 §1): the idle list is not a sub-flow, so Esc QUITS (tab-switch leaves).
    let screen = profiles_screen();
    assert_eq!(
        map(&screen, key(KeyCode::Char('a'))),
        Some(Event::BeginAddProfile),
    );
    assert_eq!(
        map(&screen, key(KeyCode::Char('e'))),
        Some(Event::BeginRenameProfile),
    );
    assert_eq!(
        map(&screen, key(KeyCode::Char('d'))),
        Some(Event::BeginDeleteProfile),
        "d deletes on the profiles tab (the new binding)",
    );
    assert_eq!(
        map(&screen, key(KeyCode::Char('x'))),
        None,
        "x no longer deletes on profiles",
    );
    assert_eq!(map(&screen, key(KeyCode::Char('r'))), Some(Event::Refresh));
    assert_eq!(map(&screen, key(KeyCode::Char('q'))), Some(Event::Quit));
    assert_eq!(map(&screen, key(KeyCode::Enter)), Some(Event::Submit));
    assert_eq!(map(&screen, key(KeyCode::Down)), Some(Event::Next));
    assert_eq!(map(&screen, key(KeyCode::Up)), Some(Event::Prev));
    // Idle post-auth list: Esc quits (no more idle-Esc-back).
    assert_eq!(map(&screen, key(KeyCode::Esc)), Some(Event::Quit));
    // An unbound printable key is ignored.
    assert_eq!(map(&screen, key(KeyCode::Char('z'))), None);
}

#[test]
fn profile_create_sub_flow_types_command_letters_literally() {
    // Once the create sub-flow is open the switcher is a text-entry context, so the command
    // letters — including the global timer keys p/d — are typed literally. Enter submits, Esc
    // cancels the sub-flow (not quit/back), Backspace edits.
    let screen = profiles_screen_creating();
    for c in ['a', 'e', 'x', 'r', 'q', 't', 'T', 'd'] {
        assert_eq!(
            map(&screen, key(KeyCode::Char(c))),
            Some(Event::Char(c)),
            "{c:?} must be literal text in the create-profile form",
        );
    }
    assert_eq!(map(&screen, key(KeyCode::Enter)), Some(Event::Submit));
    assert_eq!(map(&screen, key(KeyCode::Esc)), Some(Event::Cancel));
    assert_eq!(
        map(&screen, key(KeyCode::Backspace)),
        Some(Event::Backspace),
    );
}

#[test]
fn profile_rename_sub_flow_types_command_letters_literally() {
    // The rename sub-flow is likewise a text-entry context; Esc cancels it rather than navigating.
    let screen = profiles_screen_renaming();
    for c in ['a', 'e', 'x', 'r', 'q', 't', 'T', 'd'] {
        assert_eq!(
            map(&screen, key(KeyCode::Char(c))),
            Some(Event::Char(c)),
            "{c:?} must be literal text in the rename-profile form",
        );
    }
    assert_eq!(map(&screen, key(KeyCode::Esc)), Some(Event::Cancel));
}

#[test]
fn profile_switcher_esc_cancels_while_pending() {
    // A request in flight makes Esc mean Cancel (abandon), exactly as on the other screens.
    assert_eq!(
        map(&profiles_screen_pending(), key(KeyCode::Esc)),
        Some(Event::Cancel),
    );
}

#[test]
fn global_timer_keys_live_on_the_idle_switcher() {
    // The switcher is a post-auth screen, so the global timer toggle/config keys are live on the
    // idle list (they are suppressed only inside a text-entry sub-flow, covered above). `d` on the
    // profiles tab is the delete action (not the duration edit — that moved to `T`).
    let screen = profiles_screen();
    assert_eq!(
        map(&screen, key(KeyCode::Char('t'))),
        Some(Event::ToggleTimer),
    );
    assert_eq!(
        map(&screen, key(KeyCode::Char('T'))),
        Some(Event::BeginEditDuration),
    );
}

// ---- 0015: unified overlay-hotkey suppression (ADR-0010 §3, slice-5 acceptance 4) ----

#[test]
fn global_hotkeys_are_suppressed_while_a_dialog_captures_input() {
    // With any input-capturing overlay open — a text-entry add/edit form OR a non-text-entry
    // delete-confirmation dialog — every global hotkey (`q`/`r`/`p`/`d`/`?`/Tab) must NOT fire its
    // global action: it is either typed text (in a text-entry form), or unbound (in a confirmation
    // dialog), but crucially never the global event. This pins the suppression rule for every
    // dialog kind across the three tabs (Risk R1).
    let text_entry_dialogs = [
        task_list_screen_adding(),
        task_list_screen_editing(),
        notes_screen_creating(),
        notes_screen_editing(),
        profiles_screen_creating(),
        profiles_screen_renaming(),
    ];
    for screen in &text_entry_dialogs {
        for c in ['q', 'r', 't', 'T', 'd', '?'] {
            // In a text-entry overlay these land as literal Char, never the global action.
            assert_eq!(
                map(screen, key(KeyCode::Char(c))),
                Some(Event::Char(c)),
                "{c:?} must be literal text (global suppressed) in {screen:?}",
            );
        }
        // Tab switches the focused field, never cycles the top-level tabs.
        assert_eq!(
            map(screen, key(KeyCode::Tab)),
            Some(Event::Next),
            "Tab switches field (global tab-switch suppressed) in {screen:?}",
        );
    }

    // The non-text-entry confirmation dialogs capture input too: a global letter is NOT its global
    // action — it is unbound (so the global never fires).
    let confirm_dialogs = [
        notes_screen_confirming_delete(),
        profiles_screen_confirming_delete(),
        task_list_screen_confirming_delete(),
    ];
    for screen in &confirm_dialogs {
        for c in ['q', 'r', 't', 'T', 'd', '?'] {
            assert_eq!(
                map(screen, key(KeyCode::Char(c))),
                None,
                "{c:?} must be suppressed (no global action) in the confirm dialog {screen:?}",
            );
        }
        // Tab does not cycle tabs while a confirmation dialog is open.
        let tab = map(screen, key(KeyCode::Tab));
        assert!(
            !matches!(tab, Some(Event::NextTab)),
            "Tab must not cycle tabs in the confirm dialog {screen:?} (got {tab:?})",
        );
    }
}

#[test]
fn task_delete_confirmation_accepts_enter_to_confirm_and_esc_to_cancel() {
    // 0016 retires the old `x`-again two-step (Assumption A5): the task delete is now the 0015
    // confirm dialog, armed via `d` and CONFIRMED via `Enter` (Submit), with `Esc` to cancel. While
    // armed the dialog captures input, so every global letter — including a second `d` — is
    // suppressed (no global action fires).
    let screen = task_list_screen_confirming_delete();
    assert_eq!(
        map(&screen, key(KeyCode::Enter)),
        Some(Event::Submit),
        "Enter confirms the armed delete (the 0015 confirm dialog)",
    );
    assert_eq!(
        map(&screen, key(KeyCode::Esc)),
        Some(Event::Cancel),
        "Esc cancels the armed delete (not Quit)",
    );
    // Every global/action letter is suppressed while the confirmation is armed — including a second
    // `d` (it does not re-fire the delete) and the retired `x`.
    for c in ['q', 'r', 't', 'T', 'd', 'x', '?', 'a', 'e', 'c', ' '] {
        assert_eq!(
            map(&screen, key(KeyCode::Char(c))),
            None,
            "{c:?} must be suppressed while the task-delete confirmation is armed",
        );
    }
}

#[test]
fn question_mark_opens_help_only_on_an_idle_post_auth_screen() {
    // `?` is a global, live only on an idle post-auth screen with no overlay capturing input.
    for screen in [task_list_screen(), notes_screen(), profiles_screen()] {
        assert_eq!(
            map(&screen, key(KeyCode::Char('?'))),
            Some(Event::ToggleHelp),
            "? opens help on the idle post-auth screen {screen:?}",
        );
    }
    // `?` is inert off a post-auth screen: literal text on the auth form, unbound on offline.
    assert_eq!(
        map(&auth_screen(), key(KeyCode::Char('?'))),
        Some(Event::Char('?')),
        "? is literal text in the auth form",
    );
    assert_eq!(
        map(&offline_screen(), key(KeyCode::Char('?'))),
        None,
        "? is unbound on the offline screen",
    );
}

// ---- 0018: context-dependent commit keymap in the multiline note Content pane (ADR-0011 §2) ----

#[test]
fn enter_inserts_a_newline_only_while_editing_the_note_content_pane() {
    // ADR-0011 §2 / Risk R1: `Enter` maps to `Newline` ONLY while the multiline Content pane's edit
    // buffer is the active text-entry context; in every other commit context it stays `Submit`.
    // While editing Content, Enter → Newline.
    assert_eq!(
        map(&notes_screen_editing_content(), key(KeyCode::Enter)),
        Some(Event::Newline),
        "Enter inserts a newline while editing the multiline Content pane",
    );

    // Editing the single-line Title pane: Enter stays Submit (Title commits on Enter).
    assert_eq!(
        map(&notes_screen_editing_title(), key(KeyCode::Enter)),
        Some(Event::Submit),
        "Enter commits (Submit) while editing the single-line Title pane, never a newline",
    );

    // An open but idle note detail (no field edit): Enter stays Submit (it opens/commits, never a
    // newline) — the Content pane is not the active text-entry context.
    assert_eq!(
        map(&notes_screen_detail_idle(), key(KeyCode::Enter)),
        Some(Event::Submit),
        "Enter is Submit over an idle note detail, never a newline",
    );

    // And every other commit context keeps Enter as Submit (the broad-predicate regression, R1).
    for screen in [
        auth_screen(),
        task_list_screen(),
        notes_screen(),
        notes_screen_creating(),
        notes_screen_editing(),
        task_list_screen_editing(),
        profiles_screen(),
    ] {
        assert_eq!(
            map(&screen, key(KeyCode::Enter)),
            Some(Event::Submit),
            "Enter stays Submit (not Newline) on {screen:?}",
        );
    }
}

#[test]
fn ctrl_s_commits_while_a_text_entry_context_is_active_and_is_inert_otherwise() {
    // ADR-0011 §2 / Assumption A2: `Ctrl+S` maps to `Commit` while a text-entry context is active
    // (the multiline Content pane's commit key), and is INERT (None) everywhere else so it never
    // collides with a global hotkey.
    assert_eq!(
        map(&notes_screen_editing_content(), ctrl('s')),
        Some(Event::Commit),
        "Ctrl+S commits the focused field while editing the multiline Content pane",
    );
    // Ctrl+S also commits while editing the Title pane (a text-entry context); the note detail
    // handler treats Submit/Commit identically, so this stays consistent (A2).
    assert_eq!(
        map(&notes_screen_editing_title(), ctrl('s')),
        Some(Event::Commit),
        "Ctrl+S commits while editing a single-line field too (it is a text-entry context)",
    );
    // Ctrl+S in the create/edit note forms (text-entry contexts) commits as well.
    for screen in [
        notes_screen_creating(),
        notes_screen_editing(),
        task_list_screen_adding(),
        auth_screen(),
    ] {
        assert_eq!(
            map(&screen, ctrl('s')),
            Some(Event::Commit),
            "Ctrl+S commits in the text-entry context {screen:?}",
        );
    }

    // Inert (None) on every NON-text-entry context: idle lists, an idle detail, dialogs, offline —
    // it must never become a global action there (no collision, A2).
    for screen in [
        task_list_screen(),
        notes_screen(),
        notes_screen_detail_idle(),
        notes_screen_confirming_delete(),
        profiles_screen(),
        offline_screen(),
    ] {
        assert_eq!(
            map(&screen, ctrl('s')),
            None,
            "Ctrl+S is inert (no global collision) when no text entry is active on {screen:?}",
        );
    }
}

#[test]
fn ctrl_c_still_quits_over_ctrl_s_while_editing_content() {
    // Ctrl+C is checked first and always quits, even in the multiline Content edit where Ctrl+S is
    // the commit key — the two modifier branches do not interfere.
    assert_eq!(
        map(&notes_screen_editing_content(), ctrl('c')),
        Some(Event::Quit),
        "Ctrl+C quits even while editing the Content pane (checked before Ctrl+S)",
    );
}

#[test]
fn esc_cancels_inside_every_dialog_kind() {
    // The two-tiered Esc (ADR-0010 §3): Esc inside ANY open dialog cancels it (Event::Cancel),
    // never Quit — text-entry forms and confirmation dialogs alike.
    for screen in [
        task_list_screen_adding(),
        task_list_screen_editing(),
        task_list_screen_confirming_delete(),
        notes_screen_creating(),
        notes_screen_editing(),
        notes_screen_confirming_delete(),
        profiles_screen_creating(),
        profiles_screen_renaming(),
        profiles_screen_confirming_delete(),
    ] {
        assert_eq!(
            map(&screen, key(KeyCode::Esc)),
            Some(Event::Cancel),
            "Esc cancels (never Quit) inside the dialog {screen:?}",
        );
    }
}
