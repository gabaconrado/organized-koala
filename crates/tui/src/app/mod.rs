//! The app core: a screen state machine advanced by two pure update functions over [`Event`]s
//! and server [`ClientResponse`]s.
//!
//! [`App`] owns the session ([`Session`]), the current [`Screen`], and the account-global
//! [`Timer`] (a persistent widget rendered on every post-auth screen, ADR-0006 §8.1, not a
//! navigable screen). It performs **no** I/O and holds **no** client: [`App::handle_event`] is
//! pure and returns an [`Option<Dispatch>`] describing the [`ClientRequest`] to execute (the
//! worker thread runs it), and [`App::apply_response`] folds a completed [`ClientResponse`] back
//! into state. The whole interactive surface is driveable through a `ratatui` `TestBackend` with
//! no client and no threads (ADR-0003 / ADR-0006). All state lives in memory for the process
//! lifetime only (hard-constraint #1) — there is no on-disk or cross-run persistence.
//!
//! Add/edit/delete-confirm sub-flows, the timer duration edit, and the `?` help overlay are
//! **input-capturing overlays** unified by [`App::overlay_capturing_input`] (ADR-0010 §3): while
//! one owns input the terminal layer suppresses every global hotkey and routes `Esc` to
//! [`Event::Cancel`]. They render as centred dialogs over the active pane.

pub mod auth;
pub mod main_view;
pub mod notes;
pub mod profiles;
pub mod protocol;
pub mod task_add;
pub mod task_detail;
pub mod task_list;
pub mod timer;
pub mod token;

pub use auth::{AuthField, AuthMode, AuthState};
pub use main_view::{MainState, Tab};
pub use notes::{NoteDetail, NoteForm, NotePane, NotesMode, NotesState};
pub use profiles::{ProfileForm, ProfilesMode, ProfilesState};
pub use protocol::{ClientRequest, ClientResponse, Outcome, RequestId};
pub use task_add::{AddSubtaskState, AddTaskState, EditSubtaskState, EditTaskState};
pub use task_detail::{TaskDetail, TaskPane};
pub use task_list::{OLDER_SEPARATOR_LABEL, TaskListState, VisibleRow};
pub use timer::{DurationEditState, Timer};
pub use token::SessionToken;

use contract::ErrorCode;

use crate::client::ClientError;

/// An input event the app reacts to. The terminal layer translates crossterm key events into
/// these; tests construct them directly. Keeping the app's input alphabet transport-agnostic
/// is what makes the core unit-testable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// A printable character was typed into the focused field.
    Char(char),
    /// Delete the character before the cursor in the focused field.
    Backspace,
    /// Move focus to the next field / list item.
    Next,
    /// Move focus to the previous field / list item.
    Prev,
    /// Confirm the current screen's primary action (submit a form, confirm input).
    Submit,
    /// Commit the focused field explicitly (the multiline Content pane's `Ctrl+S`, ADR-0011 §2).
    /// The note detail handler treats this identically to [`Event::Submit`] so the single-line
    /// Title commits on `Enter` ([`Event::Submit`]) while the multiline Content commits on
    /// `Ctrl+S` ([`Event::Commit`]). Inert outside a text-entry context.
    Commit,
    /// Insert a line break into the focused multiline edit buffer (the Content pane's `Enter`,
    /// ADR-0011 §2). `Enter` maps here **only** while editing the note detail's Content pane;
    /// everywhere else `Enter` stays [`Event::Submit`].
    Newline,
    /// Toggle the auth screen between login and register modes.
    ToggleAuthMode,
    /// Begin the add-task input flow (task list only).
    BeginAddTask,
    /// Begin the add-sub-task input flow for the parent task of the current selection (task list
    /// only): the selected task, or the selected sub-task's parent (`A` / Shift+a, ADR-0012).
    BeginAddSubtask,
    /// Toggle collapse/expand of the selected task's sub-tasks in the list, recording an in-session
    /// override over the status-derived default (task list only; `x`, ADR-0012 §5).
    ToggleCollapse,
    /// Toggle whether the created-before-today ("older") task group and its "Older tasks" separator
    /// are hidden (task list only; `h`, ADR-0014 §5). Default shown; ephemeral view state (#1).
    ToggleHideOlder,
    /// Begin editing the selected task's title/description (task list only).
    BeginEditTask,
    /// Toggle the selected task between done and open (task list only): a done task is reopened,
    /// an open task is marked done.
    ToggleDone,
    /// Begin (or, when already armed, confirm) deletion of the selected task (task list only). A
    /// two-step confirm affordance: the first press arms, a second confirms.
    DeleteSelected,
    /// Switch to the next post-auth tab (`Tasks → Notes → Profiles → Tasks`); `Tab` on a list.
    NextTab,
    /// Switch to the previous post-auth tab (the reverse cycle); `Shift+Tab` on a list.
    PrevTab,
    /// Begin the create-profile sub-flow (switcher list only).
    BeginAddProfile,
    /// Begin renaming the selected profile (switcher list only).
    BeginRenameProfile,
    /// Begin the delete-confirmation sub-flow for the selected profile (switcher list only).
    BeginDeleteProfile,
    /// Begin the create-note sub-flow (notes list only).
    BeginAddNote,
    /// Begin editing the selected note (notes list only).
    BeginEditNote,
    /// Begin the delete-confirmation sub-flow for the selected note (notes list only).
    BeginDeleteNote,
    /// Toggle the account-global focus session: start when idle/completed, stop when running.
    /// Global on every post-auth screen (ADR-0006 §8.2).
    ToggleTimer,
    /// Begin editing the global session duration (on any post-auth screen; ADR-0006 §8).
    BeginEditDuration,
    /// Refresh the current view from the server (also the manual retry from the offline
    /// screen).
    Refresh,
    /// Toggle the help overlay (the `?` key): open it on an idle post-auth screen, close it when
    /// already open. Inert while another dialog is capturing input (Assumption A3).
    ToggleHelp,
    /// Cancel the current sub-flow (e.g. abandon the add-task input), close the help overlay, or
    /// abandon an in-flight request.
    Cancel,
    /// Request to quit the application.
    Quit,
}

/// A request the core wants dispatched: the [`ClientRequest`] plus the [`RequestId`] the core
/// stamped it with (and recorded as the matching in-flight marker). The edge ships this to the
/// worker thread verbatim.
#[derive(Debug, Clone)]
pub struct Dispatch {
    /// The id the in-flight marker was set to; the matching [`ClientResponse`] must echo it.
    pub id: RequestId,
    /// The work to execute.
    pub request: ClientRequest,
}

/// The number of tasks the TUI requests per task-list load. The pagination-ready `limit` capability
/// lives on the wire (`contract::TaskListQuery`, bounded by
/// [`MAX_TASK_LIST_LIMIT`](contract::MAX_TASK_LIST_LIMIT)); this is the **caller's** choice of it, a
/// `tui`-local constant, not a wire constant (ADR-0014 §2). Every task-list load sends this limit
/// and offset 0 — the TUI does not paginate.
pub const TASK_LIST_LIMIT: u32 = 200;

/// The task-list query the TUI sends on every `ListTasks`: [`TASK_LIST_LIMIT`] tasks, offset 0.
fn task_list_query() -> contract::TaskListQuery {
    contract::TaskListQuery {
        limit: Some(TASK_LIST_LIMIT),
        offset: Some(0),
    }
}

/// The current civil day number (days since the Unix epoch, UTC) from the wall clock — the "today"
/// reference for the task-list today/older split (ADR-0014 §4). Reading the clock is an effect, so
/// this lives at the edge like the timer countdown's `Instant`-based derivation; the pure task-list
/// core takes the resulting day number as data (`day_number`), keeping it deterministic under test.
/// Day boundaries are UTC (Assumption A5-note): the `tui` crate holds no `chrono`/timezone
/// dependency (A8), and the server stores/renders timestamps in UTC.
#[must_use]
pub fn current_day_number() -> i64 {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
        .unwrap_or(0);
    task_list::day_number(secs)
}

