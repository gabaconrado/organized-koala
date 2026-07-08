//! `TestBackend` coverage for the 0023 Tasks-pane date-window + filter-by-day feature (ADR-0015):
//!
//! - **Default window applied:** the bootstrap `ListTasks` query carries the day-aligned window
//!   `created_from = (today − 3) · 86400` / `created_until = (today + 1) · 86400`, so the default
//!   list is windowed to the last 3 days end-to-end. The TUI's contribution is the *query bounds*;
//!   the server enforces which rows come back (covered by the server integration suite) — the fake
//!   `Client` echoes exactly what is scripted, as a windowing server would.
//! - **`F` window editor:** opens a numeric dialog, re-fetches with the new window on submit, and
//!   the older-group separator reads the dynamic `Last {X} days` label; `0` / an empty (non-numeric)
//!   buffer is rejected inline with no re-fetch.
//! - **`f` date-filter editor:** a `DD/MM/YYYY` selector seeded to today; `Tab` cycles the
//!   components; Up/Down adjust the focused component with wrap-in-place, no carry; components are
//!   bounded (day 1–31, month 1–12, year ≥ 1970); submit re-anchors the window on the selected day,
//!   re-fetches, and re-titles the date header.
//! - **Past-date re-anchor + `h`:** with a past day selected, the window bounds and today/older
//!   split re-anchor on it, and the `h` hide-older toggle still collapses the older group within the
//!   fetched window.
//!
//! Every flow runs through the real `map_key` keymap (`press`) and the pure two-step core, driven by
//! the shared synchronous worker-analogue executor; the only mock is the `Client` trait (the HTTP
//! server), exactly as ADR-0003 layer 2 / ADR-0006 prescribe.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

mod common;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use common::{
    Call, FakeClient, iso_at_day, open_task_on_day, profile, render, session, submit, tasks_pane,
    today_open_task,
};
use tui::app::task_list::{DEFAULT_HIDE_WINDOW_DAYS, MIN_FILTER_YEAR};
use tui::app::{App, DateComponent, Event, Screen, current_day_number};
use tui::terminal::map_key;
use tui::ui::{civil_from_days, days_from_civil, today_header};

const W: u16 = 80;
const H: u16 = 24;

/// Seconds in a UTC civil day — the multiplier the day-aligned window bounds are built from
/// (`(anchor ± n) · 86400`), mirroring `tui::app::task_list::SECS_PER_DAY`.
const SECS_PER_DAY: i64 = 86_400;

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

/// A freshly-logged-in app on the `work` Tasks tab, plus the shared fake (login → profiles → tasks
/// chain scripted; active profile `p1`/`work`). Mirrors the `dialogs`/`tasks` suite helper.
fn logged_in(tasks: Vec<contract::Task>) -> (FakeClient, App) {
    let client = FakeClient::new();
    client.push_login(Ok(session("jwt")));
    client.push_profiles(Ok(vec![profile("p1", "work")]));
    client.push_tasks(Ok(tasks));
    let mut app = App::new();
    submit(&mut app, &client, Event::Submit);
    assert!(
        matches!(app.screen(), Screen::Main(_)),
        "reached the tabbed view"
    );
    (client, app)
}

/// Feed a key through the *real* keymap and then the update path, driving any dispatch to
/// completion — the end-to-end path proving suppression (`map_key`) and folding agree.
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

/// The `(created_from, created_until)` bounds of the most recent `ListTasks` call the app made.
fn last_window(client: &FakeClient) -> (Option<i64>, Option<i64>) {
    client
        .calls()
        .into_iter()
        .rev()
        .find_map(|c| match c {
            Call::ListTasks {
                created_from,
                created_until,
                ..
            } => Some((created_from, created_until)),
            _ => None,
        })
        .expect("a ListTasks call was recorded")
}

/// The number of `ListTasks` calls recorded so far (to prove a rejected edit issues no re-fetch).
fn list_calls(client: &FakeClient) -> usize {
    client
        .calls()
        .iter()
        .filter(|c| matches!(c, Call::ListTasks { .. }))
        .count()
}

// ---- 1. Default window applied to the bootstrap list load ----

