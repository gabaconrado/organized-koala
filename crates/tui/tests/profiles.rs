//! The profile-switcher `TestBackend`/core suite (item 0012, slice 4t) — the ADR-0003 layer-2
//! home for interactive switcher behaviour, driven through the public two-step `App` API
//! (`handle_event` → synchronous executor → `apply_response`) against the held fake client:
//!
//! - list render: the switcher mirrors exactly the server's profile list, with the active profile
//!   selected;
//! - pick-active (`Enter`): rebinds the in-memory active profile so the NEXT `ListTasks` carries
//!   the new id — and issues NO server "switch" call (there is no such endpoint, #1 / ADR-0009 §5);
//! - create (`a`): a `CreateProfile` request is issued and the list reflects it after the chained
//!   refresh;
//! - rename (`e`): an `UpdateProfile` request is issued and the change reflects after the refresh;
//! - delete (`x`): a `DeleteProfile` request is issued and the profile is removed from the list;
//! - duplicate-name → inline `ProfileNameTaken` message; last-profile delete → inline `LastProfile`
//!   message;
//! - in-flight spinner / pending state while a request is outstanding;
//! - cancel / stale-RequestId drop: a late outcome for a superseded request is ignored;
//! - deleting the ACTIVE profile re-points the active id to the first remaining (the subsequent
//!   scoped read uses the new id).
//!
//! These exercise the switcher with no live server and no worker thread — the only mock is the
//! sanctioned `Client` trait (the HTTP server), exactly as ADR-0003 / ADR-0006 prescribe.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

mod common;

use common::{
    Call, FakeClient, api_err, drive, execute, offline_err, on_tab, profile, profiles_pane, render,
    session, submit,
};
use contract::ErrorCode;
use tui::app::{App, Event, ProfilesMode, Screen, Tab};

const W: u16 = 80;
const H: u16 = 24;

/// Type a string into the focused field (local edits never dispatch).
fn type_str(app: &mut App, s: &str) {
    for c in s.chars() {
        let _ = app.handle_event(Event::Char(c));
    }
}

/// A freshly-logged-in app on the `work` Tasks tab, plus the shared fake. The login chain
/// (login → profiles → tasks) is scripted; the active profile is `p1`/`work`.
fn logged_in() -> (FakeClient, App) {
    let client = FakeClient::new();
    client.push_login(Ok(session("jwt")));
    client.push_profiles(Ok(vec![profile("p1", "work")]));
    client.push_tasks(Ok(vec![]));
    let mut app = App::new();
    submit(&mut app, &client, Event::Submit);
    assert!(matches!(app.screen(), Screen::Main(_)));
    (client, app)
}

/// Log in and switch to the Profiles tab (`Shift+Tab` cycles Tasks→Profiles directly), which issues
/// a `ListProfiles` load populated from the scripted `profiles` list response — the new tab-based
/// reachability for the switcher (ADR-0010 §1).
fn enter_switcher(profiles: Vec<contract::Profile>) -> (FakeClient, App) {
    let (client, mut app) = logged_in();
    client.push_profiles(Ok(profiles));
    submit(&mut app, &client, Event::PrevTab); // Tasks -> Profiles (reverse cycle)
    assert!(
        on_tab(&app, Tab::Profiles),
        "switched to the Profiles tab: {}",
        common::screen_name(&app),
    );
    (client, app)
}

/// The switcher (Profiles pane) state, panicking if the app is not on the tabbed view.
fn switcher(app: &App) -> &tui::app::ProfilesState {
    profiles_pane(app)
}

/// Leave the Profiles tab back to the Tasks tab (`Tab` cycles Profiles→Tasks), the new equivalent
/// of the old idle-`Esc`-back. The switch issues a fresh `ListTasks` for the active profile.
fn back_to_tasks(app: &mut App, client: &FakeClient) {
    submit(app, client, Event::NextTab);
}

// ---- list render ----