/// Fixed title for the focus-session completion notification (Assumption A5).
const TIMER_COMPLETE_TITLE: &str = "Focus timer";
/// Fixed body for the focus-session completion notification (Assumption A5).
const TIMER_COMPLETE_BODY: &str = "Your focus session has ended.";

/// The fixed copy for the single desktop notification fired when a focus session completes
/// (Assumption A5: plain title + body, no sound, no actions). Produced once per completion by
/// [`App::take_pending_notification`] and handed to the injected
/// [`Notifier`](crate::client::Notifier) at the edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimerNotification {
    /// The notification title.
    pub title: &'static str,
    /// The notification body.
    pub body: &'static str,
}

/// The session held in memory for the process lifetime: the JWT, the account identifier the user
/// authenticated with, and the active profile id. Never persisted (hard-constraint #1).
#[derive(Debug, Clone)]
pub struct Session {
    /// The bearer token returned by register/login, held redacted so it never leaks through a
    /// derived `Debug`, a log line, or a trace span (see [`SessionToken`]).
    pub token: SessionToken,
    /// The account identifier the user entered on the login/register form (the login identifier, or
    /// the registered username). Captured client-side at auth time for the post-auth title; never a
    /// new wire field (ADR-0010 §2).
    pub account: String,
    /// The auto-selected active profile id.
    pub profile_id: String,
    /// The active profile's display name.
    pub profile_name: String,
}

/// The current screen of the state machine. The timer is **not** a screen — it is a global widget
/// rendered on every post-auth screen (ADR-0006 §8.1).
#[derive(Debug, Clone)]
pub enum Screen {
    /// The auth screen (login or register).
    Auth(AuthState),
    /// The single post-auth tabbed view: the `Tasks | Notes | Profiles` tab bar over the three
    /// list panes for the active profile (ADR-0010 §1). Boxed because the three panes together
    /// dwarf the other variants (`clippy::large_enum_variant`).
    Main(Box<MainState>),
    /// The blocking "server unreachable" screen. Carries the message and the in-flight marker
    /// while a retry probe is outstanding.
    Offline {
        /// Human-readable description of the connectivity failure.
        message: String,
        /// The in-flight request id while a retry health-probe is outstanding; `None` when idle.
        pending: Option<RequestId>,
    },
}

impl Screen {
    /// Whether this is a post-auth screen (the timer widget is shown and the global timer
    /// keybindings are live). The auth and offline screens are excluded.
    #[must_use]
    fn is_post_auth(&self) -> bool {
        matches!(self, Screen::Main(_))
    }
}

/// The application: the screen state machine, the account-global timer, the in-memory session,
/// and the request-id counter.
///
/// Advance it by feeding [`Event`]s to [`App::handle_event`] (which may return a [`Dispatch`] to
/// run) and completed results to [`App::apply_response`]; render the current state with the
/// [`crate::ui`] draw functions. [`App::should_quit`] reports when a quit was requested.
#[derive(Debug)]
pub struct App {
    session: Option<Session>,
    screen: Screen,
    timer: Timer,
    /// Whether the `?` help overlay is open. Transient process-lifetime UI state (#1); it
    /// participates in [`App::overlay_capturing_input`] like any other dialog.
    help_open: bool,
    quit: bool,
    next_id: u64,
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

impl App {
    /// Creates a new app on the auth (login) screen.
    #[must_use]
    pub fn new() -> Self {
        Self {
            session: None,
            screen: Screen::Auth(AuthState::new()),
            timer: Timer::new(),
            help_open: false,
            quit: false,
            next_id: 0,
        }
    }

    /// The current screen, for rendering.
    #[must_use]
    pub fn screen(&self) -> &Screen {
        &self.screen
    }

    /// The account-global timer, for rendering the global widget.
    #[must_use]
    pub fn timer(&self) -> &Timer {
        &self.timer
    }

    /// The active session, if authenticated.
    #[must_use]
    pub fn session(&self) -> Option<&Session> {
        self.session.as_ref()
    }

    /// Whether a quit has been requested.
    #[must_use]
    pub fn should_quit(&self) -> bool {
        self.quit
    }

    /// Whether a server request is currently outstanding on the active screen. While true the UI
    /// renders the spinner and request-triggering screen events are no-ops.
    #[must_use]
    pub fn is_pending(&self) -> bool {
        self.screen_pending_id().is_some()
    }

    /// Whether the duration-edit sub-flow owns keystrokes (it overlays the active post-auth
    /// screen as a global text-entry mode).
    #[must_use]
    pub fn is_editing_duration(&self) -> bool {
        self.timer.is_editing()
    }

    /// Whether the `?` help overlay is open (for rendering).
    #[must_use]
    pub fn help_open(&self) -> bool {
        self.help_open
    }

    /// The single "input-capturing overlay" predicate (ADR-0010 §3/§4): `true` whenever **any**
    /// dialog/overlay owns input — a task add/edit form, a task delete confirmation, an **open task
    /// detail view**, a notes create/edit/confirm-delete, a profiles create/rename/confirm-delete,
    /// the duration edit, **or** the help overlay. While true, the terminal layer suppresses every
    /// global hotkey (`q`/`r`/`t`/`T`/tab-switch) and routes `Esc` to [`Event::Cancel`]; text,
    /// field/pane-switch, and submit still reach the focused surface. Confirmation dialogs capture
    /// no text but still count — they suppress globals and are `Esc`-cancelled. An open detail view
    /// counts here (so globals/tab-switch are suppressed), but `?` help stays reachable over an
    /// *idle* detail view (Assumption A7 — see the [`Event::ToggleHelp`] guard in
    /// [`Self::handle_event`]). `false` on the auth/offline screens (the auth form is its own
    /// always-text-entry context).
    #[must_use]
    pub fn overlay_capturing_input(&self) -> bool {
        if self.help_open || self.timer.is_editing() {
            return true;
        }
        match &self.screen {
            Screen::Main(main) => match main.active_tab {
                Tab::Tasks => main.tasks.in_sub_flow() || main.tasks.confirming_delete.is_some(),
                Tab::Notes => main.notes.in_sub_flow() || main.notes.detail_open(),
                Tab::Profiles => main.profiles.in_sub_flow(),
            },
            Screen::Auth(_) | Screen::Offline { .. } => false,
        }
    }

    /// Whether a per-field detail view (task or note) is open on the active tab.
    #[must_use]
    fn detail_view_open(&self) -> bool {
        match &self.screen {
            Screen::Main(main) => match main.active_tab {
                Tab::Tasks => main.tasks.detail.is_some(),
                Tab::Notes => main.notes.detail_open(),
                Tab::Profiles => false,
            },
            _ => false,
        }
    }

    /// Whether a detail view is open with a field edit in progress (a text-entry context).
    #[must_use]
    fn detail_field_editing(&self) -> bool {
        match &self.screen {
            Screen::Main(main) => match main.active_tab {
                Tab::Tasks => main.tasks.detail_editing(),
                Tab::Notes => main.notes.detail_editing(),
                Tab::Profiles => false,
            },
            _ => false,
        }
    }

    /// Whether `?` may open the help overlay now: from an idle post-auth screen, or over an **idle**
    /// detail view (no field edit in progress) — but never over a modal dialog/form or a detail
    /// field edit (Assumption A7).
    #[must_use]
    fn can_open_help(&self) -> bool {
        if !self.screen.is_post_auth() {
            return false;
        }
        if self.detail_view_open() {
            return !self.detail_field_editing();
        }
        !self.overlay_capturing_input()
    }

    /// The id of the request currently awaited on the active surface, if any. On the tabbed view
    /// this is the active pane's marker (each pane owns its own).
    #[must_use]
    fn screen_pending_id(&self) -> Option<RequestId> {
        match &self.screen {
            Screen::Auth(auth) => auth.pending,
            Screen::Main(main) => match main.active_tab {
                Tab::Tasks => main.tasks.pending,
                Tab::Notes => main.notes.pending,
                Tab::Profiles => main.profiles.pending,
            },
            Screen::Offline { pending, .. } => *pending,
        }
    }