#[test]
fn bootstrap_list_query_carries_the_default_last_three_day_window() {
    let today = current_day_number();
    let (client, _app) = logged_in(vec![]);

    // The default window is `[today − DEFAULT_HIDE_WINDOW_DAYS, today]`, expressed as day-aligned
    // epoch-second bounds: inclusive lower `(today − 3) · 86400`, exclusive upper `(today + 1) · 86400`.
    let expected_from = (today - i64::from(DEFAULT_HIDE_WINDOW_DAYS)) * SECS_PER_DAY;
    let expected_until = (today + 1) * SECS_PER_DAY;
    assert_eq!(
        last_window(&client),
        (Some(expected_from), Some(expected_until)),
        "the bootstrap ListTasks carries the default 3-day window",
    );

    // A task created ≥ 4 days ago falls strictly below the window's inclusive lower bound, so the
    // server (which the fake stands in for) would not return it — the "not shown" half of
    // acceptance #1. The TUI's role is to send the correct lower bound; this pins that it excludes
    // a 4-day-old task.
    let four_days_old = open_task_on_day("x", "stale", today - 4, "12:00:00");
    assert!(
        four_days_old.created_at.timestamp() < expected_from,
        "a task created 4 days ago is below the default window's lower bound",
    );
}

// ---- 2. `F` window editor: re-fetch + dynamic label; reject 0 / empty with no re-fetch ----

#[test]
fn window_editor_submit_refetches_with_new_window_and_dynamic_label() {
    let today = current_day_number();
    let today_task = today_open_task("t1", "today task", "12:00:00");
    // An older-but-in-window task so the older-group separator renders (window default is 3 days).
    let older_task = open_task_on_day("o1", "older task", today - 2, "10:00:00");
    let (client, mut app) = logged_in(vec![today_task.clone(), older_task.clone()]);

    // `F` opens the numeric window editor (seeded with the current value).
    press(&mut app, &client, KeyCode::Char('F'));
    let editor = tasks_pane(&app)
        .editing_window
        .as_ref()
        .expect("F opened the window editor");
    assert_eq!(editor.buffer, DEFAULT_HIDE_WINDOW_DAYS.to_string());

    // Script the re-fetch the submit will trigger (a windowing server returns the same in-window
    // set; the fake echoes it).
    client.push_tasks(Ok(vec![today_task, older_task]));

    // Replace the seed `3` with `5` and submit.
    press(&mut app, &client, KeyCode::Backspace);
    press(&mut app, &client, KeyCode::Char('5'));
    press(&mut app, &client, KeyCode::Enter);

    assert!(
        tasks_pane(&app).editing_window.is_none(),
        "a valid submit closes the editor",
    );
    assert_eq!(tasks_pane(&app).hide_window_days, 5, "window size updated");

    // The re-fetch carries the widened window `[today − 5, today]`.
    assert_eq!(
        last_window(&client),
        (
            Some((today - 5) * SECS_PER_DAY),
            Some((today + 1) * SECS_PER_DAY)
        ),
        "the re-fetch query anchors on today with the new 5-day window",
    );

    // The older-group separator label now renders the numeric window size.
    let text = render(&app, W, H);
    assert!(
        text.contains("Last 5 days"),
        "the older separator reads the dynamic `Last 5 days` label:\n{text}",
    );
}

#[test]
fn window_editor_rejects_zero_inline_without_refetching() {
    let (client, mut app) = logged_in(vec![today_open_task("t1", "today", "12:00:00")]);
    let before = list_calls(&client);

    press(&mut app, &client, KeyCode::Char('F'));
    // Clear the seed `3` and enter `0`.
    press(&mut app, &client, KeyCode::Backspace);
    press(&mut app, &client, KeyCode::Char('0'));
    press(&mut app, &client, KeyCode::Enter);

    let editor = tasks_pane(&app)
        .editing_window
        .as_ref()
        .expect("a rejected `0` leaves the editor open");
    assert!(
        editor.error.is_some(),
        "`0` surfaces an inline error (min window is 1 day)",
    );
    assert_eq!(
        tasks_pane(&app).hide_window_days,
        DEFAULT_HIDE_WINDOW_DAYS,
        "the window size is unchanged by a rejected edit",
    );
    assert_eq!(
        list_calls(&client),
        before,
        "a rejected `0` issues no re-fetch",
    );
}

#[test]
fn window_editor_rejects_empty_buffer_inline_without_refetching() {
    let (client, mut app) = logged_in(vec![today_open_task("t1", "today", "12:00:00")]);
    let before = list_calls(&client);

    press(&mut app, &client, KeyCode::Char('F'));
    // Clear the seed `3` to an empty (non-numeric) buffer, then submit.
    press(&mut app, &client, KeyCode::Backspace);
    press(&mut app, &client, KeyCode::Enter);

    let editor = tasks_pane(&app)
        .editing_window
        .as_ref()
        .expect("an empty buffer leaves the editor open");
    assert!(
        editor.error.is_some(),
        "an empty (non-numeric) buffer surfaces an inline error",
    );
    assert_eq!(
        list_calls(&client),
        before,
        "a rejected empty buffer issues no re-fetch",
    );
}