#[test]
fn switcher_lists_exactly_the_accounts_profiles() {
    // The rendered switcher equals what the server returned — order and count — with no fabricated
    // or cached entries (hard-constraint #1).
    let server_profiles = vec![profile("p1", "work"), profile("p2", "personal")];
    let (_client, app) = enter_switcher(server_profiles.clone());

    let state = switcher(&app);
    assert_eq!(state.profiles, server_profiles, "view is the server's list");
    // The active profile (`p1`/`work`, from the login bootstrap) is selected.
    assert_eq!(state.selected, Some(0));

    let text = render(&app, W, H);
    assert!(text.contains("work"), "active profile listed:\n{text}");
    assert!(text.contains("personal"), "other profile listed:\n{text}");
}

#[test]
fn opening_the_switcher_lists_profiles_for_the_account() {
    // Navigating into the switcher issues exactly a `ListProfiles` for the account's token; the
    // listing is account-wide (not profile-scoped — profiles are the namespaces themselves).
    let (client, _app) = enter_switcher(vec![profile("p1", "work")]);
    let calls = client.calls();
    assert!(
        matches!(calls.last(), Some(Call::ListProfiles { token }) if token == "jwt"),
        "switcher listed for the account: {calls:?}",
    );
}

// ---- pick-active (client-side switch, NO server call) ----

#[test]
fn picking_a_profile_rescopes_the_active_id_with_no_server_switch_call() {
    // The headline switch behaviour (#1 / ADR-0009 §5): selecting a different profile and pressing
    // Enter rebinds the in-memory active id so the NEXT `ListTasks` carries the new id — and issues
    // NO server "switch" call (there is no such endpoint; the only calls are list/create/rename/
    // delete on profiles plus the scoped task read).
    let (client, mut app) = enter_switcher(vec![profile("p1", "work"), profile("p2", "personal")]);

    // Select the second profile (`p2`) and pick it. Pick-active navigates to its task list, which
    // issues a `ListTasks` for the newly-active profile.
    let _ = app.handle_event(Event::Next); // selection -> p2
    client.push_tasks(Ok(vec![]));
    submit(&mut app, &client, Event::Submit);

    assert!(
        on_tab(&app, Tab::Tasks),
        "pick-active lands on the Tasks tab of the chosen profile",
    );

    // The re-scope's read ends with the two-call tree load (ListTasks → ListSubtasks, 0019), both
    // scoped to the NEWLY-active profile id `p2`.
    let calls = client.calls();
    assert!(
        matches!(calls.last(), Some(Call::ListSubtasks { token, profile_id })
            if token == "jwt" && profile_id == "p2"),
        "the tree load's second call carries the picked profile id p2: {calls:?}",
    );
    assert!(
        calls
            .iter()
            .any(|c| matches!(c, Call::ListTasks { token, profile_id }
            if token == "jwt" && profile_id == "p2")),
        "next scoped task read carries the picked profile id p2: {calls:?}",
    );

    // There is NO server switch call — assert the recorded calls only ever touch the sanctioned
    // surface (no fabricated "switch"/"select" variant exists on the client at all).
    assert!(
        calls.iter().all(|c| matches!(
            c,
            Call::Login { .. }
                | Call::ListProfiles { .. }
                | Call::ListTasks { .. }
                | Call::ListSubtasks { .. }
                | Call::CreateProfile { .. }
                | Call::UpdateProfile { .. }
                | Call::DeleteProfile { .. }
        )),
        "no server switch call issued — switch is client-side only: {calls:?}",
    );
}

#[test]
fn picking_the_already_active_profile_keeps_the_same_scope() {
    // Picking the currently-active profile (`p1`) re-scopes to the same id — still no switch call.
    let (client, mut app) = enter_switcher(vec![profile("p1", "work"), profile("p2", "personal")]);
    client.push_tasks(Ok(vec![]));
    submit(&mut app, &client, Event::Submit); // Enter on the selected (active) p1

    let calls = client.calls();
    assert!(
        matches!(calls.last(), Some(Call::ListSubtasks { token, profile_id })
            if token == "jwt" && profile_id == "p1"),
        "scoped read (tree load) stays on p1: {calls:?}",
    );
    assert!(
        calls
            .iter()
            .any(|c| matches!(c, Call::ListTasks { token, profile_id }
            if token == "jwt" && profile_id == "p1")),
        "scoped task read stays on p1: {calls:?}",
    );
}