    /// The pure update entry point: apply one [`Event`] to the current state, returning a
    /// [`Dispatch`] if the event triggers a server request.
    ///
    /// This is purely a state transition (no I/O, no client). The same event on the same state
    /// always produces the same `(next state, Option<Dispatch>)`, so the whole interactive
    /// surface is driveable from tests with no client and no threads. At most one request is in
    /// flight per surface (the screen and the timer each have their own marker); a
    /// request-triggering event while that surface's request is outstanding is a no-op. `Quit`
    /// and `Cancel` stay live during a request.
    pub fn handle_event(&mut self, event: Event) -> Option<Dispatch> {
        // The help overlay owns input while open: `Cancel` (Esc / `?`) closes it; everything else
        // is inert (Assumption A3). Checked before the duration edit so the two never stack.
        if self.help_open {
            if matches!(event, Event::Cancel | Event::ToggleHelp) {
                self.help_open = false;
            }
            return None;
        }
        // The duration-edit sub-flow is a global text-entry mode overlaying the active post-auth
        // screen: while open it owns keystrokes, so they never reach the screen handler.
        if self.timer.is_editing() {
            return self.handle_edit_event(event);
        }
        match event {
            Event::Quit => {
                self.quit = true;
                None
            }
            // Open the help overlay on an idle post-auth screen or over an idle detail view; inert
            // while a dialog/form or a detail field edit captures input (Assumption A3/A7).
            Event::ToggleHelp if self.can_open_help() => {
                self.help_open = true;
                None
            }
            Event::Cancel if self.is_pending() => {
                self.cancel_in_flight();
                None
            }
            // Global timer controls, live on every post-auth screen (ADR-0006 §8.2).
            Event::ToggleTimer if self.screen.is_post_auth() => self.toggle_timer(),
            Event::BeginEditDuration if self.screen.is_post_auth() => {
                self.timer.begin_edit();
                None
            }
            _ => self.handle_screen_event(event),
        }
    }

    /// Dispatch a non-edit event to the active surface, stamping the active pane's in-flight
    /// marker.
    fn handle_screen_event(&mut self, event: Event) -> Option<Dispatch> {
        // Tab switching and pick-active are tabbed-view concerns the container owns; the per-pane
        // states never see them.
        if let Screen::Main(main) = &self.screen {
            match (&event, main.active_tab) {
                // Tab/Shift+Tab cycle the active tab, but only when no per-pane sub-flow is
                // capturing input (a sub-flow uses Tab to switch fields). A switch issues a fresh
                // list load for the destination so the pane derives from a server response (#1).
                (Event::NextTab, _) if !self.active_pane_in_sub_flow() => {
                    return self.switch_tab(main.active_tab.next());
                }
                (Event::PrevTab, _) if !self.active_pane_in_sub_flow() => {
                    return self.switch_tab(main.active_tab.prev());
                }
                // Pick-active: `Submit` on the idle switcher pane rebinds the in-memory active
                // profile and re-scopes the reads — no server "switch" call (#1, ADR-0009 §5).
                (Event::Submit, Tab::Profiles) if !main.profiles.in_sub_flow() => {
                    return self.pick_active_profile();
                }
                _ => {}
            }
        }
        let request = match &mut self.screen {
            Screen::Auth(auth) => auth.handle_event(event),
            Screen::Main(main) => match main.active_tab {
                Tab::Tasks => {
                    main.tasks
                        .handle_event(event, self.session.as_ref(), current_day_number())
                }
                Tab::Notes => main.notes.handle_event(event, self.session.as_ref()),
                Tab::Profiles => main.profiles.handle_event(event, self.session.as_ref()),
            },
            Screen::Offline { pending, .. } => {
                if pending.is_some() {
                    None
                } else if matches!(event, Event::Refresh | Event::Submit) {
                    Some(ClientRequest::Health)
                } else {
                    None
                }
            }
        };
        request.map(|request| self.dispatch_screen(request))
    }

    /// Whether the active tabbed pane has a text-entry / confirmation sub-flow **or an open detail
    /// view** (so `Tab` must switch fields/panes, not top-level tabs). `false` on the auth/offline
    /// screens.
    fn active_pane_in_sub_flow(&self) -> bool {
        match &self.screen {
            Screen::Main(main) => match main.active_tab {
                Tab::Tasks => main.tasks.in_sub_flow(),
                Tab::Notes => main.notes.in_sub_flow() || main.notes.detail_open(),
                Tab::Profiles => main.profiles.in_sub_flow(),
            },
            _ => false,
        }
    }

    /// Switch the tabbed view to `tab` and dispatch a fresh list load for the destination pane,
    /// preserving that pane's selected row across the reload (transient UI state). The pane data is
    /// always re-derived from a server response for the active profile (#1, #4). A no-op without a
    /// session.
    fn switch_tab(&mut self, tab: Tab) -> Option<Dispatch> {
        let session = self.session.clone()?;
        let Screen::Main(main) = &mut self.screen else {
            return None;
        };
        main.active_tab = tab;
        match tab {
            Tab::Tasks => Some(self.dispatch_screen(ClientRequest::ListTasks {
                token: session.token,
                profile_id: session.profile_id,
                query: task_list_query(),
            })),
            Tab::Notes => Some(self.dispatch_screen(ClientRequest::ListNotes {
                token: session.token,
                profile_id: session.profile_id,
            })),
            Tab::Profiles => Some(self.dispatch_screen(ClientRequest::ListProfiles {
                token: session.token,
            })),
        }
    }

    /// Pick-active: rebind the in-memory active profile to the selected one and re-scope the reads
    /// by re-loading the Tasks pane and switching to it. This is **client-side only** — no server
    /// "switch" call and no persistence (#1, Assumption A6). A no-op if nothing is selected or the
    /// session is gone.
    fn pick_active_profile(&mut self) -> Option<Dispatch> {
        let mut session = self.session.clone()?;
        let Screen::Main(main) = &self.screen else {
            return None;
        };
        let picked = main.profiles.selected_profile()?;
        session.profile_id = picked.id.clone();
        session.profile_name = picked.name.clone();
        self.session = Some(session.clone());
        self.switch_tab(Tab::Tasks)
    }

    /// Handle a keystroke while the duration-edit sub-flow is open. `Submit` issues the update
    /// (stamping the timer marker); `Cancel` abandons the edit; the rest mutate the buffer.
    fn handle_edit_event(&mut self, event: Event) -> Option<Dispatch> {
        if self.timer.is_pending() {
            return None;
        }
        match event {
            Event::Char(c) => self.timer.edit_char(c),
            Event::Backspace => self.timer.edit_backspace(),
            Event::Cancel => self.timer.cancel_edit(),
            Event::Submit => {
                let session = self.session.clone()?;
                let request = self.timer.submit_edit(&session)?;
                return Some(self.dispatch_timer(request));
            }
            _ => {}
        }
        None
    }

    /// Resolve the global `p` toggle to a start/stop request, stamping the timer's marker.
    fn toggle_timer(&mut self) -> Option<Dispatch> {
        let session = self.session.clone()?;
        let request = self.timer.toggle(&session)?;
        Some(self.dispatch_timer(request))
    }

    /// Issue the initial timer config→session load if a session exists and it has not loaded yet.
    /// Called by the edge once a post-auth screen is shown. Stamps the timer marker.
    pub fn load_timer_if_needed(&mut self) -> Option<Dispatch> {
        if !self.screen.is_post_auth() {
            return None;
        }
        let session = self.session.clone()?;
        let request = self.timer.initial_load(&session)?;
        Some(self.dispatch_timer(request))
    }

    /// Issue the coarse timer-session refresh (the ~1-minute cadence). Called by the edge on the
    /// cadence boundary while a post-auth screen is shown. Stamps the timer marker.
    pub fn refresh_timer(&mut self) -> Option<Dispatch> {
        if !self.screen.is_post_auth() {
            return None;
        }
        let session = self.session.clone()?;
        let request = self.timer.refresh(&session)?;
        Some(self.dispatch_timer(request))
    }

    /// Stamp a request with a fresh id, record it as the active screen's in-flight marker, and
    /// return the [`Dispatch`] for the edge to run.
    fn dispatch_screen(&mut self, request: ClientRequest) -> Dispatch {
        let id = self.next_request_id();
        self.set_screen_pending(Some(id));
        Dispatch { id, request }
    }

