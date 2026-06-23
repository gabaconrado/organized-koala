//! The focus/timer view, driven through the public two-step `App` API (`handle_event` →
//! synchronous executor → `apply_response`) with the fake client as the only mock — the same
//! `TestBackend`/core idiom as the task suites. Maps the slice-4t acceptance criteria:
//!
//! - **Navigation**: `t` opens the timer (loading config→session from the server); the back key
//!   (`Cancel`) returns to the task list, re-listing tasks.
//! - **Start → running countdown**: starting renders a `MM:SS` countdown derived from the
//!   server's `ends_at` + `server_now` (via the rendered buffer and `countdown_label`).
//! - **Stop → idle render**.
//! - **Set-duration sub-flow** (`d` then input + submit) issues an `UpdateTimerConfig` carrying
//!   the typed minutes and reflects the new duration.
//! - **Completed render**: a `completed` session shows the completed state.
//! - **In-flight spinner** while a timer request is pending; **cancel** and a **stale/superseded
//!   RequestId** drop behave like the task flows.
//! - **Profile-switch leaves the timer unchanged** (account-global): the timer requests carry
//!   only the token, never a `profile_id` — verified by inspecting the recorded calls.
//!
//! Statelessness (hard-constraint #1): every value shown derives from a server response, and no
//! authoritative remaining-seconds integer is stored — the countdown is recomputed each draw.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

mod common;

use common::{
    Call, FakeClient, completed_session, drive, execute, open_task, profile, render, render_at,
    running_session, session, submit, timer_config,
};
use contract::{ErrorCode, TimerSession};
use tui::app::{App, Event, Screen};
use tui::ui::countdown_label;

const W: u16 = 80;
const H: u16 = 24;

/// Drive a fresh app from login to the `work` task list (no tasks), returning the shared fake and
/// the app. The fake is held so a test scripts the timer responses that follow.
fn logged_in() -> (FakeClient, App) {
    let client = FakeClient::new();
    client.push_login(Ok(session("jwt")));
    client.push_profiles(Ok(vec![profile("p1", "work")]));
    client.push_tasks(Ok(vec![]));
    let mut app = App::new();
    submit(&mut app, &client, Event::Submit);
    assert!(matches!(app.screen(), Screen::TaskList(_)), "logged in");
    (client, app)
}

/// Open the timer view: script the entry config→session load (idle by default unless overridden)
/// and submit `OpenTimer`. Returns with the app on the timer screen.
fn open_timer_idle(client: &FakeClient, app: &mut App) {
    client.push_timer_config(Ok(timer_config(30)));
    client.push_timer_session(Ok(TimerSession::Idle));
    submit(app, client, Event::OpenTimer);
    assert!(matches!(app.screen(), Screen::Timer(_)), "on timer screen");
}

// ---- Navigation ----

#[test]
fn open_timer_loads_config_then_session_from_server() {
    let (client, mut app) = logged_in();
    // Entry chains GetTimerConfig -> GetTimerSession (both server-derived, #1).
    client.push_timer_config(Ok(timer_config(45)));
    client.push_timer_session(Ok(TimerSession::Idle));
    submit(&mut app, &client, Event::OpenTimer);

    let Screen::Timer(timer) = app.screen() else {
        panic!("opened the timer view");
    };
    assert_eq!(timer.config.duration_minutes, 45, "config from the server");
    assert!(matches!(timer.session, TimerSession::Idle));

    // The entry calls are exactly config then session, account-global (token only, no profile).
    let calls = client.calls();
    assert!(
        matches!(calls.get(calls.len() - 2), Some(Call::GetTimerConfig { token }) if token == "jwt"),
        "config loaded on entry: {calls:?}",
    );
    assert!(
        matches!(calls.last(), Some(Call::GetTimerSession { token }) if token == "jwt"),
        "session loaded after config: {calls:?}",
    );

    // The rendered view shows the server's duration.
    let text = render(&app, W, H);
    assert!(text.contains("Duration: 45 min"), "duration shown:\n{text}");
    assert!(text.contains("Idle"), "idle state shown:\n{text}");
}

