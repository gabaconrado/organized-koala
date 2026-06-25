//! The account-global focus timer — now a persistent widget rendered on every post-auth screen
//! with a global `p` toggle, not a navigable screen (ADR-0006 §8 / Board 0008-R1). Driven through
//! the public two-step `App` API (`handle_event` / the `load_timer`/`refresh_timer` edge hooks →
//! synchronous executor → `apply_response`) with the fake client as the only mock — the same
//! `TestBackend`/core idiom as the task suites. Maps the 0008-R1 re-entry acceptance criteria:
//!
//! - **Global `p` toggle** (criterion 1): from a non-timer post-auth screen, `p` starts when
//!   idle/completed and stops when running; a second `p` while the toggle is pending is a no-op.
//! - **`p` suppressed in text-entry** (criterion 2): a literal `p` typed into the add-task or
//!   duration-edit field does not trigger the toggle (covered as keybinding contract in
//!   `keybindings.rs`; here we prove the duration-edit overlay swallows `p` end-to-end).
//! - **Global widget renders** (criterion 3): the timer widget (idle/running countdown/completed)
//!   shows in the bottom area of the task-list buffer — not a dedicated page.
//! - **Append-spinner, no flicker** (criterion 4): with a request in flight the hotkey caption is
//!   still present and a trailing spinner is appended — the regression guard for the flicker bug.
//! - **No dedicated timer page** (criterion 5): there is no `Screen::Timer` to navigate to and no
//!   open-timer event (compile-time absence + the behavioural keybinding guards).
//! - **Account-global unchanged** (criterion 6): every timer request carries only the token.
//! - **`p` in the caption** (criterion 7): the bottom-left caption lists the `p` start/stop hotkey.
//! - **Existing behaviour preserved**: start→running countdown, stop→idle, set-duration sub-flow,
//!   completed render, in-flight cancel, stale/superseded RequestId drop.
//!
//! Statelessness (hard-constraint #1): every value shown derives from a server response, and no
//! authoritative remaining-seconds integer is stored — the countdown is recomputed each draw.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

mod common;

use common::{
    Call, FakeClient, completed_session, drive, execute, load_timer, profile, refresh_timer,
    render, render_at, running_session, session, submit, timer_config,
};
use contract::{ErrorCode, TimerSession};
use tui::app::{App, Event, Screen};
use tui::ui::countdown_label;

const W: u16 = 80;
const H: u16 = 24;

/// Drive a fresh app from login to the `work` task list (no tasks), then run the edge's initial
/// timer load (`load_timer_if_needed`) so the global widget reflects a server-returned config +
/// session — the state the real loop reaches on the first post-login frame. `idle_minutes` is the
/// configured duration the (idle) timer loads with. Returns the shared fake and the app.
fn logged_in_with_idle_timer(idle_minutes: u32) -> (FakeClient, App) {
    let client = FakeClient::new();
    client.push_login(Ok(session("jwt")));
    client.push_profiles(Ok(vec![profile("p1", "work")]));
    client.push_tasks(Ok(vec![]));
    let mut app = App::new();
    submit(&mut app, &client, Event::Submit);
    assert!(
        matches!(app.screen(), Screen::TaskList(_)),
        "logged in to the task list",
    );

    // The edge loads the account-global timer once a post-auth screen is shown (config→session).
    client.push_timer_config(Ok(timer_config(idle_minutes)));
    client.push_timer_session(Ok(TimerSession::Idle));
    load_timer(&mut app, &client);
    assert!(
        matches!(app.timer().session, TimerSession::Idle),
        "timer loaded idle from the server",
    );
    (client, app)
}

// ---- Criterion 3 + 7: the global widget renders on a post-auth screen, with `p` in the caption ----

#[test]
fn global_timer_widget_renders_on_the_task_list() {
    let (_client, app) = logged_in_with_idle_timer(30);
    // We are on the task list (NOT a dedicated timer page) and the timer widget shows there.
    assert!(matches!(app.screen(), Screen::TaskList(_)), "on task list");

    let text = render(&app, W, H);
    assert!(
        text.contains("timer idle") && text.contains("30 min"),
        "idle timer widget shows on the task-list buffer:\n{text}",
    );
    // Criterion 7: the bottom-left caption lists the `p` start/stop hotkey (the help-menu entry).
    assert!(
        text.contains("p: timer"),
        "the `p` toggle is listed in the hotkey caption:\n{text}",
    );
}