    /// Stamp a request with a fresh id, record it as the timer's in-flight marker, and return the
    /// [`Dispatch`] for the edge to run.
    fn dispatch_timer(&mut self, request: ClientRequest) -> Dispatch {
        let id = self.next_request_id();
        self.timer.pending = Some(id);
        Dispatch { id, request }
    }

    fn next_request_id(&mut self) -> RequestId {
        let id = RequestId(self.next_id);
        self.next_id = self.next_id.wrapping_add(1);
        id
    }

    fn set_screen_pending(&mut self, id: Option<RequestId>) {
        match &mut self.screen {
            Screen::Auth(auth) => auth.pending = id,
            Screen::Main(main) => match main.active_tab {
                Tab::Tasks => main.tasks.pending = id,
                Tab::Notes => main.notes.pending = id,
                Tab::Profiles => main.profiles.pending = id,
            },
            Screen::Offline { pending, .. } => *pending = id,
        }
    }

    /// Abandon the in-flight request on the active screen (user pressed cancel): clear the marker
    /// so the screen is interactive again. The worker still runs the abandoned request to
    /// completion, but its response will be dropped by [`apply_response`] on id mismatch.
    fn cancel_in_flight(&mut self) {
        match &mut self.screen {
            Screen::Auth(auth) => auth.pending = None,
            Screen::Main(main) => match main.active_tab {
                Tab::Tasks => {
                    let list = &mut main.tasks;
                    list.pending = None;
                    if let Some(add) = &mut list.adding {
                        add.error = None;
                    }
                    if let Some(edit) = &mut list.editing {
                        edit.error = None;
                    }
                }
                Tab::Notes => {
                    let notes = &mut main.notes;
                    notes.pending = None;
                    match &mut notes.mode {
                        NotesMode::Creating(form) | NotesMode::Editing { form, .. } => {
                            form.error = None;
                        }
                        _ => {}
                    }
                }
                Tab::Profiles => {
                    let profiles = &mut main.profiles;
                    profiles.pending = None;
                    match &mut profiles.mode {
                        ProfilesMode::Creating(form) | ProfilesMode::Renaming { form, .. } => {
                            form.error = None;
                        }
                        _ => {}
                    }
                }
            },
            Screen::Offline { pending, .. } => *pending = None,
        }
    }

    /// The pure response-folding seam: apply a completed [`ClientResponse`] to the matching
    /// in-flight surface (the active screen or the global timer), running the same success /
    /// error-code branching the inline code ran pre-split.
    ///
    /// A response whose id does not match the awaited request on its surface is **dropped** (it
    /// was cancelled or superseded). Returns a follow-up [`Dispatch`] when the response chains
    /// into the next request (post-auth profile/task load, a refresh after create, or the
    /// config→session chain).
    pub fn apply_response(&mut self, response: ClientResponse) -> Option<Dispatch> {
        match response.outcome {
            Outcome::GetTimerConfig(result) => {
                self.apply_timer(response.id, |app| app.apply_timer_config(result, true))
            }
            Outcome::UpdateTimerConfig(result) => {
                self.apply_timer(response.id, |app| app.apply_timer_config(result, false))
            }
            Outcome::GetTimerSession(result)
            | Outcome::StartTimerSession(result)
            | Outcome::StopTimerSession(result) => {
                self.apply_timer(response.id, |app| app.apply_timer_session(result))
            }
            outcome => {
                if self.screen_pending_id() != Some(response.id) {
                    // Stale: the screen request was cancelled or superseded — never mutate state.
                    return None;
                }
                match outcome {
                    Outcome::Health(result) => self.apply_health(result),
                    Outcome::Register(result) | Outcome::Login(result) => self.apply_auth(result),
                    Outcome::ListProfiles { token, result } => self.apply_profiles(token, result),
                    Outcome::CreateProfile(result) => self.apply_create_profile(result),
                    Outcome::UpdateProfile(result) => self.apply_update_profile(result),
                    Outcome::DeleteProfile(result) => self.apply_delete_profile(result),
                    Outcome::ListTasks(result) => self.apply_tasks(result),
                    Outcome::CreateTask(result) => self.apply_create(result),
                    Outcome::UpdateTask(result) => self.apply_update(result),
                    Outcome::DeleteTask(result) => self.apply_delete(result),
                    Outcome::ListSubtasks(result) => self.apply_subtasks(result),
                    Outcome::ListTaskSubtasks(result) => self.apply_task_subtasks(result),
                    Outcome::CreateSubtask(result) => self.apply_create_subtask(result),
                    Outcome::UpdateSubtask(result) => self.apply_update_subtask(result),
                    Outcome::DeleteSubtask(result) => self.apply_delete_subtask(result),
                    Outcome::ListNotes(result) => self.apply_notes(result),
                    Outcome::CreateNote(result) => self.apply_create_note(result),
                    Outcome::GetNote(result) => self.apply_get_note(result),
                    Outcome::UpdateNote(result) => self.apply_update_note(result),
                    Outcome::DeleteNote(result) => self.apply_delete_note(result),
                    // Timer outcomes are handled above.
                    Outcome::GetTimerConfig(_)
                    | Outcome::UpdateTimerConfig(_)
                    | Outcome::GetTimerSession(_)
                    | Outcome::StartTimerSession(_)
                    | Outcome::StopTimerSession(_) => None,
                }
            }
        }
    }

    /// Guard a timer outcome by the timer's own in-flight marker, dropping a stale response, and
    /// run the folding closure on a match.
    fn apply_timer(
        &mut self,
        id: RequestId,
        fold: impl FnOnce(&mut Self) -> Option<Dispatch>,
    ) -> Option<Dispatch> {
        if self.timer.pending != Some(id) {
            // Stale: the timer request was cancelled or superseded.
            return None;
        }
        fold(self)
    }

    fn apply_health(&mut self, result: crate::client::ClientResult<()>) -> Option<Dispatch> {
        self.set_screen_pending(None);
        match result {
            Ok(()) => {
                if let Some(session) = self.session.clone() {
                    Some(self.dispatch_screen(ClientRequest::ListTasks {
                        token: session.token,
                        profile_id: session.profile_id,
                        query: task_list_query(),
                    }))
                } else {
                    self.go_to_login();
                    None
                }
            }
            Err(err) => {
                self.go_offline(&err);
                None
            }
        }
    }

    fn apply_auth(
        &mut self,
        result: crate::client::ClientResult<contract::SessionResponse>,
    ) -> Option<Dispatch> {
        match result {
            Ok(session) => {
                // Capture the account identifier the user entered (the login identifier or the
                // registered username) so the post-auth title can render `<user>` — client-side
                // only, no new wire (ADR-0010 §2). Stashed on the auth state until the session is
                // established in `apply_profiles`.
                if let Screen::Auth(auth) = &mut self.screen {
                    auth.account = match auth.mode {
                        AuthMode::Login => auth.identifier.trim().to_owned(),
                        AuthMode::Register => auth.username.trim().to_owned(),
                    };
                }
                let token = SessionToken::new(session.token);
                Some(self.dispatch_screen(ClientRequest::ListProfiles { token }))
            }
            Err(err) => {
                self.set_screen_pending(None);
                self.handle_auth_error(err);
                None
            }
        }
    }