// ---- create ----

#[test]
fn create_issues_request_then_reflects_in_list_after_refresh() {
    let (client, mut app) = enter_switcher(vec![profile("p1", "work")]);

    // Script the create response and the post-create refresh list.
    let created = profile("p2", "personal");
    client.push_create_profile(Ok(created.clone()));
    client.push_profiles(Ok(vec![profile("p1", "work"), created]));

    // Open create, type the name, submit.
    let _ = app.handle_event(Event::BeginAddProfile);
    type_str(&mut app, "personal");
    submit(&mut app, &client, Event::Submit);

    // A CreateProfile call carried the trimmed name and the account token.
    let calls = client.calls();
    assert!(
        calls
            .iter()
            .any(|c| matches!(c, Call::CreateProfile { token, name }
            if token == "jwt" && name == "personal")),
        "create issued with the typed name: {calls:?}",
    );

    // The create sub-flow closed and the list now shows the server's two profiles.
    let state = switcher(&app);
    assert!(
        matches!(state.mode, ProfilesMode::List),
        "create sub-flow closed after success",
    );
    assert_eq!(state.profiles.len(), 2);
    assert!(state.profiles.iter().any(|p| p.name == "personal"));
}

// ---- rename ----

#[test]
fn rename_issues_update_and_reflects_change() {
    let (client, mut app) = enter_switcher(vec![profile("p1", "work"), profile("p2", "personal")]);

    // Rename the selected profile (`p1`/work) → "office".
    client.push_update_profile(Ok(profile("p1", "office")));
    client.push_profiles(Ok(vec![profile("p1", "office"), profile("p2", "personal")]));

    let _ = app.handle_event(Event::BeginRenameProfile);
    // The rename form is prefilled with the current name; clear it then type the new one.
    for _ in 0.."work".len() {
        let _ = app.handle_event(Event::Backspace);
    }
    type_str(&mut app, "office");
    submit(&mut app, &client, Event::Submit);

    let calls = client.calls();
    assert!(
        calls.iter().any(
            |c| matches!(c, Call::UpdateProfile { token, profile_id, name }
            if token == "jwt" && profile_id == "p1" && name == "office")
        ),
        "rename issued for p1 with the new name: {calls:?}",
    );

    let state = switcher(&app);
    assert!(matches!(state.mode, ProfilesMode::List), "rename closed");
    assert!(
        state
            .profiles
            .iter()
            .any(|p| p.id == "p1" && p.name == "office"),
        "renamed profile reflected in the list",
    );
}

// ---- delete ----

#[test]
fn delete_issues_request_and_removes_from_list() {
    let (client, mut app) = enter_switcher(vec![profile("p1", "work"), profile("p2", "personal")]);

    // Delete the second profile (`p2`). Select it first, then confirm the delete.
    let _ = app.handle_event(Event::Next); // selection -> p2
    client.push_delete_profile(Ok(()));
    client.push_profiles(Ok(vec![profile("p1", "work")]));

    let _ = app.handle_event(Event::BeginDeleteProfile);
    submit(&mut app, &client, Event::Submit); // confirm

    let calls = client.calls();
    assert!(
        calls
            .iter()
            .any(|c| matches!(c, Call::DeleteProfile { token, profile_id }
            if token == "jwt" && profile_id == "p2")),
        "delete issued for p2: {calls:?}",
    );

    let state = switcher(&app);
    assert!(matches!(state.mode, ProfilesMode::List), "confirm closed");
    assert_eq!(state.profiles.len(), 1);
    assert!(
        state.profiles.iter().all(|p| p.id != "p2"),
        "deleted profile gone from the list",
    );
}