#[test]
fn running_widget_renders_mmss_countdown_on_the_task_list() {
    let (_client, app) = logged_in_with_idle_timer_running();
    let text = render(&app, W, H);
    assert!(
        text.contains("timer 25:00"),
        "running countdown rendered in the global widget:\n{text}",
    );
}

#[test]
fn completed_widget_renders_completed_on_the_task_list() {
    let client = FakeClient::new();
    client.push_login(Ok(session("jwt")));
    client.push_profiles(Ok(vec![profile("p1", "work")]));
    client.push_tasks(Ok(vec![]));
    let mut app = App::new();
    submit(&mut app, &client, Event::Submit);
    // Load onto a completed session (server's verdict, server_now >= ends_at).
    client.push_timer_config(Ok(timer_config(30)));
    client.push_timer_session(Ok(completed_session(
        "2026-06-11T12:00:00Z",
        "2026-06-11T12:30:00Z",
        30,
        "2026-06-11T12:31:00Z",
    )));
    load_timer(&mut app, &client);

    assert!(
        matches!(app.timer().session, TimerSession::Completed { .. }),
        "session is the server's completed verdict",
    );
    let text = render(&app, W, H);
    assert!(
        text.contains("timer completed"),
        "completed state shown in the widget:\n{text}",
    );
}

// ---- Initial load: config then session, account-global ----

#[test]
fn initial_load_pulls_config_then_session_account_global() {
    let (client, app) = logged_in_with_idle_timer(45);
    // Entry chains GetTimerConfig -> GetTimerSession (both server-derived, #1), token-only.
    let calls = client.calls();
    assert!(
        calls
            .iter()
            .any(|c| matches!(c, Call::GetTimerConfig { token } if token == "jwt"),),
        "config loaded on entry: {calls:?}",
    );
    assert!(
        calls
            .iter()
            .any(|c| matches!(c, Call::GetTimerSession { token } if token == "jwt"),),
        "session loaded after config: {calls:?}",
    );
    assert_eq!(
        app.timer().config.duration_minutes,
        45,
        "config came from the server",
    );
    let text = render(&app, W, H);
    assert!(text.contains("45 min"), "server duration shown:\n{text}");
}

// ---- Criterion 1: the global `p` toggle ----

#[test]
fn p_starts_the_session_when_idle() {
    let (client, mut app) = logged_in_with_idle_timer(30);

    client.push_start_timer(Ok(running_session(
        "2026-06-11T12:00:00Z",
        "2026-06-11T12:30:00Z",
        30,
        "2026-06-11T12:05:00Z",
    )));
    submit(&mut app, &client, Event::ToggleTimer);

    // Idle → toggle issues StartTimerSession (token only, account-global).
    assert!(
        matches!(client.calls().last(), Some(Call::StartTimerSession { token }) if token == "jwt"),
        "p while idle starts the session (token only): {:?}",
        client.calls(),
    );
    assert!(
        matches!(app.timer().session, TimerSession::Running { .. }),
        "session is running after the start toggle",
    );
    // Still on the task list — the toggle is global, it does not navigate.
    assert!(matches!(app.screen(), Screen::TaskList(_)), "no navigation");
}

#[test]
fn p_stops_the_session_when_running() {
    let (client, mut app) = logged_in_with_idle_timer_running();

    // Running → toggle issues StopTimerSession; stop resets to idle (no pause; ADR-0002 §5).
    client.push_stop_timer(Ok(TimerSession::Idle));
    submit(&mut app, &client, Event::ToggleTimer);

    assert!(
        matches!(client.calls().last(), Some(Call::StopTimerSession { token }) if token == "jwt"),
        "p while running stops the session (token only): {:?}",
        client.calls(),
    );
    assert!(
        matches!(app.timer().session, TimerSession::Idle),
        "stop reset the session to idle",
    );
    let text = render(&app, W, H);
    assert!(
        text.contains("timer idle"),
        "idle widget after stop:\n{text}",
    );
}