    /// Fold a `ListProfiles` response. While the switcher is open it is a list/refresh result that
    /// repopulates the switcher (the active profile stays selected if still present); during the
    /// post-auth bootstrap it establishes the session and chains the initial task load.
    fn apply_profiles(
        &mut self,
        token: SessionToken,
        result: crate::client::ClientResult<Vec<contract::Profile>>,
    ) -> Option<Dispatch> {
        // On the tabbed view a `ListProfiles` response is a switcher list/refresh; only during the
        // post-auth bootstrap (still on the auth screen) does it establish the session.
        if matches!(self.screen, Screen::Main(_)) {
            return self.apply_profiles_list(result);
        }
        match result {
            Ok(profiles) => {
                let Some(profile) = profiles.into_iter().next() else {
                    self.set_screen_pending(None);
                    if let Screen::Auth(auth) = &mut self.screen {
                        auth.error = Some("account has no profile".to_owned());
                    }
                    return None;
                };
                let account = match &self.screen {
                    Screen::Auth(auth) => auth.account.clone(),
                    _ => String::new(),
                };
                self.session = Some(Session {
                    token: token.clone(),
                    account,
                    profile_id: profile.id.clone(),
                    profile_name: profile.name,
                });
                // The auth screen still holds the in-flight marker; carry it forward by
                // re-dispatching the task load (the new id replaces it on the auth screen until
                // the task list materialises in `apply_tasks`).
                Some(self.dispatch_screen(ClientRequest::ListTasks {
                    token,
                    profile_id: profile.id,
                    query: task_list_query(),
                }))
            }
            Err(err) => {
                self.set_screen_pending(None);
                self.handle_post_auth_error(err);
                None
            }
        }
    }

    /// Fold a switcher list/refresh response into the open switcher. On success it rebuilds the
    /// list (keeping the active profile selected if still present), preserving an in-progress
    /// create/rename sub-flow across the refresh; errors surface on the switcher.
    fn apply_profiles_list(
        &mut self,
        result: crate::client::ClientResult<Vec<contract::Profile>>,
    ) -> Option<Dispatch> {
        match result {
            Ok(profiles) => {
                // Re-point the in-memory active profile if it is gone from the list (it was just
                // deleted): pick the first remaining (#1, Assumption A6). No-op while it is present.
                self.repoint_active(&profiles);
                let preserved = self.profiles_pane_selection();
                let active_id = self.active_profile_id();
                let mut state = ProfilesState::new(profiles, &active_id);
                if let Some((selected, mode)) = preserved {
                    if matches!(
                        mode,
                        ProfilesMode::Creating(_) | ProfilesMode::Renaming { .. }
                    ) {
                        state.mode = mode;
                    }
                    // Preserve the selected row across the refresh (transient UI state, #1).
                    if selected.is_some() {
                        state.selected = selected;
                    }
                }
                if let Some(main) = self.main_mut() {
                    main.profiles = state;
                }
            }
            Err(err) => {
                self.set_screen_pending(None);
                self.handle_post_auth_error(err);
            }
        }
        None
    }

    /// The post-auth tabbed view, if it is showing.
    fn main_mut(&mut self) -> Option<&mut MainState> {
        match &mut self.screen {
            Screen::Main(main) => Some(main.as_mut()),
            _ => None,
        }
    }

    /// The profiles pane's current `(selected, mode)`, used to carry transient UI state across a
    /// server refresh. `None` when the tabbed view is not showing.
    fn profiles_pane_selection(&self) -> Option<(Option<usize>, ProfilesMode)> {
        match &self.screen {
            Screen::Main(main) => Some((main.profiles.selected, main.profiles.mode.clone())),
            _ => None,
        }
    }

    /// The tasks pane of the post-auth tabbed view, if it is showing.
    fn tasks_pane_mut(&mut self) -> Option<&mut TaskListState> {
        match &mut self.screen {
            Screen::Main(main) => Some(&mut main.tasks),
            _ => None,
        }
    }

    /// The notes pane of the post-auth tabbed view, if it is showing.
    fn notes_pane_mut(&mut self) -> Option<&mut NotesState> {
        match &mut self.screen {
            Screen::Main(main) => Some(&mut main.notes),
            _ => None,
        }
    }

    /// The profiles pane of the post-auth tabbed view, if it is showing.
    fn profiles_pane_mut(&mut self) -> Option<&mut ProfilesState> {
        match &mut self.screen {
            Screen::Main(main) => Some(&mut main.profiles),
            _ => None,
        }
    }

    /// The active profile id held in the in-memory session (empty if no session yet).
    fn active_profile_id(&self) -> String {
        self.session
            .as_ref()
            .map_or_else(String::new, |s| s.profile_id.clone())
    }

    /// Re-point the in-memory active profile to the first profile in `profiles` when the current
    /// active id is no longer present in the list (it was just deleted) — keeping the TUI scoped to
    /// a real namespace (#1, Assumption A7). A no-op while the active profile is still in the list;
    /// the account always retains ≥1 profile (the last-profile delete is server-refused), so a
    /// non-empty list is the norm here.
    fn repoint_active(&mut self, profiles: &[contract::Profile]) {
        let Some(session) = &mut self.session else {
            return;
        };
        let still_present = profiles.iter().any(|p| p.id == session.profile_id);
        if still_present {
            return;
        }
        if let Some(first) = profiles.first() {
            session.profile_id = first.id.clone();
            session.profile_name = first.name.clone();
        }
    }

    /// Fold a create-profile response. On success the create sub-flow closes and the switcher is
    /// re-fetched from the server (#1); a duplicate-name (or other) error surfaces inline in the
    /// open form.
    fn apply_create_profile(
        &mut self,
        result: crate::client::ClientResult<contract::Profile>,
    ) -> Option<Dispatch> {
        match result {
            Ok(_) => {
                if let Some(profiles) = self.profiles_pane_mut() {
                    profiles.mode = ProfilesMode::List;
                }
                self.refresh_profiles()
            }
            Err(err) => {
                self.set_screen_pending(None);
                self.handle_profile_form_error(err);
                None
            }
        }
    }

    /// Fold a rename-profile response. On success the rename sub-flow closes, the active profile's
    /// cached name is refreshed if it was the one renamed, and the switcher is re-fetched (#1); a
    /// duplicate-name (or other) error surfaces inline in the open form.
    fn apply_update_profile(
        &mut self,
        result: crate::client::ClientResult<contract::Profile>,
    ) -> Option<Dispatch> {
        match result {
            Ok(profile) => {
                // Keep the in-memory session label current if the active profile was renamed.
                if let Some(session) = &mut self.session
                    && session.profile_id == profile.id
                {
                    session.profile_name = profile.name.clone();
                }
                if let Some(profiles) = self.profiles_pane_mut() {
                    profiles.mode = ProfilesMode::List;
                }
                self.refresh_profiles()
            }
            Err(err) => {
                self.set_screen_pending(None);
                self.handle_profile_form_error(err);
                None
            }
        }
    }

    /// Fold a delete-profile response. On success the confirmation closes; if the deleted profile
    /// was the active one the in-memory active profile is re-pointed to another from the refreshed
    /// list (#1, Assumption A7). A `last_profile` (or other) error surfaces on the switcher.
    fn apply_delete_profile(
        &mut self,
        result: crate::client::ClientResult<()>,
    ) -> Option<Dispatch> {
        match result {
            Ok(()) => {
                let deleted = if let Some(profiles) = self.profiles_pane_mut() {
                    let deleted = match &profiles.mode {
                        ProfilesMode::ConfirmingDelete { profile_id, .. } => {
                            Some(profile_id.clone())
                        }
                        _ => None,
                    };
                    profiles.mode = ProfilesMode::List;
                    deleted
                } else {
                    None
                };
                // If the active profile was the one deleted, drop the cached id so the refreshed
                // list re-points it to the first remaining profile (see `repoint_active`).
                if let (Some(deleted), Some(session)) = (deleted, &mut self.session)
                    && session.profile_id == deleted
                {
                    session.profile_id = String::new();
                    session.profile_name = String::new();
                }
                self.refresh_profiles()
            }
            Err(err) => {
                self.set_screen_pending(None);
                self.handle_profile_delete_error(err);
                None
            }
        }
    }

    /// Error routing for a delete: offline → blocking, `unauthenticated` → login, and a
    /// `last_profile` refusal (or other error) surfaces inline on the switcher with a clear message
    /// (the account must keep ≥1 namespace, ADR-0009 §4).
    fn handle_profile_delete_error(&mut self, err: ClientError) {
        if err.is_offline() {
            self.go_offline(&err);
            return;
        }
        if err.code() == Some(&ErrorCode::Unauthenticated) {
            self.go_to_login();
            return;
        }
        let message = if err.code() == Some(&ErrorCode::LastProfile) {
            "cannot delete the last profile — the account must keep at least one".to_owned()
        } else {
            err.to_string()
        };
        if let Some(profiles) = self.profiles_pane_mut() {
            profiles.message = Some(message);
        }
    }

