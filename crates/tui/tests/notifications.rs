//! Timer-completion desktop-notification suite (feature 0017): the pure fire-once edge-detection
//! core plus a thin edge-level test through a spy [`Notifier`].
//!
//! The *decision* to notify is pure `App` state — [`App::apply_timer_session`] folds a server
//! [`TimerSession`] and, on a real Running→Completed edge, sets a one-shot signal drained by
//! [`App::take_pending_notification`] (consume-once). So the bulk of this suite drives that pure
//! seam (via the public two-step `App` API + the synchronous worker-analogue executor in
//! `common`) and asserts the signal directly — no notifier, no daemon, no thread (ADR-0003 layer
//! 2, rust-standards learned 0005). The mock is only the sanctioned external-service trait.
//!
//! Coverage (Decision 3 / Decision 4 / Assumption A4):
//! 1. fires exactly once on Running→Completed, then `None` on the immediate re-call (consume-once);
//! 2. does NOT fire on Idle→Running, Running→Running re-pull, Completed→Completed re-pull,
//!    Running→Idle (stop), or Idle→Idle;
//! 3. initial-load (A4): the first session fold returning `Completed` only arms — no emit;
//! 4. re-arm: a new Running after a fired Completed, then Completed again ⇒ fires a second time;
//! 5. logout (`Timer::reset`) re-arms the guard (clears `notified_for_session`/`notify_pending`);
//! 6. an edge-level test pumping the signal through a spy [`Notifier`] (the only mock is the
//!    sanctioned external-service trait) asserts the fired copy and the one-call count.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

mod common;

use std::cell::RefCell;

use common::{
    FakeClient, completed_session, load_timer, profile, refresh_timer, running_session, session,
    submit, timer_config,
};
use contract::{ErrorCode, TimerSession};
use tui::app::{App, Event, Screen};
use tui::client::Notifier;

// Canonical wire instants reused across the suite (the exact values are immaterial to the
// edge logic — only the `state` discriminant drives the fire-once decision).
const STARTED: &str = "2026-06-11T12:00:00Z";
const ENDS: &str = "2026-06-11T12:30:00Z";
const NOW_RUNNING: &str = "2026-06-11T12:05:00Z";
const NOW_COMPLETED: &str = "2026-06-11T12:31:00Z";

/// The fixed notification copy `tui-dev` pinned (Assumption A5). Asserted by the spy-notifier
/// edge test so the wire from the pure signal to the effect carries the exact text.
const EXPECT_TITLE: &str = "Focus timer";
const EXPECT_BODY: &str = "Your focus session has ended.";

fn a_running() -> TimerSession {
    running_session(STARTED, ENDS, 30, NOW_RUNNING)
}

fn a_completed() -> TimerSession {
    completed_session(STARTED, ENDS, 30, NOW_COMPLETED)
}

/// A logged-in app whose global timer has loaded onto an **idle** session — the initial
/// config→session chain has run, so `applied_at` is now set (the initial-load guard A4 is past).
/// Returns the shared fake and the app, ready to script subsequent session outcomes.
fn logged_in_idle() -> (FakeClient, App) {
    let client = FakeClient::new();
    client.push_login(Ok(session("jwt")));
    client.push_profiles(Ok(vec![profile("p1", "work")]));
    client.push_tasks(Ok(vec![]));
    let mut app = App::new();
    submit(&mut app, &client, Event::Submit);
    assert!(matches!(app.screen(), Screen::Main(_)), "logged in");

    client.push_timer_config(Ok(timer_config(30)));
    client.push_timer_session(Ok(TimerSession::Idle));
    load_timer(&mut app, &client);
    assert!(
        matches!(app.timer().session, TimerSession::Idle),
        "timer loaded idle",
    );
    // The initial load drained any signal; assert a clean baseline.
    assert!(
        !app.timer().notify_pending && !app.timer().notified_for_session,
        "idle baseline: no pending signal, guard un-fired",
    );
    (client, app)
}

/// Drive the timer to a `Running` session via the start toggle (Idle→Running).
fn start_running(app: &mut App, client: &FakeClient) {
    client.push_start_timer(Ok(a_running()));
    submit(app, client, Event::ToggleTimer);
    assert!(
        matches!(app.timer().session, TimerSession::Running { .. }),
        "session running after start",
    );
}

/// Drive a coarse session refresh that returns `next`, folding it into the timer.
fn refresh_with(app: &mut App, client: &FakeClient, next: TimerSession) {
    client.push_timer_session(Ok(next));
    refresh_timer(app, client);
}

