//! In-flight behaviour (0005 acceptance: at most one request in flight; cancel; stale-response
//! drop), driven through the public two-step `App` API:
//!
//! - a request-triggering event while a request is outstanding is a **no-op** (no new
//!   `Dispatch`, state unchanged);
//! - `Cancel` while pending clears the in-flight marker, leaving the screen interactive again;
//! - a `ClientResponse` whose `RequestId` no longer matches the awaited request (because the
//!   request was cancelled or superseded) is **dropped** by `apply_response` — state unchanged.
//!
//! These exercise the in-flight seam without a worker thread: `handle_event` returns the
//! `Dispatch`, the synchronous executor turns it into the `ClientResponse` the real edge would
//! produce, and the test controls exactly when (or whether) that response is applied.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

mod common;

use common::{FakeClient, drive, execute, open_task, profile, session, submit};
use tui::app::{App, Event, Screen};

/// A freshly-logged-in app on the `work` task list with the given tasks, plus the shared fake.
fn logged_in(tasks: Vec<contract::Task>) -> (FakeClient, App) {
    let client = FakeClient::new();
    client.push_login(Ok(session("jwt")));
    client.push_profiles(Ok(vec![profile("p1", "work")]));
    client.push_tasks(Ok(tasks));
    let mut app = App::new();
    submit(&mut app, &client, Event::Submit);
    assert!(matches!(app.screen(), Screen::TaskList(_)));
    (client, app)
}

// ---- at most one request in flight: request-triggering events are no-ops while pending ----

#[test]
fn refresh_while_pending_is_a_no_op() {
    let (client, mut app) = logged_in(vec![open_task("t1", "task", "2026-06-18T10:00:00Z")]);

    // First close puts the list in-flight (we hold its dispatch, never driving it).
    let first = app
        .handle_event(Event::CloseSelected)
        .expect("close dispatches");
    assert!(app.is_pending(), "in-flight after the first request");
    let calls_after_first = client.calls().len();

    // A second request-triggering event while pending must produce no Dispatch and no new call.
    assert!(
        app.handle_event(Event::Refresh).is_none(),
        "refresh while pending dispatches nothing",
    );
    assert!(
        app.handle_event(Event::CloseSelected).is_none(),
        "a second close while pending dispatches nothing",
    );
    assert!(app.is_pending(), "still the same single request in flight");
    assert_eq!(
        client.calls().len(),
        calls_after_first,
        "no extra server call while a request is already outstanding",
    );

    // The single in-flight request still completes normally when its response arrives.
    client.push_close(Ok(open_task("t1", "task", "2026-06-18T10:00:00Z")));
    drive(&mut app, &client, first);
    assert!(!app.is_pending(), "settled after the one request completes");
}

#[test]
fn auth_submit_while_pending_is_a_no_op() {
    let client = FakeClient::new();
    let mut app = App::new();

    let _first = app.handle_event(Event::Submit).expect("login dispatches");
    assert!(app.is_pending());

    // Typing and re-submitting while the login is outstanding changes nothing and dispatches
    // nothing — the form is frozen behind the single in-flight request.
    assert!(app.handle_event(Event::Char('x')).is_none());
    assert!(app.handle_event(Event::Submit).is_none());
    assert!(app.is_pending(), "still exactly one request in flight");
    assert!(
        client.calls().is_empty(),
        "no call crosses the wire from the executor's view until driven: {:?}",
        client.calls(),
    );
}

// ---- Cancel clears the in-flight marker ----

#[test]
fn cancel_while_pending_clears_in_flight_and_restores_interactivity() {
    let (_client, mut app) = logged_in(vec![open_task("t1", "task", "2026-06-18T10:00:00Z")]);

    let _dispatch = app
        .handle_event(Event::CloseSelected)
        .expect("close dispatches");
    assert!(app.is_pending(), "in-flight before cancel");

    // Esc maps to Cancel while pending; it clears the marker and returns no new dispatch.
    assert!(
        app.handle_event(Event::Cancel).is_none(),
        "cancel itself dispatches nothing",
    );
    assert!(!app.is_pending(), "cancel cleared the in-flight marker");

    // The screen is interactive again: a fresh request-triggering event now dispatches.
    let next = app.handle_event(Event::Refresh);
    assert!(
        next.is_some(),
        "after cancel the screen accepts a new request",
    );
    assert!(app.is_pending(), "the new request is now in flight");
}

// ---- stale-response drop by RequestId mismatch ----

#[test]
fn stale_response_after_cancel_is_dropped() {
    let (client, mut app) = logged_in(vec![open_task("t1", "Original", "2026-06-18T10:00:00Z")]);

    // Begin a close; capture the dispatch the worker would run.
    let dispatch = app
        .handle_event(Event::CloseSelected)
        .expect("close dispatches");

    // User cancels before the response arrives: the in-flight marker is cleared.
    assert!(app.handle_event(Event::Cancel).is_none());
    assert!(!app.is_pending(), "cancelled");

    // The abandoned request still ran on the (mocked) server and produces a response with the
    // now-stale RequestId. Applying it must be a no-op: the marker is gone, so the id mismatches.
    client.push_close(Ok(common::done_task(
        "t1",
        "Original",
        "2026-06-18T10:00:00Z",
        "2026-06-18T14:00:00Z",
    )));
    let stale = execute(&client, dispatch);
    let follow_up = app.apply_response(stale);

    assert!(follow_up.is_none(), "a stale response yields no follow-up");
    let Screen::TaskList(list) = app.screen() else {
        panic!("still on the task list");
    };
    assert_eq!(
        list.tasks.first().expect("task present").status,
        contract::TaskStatus::Open,
        "the dropped stale close must not flip the task to done",
    );
    assert!(
        !app.is_pending(),
        "still idle after dropping the stale response"
    );
}

#[test]
fn superseded_response_after_new_request_is_dropped() {
    // Cancel, then start a *new* request (so a fresh RequestId is in flight). The first request's
    // late response carries the old id and must be dropped rather than mis-applied to the new
    // in-flight slot.
    let (client, mut app) = logged_in(vec![open_task("t1", "task", "2026-06-18T10:00:00Z")]);

    let first = app
        .handle_event(Event::CloseSelected)
        .expect("first close dispatches");
    assert!(app.handle_event(Event::Cancel).is_none());

    // New request after cancel — gets a new RequestId, now the awaited one.
    let second = app
        .handle_event(Event::Refresh)
        .expect("refresh dispatches");
    assert!(app.is_pending());

    // The first (cancelled) request's response arrives late: dropped, the new request still awaited.
    client.push_close(Ok(common::done_task(
        "t1",
        "task",
        "2026-06-18T10:00:00Z",
        "2026-06-18T14:00:00Z",
    )));
    let stale = execute(&client, first);
    assert!(
        app.apply_response(stale).is_none(),
        "the superseded response is dropped",
    );
    assert!(
        app.is_pending(),
        "the new request is still in flight after dropping the stale one",
    );

    // The new (refresh) request then completes normally.
    client.push_tasks(Ok(vec![open_task("t2", "fresh", "2026-06-18T15:00:00Z")]));
    drive(&mut app, &client, second);
    let Screen::TaskList(list) = app.screen() else {
        panic!("task list");
    };
    assert_eq!(
        list.tasks.first().expect("task").title,
        "fresh",
        "the new request's response drove the view, not the stale one",
    );
    assert!(!app.is_pending());
}