    /// Re-dispatch a switcher list load after a successful mutation, chaining the in-flight marker
    /// forward. Returns to login if the session vanished.
    fn refresh_profiles(&mut self) -> Option<Dispatch> {
        let Some(session) = self.session.clone() else {
            self.set_screen_pending(None);
            self.go_to_login();
            return None;
        };
        Some(self.dispatch_screen(ClientRequest::ListProfiles {
            token: session.token,
        }))
    }

    /// Error routing for a create/rename: offline → blocking, `unauthenticated` → login, and a
    /// duplicate-name (`profile_name_taken`) or validation error surfaces inline in the open form
    /// so the user can correct it (mirror of [`Self::handle_note_form_error`]). The
    /// [`ErrorCode::ProfileNameTaken`] case is rendered as a clear "name already in use" message.
    fn handle_profile_form_error(&mut self, err: ClientError) {
        if err.is_offline() {
            self.go_offline(&err);
            return;
        }
        if err.code() == Some(&ErrorCode::Unauthenticated) {
            self.go_to_login();
            return;
        }
        let message = if err.code() == Some(&ErrorCode::ProfileNameTaken) {
            "a profile with that name already exists".to_owned()
        } else {
            err.to_string()
        };
        if let Some(profiles) = self.profiles_pane_mut() {
            match &mut profiles.mode {
                ProfilesMode::Creating(form) | ProfilesMode::Renaming { form, .. } => {
                    form.error = Some(message);
                }
                _ => profiles.message = Some(message),
            }
        }
    }

    fn apply_tasks(
        &mut self,
        result: crate::client::ClientResult<Vec<contract::Task>>,
    ) -> Option<Dispatch> {
        match result {
            Ok(tasks) => {
                let mut state = TaskListState::new(tasks);
                match &mut self.screen {
                    // Tabbed view already open: replace only the tasks pane, preserving an
                    // in-progress add/edit sub-flow (task or sub-task), the open detail view, the
                    // selected row, and the loaded sub-tasks + collapse overrides (transient UI
                    // state, #1) — the chained `ListSubtasks` refreshes the tree below.
                    Screen::Main(main) => {
                        state.adding = main.tasks.adding.clone();
                        state.adding_subtask = main.tasks.adding_subtask.clone();
                        state.editing_subtask = main.tasks.editing_subtask.clone();
                        state.detail = main.tasks.detail.clone();
                        state.subtasks = std::mem::take(&mut main.tasks.subtasks);
                        state.collapse_overrides =
                            std::mem::take(&mut main.tasks.collapse_overrides);
                        if main.tasks.selected.is_some() && state.selected.is_some() {
                            state.selected = main.tasks.selected;
                        }
                        main.tasks = state;
                    }
                    // Post-auth bootstrap (still on the auth screen): open the tabbed view with
                    // Tasks selected by default; Notes/Profiles panes start empty and load on first
                    // switch (each derived from its own server response, #1).
                    _ => {
                        let active_id = self.active_profile_id();
                        self.screen = Screen::Main(Box::new(MainState::new(
                            state,
                            NotesState::new(Vec::new()),
                            ProfilesState::new(Vec::new(), &active_id),
                        )));
                    }
                }
                // Chain the second call of the two-call tree load: the profile's sub-tasks, grouped
                // under their parents client-side (ADR-0013 §3, no N+1). Carries the in-flight
                // marker forward.
                let Some(session) = self.session.clone() else {
                    self.set_screen_pending(None);
                    self.go_to_login();
                    return None;
                };
                Some(self.dispatch_screen(ClientRequest::ListSubtasks {
                    token: session.token,
                    profile_id: session.profile_id,
                }))
            }
            Err(err) => {
                self.set_screen_pending(None);
                self.handle_post_auth_error(err);
                None
            }
        }
    }

    /// Fold the profile's sub-task list (the second call of the Tasks-tab tree load) into the tasks
    /// pane: replace the `subtasks` vector and prune any collapse override for a task no longer
    /// present (ADR-0012 §5 — overrides are dropped on a fresh load for absent task ids). Re-clamp
    /// the visible-row selection so it never points past the row count after the tree changes.
    fn apply_subtasks(
        &mut self,
        result: crate::client::ClientResult<Vec<contract::Subtask>>,
    ) -> Option<Dispatch> {
        self.set_screen_pending(None);
        match result {
            Ok(subtasks) => {
                if let Some(list) = self.tasks_pane_mut() {
                    list.subtasks = subtasks;
                    let present: std::collections::HashSet<&str> =
                        list.tasks.iter().map(|t| t.id.as_str()).collect();
                    list.collapse_overrides
                        .retain(|task_id, _| present.contains(task_id.as_str()));
                    let rows = list.visible_rows(current_day_number()).len();
                    list.selected = match (rows, list.selected) {
                        (0, _) => None,
                        (_, Some(i)) if i >= rows => Some(rows - 1),
                        (_, Some(i)) => Some(i),
                        (_, None) => Some(0),
                    };
                }
            }
            Err(err) => self.handle_post_auth_error(err),
        }
        None
    }

    fn apply_create(
        &mut self,
        result: crate::client::ClientResult<contract::Task>,
    ) -> Option<Dispatch> {
        match result {
            Ok(_) => {
                if let Some(list) = self.tasks_pane_mut() {
                    list.adding = None;
                }
                // Chain a refresh so the new task is shown from a server response (#1).
                let Some(session) = self.session.clone() else {
                    self.set_screen_pending(None);
                    self.go_to_login();
                    return None;
                };
                Some(self.dispatch_screen(ClientRequest::ListTasks {
                    token: session.token,
                    profile_id: session.profile_id,
                    query: task_list_query(),
                }))
            }
            Err(err) => {
                self.set_screen_pending(None);
                self.handle_add_task_error(err);
                None
            }
        }
    }

    /// Fold a task update (edit / toggle-done / reopen) response. On success the task list is
    /// re-fetched from the server so the rendered state derives from a server response (#1); a
    /// blank-title rejection surfaces inline in the edit sub-flow, other errors on the list.
    fn apply_update(
        &mut self,
        result: crate::client::ClientResult<contract::Task>,
    ) -> Option<Dispatch> {
        match result {
            Ok(task) => {
                // A commit from the per-field detail view re-derives the open detail from the
                // server's returned task (#1) and stays in the view (clearing the edit buffer); an
                // edit-dialog or toggle commit clears the dialog. Both then refresh the list.
                if let Some(list) = self.tasks_pane_mut() {
                    list.editing = None;
                    if let Some(detail) = &mut list.detail {
                        detail.refresh_from(task);
                    }
                }
                self.refresh_tasks()
            }
            Err(err) => {
                self.set_screen_pending(None);
                self.handle_update_task_error(err);
                None
            }
        }
    }

    /// Fold a task delete response. On success the list is re-fetched (#1); errors surface on the
    /// list.
    fn apply_delete(&mut self, result: crate::client::ClientResult<()>) -> Option<Dispatch> {
        match result {
            Ok(()) => {
                if let Some(list) = self.tasks_pane_mut() {
                    list.confirming_delete = None;
                }
                self.refresh_tasks()
            }
            Err(err) => {
                self.set_screen_pending(None);
                self.handle_post_auth_error(err);
                None
            }
        }
    }

    /// Fold a per-task sub-task list (the detail view's "Sub-tasks" section load) into the open
    /// task detail, re-deriving its read-only sub-task section from the server (A6). Dropped if the
    /// detail closed before the response arrived.
    fn apply_task_subtasks(
        &mut self,
        result: crate::client::ClientResult<Vec<contract::Subtask>>,
    ) -> Option<Dispatch> {
        self.set_screen_pending(None);
        match result {
            Ok(subtasks) => {
                if let Some(list) = self.tasks_pane_mut()
                    && let Some(detail) = &mut list.detail
                {
                    detail.set_subtasks(subtasks);
                }
            }
            Err(err) => self.handle_post_auth_error(err),
        }
        None
    }