// ---- 3. `f` date-filter editor: seed, Tab cycle, Up/Down wrap-in-place, bounds ----

#[test]
fn date_filter_opens_seeded_to_today() {
    let today = current_day_number();
    let (year, month, day) = civil_from_days(today);
    let (client, mut app) = logged_in(vec![]);

    press(&mut app, &client, KeyCode::Char('f'));
    let filter = tasks_pane(&app)
        .filtering_date
        .as_ref()
        .expect("f opened the date-filter editor");
    assert_eq!(
        (filter.day, filter.month, filter.year),
        (day, month, year),
        "the editor seeds to today's date",
    );
    assert_eq!(
        filter.focus,
        DateComponent::Day,
        "focus starts on the day component",
    );
}

#[test]
fn date_filter_tab_cycles_day_month_year() {
    let (client, mut app) = logged_in(vec![]);
    press(&mut app, &client, KeyCode::Char('f'));

    let focus = |app: &App| tasks_pane(app).filtering_date.as_ref().unwrap().focus;
    assert_eq!(focus(&app), DateComponent::Day);
    press(&mut app, &client, KeyCode::Tab);
    assert_eq!(focus(&app), DateComponent::Month, "Tab: day → month");
    press(&mut app, &client, KeyCode::Tab);
    assert_eq!(focus(&app), DateComponent::Year, "Tab: month → year");
    press(&mut app, &client, KeyCode::Tab);
    assert_eq!(focus(&app), DateComponent::Day, "Tab wraps year → day");
    // Shift+Tab cycles the other way.
    press(&mut app, &client, KeyCode::BackTab);
    assert_eq!(focus(&app), DateComponent::Year, "Shift+Tab: day → year");
}

#[test]
fn date_filter_up_down_wrap_in_place_with_no_carry() {
    let today = current_day_number();
    let (seed_year, seed_month, _seed_day) = civil_from_days(today);
    let (client, mut app) = logged_in(vec![]);
    press(&mut app, &client, KeyCode::Char('f'));

    let read = |app: &App| {
        let f = tasks_pane(app).filtering_date.as_ref().unwrap();
        (f.day, f.month, f.year)
    };

    // --- Month wraps 1 → 12 on Down, with NO carry into the year. Focus the month component. ---
    press(&mut app, &client, KeyCode::Tab); // day → month
    // Walk the month down to 1 (seed_month − 1 presses), then one more press wraps to 12.
    for _ in 0..(seed_month - 1) {
        press(&mut app, &client, KeyCode::Down);
    }
    assert_eq!(read(&app).1, 1, "month walked down to 1");
    assert_eq!(
        read(&app).2,
        seed_year,
        "walking the month never changed the year"
    );
    press(&mut app, &client, KeyCode::Down);
    assert_eq!(read(&app).1, 12, "month 1 − 1 wraps in place to 12");
    assert_eq!(
        read(&app).2,
        seed_year,
        "the 12 → wrap carries NOTHING into the year"
    );

    // --- Day wraps 1 → 31 on Down and 31 → 1 on Up (wrap-in-place). Focus the day component. ---
    // Cycle focus back to day (month → year → day).
    press(&mut app, &client, KeyCode::Tab); // month → year
    press(&mut app, &client, KeyCode::Tab); // year → day
    let seed_day = read(&app).0;
    for _ in 0..(seed_day - 1) {
        press(&mut app, &client, KeyCode::Down);
    }
    assert_eq!(read(&app).0, 1, "day walked down to 1");
    press(&mut app, &client, KeyCode::Down);
    assert_eq!(read(&app).0, 31, "day 1 − 1 wraps in place to 31");
    press(&mut app, &client, KeyCode::Up);
    assert_eq!(read(&app).0, 1, "day 31 + 1 wraps in place to 1");
}