#[test]
fn p_starts_the_session_when_completed() {
    let client = FakeClient::new();
    client.push_login(Ok(session("jwt")));
    client.push_profiles(Ok(vec![profile("p1", "work")]));
    client.push_tasks(Ok(vec![]));
    let mut app = App::new();
    submit(&mut app, &client, Event::Submit);
    client.push_timer_config(Ok(timer_config(30)));
    client.push_timer_session(Ok(completed_session(
        "2026-06-11T12:00:00Z",
        "2026-06-11T12:30:00Z",
        30,
        "2026-06-11T12:31:00Z",
    )));
    load_timer(&mut app, &client);
    assert!(matches!(
        app.timer().session,
        TimerSession::Completed { .. }
    ));

    // Completed is treated like idle by the toggle: `p` starts a fresh session.
    client.push_start_timer(Ok(running_session(
        "2026-06-11T13:00:00Z",
        "2026-06-11T13:30:00Z",
        30,
        "2026-06-11T13:00:00Z",
    )));
    submit(&mut app, &client, Event::ToggleTimer);
    assert!(
        matches!(client.calls().last(), Some(Call::StartTimerSession { token }) if token == "jwt"),
        "p while completed starts a new session: {:?}",
        client.calls(),
    );
    assert!(matches!(app.timer().session, TimerSession::Running { .. }));
}

#[test]
fn second_p_while_the_toggle_is_pending_is_a_no_op() {
    let (client, mut app) = logged_in_with_idle_timer(30);

    // First `p` dispatches a start and the timer is now in flight (hold the dispatch, don't drive).
    let dispatch = app
        .handle_event(Event::ToggleTimer)
        .expect("p dispatches a start");
    assert!(
        app.timer().is_pending(),
        "timer in flight after the first p"
    );
    let calls_after_first = client.calls().len();

    // A second `p` while the toggle is already pending dispatches nothing — no duplicate request.
    assert!(
        app.handle_event(Event::ToggleTimer).is_none(),
        "a second p while pending is a no-op",
    );
    assert_eq!(
        client.calls().len(),
        calls_after_first,
        "no duplicate timer request while one is in flight",
    );

    // Completing the held start settles the timer.
    client.push_start_timer(Ok(running_session(
        "2026-06-11T12:00:00Z",
        "2026-06-11T12:30:00Z",
        30,
        "2026-06-11T12:05:00Z",
    )));
    drive(&mut app, &client, dispatch);
    assert!(
        !app.timer().is_pending(),
        "settled after the start completes"
    );
}

// ---- Criterion 4: append-spinner, no flicker (the key feedback fix) ----

#[test]
fn in_flight_appends_a_spinner_without_replacing_the_caption() {
    let (_client, mut app) = logged_in_with_idle_timer(30);

    // Begin a toggle but hold its dispatch (don't drive it): the timer is in flight.
    let _dispatch = app.handle_event(Event::ToggleTimer).expect("p dispatches");
    assert!(app.timer().is_pending(), "in-flight after the toggle");

    // The regression guard: the stable hotkey caption is STILL present (not replaced by a
    // "working…" string), and the cancel affordance plus a trailing spinner glyph are appended.
    let text = render_at(&app, W, H, 1);
    assert!(
        text.contains("p: timer"),
        "the hotkey caption is NOT replaced while in flight (no flicker):\n{text}",
    );
    assert!(
        text.contains("a: add") && text.contains("q: quit"),
        "the full caption stays present while in flight:\n{text}",
    );
    assert!(
        text.contains("Esc to cancel"),
        "the cancel affordance is appended:\n{text}",
    );
    // A spinner glyph is appended (tick 1 → "/").
    assert!(
        text.contains('/'),
        "a trailing spinner glyph is appended at tick 1:\n{text}",
    );
}

#[test]
fn idle_caption_has_no_spinner_or_cancel_affordance() {
    // Contrast: with nothing in flight the caption is the bare hotkey list — no spinner, no cancel.
    let (_client, app) = logged_in_with_idle_timer(30);
    let text = render(&app, W, H);
    assert!(text.contains("p: timer"), "caption present:\n{text}");
    assert!(
        !text.contains("Esc to cancel"),
        "no cancel affordance when idle:\n{text}",
    );
}

// ---- Set-duration sub-flow (preserved, reached by `d` from a post-auth screen) ----