#[test]
fn back_key_returns_to_task_list_and_relists() {
    let (client, mut app) = logged_in();
    open_timer_idle(&client, &mut app);

    // Esc maps to Cancel on the timer; the core resolves it to back-to-task-list, re-listing.
    client.push_tasks(Ok(vec![open_task(
        "t1",
        "back home",
        "2026-06-18T10:00:00Z",
    )]));
    submit(&mut app, &client, Event::Cancel);

    let Screen::TaskList(list) = app.screen() else {
        panic!("returned to the task list");
    };
    assert_eq!(list.tasks.first().expect("task").title, "back home");
    assert!(
        matches!(client.calls().last(), Some(Call::ListTasks { profile_id, .. }) if profile_id == "p1"),
        "re-listed the active profile's tasks on return",
    );
}

// ---- Start -> running countdown ----

#[test]
fn start_renders_running_countdown_from_ends_at_and_server_now() {
    let (client, mut app) = logged_in();
    open_timer_idle(&client, &mut app);

    // Start: the server returns a running session whose ends_at - server_now = 25:00.
    client.push_start_timer(Ok(running_session(
        "2026-06-11T12:00:00Z",
        "2026-06-11T12:30:00Z",
        30,
        "2026-06-11T12:05:00Z",
    )));
    submit(&mut app, &client, Event::StartTimer);

    let Screen::Timer(timer) = app.screen() else {
        panic!("still on the timer view");
    };
    // Derive the label the view computes from the server's instants (via the session DTO's own
    // `DateTime`s — no direct chrono construction in the test), pinning the countdown to
    // `ends_at − server_now` (since_response ~ 0 just-applied).
    let TimerSession::Running {
        ends_at,
        server_now,
        ..
    } = &timer.session
    else {
        panic!("session is running after start");
    };
    assert_eq!(
        countdown_label(ends_at.timestamp(), server_now.timestamp(), 0),
        "25:00",
        "label derived from ends_at - server_now",
    );
    assert!(
        matches!(client.calls().last(), Some(Call::StartTimerSession { token }) if token == "jwt"),
        "start carried only the token (account-global)",
    );

    // And the rendered buffer shows that running countdown.
    let text = render(&app, W, H);
    assert!(
        text.contains("25:00"),
        "running countdown rendered:\n{text}"
    );
    assert!(text.contains("Running"), "running state shown:\n{text}");
}

// ---- Stop -> idle render ----

#[test]
fn stop_returns_to_idle_render() {
    let (client, mut app) = logged_in();
    open_timer_idle(&client, &mut app);
    client.push_start_timer(Ok(running_session(
        "2026-06-11T12:00:00Z",
        "2026-06-11T12:30:00Z",
        30,
        "2026-06-11T12:05:00Z",
    )));
    submit(&mut app, &client, Event::StartTimer);

    // Stop resets to idle (no pause; ADR-0002 §5).
    client.push_stop_timer(Ok(TimerSession::Idle));
    submit(&mut app, &client, Event::StopTimer);

    let Screen::Timer(timer) = app.screen() else {
        panic!("still on the timer view");
    };
    assert!(
        matches!(timer.session, TimerSession::Idle),
        "stop reset the session to idle",
    );
    assert!(
        matches!(client.calls().last(), Some(Call::StopTimerSession { token }) if token == "jwt"),
        "stop carried only the token",
    );
    let text = render(&app, W, H);
    assert!(text.contains("Idle"), "idle render after stop:\n{text}");
    assert!(!text.contains("Running"), "no running state:\n{text}");
}

// ---- Set-duration sub-flow ----