// ---- inline error surfacing ----

#[test]
fn duplicate_name_on_create_surfaces_inline_profile_name_taken() {
    let (client, mut app) = enter_switcher(vec![profile("p1", "work")]);

    // The server rejects the create with the 409 profile_name_taken code.
    client.push_create_profile(Err(api_err(ErrorCode::ProfileNameTaken, "name in use")));

    let _ = app.handle_event(Event::BeginAddProfile);
    type_str(&mut app, "work");
    submit(&mut app, &client, Event::Submit);

    // The create sub-flow stays open with an inline error, the list is untouched, and no refresh
    // call followed (the create failed).
    let state = switcher(&app);
    match &state.mode {
        ProfilesMode::Creating(form) => {
            assert!(
                form.error.as_deref().is_some_and(|m| m.contains("already")),
                "ProfileNameTaken surfaced inline in the create form: {:?}",
                form.error,
            );
        }
        other => panic!("create sub-flow should stay open on a name clash, got {other:?}"),
    }
    assert_eq!(
        state.profiles.len(),
        1,
        "list untouched after the failed create"
    );
    assert!(!app.is_pending(), "settled — no request in flight");

    // The inline message is visible in the rendered buffer.
    let text = render(&app, W, H);
    assert!(
        text.contains("already"),
        "duplicate-name message shown:\n{text}"
    );
}

#[test]
fn duplicate_name_on_rename_surfaces_inline_profile_name_taken() {
    let (client, mut app) = enter_switcher(vec![profile("p1", "work"), profile("p2", "personal")]);

    // Rename p2 → "work" (already taken by p1): server rejects with profile_name_taken.
    let _ = app.handle_event(Event::Next); // selection -> p2
    client.push_update_profile(Err(api_err(ErrorCode::ProfileNameTaken, "name in use")));

    let _ = app.handle_event(Event::BeginRenameProfile);
    for _ in 0.."personal".len() {
        let _ = app.handle_event(Event::Backspace);
    }
    type_str(&mut app, "work");
    submit(&mut app, &client, Event::Submit);

    let state = switcher(&app);
    match &state.mode {
        ProfilesMode::Renaming { form, .. } => {
            assert!(
                form.error.as_deref().is_some_and(|m| m.contains("already")),
                "ProfileNameTaken surfaced inline in the rename form: {:?}",
                form.error,
            );
        }
        other => panic!("rename sub-flow should stay open on a name clash, got {other:?}"),
    }
    assert!(!app.is_pending(), "settled");
}

#[test]
fn last_profile_delete_surfaces_inline_last_profile() {
    // The account's only profile cannot be deleted: the server returns 409 last_profile, surfaced
    // as a switcher message; the profile stays in the list.
    let (client, mut app) = enter_switcher(vec![profile("p1", "work")]);

    client.push_delete_profile(Err(api_err(ErrorCode::LastProfile, "must keep one")));

    let _ = app.handle_event(Event::BeginDeleteProfile);
    submit(&mut app, &client, Event::Submit); // confirm the (refused) delete

    let state = switcher(&app);
    assert!(
        state.message.as_deref().is_some_and(|m| m.contains("last")),
        "LastProfile surfaced as a switcher message: {:?}",
        state.message,
    );
    assert_eq!(state.profiles.len(), 1, "the profile was not removed");
    assert!(!app.is_pending(), "settled");
}

// ---- in-flight spinner / pending ----