#[test]
fn set_duration_issues_update_and_reflects_new_value() {
    let (client, mut app) = logged_in_with_idle_timer(30);

    // `d` opens the edit sub-flow seeded with the current duration; it does not dispatch.
    assert!(
        app.handle_event(Event::BeginEditDuration).is_none(),
        "opening the edit sub-flow dispatches nothing",
    );
    assert!(app.is_editing_duration(), "edit sub-flow open");

    // Clear the seeded buffer and type 25.
    let _ = app.handle_event(Event::Backspace);
    let _ = app.handle_event(Event::Backspace);
    let _ = app.handle_event(Event::Char('2'));
    let _ = app.handle_event(Event::Char('5'));

    // Submit issues UpdateTimerConfig; success closes the edit and stores the new config.
    client.push_update_timer_config(Ok(timer_config(25)));
    submit(&mut app, &client, Event::Submit);

    assert!(!app.is_editing_duration(), "edit closed after success");
    assert_eq!(
        app.timer().config.duration_minutes,
        25,
        "new duration stored",
    );
    assert!(
        matches!(
            client.calls().last(),
            Some(Call::UpdateTimerConfig { token, duration_minutes })
                if token == "jwt" && *duration_minutes == 25
        ),
        "the update carried the typed minutes, token only: {:?}",
        client.calls(),
    );
    let text = render(&app, W, H);
    assert!(
        text.contains("25 min"),
        "new duration shown in the widget:\n{text}",
    );
}

#[test]
fn set_duration_validation_error_surfaces_inline_in_edit() {
    let (client, mut app) = logged_in_with_idle_timer(30);

    let _ = app.handle_event(Event::BeginEditDuration);
    let _ = app.handle_event(Event::Backspace);
    let _ = app.handle_event(Event::Backspace);
    let _ = app.handle_event(Event::Char('0'));

    // The server rejects 0 with validation_failed; the error surfaces in the edit sub-flow and the
    // edit stays open (no navigation — there is no timer screen to leave).
    client.push_update_timer_config(Err(common::api_err(
        ErrorCode::ValidationFailed,
        "duration must be between 1 and 1440",
    )));
    submit(&mut app, &client, Event::Submit);

    assert!(
        app.is_editing_duration(),
        "edit sub-flow stays open on error",
    );
    let text = render(&app, W, H);
    assert!(
        text.contains("1 and 1440"),
        "validation error shown inline in the edit overlay:\n{text}",
    );
}

#[test]
fn p_is_suppressed_while_editing_duration_end_to_end() {
    // Criterion 2 (end-to-end): while the duration-edit overlay owns keystrokes, a `Char('p')` is
    // fed into the buffer, not interpreted as ToggleTimer — no timer request is dispatched.
    let (client, mut app) = logged_in_with_idle_timer(30);
    let _ = app.handle_event(Event::BeginEditDuration);
    assert!(app.is_editing_duration());
    let calls_before = client.calls().len();

    // A literal 'p' while editing edits the buffer (digits-only filter drops it) but never toggles.
    assert!(
        app.handle_event(Event::Char('p')).is_none(),
        "a literal p while editing dispatches nothing (not a toggle)",
    );
    assert!(app.is_editing_duration(), "still editing after a literal p");
    assert_eq!(
        client.calls().len(),
        calls_before,
        "no timer request issued by a literal p in the edit field",
    );
}

// ---- Start -> running countdown (preserved; now derived through the global widget) ----

#[test]
fn start_renders_running_countdown_from_ends_at_and_server_now() {
    let (client, mut app) = logged_in_with_idle_timer(30);

    // Start: the server returns a running session whose ends_at - server_now = 25:00.
    client.push_start_timer(Ok(running_session(
        "2026-06-11T12:00:00Z",
        "2026-06-11T12:30:00Z",
        30,
        "2026-06-11T12:05:00Z",
    )));
    submit(&mut app, &client, Event::ToggleTimer);

    // Derive the label the view computes from the server's instants, pinning the countdown to
    // `ends_at − server_now` (since_response ~ 0 just-applied).
    let TimerSession::Running {
        ends_at,
        server_now,
        ..
    } = &app.timer().session
    else {
        panic!("session is running after start");
    };
    assert_eq!(
        countdown_label(ends_at.timestamp(), server_now.timestamp(), 0),
        "25:00",
        "label derived from ends_at - server_now",
    );
    let text = render(&app, W, H);
    assert!(
        text.contains("timer 25:00"),
        "running countdown rendered in the widget:\n{text}",
    );
}

// ---- Stale-id drop (keyed on the timer's own in-flight marker, independent of the screen) ----