#[test]
fn set_duration_issues_update_and_reflects_new_value() {
    let (client, mut app) = logged_in();
    open_timer_idle(&client, &mut app);

    // `d` opens the edit sub-flow seeded with the current duration; it does not dispatch.
    assert!(
        app.handle_event(Event::BeginEditDuration).is_none(),
        "opening the edit sub-flow dispatches nothing",
    );
    let Screen::Timer(timer) = app.screen() else {
        panic!("on the timer view");
    };
    assert!(timer.editing.is_some(), "edit sub-flow open");

    // Clear the seeded buffer and type 25.
    let _ = app.handle_event(Event::Backspace);
    let _ = app.handle_event(Event::Backspace);
    let _ = app.handle_event(Event::Char('2'));
    let _ = app.handle_event(Event::Char('5'));

    // Submit issues UpdateTimerConfig; success closes the edit and stores the new config.
    client.push_update_timer_config(Ok(timer_config(25)));
    submit(&mut app, &client, Event::Submit);

    let Screen::Timer(timer) = app.screen() else {
        panic!("still on the timer view");
    };
    assert!(timer.editing.is_none(), "edit closed after success");
    assert_eq!(timer.config.duration_minutes, 25, "new duration stored");
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
        text.contains("Duration: 25 min"),
        "new duration shown:\n{text}"
    );
}

#[test]
fn set_duration_validation_error_surfaces_inline_in_edit() {
    let (client, mut app) = logged_in();
    open_timer_idle(&client, &mut app);

    let _ = app.handle_event(Event::BeginEditDuration);
    let _ = app.handle_event(Event::Backspace);
    let _ = app.handle_event(Event::Backspace);
    let _ = app.handle_event(Event::Char('0'));

    // The server rejects 0 with validation_failed; the error surfaces in the edit sub-flow and
    // the view stays on the timer (no navigation).
    client.push_update_timer_config(Err(common::api_err(
        ErrorCode::ValidationFailed,
        "duration must be between 1 and 1440",
    )));
    submit(&mut app, &client, Event::Submit);

    let Screen::Timer(timer) = app.screen() else {
        panic!("stayed on the timer view");
    };
    let edit = timer
        .editing
        .as_ref()
        .expect("edit sub-flow stays open on error");
    assert!(
        edit.error.as_deref().unwrap_or("").contains("1 and 1440"),
        "validation error shown inline: {:?}",
        edit.error,
    );
}

// ---- Completed render ----

#[test]
fn completed_session_renders_completed_state() {
    let (client, mut app) = logged_in();
    // Open straight onto a completed session (server's verdict, server_now >= ends_at).
    client.push_timer_config(Ok(timer_config(30)));
    client.push_timer_session(Ok(completed_session(
        "2026-06-11T12:00:00Z",
        "2026-06-11T12:30:00Z",
        30,
        "2026-06-11T12:31:00Z",
    )));
    submit(&mut app, &client, Event::OpenTimer);

    let Screen::Timer(timer) = app.screen() else {
        panic!("on the timer view");
    };
    assert!(
        matches!(timer.session, TimerSession::Completed { .. }),
        "session is the server's completed verdict",
    );
    let text = render(&app, W, H);
    assert!(text.contains("Completed"), "completed state shown:\n{text}");
    assert!(text.contains("00:00"), "completed shows 00:00:\n{text}");
}

// ---- In-flight spinner ----

#[test]
fn start_shows_in_flight_spinner_until_response() {
    let (client, mut app) = logged_in();
    open_timer_idle(&client, &mut app);

    // Begin a start but hold its dispatch (don't drive it): the timer is now in flight.
    let dispatch = app
        .handle_event(Event::StartTimer)
        .expect("start dispatches");
    assert!(app.is_pending(), "in-flight after start");

    // The render shows the spinner + "working…" hint (tick drives the glyph).
    let text = render_at(&app, W, H, 1);
    assert!(
        text.contains("working…") && text.contains("Esc to cancel"),
        "in-flight working hint rendered:\n{text}",
    );

    // Completing the held request settles the screen back to idle interactivity.
    client.push_start_timer(Ok(running_session(
        "2026-06-11T12:00:00Z",
        "2026-06-11T12:30:00Z",
        30,
        "2026-06-11T12:05:00Z",
    )));
    drive(&mut app, &client, dispatch);
    assert!(!app.is_pending(), "settled after the start completes");
}