#[test]
fn create_shows_pending_and_spinner_while_outstanding() {
    let (client, mut app) = enter_switcher(vec![profile("p1", "work")]);

    // Open create, type a name, submit but hold the dispatch (don't drive it).
    let _ = app.handle_event(Event::BeginAddProfile);
    type_str(&mut app, "personal");
    let dispatch = app
        .handle_event(Event::Submit)
        .expect("create submit dispatches");
    assert!(app.is_pending(), "create request is in flight");

    // The spinner glyph is appended to the caption while pending (tick 1 → "/"); the caption may
    // wrap, so assert the glyph and the cancel affordance keyword are both present.
    let text = common::render_at(&app, W, H, 1);
    assert!(
        text.contains('/') && text.contains("cancel"),
        "spinner + cancel affordance shown while pending:\n{text}",
    );

    // A request-triggering event while pending is a no-op (no new dispatch, no new call).
    let calls_before = client.calls().len();
    assert!(
        app.handle_event(Event::Refresh).is_none(),
        "refresh while pending dispatches nothing",
    );
    assert_eq!(
        client.calls().len(),
        calls_before,
        "no extra call while pending",
    );

    // Completing the held request settles the flow (create → chained list refresh).
    client.push_create_profile(Ok(profile("p2", "personal")));
    client.push_profiles(Ok(vec![profile("p1", "work"), profile("p2", "personal")]));
    drive(&mut app, &client, dispatch);
    assert!(!app.is_pending(), "settled after the request completes");
}

// ---- cancel / stale-RequestId drop ----

#[test]
fn stale_delete_response_after_cancel_is_dropped() {
    let (client, mut app) = enter_switcher(vec![profile("p1", "work"), profile("p2", "personal")]);

    // Begin a delete of the second profile; capture the dispatch the worker would run.
    let _ = app.handle_event(Event::Next); // selection -> p2
    let _ = app.handle_event(Event::BeginDeleteProfile);
    let dispatch = app
        .handle_event(Event::Submit)
        .expect("delete confirm dispatches");
    assert!(app.is_pending(), "delete is in flight");

    // User cancels before the response arrives: the in-flight marker is cleared.
    assert!(app.handle_event(Event::Cancel).is_none());
    assert!(!app.is_pending(), "cancelled");

    // The abandoned request still ran on the (mocked) server and produces a now-stale response.
    // Applying it must be a no-op (the id mismatches): the profile must NOT be removed.
    client.push_delete_profile(Ok(()));
    let stale = execute(&client, dispatch);
    let follow_up = app.apply_response(stale);

    assert!(follow_up.is_none(), "a stale response yields no follow-up");
    let state = switcher(&app);
    assert_eq!(
        state.profiles.len(),
        2,
        "the dropped stale delete left both profiles in place",
    );
    assert!(state.profiles.iter().any(|p| p.id == "p2"));
    assert!(
        !app.is_pending(),
        "still idle after dropping the stale response"
    );
}

#[test]
fn superseded_response_after_new_request_is_dropped() {
    // Cancel a delete, then re-confirm (new RequestId). The first request's late response carries
    // the old id and must be dropped, not mis-applied to the new in-flight slot.
    let (client, mut app) = enter_switcher(vec![profile("p1", "work"), profile("p2", "personal")]);

    let _ = app.handle_event(Event::Next); // selection -> p2
    let _ = app.handle_event(Event::BeginDeleteProfile);
    let first = app
        .handle_event(Event::Submit)
        .expect("first delete dispatches");
    assert!(app.handle_event(Event::Cancel).is_none());
    assert!(!app.is_pending(), "first delete cancelled");

    // Re-arm and re-confirm the delete after cancel — a fresh RequestId, now the awaited one.
    let _ = app.handle_event(Event::BeginDeleteProfile);
    let second = app
        .handle_event(Event::Submit)
        .expect("re-confirm delete dispatches");
    assert!(app.is_pending());

    // The first (cancelled) delete's response arrives late: dropped; the second still awaited.
    client.push_delete_profile(Ok(()));
    let stale = execute(&client, first);
    assert!(
        app.apply_response(stale).is_none(),
        "the superseded response is dropped",
    );
    assert!(app.is_pending(), "the new request is still in flight");

    // The new (re-confirm) request then completes normally — delete + chained list refresh drive
    // the view to the post-delete list.
    client.push_delete_profile(Ok(()));
    client.push_profiles(Ok(vec![profile("p1", "work")]));
    drive(&mut app, &client, second);
    let state = switcher(&app);
    assert_eq!(
        state.profiles.len(),
        1,
        "the new request's response drove the view"
    );
    assert!(state.profiles.iter().all(|p| p.id != "p2"));
    assert!(!app.is_pending());
}