// ---- 1. Fires exactly once on the Running→Completed edge (consume-once) ----

#[test]
fn fires_exactly_once_on_running_to_completed_edge() {
    let (client, mut app) = logged_in_idle();
    start_running(&mut app, &client);

    // The coarse refresh re-pulls the session and the server now reports completed: this is the
    // Running→Completed edge.
    refresh_with(&mut app, &client, a_completed());
    assert!(
        matches!(app.timer().session, TimerSession::Completed { .. }),
        "server's completed verdict folded in",
    );

    // The pure signal fired: the first drain returns the fixed copy.
    let first = app.take_pending_notification();
    let note = first.expect("the Running→Completed edge emits a one-shot notification signal");
    assert_eq!(note.title, EXPECT_TITLE, "fixed notification title");
    assert_eq!(note.body, EXPECT_BODY, "fixed notification body");

    // Consume-once: an immediate re-call returns None (the signal was cleared by the first drain).
    assert!(
        app.take_pending_notification().is_none(),
        "the signal is consume-once — it does not re-emit on the next drain",
    );
}

// ---- 2. Does NOT fire on the non-completion transitions ----

#[test]
fn does_not_fire_on_idle_to_running() {
    let (client, mut app) = logged_in_idle();
    start_running(&mut app, &client);
    assert!(
        app.take_pending_notification().is_none(),
        "Idle→Running (start) must not emit a notification",
    );
}

#[test]
fn does_not_fire_on_running_to_running_repull() {
    let (client, mut app) = logged_in_idle();
    start_running(&mut app, &client);
    let _ = app.take_pending_notification();

    // A coarse refresh that still reports running (the session has not ended yet).
    refresh_with(
        &mut app,
        &client,
        running_session(STARTED, ENDS, 30, "2026-06-11T12:06:00Z"),
    );
    assert!(
        matches!(app.timer().session, TimerSession::Running { .. }),
        "still running after the re-pull",
    );
    assert!(
        app.take_pending_notification().is_none(),
        "Running→Running re-pull must not emit",
    );
}

#[test]
fn does_not_re_fire_on_completed_to_completed_repull() {
    let (client, mut app) = logged_in_idle();
    start_running(&mut app, &client);

    // First completion fires once.
    refresh_with(&mut app, &client, a_completed());
    assert!(
        app.take_pending_notification().is_some(),
        "the first completion fires",
    );

    // A subsequent refresh still reports completed (the session sits in the completed state). The
    // guard is already fired, so no second notification — this is the per-render-tick guard.
    refresh_with(&mut app, &client, a_completed());
    assert!(
        matches!(app.timer().session, TimerSession::Completed { .. }),
        "still completed on the re-pull",
    );
    assert!(
        app.take_pending_notification().is_none(),
        "Completed→Completed re-pull must NOT re-fire (the fire-once guard)",
    );
}

#[test]
fn does_not_fire_on_running_to_idle_stop() {
    let (client, mut app) = logged_in_idle();
    start_running(&mut app, &client);
    let _ = app.take_pending_notification();

    // Stopping resets to idle (no pause; ADR-0002 §5). A stop is never a completion.
    client.push_stop_timer(Ok(TimerSession::Idle));
    submit(&mut app, &client, Event::ToggleTimer);
    assert!(
        matches!(app.timer().session, TimerSession::Idle),
        "stop reset to idle",
    );
    assert!(
        app.take_pending_notification().is_none(),
        "Running→Idle (stop) must not emit",
    );
}

#[test]
fn does_not_fire_on_idle_to_idle_repull() {
    let (client, mut app) = logged_in_idle();

    // A coarse refresh that still reports idle.
    refresh_with(&mut app, &client, TimerSession::Idle);
    assert!(
        matches!(app.timer().session, TimerSession::Idle),
        "still idle after the re-pull",
    );
    assert!(
        app.take_pending_notification().is_none(),
        "Idle→Idle re-pull must not emit",
    );
}

// ---- 3. Initial-load (Assumption A4): a first-fold Completed only arms, never emits ----