#[test]
fn request_triggering_event_while_pending_is_a_no_op() {
    let (client, mut app) = logged_in();
    open_timer_idle(&client, &mut app);

    let first = app
        .handle_event(Event::StartTimer)
        .expect("start dispatches");
    assert!(app.is_pending());
    let calls_after_first = client.calls().len();

    // A second request-triggering event while pending dispatches nothing and makes no call.
    assert!(app.handle_event(Event::StopTimer).is_none());
    assert!(app.handle_event(Event::Refresh).is_none());
    assert_eq!(
        client.calls().len(),
        calls_after_first,
        "no extra call while a timer request is outstanding",
    );

    client.push_start_timer(Ok(TimerSession::Idle));
    drive(&mut app, &client, first);
    assert!(!app.is_pending());
}

// ---- Cancel / stale-id drop ----

#[test]
fn cancel_while_pending_clears_in_flight() {
    let (client, mut app) = logged_in();
    open_timer_idle(&client, &mut app);

    let _dispatch = app
        .handle_event(Event::StartTimer)
        .expect("start dispatches");
    assert!(app.is_pending(), "in-flight before cancel");

    // While pending, Cancel clears the marker (it does not leave the timer view).
    assert!(app.handle_event(Event::Cancel).is_none());
    assert!(!app.is_pending(), "cancel cleared the in-flight marker");
    assert!(
        matches!(app.screen(), Screen::Timer(_)),
        "still on the timer view after cancelling an in-flight request",
    );

    // Interactive again: a fresh request dispatches.
    let next = app.handle_event(Event::Refresh);
    assert!(next.is_some(), "screen accepts a new request after cancel");
    let _ = client;
}

#[test]
fn stale_response_after_cancel_is_dropped() {
    let (client, mut app) = logged_in();
    open_timer_idle(&client, &mut app);

    // Begin a start, capture the dispatch, then cancel before the response lands.
    let dispatch = app
        .handle_event(Event::StartTimer)
        .expect("start dispatches");
    assert!(app.handle_event(Event::Cancel).is_none());
    assert!(!app.is_pending(), "cancelled");

    // The abandoned request still ran on the (mocked) server; its stale response must be dropped
    // (RequestId mismatch) — the session stays idle, not flipped to running.
    client.push_start_timer(Ok(running_session(
        "2026-06-11T12:00:00Z",
        "2026-06-11T12:30:00Z",
        30,
        "2026-06-11T12:05:00Z",
    )));
    let stale = execute(&client, dispatch);
    assert!(
        app.apply_response(stale).is_none(),
        "a stale response yields no follow-up",
    );
    let Screen::Timer(timer) = app.screen() else {
        panic!("still on the timer view");
    };
    assert!(
        matches!(timer.session, TimerSession::Idle),
        "the dropped stale start must not flip the session to running",
    );
    assert!(
        !app.is_pending(),
        "still idle after dropping the stale response"
    );
}

#[test]
fn superseded_response_after_new_request_is_dropped() {
    let (client, mut app) = logged_in();
    open_timer_idle(&client, &mut app);

    let first = app
        .handle_event(Event::StartTimer)
        .expect("start dispatches");
    assert!(app.handle_event(Event::Cancel).is_none());

    // New request after cancel gets a fresh RequestId — now the awaited one.
    let second = app
        .handle_event(Event::Refresh)
        .expect("refresh dispatches");
    assert!(app.is_pending());

    // The first (cancelled) start's late response carries the old id and is dropped.
    client.push_start_timer(Ok(running_session(
        "2026-06-11T12:00:00Z",
        "2026-06-11T12:30:00Z",
        30,
        "2026-06-11T12:05:00Z",
    )));
    let stale = execute(&client, first);
    assert!(
        app.apply_response(stale).is_none(),
        "the superseded response is dropped",
    );
    assert!(app.is_pending(), "the new request is still in flight");

    // The new (refresh) request then completes normally, driving the view.
    client.push_timer_session(Ok(completed_session(
        "2026-06-11T12:00:00Z",
        "2026-06-11T12:30:00Z",
        30,
        "2026-06-11T12:31:00Z",
    )));
    drive(&mut app, &client, second);
    let Screen::Timer(timer) = app.screen() else {
        panic!("on the timer view");
    };
    assert!(
        matches!(timer.session, TimerSession::Completed { .. }),
        "the new request's response drove the view, not the stale start",
    );
    assert!(!app.is_pending());
}