#[test]
fn superseded_timer_response_is_dropped() {
    let (client, mut app) = logged_in_with_idle_timer(30);

    // First toggle: a start dispatch we will hold and let go stale.
    let first = app
        .handle_event(Event::ToggleTimer)
        .expect("p dispatches a start");
    assert!(app.timer().is_pending());

    // Complete the first start normally so the timer marker clears and the session goes running.
    client.push_start_timer(Ok(running_session(
        "2026-06-11T12:00:00Z",
        "2026-06-11T12:30:00Z",
        30,
        "2026-06-11T12:05:00Z",
    )));
    drive(&mut app, &client, first.clone());
    assert!(!app.timer().is_pending(), "first start settled");
    assert!(matches!(app.timer().session, TimerSession::Running { .. }));

    // A LATE duplicate of the first response (same old RequestId) arriving after the marker cleared
    // must be dropped — it must not re-apply or disturb the settled state.
    client.push_start_timer(Ok(completed_session(
        "2026-06-11T12:00:00Z",
        "2026-06-11T12:30:00Z",
        30,
        "2026-06-11T12:31:00Z",
    )));
    let stale = execute(&client, first);
    assert!(
        app.apply_response(stale).is_none(),
        "a response whose id no longer matches the timer marker is dropped",
    );
    assert!(
        matches!(app.timer().session, TimerSession::Running { .. }),
        "the dropped stale response did not flip the settled running session to completed",
    );
}

// ---- Coarse refresh: account-global, picks up the server's verdict ----

#[test]
fn coarse_refresh_repulls_session_account_global() {
    let (client, mut app) = logged_in_with_idle_timer_running();

    // The coarse refresh (the ~1-min cadence) re-pulls the session; the server now reports
    // completed. The refresh carries only the token (account-global, #4 / ADR-0002 §5).
    client.push_timer_session(Ok(completed_session(
        "2026-06-11T12:00:00Z",
        "2026-06-11T12:30:00Z",
        30,
        "2026-06-11T12:31:00Z",
    )));
    refresh_timer(&mut app, &client);

    assert!(
        matches!(client.calls().last(), Some(Call::GetTimerSession { token }) if token == "jwt"),
        "coarse refresh re-pulled the session, token only: {:?}",
        client.calls(),
    );
    assert!(
        matches!(app.timer().session, TimerSession::Completed { .. }),
        "the server's completed verdict was folded in on refresh",
    );
}

// ---- Criterion 6: every timer request is account-global, never profile-scoped ----

#[test]
fn timer_requests_are_account_global_not_profile_scoped() {
    // The highest-value account-global assertion (#4 / ADR-0002 §5): every timer request carries
    // only the token, never a profile_id — unlike ListTasks/CreateTask/UpdateTask. Exercise the full
    // timer command surface (load, start, refresh, stop, update) and check the call shapes.
    let (client, mut app) = logged_in_with_idle_timer(30);

    client.push_start_timer(Ok(running_session(
        "2026-06-11T12:00:00Z",
        "2026-06-11T12:30:00Z",
        30,
        "2026-06-11T12:05:00Z",
    )));
    submit(&mut app, &client, Event::ToggleTimer);
    client.push_timer_session(Ok(running_session(
        "2026-06-11T12:00:00Z",
        "2026-06-11T12:30:00Z",
        30,
        "2026-06-11T12:06:00Z",
    )));
    refresh_timer(&mut app, &client);
    client.push_stop_timer(Ok(TimerSession::Idle));
    submit(&mut app, &client, Event::ToggleTimer);

    for call in client.calls() {
        match call {
            Call::GetTimerConfig { token }
            | Call::UpdateTimerConfig { token, .. }
            | Call::GetTimerSession { token }
            | Call::StartTimerSession { token }
            | Call::StopTimerSession { token } => {
                assert_eq!(
                    token, "jwt",
                    "timer call keyed on the account token only: {token}",
                );
            }
            // Task/auth calls may carry a profile; that is the profile-namespaced surface (#4),
            // which is exactly what the timer surface must NOT do.
            _ => {}
        }
    }
    assert!(
        matches!(app.timer().session, TimerSession::Idle),
        "stopped to idle",
    );
}

// ---- Helpers ----

/// A logged-in app whose global timer has been loaded onto a running session (ends_at − server_now
/// = 25:00), via the start toggle. Shared by the running-state tests.
fn logged_in_with_idle_timer_running() -> (FakeClient, App) {
    let (client, mut app) = logged_in_with_idle_timer(30);
    client.push_start_timer(Ok(running_session(
        "2026-06-11T12:00:00Z",
        "2026-06-11T12:30:00Z",
        30,
        "2026-06-11T12:05:00Z",
    )));
    submit(&mut app, &client, Event::ToggleTimer);
    assert!(
        matches!(app.timer().session, TimerSession::Running { .. }),
        "timer running after the start toggle",
    );
    (client, app)
}