// ---- deleting the ACTIVE profile re-points to the first remaining (#1, Assumption A7) ----

#[test]
fn deleting_the_active_profile_repoints_to_the_first_remaining() {
    // The active profile is `p1` (from the login bootstrap). Deleting it must re-point the
    // in-memory active id to the first remaining profile, so the NEXT scoped task read uses the new
    // id — proving the switch is purely client-side (#1) and the TUI never scopes to a dead id.
    let (client, mut app) = enter_switcher(vec![profile("p1", "work"), profile("p2", "personal")]);

    // Delete the selected (active) p1. The post-delete refresh returns only p2.
    client.push_delete_profile(Ok(()));
    client.push_profiles(Ok(vec![profile("p2", "personal")]));

    let _ = app.handle_event(Event::BeginDeleteProfile);
    submit(&mut app, &client, Event::Submit); // confirm

    // The switcher now lists only p2, and it is the selected/active entry.
    let state = switcher(&app);
    assert_eq!(state.profiles.len(), 1);
    assert_eq!(state.profiles.first().expect("p2").id, "p2");

    // Now switch back to the Tasks tab: the scoped read must carry the re-pointed id p2, never the
    // deleted p1.
    client.push_tasks(Ok(vec![]));
    back_to_tasks(&mut app, &client);
    let calls = client.calls();
    assert!(
        calls
            .iter()
            .any(|c| matches!(c, Call::ListTasks { token, profile_id }
            if token == "jwt" && profile_id == "p2")),
        "after deleting the active profile, the scoped read uses the re-pointed id p2: {calls:?}",
    );
    // The tree load's second call (ListSubtasks) carries the same re-pointed id.
    assert!(
        matches!(calls.last(), Some(Call::ListSubtasks { profile_id, .. }) if profile_id == "p2"),
        "the tree load's second call carries the re-pointed id p2: {calls:?}",
    );
}

// ---- navigation / offline ----

#[test]
fn switching_away_from_the_idle_switcher_returns_to_the_tasks_tab() {
    // The old idle-`Esc`-back is gone; `Tab` from the idle Profiles tab cycles to Tasks, which
    // re-lists the active profile's tasks (ADR-0010 §1).
    let (client, mut app) = enter_switcher(vec![profile("p1", "work")]);
    client.push_tasks(Ok(vec![]));
    back_to_tasks(&mut app, &client);
    assert!(
        on_tab(&app, Tab::Tasks),
        "Tab from the idle switcher returns to the Tasks tab",
    );
    let calls = client.calls();
    assert!(
        calls
            .iter()
            .any(|c| matches!(c, Call::ListTasks { profile_id, .. } if profile_id == "p1")),
        "the tab switch re-lists the active profile's tasks: {calls:?}",
    );
    // The tree load's second call (ListSubtasks) carries the same active id.
    assert!(
        matches!(calls.last(), Some(Call::ListSubtasks { profile_id, .. }) if profile_id == "p1"),
        "the tab switch chains the tree load's ListSubtasks for the active profile: {calls:?}",
    );
}

#[test]
fn offline_during_list_refresh_blocks_with_the_offline_screen() {
    // A transport failure on the switcher's own refresh routes to the blocking offline screen
    // (#1: every view derives from a server response; lose the server, lose the view).
    let (client, mut app) = enter_switcher(vec![profile("p1", "work")]);
    client.push_profiles(Err(offline_err("connection refused")));
    submit(&mut app, &client, Event::Refresh);
    assert!(
        matches!(app.screen(), Screen::Offline { .. }),
        "switcher refresh offline → blocking offline screen",
    );
}