// ---- Profile-switch leaves the timer unchanged (account-global) ----

#[test]
fn timer_requests_are_account_global_not_profile_scoped() {
    // The highest-value account-global assertion (#4 / ADR-0002 §5): every timer request carries
    // only the token, never a profile_id — so the timer is keyed on the account, and switching the
    // active profile cannot change which session/config is read. The TUI model exposes this by
    // the call shape: timer Calls have no profile field, unlike ListTasks/CreateTask/CloseTask.
    let (client, mut app) = logged_in();
    open_timer_idle(&client, &mut app);

    // Exercise the full timer command surface.
    client.push_start_timer(Ok(running_session(
        "2026-06-11T12:00:00Z",
        "2026-06-11T12:30:00Z",
        30,
        "2026-06-11T12:05:00Z",
    )));
    submit(&mut app, &client, Event::StartTimer);
    client.push_timer_session(Ok(running_session(
        "2026-06-11T12:00:00Z",
        "2026-06-11T12:30:00Z",
        30,
        "2026-06-11T12:06:00Z",
    )));
    submit(&mut app, &client, Event::Refresh);
    client.push_stop_timer(Ok(TimerSession::Idle));
    submit(&mut app, &client, Event::StopTimer);

    // None of the timer calls carry a profile id; the same token addresses them all.
    for call in client.calls() {
        match call {
            Call::GetTimerConfig { token }
            | Call::UpdateTimerConfig { token, .. }
            | Call::GetTimerSession { token }
            | Call::StartTimerSession { token }
            | Call::StopTimerSession { token } => {
                assert_eq!(
                    token, "jwt",
                    "timer call keyed on the account token only: {token}"
                );
            }
            // Task/auth calls may carry a profile; that is the profile-namespaced surface (#4),
            // which is exactly what the timer surface must NOT do.
            _ => {}
        }
    }

    // The session derived purely from the timer endpoints — independent of the active profile.
    let Screen::Timer(timer) = app.screen() else {
        panic!("on the timer view");
    };
    assert!(
        matches!(timer.session, TimerSession::Idle),
        "stopped to idle"
    );
}

#[test]
fn timer_session_unaffected_by_returning_through_the_task_list() {
    // Leaving and re-entering the timer (the only navigation path) reloads it from the
    // account-global endpoints; the server's session is the single source of truth and is the
    // same regardless of which profile's task list we passed through.
    let (client, mut app) = logged_in();

    // Open onto a running session.
    client.push_timer_config(Ok(timer_config(30)));
    client.push_timer_session(Ok(running_session(
        "2026-06-11T12:00:00Z",
        "2026-06-11T12:30:00Z",
        30,
        "2026-06-11T12:05:00Z",
    )));
    submit(&mut app, &client, Event::OpenTimer);
    assert!(matches!(app.screen(), Screen::Timer(_)));

    // Back to the task list (re-list), then re-open: the running session is still observed
    // because it lives server-side, not in the (stateless, #1) TUI.
    client.push_tasks(Ok(vec![]));
    submit(&mut app, &client, Event::Cancel);
    assert!(matches!(app.screen(), Screen::TaskList(_)));

    client.push_timer_config(Ok(timer_config(30)));
    client.push_timer_session(Ok(running_session(
        "2026-06-11T12:00:00Z",
        "2026-06-11T12:30:00Z",
        30,
        "2026-06-11T12:07:00Z",
    )));
    submit(&mut app, &client, Event::OpenTimer);

    let Screen::Timer(timer) = app.screen() else {
        panic!("re-opened the timer view");
    };
    assert!(
        matches!(timer.session, TimerSession::Running { .. }),
        "the server-side running session is observed again on re-entry",
    );
}