#[test]
fn initial_load_completed_only_arms_does_not_fire() {
    // A user started a session, closed the TUI, and it completed while closed: the very first
    // GetTimerSession after re-login returns `Completed`. Per A4 we do not replay a stale
    // completion at launch — the first fold only ARMS the guard, it never emits.
    let client = FakeClient::new();
    client.push_login(Ok(session("jwt")));
    client.push_profiles(Ok(vec![profile("p1", "work")]));
    client.push_tasks(Ok(vec![]));
    let mut app = App::new();
    submit(&mut app, &client, Event::Submit);

    client.push_timer_config(Ok(timer_config(30)));
    client.push_timer_session(Ok(a_completed()));
    load_timer(&mut app, &client);

    assert!(
        matches!(app.timer().session, TimerSession::Completed { .. }),
        "initial load landed on the server's completed verdict",
    );
    assert!(
        app.take_pending_notification().is_none(),
        "the initial-load Completed only ARMS the guard — it must not emit (A4)",
    );
    assert!(
        app.timer().notified_for_session,
        "the initial-load Completed armed the guard (so it stays silent on subsequent re-pulls)",
    );
}

#[test]
fn initial_load_completed_then_repull_still_silent() {
    // Following on from A4: after a silent initial-load Completed, a coarse re-pull that still
    // reports completed stays silent (the guard is armed). Only a fresh Running→Completed fires.
    let client = FakeClient::new();
    client.push_login(Ok(session("jwt")));
    client.push_profiles(Ok(vec![profile("p1", "work")]));
    client.push_tasks(Ok(vec![]));
    let mut app = App::new();
    submit(&mut app, &client, Event::Submit);
    client.push_timer_config(Ok(timer_config(30)));
    client.push_timer_session(Ok(a_completed()));
    load_timer(&mut app, &client);
    assert!(app.take_pending_notification().is_none(), "initial silent");

    refresh_with(&mut app, &client, a_completed());
    assert!(
        app.take_pending_notification().is_none(),
        "a re-pull after a silent initial Completed stays silent",
    );
}

// ---- 4. Re-arm: a new Running after a fired Completed, then Completed again ⇒ fires again ----

#[test]
fn re_arms_and_fires_again_on_a_second_completion() {
    let (client, mut app) = logged_in_idle();

    // First session: start → complete → fires once.
    start_running(&mut app, &client);
    refresh_with(&mut app, &client, a_completed());
    assert!(
        app.take_pending_notification().is_some(),
        "first completion fires",
    );

    // A fresh start (Completed→Running via the toggle) re-arms the guard.
    client.push_start_timer(Ok(running_session(
        "2026-06-11T13:00:00Z",
        "2026-06-11T13:30:00Z",
        30,
        "2026-06-11T13:00:00Z",
    )));
    submit(&mut app, &client, Event::ToggleTimer);
    assert!(
        matches!(app.timer().session, TimerSession::Running { .. }),
        "a new session is running",
    );
    assert!(
        !app.timer().notified_for_session,
        "the new Running re-armed the guard",
    );
    assert!(
        app.take_pending_notification().is_none(),
        "the re-arming Running does not itself emit",
    );

    // Second completion of the new session fires a SECOND time.
    refresh_with(
        &mut app,
        &client,
        completed_session(
            "2026-06-11T13:00:00Z",
            "2026-06-11T13:30:00Z",
            30,
            "2026-06-11T13:31:00Z",
        ),
    );
    assert!(
        app.take_pending_notification().is_some(),
        "the second completion fires again after the re-arm",
    );
}

// ---- 5. Logout (Timer::reset) re-arms the guard ----

#[test]
fn logout_resets_and_re_arms_the_guard() {
    // `Timer::reset()` is invoked on logout (`go_to_login`), which an `unauthenticated` timer
    // error triggers. We arm-and-fire a completion first, then drive an unauthenticated refresh:
    // the reset must clear both `notified_for_session` and `notify_pending`.
    let (client, mut app) = logged_in_idle();
    start_running(&mut app, &client);
    refresh_with(&mut app, &client, a_completed());
    assert!(
        app.timer().notified_for_session,
        "guard fired by the completion",
    );

    // An unauthenticated timer refresh returns to login, which calls `Timer::reset()`.
    client.push_timer_session(Err(common::api_err(
        ErrorCode::Unauthenticated,
        "token expired",
    )));
    refresh_timer(&mut app, &client);
    assert!(
        matches!(app.screen(), Screen::Auth(_)),
        "unauthenticated returned to login",
    );
    assert!(
        !app.timer().notified_for_session && !app.timer().notify_pending,
        "logout reset cleared the fire-once guard and any pending signal (re-armed)",
    );
}