    /// Fold a create-sub-task response. On success the add-sub-task sub-flow closes and the Tasks
    /// tree is re-fetched from the server (#1); a blank-title (or other) error surfaces inline in
    /// the open form.
    fn apply_create_subtask(
        &mut self,
        result: crate::client::ClientResult<contract::Subtask>,
    ) -> Option<Dispatch> {
        match result {
            Ok(_) => {
                if let Some(list) = self.tasks_pane_mut() {
                    list.adding_subtask = None;
                }
                self.refresh_tasks()
            }
            Err(err) => {
                self.set_screen_pending(None);
                self.handle_add_subtask_error(err);
                None
            }
        }
    }

    /// Fold a sub-task update (edit-title / toggle) response. On success the edit sub-flow closes,
    /// the open detail's "Sub-tasks" section is refreshed if it owns the parent, and the Tasks tree
    /// is re-fetched (#1); a blank-title rejection surfaces inline in the edit sub-flow.
    fn apply_update_subtask(
        &mut self,
        result: crate::client::ClientResult<contract::Subtask>,
    ) -> Option<Dispatch> {
        match result {
            Ok(_) => {
                if let Some(list) = self.tasks_pane_mut() {
                    list.editing_subtask = None;
                }
                self.refresh_tasks()
            }
            Err(err) => {
                self.set_screen_pending(None);
                self.handle_update_subtask_error(err);
                None
            }
        }
    }

    /// Fold a sub-task delete response. On success the Tasks tree is re-fetched (#1); errors
    /// surface on the list. (No `tui` key deletes a sub-task today, but the fold keeps the surface
    /// complete and consistent with the task delete path.)
    fn apply_delete_subtask(
        &mut self,
        result: crate::client::ClientResult<()>,
    ) -> Option<Dispatch> {
        match result {
            Ok(()) => self.refresh_tasks(),
            Err(err) => {
                self.set_screen_pending(None);
                self.handle_post_auth_error(err);
                None
            }
        }
    }

    /// Error routing for a create-sub-task: offline → blocking, `unauthenticated` → login, and a
    /// validation (blank-title) or other error surfaces inline in the open add-sub-task form.
    fn handle_add_subtask_error(&mut self, err: ClientError) {
        if err.is_offline() {
            self.go_offline(&err);
            return;
        }
        if err.code() == Some(&ErrorCode::Unauthenticated) {
            self.go_to_login();
            return;
        }
        let message = err.to_string();
        if let Some(list) = self.tasks_pane_mut()
            && let Some(add) = &mut list.adding_subtask
        {
            add.error = Some(message);
        }
    }

    /// Error routing for a sub-task update: offline → blocking, `unauthenticated` → login. A
    /// validation (blank-title) error surfaces inline in the edit sub-flow if one is open;
    /// otherwise (a toggle with no sub-flow) it surfaces on the list.
    fn handle_update_subtask_error(&mut self, err: ClientError) {
        if err.is_offline() {
            self.go_offline(&err);
            return;
        }
        if err.code() == Some(&ErrorCode::Unauthenticated) {
            self.go_to_login();
            return;
        }
        let message = err.to_string();
        if let Some(list) = self.tasks_pane_mut() {
            if let Some(edit) = &mut list.editing_subtask {
                edit.error = Some(message);
            } else {
                list.message = Some(message);
            }
        }
    }

    /// Re-dispatch a task-list load after a successful mutation, chaining the in-flight marker
    /// forward. Returns to login if the session vanished.
    fn refresh_tasks(&mut self) -> Option<Dispatch> {
        let Some(session) = self.session.clone() else {
            self.set_screen_pending(None);
            self.go_to_login();
            return None;
        };
        Some(self.dispatch_screen(ClientRequest::ListTasks {
            token: session.token,
            profile_id: session.profile_id,
            query: task_list_query(),
        }))
    }

    fn apply_notes(
        &mut self,
        result: crate::client::ClientResult<Vec<contract::Note>>,
    ) -> Option<Dispatch> {
        match result {
            Ok(notes) => {
                let mut state = NotesState::new(notes);
                if let Some(main) = self.main_mut() {
                    // Preserve an in-progress create/edit sub-flow and the selected row across the
                    // refresh (transient UI state, #1).
                    let prev = &main.notes;
                    if matches!(
                        prev.mode,
                        NotesMode::Creating(_) | NotesMode::Editing { .. } | NotesMode::Detail(_)
                    ) {
                        state.mode = prev.mode.clone();
                    }
                    if prev.selected.is_some() && state.selected.is_some() {
                        state.selected = prev.selected;
                    }
                    main.notes = state;
                }
            }
            Err(err) => {
                self.set_screen_pending(None);
                self.handle_post_auth_error(err);
            }
        }
        None
    }

    fn apply_create_note(
        &mut self,
        result: crate::client::ClientResult<contract::Note>,
    ) -> Option<Dispatch> {
        match result {
            Ok(_) => {
                if let Some(notes) = self.notes_pane_mut() {
                    notes.mode = NotesMode::List;
                }
                // Chain a refresh so the new note is shown from a server response (#1).
                let Some(session) = self.session.clone() else {
                    self.set_screen_pending(None);
                    self.go_to_login();
                    return None;
                };
                Some(self.dispatch_screen(ClientRequest::ListNotes {
                    token: session.token,
                    profile_id: session.profile_id,
                }))
            }
            Err(err) => {
                self.set_screen_pending(None);
                self.handle_note_form_error(err);
                None
            }
        }
    }

    fn apply_get_note(
        &mut self,
        result: crate::client::ClientResult<contract::Note>,
    ) -> Option<Dispatch> {
        self.set_screen_pending(None);
        match result {
            Ok(note) => {
                if let Some(notes) = self.notes_pane_mut() {
                    notes.mode = NotesMode::Detail(NoteDetail::new(note));
                }
            }
            Err(err) => self.handle_post_auth_error(err),
        }
        None
    }

    fn apply_update_note(
        &mut self,
        result: crate::client::ClientResult<contract::Note>,
    ) -> Option<Dispatch> {
        match result {
            Ok(note) => {
                // A commit from the per-field detail view re-derives the open detail from the
                // server's returned note (#1) and stays in the view (clearing the edit buffer); a
                // commit from the legacy edit dialog returns to the list. Both then refresh the list
                // so it reflects the change from a server response.
                if let Some(notes) = self.notes_pane_mut() {
                    match &mut notes.mode {
                        NotesMode::Detail(detail) => detail.refresh_from(note),
                        _ => notes.mode = NotesMode::List,
                    }
                }
                // Chain a refresh so the edited note is shown from a server response (#1).
                let Some(session) = self.session.clone() else {
                    self.set_screen_pending(None);
                    self.go_to_login();
                    return None;
                };
                Some(self.dispatch_screen(ClientRequest::ListNotes {
                    token: session.token,
                    profile_id: session.profile_id,
                }))
            }
            Err(err) => {
                self.set_screen_pending(None);
                self.handle_note_form_error(err);
                None
            }
        }
    }

    fn apply_delete_note(&mut self, result: crate::client::ClientResult<()>) -> Option<Dispatch> {
        match result {
            Ok(()) => {
                if let Some(notes) = self.notes_pane_mut() {
                    notes.mode = NotesMode::List;
                }
                // Chain a refresh so the list reflects the deletion from a server response (#1).
                let Some(session) = self.session.clone() else {
                    self.set_screen_pending(None);
                    self.go_to_login();
                    return None;
                };
                Some(self.dispatch_screen(ClientRequest::ListNotes {
                    token: session.token,
                    profile_id: session.profile_id,
                }))
            }
            Err(err) => {
                self.set_screen_pending(None);
                self.handle_post_auth_error(err);
                None
            }
        }
    }

    /// Error routing for a note create/update: offline → blocking, `unauthenticated` → login, and
    /// a validation (or other) error surfaces inline in the open create/edit form so the user can
    /// correct it (mirror of [`Self::handle_add_task_error`]).
    fn handle_note_form_error(&mut self, err: ClientError) {
        if err.is_offline() {
            self.go_offline(&err);
            return;
        }
        if err.code() == Some(&ErrorCode::Unauthenticated) {
            self.go_to_login();
            return;
        }
        let message = err.to_string();
        if let Some(notes) = self.notes_pane_mut() {
            match &mut notes.mode {
                NotesMode::Creating(form) | NotesMode::Editing { form, .. } => {
                    form.error = Some(message);
                }
                _ => notes.message = Some(message),
            }
        }
    }