#[test]
fn date_filter_year_is_bounded_below_at_1970() {
    let today = current_day_number();
    let (seed_year, _m, _d) = civil_from_days(today);
    let (client, mut app) = logged_in(vec![]);
    press(&mut app, &client, KeyCode::Char('f'));

    // Focus the year, then walk it down past 1970 (a few extra presses beyond the bound).
    press(&mut app, &client, KeyCode::Tab); // day → month
    press(&mut app, &client, KeyCode::Tab); // month → year
    let steps = (seed_year - MIN_FILTER_YEAR) + 5;
    for _ in 0..steps {
        press(&mut app, &client, KeyCode::Down);
    }
    let year = tasks_pane(&app).filtering_date.as_ref().unwrap().year;
    assert_eq!(
        year, MIN_FILTER_YEAR,
        "the year clamps at its lower bound (≥ 1970) and never goes below",
    );
}

// ---- 4. Submit a past date D: re-anchor window + header, then `h` hides older within the window ----

#[test]
fn past_date_filter_reanchors_window_header_and_h_toggle_hides_older() {
    let today = current_day_number();
    let (year, month, day) = civil_from_days(today);
    // Selecting one year ago (same month/day) yields a clean past anchor day-number.
    let anchor = days_from_civil(year - 1, month, day);

    // The re-fetch after the filter submit returns tasks the (windowing) server would send: one on
    // the anchor day, one the day before (older-but-in-window). The fake echoes the scripted set.
    let on_anchor = open_task_on_day("a1", "on the anchor day", anchor, "12:00:00");
    let before_anchor = open_task_on_day("b1", "the day before", anchor - 1, "10:00:00");

    let (client, mut app) = logged_in(vec![]);
    press(&mut app, &client, KeyCode::Char('f'));
    // Focus the year and step back one year.
    press(&mut app, &client, KeyCode::Tab); // day → month
    press(&mut app, &client, KeyCode::Tab); // month → year
    press(&mut app, &client, KeyCode::Down); // year − 1

    client.push_tasks(Ok(vec![on_anchor, before_anchor]));
    press(&mut app, &client, KeyCode::Enter); // submit the filter

    assert!(
        tasks_pane(&app).filtering_date.is_none(),
        "submitting closes the date editor",
    );
    assert_eq!(
        tasks_pane(&app).filter_date,
        Some(anchor),
        "the filter day-number is the selected past day",
    );

    // The re-fetch re-anchors the window on D: `[D − 3, D]`. The exclusive upper `(D + 1) · 86400`
    // excludes any task created after D server-side (the "tasks after D hidden" half of acceptance #4).
    assert_eq!(
        last_window(&client),
        (
            Some((anchor - i64::from(DEFAULT_HIDE_WINDOW_DAYS)) * SECS_PER_DAY),
            Some((anchor + 1) * SECS_PER_DAY)
        ),
        "the window re-anchors on the selected past day",
    );

    // The date header is re-titled to the selected day, and both in-window tasks render, split by
    // the `Last 3 days` older separator.
    let text = render(&app, W, H);
    let header = today_header(anchor);
    assert!(
        text.contains(&header),
        "the date header re-titles to the selected day ({header}):\n{text}",
    );
    assert!(
        text.contains("on the anchor day"),
        "the anchor-day task renders:\n{text}",
    );
    assert!(
        text.contains("the day before") && text.contains("Last 3 days"),
        "the older-but-in-window task renders under the `Last 3 days` separator:\n{text}",
    );

    // `h` still collapses/hides the older group within the fetched window.
    press(&mut app, &client, KeyCode::Char('h'));
    assert!(tasks_pane(&app).hide_older, "h toggled hide-older on");
    let hidden = render(&app, W, H);
    assert!(
        !hidden.contains("the day before"),
        "the older task is hidden by the `h` toggle:\n{hidden}",
    );
    assert!(
        !hidden.contains("Last 3 days"),
        "the older separator is hidden with the group:\n{hidden}",
    );
    assert!(
        hidden.contains("on the anchor day"),
        "the anchor-day task stays visible after hiding older:\n{hidden}",
    );
}

// A tiny cross-check that `iso_at_day` (harness builder) and the civil-day helpers agree, so a
// fixture placed on day N really carries a `created_at` on that civil day.
#[test]
fn iso_at_day_builds_a_created_at_on_the_requested_civil_day() {
    let day = current_day_number() - 2;
    let task = open_task_on_day("d", "two days ago", day, "08:30:00");
    assert_eq!(
        task.created_at.timestamp().div_euclid(SECS_PER_DAY),
        day,
        "the built task's created_at lands on the requested civil day",
    );
    // And the ISO string is on that day (sanity on the builder used across the suite).
    assert!(iso_at_day(day, "08:30:00").ends_with("T08:30:00Z"));
}