#[test]
fn pending_signal_left_undrained_is_cleared_by_logout_reset() {
    // A completion fires (signal set) but the edge has not yet drained it when an unauthenticated
    // error logs the user out: the reset must clear the dangling `notify_pending` so a fresh login
    // never replays a stale signal.
    let (client, mut app) = logged_in_idle();
    start_running(&mut app, &client);
    refresh_with(&mut app, &client, a_completed());
    // Deliberately do NOT drain the signal here.
    assert!(
        app.timer().notify_pending,
        "completion left a pending signal undrained",
    );

    client.push_timer_session(Err(common::api_err(
        ErrorCode::Unauthenticated,
        "token expired",
    )));
    refresh_timer(&mut app, &client);
    assert!(
        app.take_pending_notification().is_none(),
        "logout cleared the dangling signal — no stale notification survives the reset",
    );
}

// ---- 6. Edge-level test through a spy Notifier (the only mock is the sanctioned trait) ----

/// A test-side [`Notifier`] that records each `notify_timer_complete` call (count + last
/// title/body) behind a [`RefCell`]. It is the sanctioned external-service mock — the same seam
/// the binary fills with `DesktopNotifier`; no internal collaborator is mocked.
#[derive(Debug, Default)]
struct SpyNotifier {
    calls: RefCell<Vec<(String, String)>>,
}

impl SpyNotifier {
    fn new() -> Self {
        Self::default()
    }

    fn count(&self) -> usize {
        self.calls.borrow().len()
    }

    fn last(&self) -> Option<(String, String)> {
        self.calls.borrow().last().cloned()
    }
}

impl Notifier for SpyNotifier {
    fn notify_timer_complete(&self, title: &str, body: &str) {
        self.calls
            .borrow_mut()
            .push((title.to_owned(), body.to_owned()));
    }
}

/// The edge's drain-and-fire step: exactly what `terminal::run` does after applying responses —
/// `take_pending_notification()` and, if `Some`, fire the injected notifier. Pumping the pure
/// signal through the spy proves the wire from the core's decision to the effect. Takes the
/// notifier by shared reference (the `Notifier` method is `&self`), so the same spy is reused
/// across pumps.
fn pump(app: &mut App, notifier: &impl Notifier) {
    if let Some(note) = app.take_pending_notification() {
        notifier.notify_timer_complete(note.title, note.body);
    }
}

#[test]
fn edge_fires_the_spy_once_with_the_fixed_copy() {
    let (client, mut app) = logged_in_idle();
    let spy = SpyNotifier::new();

    start_running(&mut app, &client);
    pump(&mut app, &spy);
    assert_eq!(spy.count(), 0, "no notification fired on start");

    // Running→Completed edge: the pump fires the spy exactly once with the fixed copy.
    refresh_with(&mut app, &client, a_completed());
    pump(&mut app, &spy);
    assert_eq!(
        spy.count(),
        1,
        "the completion edge fired the notifier exactly once",
    );
    assert_eq!(
        spy.last(),
        Some((EXPECT_TITLE.to_owned(), EXPECT_BODY.to_owned())),
        "the notifier received the fixed title + body",
    );

    // A subsequent completed re-pull pumps nothing (the guard already fired) — still one call.
    refresh_with(&mut app, &client, a_completed());
    pump(&mut app, &spy);
    assert_eq!(
        spy.count(),
        1,
        "the completed re-pull did not fire a second notification",
    );
}

#[test]
fn edge_fires_the_spy_twice_across_two_sessions() {
    let (client, mut app) = logged_in_idle();
    let spy = SpyNotifier::new();

    // First session completes → one call.
    start_running(&mut app, &client);
    refresh_with(&mut app, &client, a_completed());
    pump(&mut app, &spy);
    assert_eq!(spy.count(), 1, "first completion fired once");

    // A fresh session re-arms, then completes → a second call.
    client.push_start_timer(Ok(running_session(
        "2026-06-11T13:00:00Z",
        "2026-06-11T13:30:00Z",
        30,
        "2026-06-11T13:00:00Z",
    )));
    submit(&mut app, &client, Event::ToggleTimer);
    pump(&mut app, &spy); // the re-arming Running emits nothing
    assert_eq!(spy.count(), 1, "re-arm did not fire");

    refresh_with(
        &mut app,
        &client,
        completed_session(
            "2026-06-11T13:00:00Z",
            "2026-06-11T13:30:00Z",
            30,
            "2026-06-11T13:31:00Z",
        ),
    );
    pump(&mut app, &spy);
    assert_eq!(
        spy.count(),
        2,
        "the second session's completion fired a second notification",
    );
}