    /// Fold a timer-config response into the global timer. On the initial read (`chain_session`),
    /// chains a `GetTimerSession` so the session state loads too; on a duration update it stores
    /// the new config and closes the edit sub-flow.
    fn apply_timer_config(
        &mut self,
        result: crate::client::ClientResult<contract::TimerConfig>,
        chain_session: bool,
    ) -> Option<Dispatch> {
        match result {
            Ok(config) => {
                self.timer.config = config;
                self.timer.editing = None;
                if chain_session && let Some(session) = self.session.clone() {
                    let request = ClientRequest::GetTimerSession {
                        token: session.token,
                    };
                    return Some(self.dispatch_timer(request));
                }
                self.timer.pending = None;
                None
            }
            Err(err) => {
                self.timer.pending = None;
                self.handle_timer_config_error(err);
                None
            }
        }
    }

    /// Fold a timer-session response (get / start / stop) into the global timer, capturing the
    /// monotonic instant the session was applied so the rendered countdown advances between coarse
    /// refreshes (ADR-0002 §3). No remaining-seconds integer is stored (#1).
    ///
    /// Detects the **Running→Completed edge** before overwriting the session, so a single
    /// completion notification can be fired once per session (Decision 4). The decision is recorded
    /// as a pure one-shot signal on the timer (`notify_pending`); the effect itself happens at the
    /// edge via the injected [`Notifier`](crate::client::Notifier) (Decision 3).
    fn apply_timer_session(
        &mut self,
        result: crate::client::ClientResult<contract::TimerSession>,
    ) -> Option<Dispatch> {
        self.timer.pending = None;
        match result {
            Ok(session) => {
                self.detect_completion_edge(&session);
                self.timer.session = session;
                self.timer.applied_at = Some(std::time::Instant::now());
            }
            Err(err) => self.handle_timer_error(err),
        }
        None
    }

    /// Apply the fire-once arm/fire/re-arm rules for the completion notification, comparing the
    /// just-applied state (`self.timer`) against the incoming `new` session, **before** the new
    /// session is stored (Decision 4):
    ///
    /// - A new `Completed` while the guard is un-fired is the completion edge: set `notify_pending`
    ///   and arm the guard so subsequent `Completed` re-pulls do nothing — **except** when this is
    ///   the first session fold after a load/reset, where we only **arm** the guard (no emit), so a
    ///   stale completion is never replayed at launch (Assumption A4).
    /// - A new `Running` (a fresh start) or `Idle` (stop/reset) re-arms the guard for the next
    ///   completion.
    fn detect_completion_edge(&mut self, new: &contract::TimerSession) {
        use contract::TimerSession;
        match new {
            TimerSession::Completed { .. } => {
                if !self.timer.notified_for_session {
                    // Arm the guard either way; only emit when this is a real Running→Completed
                    // edge, not the initial fold of an already-completed session (A4).
                    let is_initial_load = self.timer.applied_at.is_none();
                    self.timer.notified_for_session = true;
                    if !is_initial_load {
                        self.timer.notify_pending = true;
                    }
                }
            }
            // A fresh Running (new start) or Idle (stop/reset) re-arms for the next completion.
            TimerSession::Running { .. } | TimerSession::Idle => {
                self.timer.notified_for_session = false;
            }
        }
    }

    /// Consume the one-shot completion-notification signal: returns the fixed notification copy
    /// exactly once after a Running→Completed edge, then clears the signal. The edge thread (the
    /// poll loop) calls this after folding a response and fires the injected
    /// [`Notifier`](crate::client::Notifier) if it returns `Some` (Decision 3).
    pub fn take_pending_notification(&mut self) -> Option<TimerNotification> {
        if self.timer.notify_pending {
            self.timer.notify_pending = false;
            Some(TimerNotification {
                title: TIMER_COMPLETE_TITLE,
                body: TIMER_COMPLETE_BODY,
            })
        } else {
            None
        }
    }

    /// Error routing for a timer-session call (ADR-0006 §6): offline → blocking screen,
    /// `unauthenticated` → login, anything else → inline message on the timer widget.
    fn handle_timer_error(&mut self, err: ClientError) {
        if err.is_offline() {
            self.go_offline(&err);
            return;
        }
        if err.code() == Some(&ErrorCode::Unauthenticated) {
            self.go_to_login();
            return;
        }
        self.timer.message = Some(err.to_string());
    }

    /// Error routing for a duration update: offline → blocking, `unauthenticated` → login, and a
    /// validation (or other) error surfaces inline in the edit sub-flow so the user can correct it.
    fn handle_timer_config_error(&mut self, err: ClientError) {
        if err.is_offline() {
            self.go_offline(&err);
            return;
        }
        if err.code() == Some(&ErrorCode::Unauthenticated) {
            self.go_to_login();
            return;
        }
        if let Some(edit) = &mut self.timer.editing {
            edit.error = Some(err.to_string());
        } else {
            self.timer.message = Some(err.to_string());
        }
    }

    fn handle_auth_error(&mut self, err: ClientError) {
        if err.is_offline() {
            self.go_offline(&err);
            return;
        }
        if let Screen::Auth(auth) = &mut self.screen {
            auth.error = Some(err.to_string());
        }
    }

    /// Map an error encountered after authentication: an `unauthenticated` code returns to
    /// login; offline goes to the blocking screen; anything else surfaces inline on the task
    /// list (or, if we are mid-auth, on the auth screen).
    fn handle_post_auth_error(&mut self, err: ClientError) {
        if err.is_offline() {
            self.go_offline(&err);
            return;
        }
        if err.code() == Some(&ErrorCode::Unauthenticated) {
            self.go_to_login();
            return;
        }
        let message = err.to_string();
        match &mut self.screen {
            // Surface on the active pane of the tabbed view.
            Screen::Main(main) => match main.active_tab {
                Tab::Tasks => main.tasks.message = Some(message),
                Tab::Notes => main.notes.message = Some(message),
                Tab::Profiles => main.profiles.message = Some(message),
            },
            Screen::Auth(auth) => auth.error = Some(message),
            Screen::Offline { .. } => {}
        }
    }

    fn handle_add_task_error(&mut self, err: ClientError) {
        if err.is_offline() {
            self.go_offline(&err);
            return;
        }
        if err.code() == Some(&ErrorCode::Unauthenticated) {
            self.go_to_login();
            return;
        }
        let message = err.to_string();
        if let Some(list) = self.tasks_pane_mut()
            && let Some(add) = &mut list.adding
        {
            add.error = Some(message);
        }
    }

    /// Map an error from a task update: offline → blocking, `unauthenticated` → login. A
    /// validation (blank-title) error surfaces inline in the edit sub-flow if one is open;
    /// otherwise (a toggle/reopen with no sub-flow) it surfaces on the list.
    fn handle_update_task_error(&mut self, err: ClientError) {
        if err.is_offline() {
            self.go_offline(&err);
            return;
        }
        if err.code() == Some(&ErrorCode::Unauthenticated) {
            self.go_to_login();
            return;
        }
        let message = err.to_string();
        if let Some(list) = self.tasks_pane_mut() {
            if let Some(edit) = &mut list.editing {
                edit.error = Some(message);
            } else {
                list.message = Some(message);
            }
        }
    }

    fn go_offline(&mut self, err: &ClientError) {
        self.screen = Screen::Offline {
            message: err.to_string(),
            pending: None,
        };
    }

    fn go_to_login(&mut self) {
        // Expiry / unauthenticated drops the in-memory session and the timer state, returning to
        // login; the next login re-loads the account-global timer afresh.
        self.session = None;
        self.timer.reset();
        self.help_open = false;
        let mut auth = AuthState::new();
        auth.error = Some("session expired — please log in again".to_owned());
        self.screen = Screen::Auth(auth);
    }
}
