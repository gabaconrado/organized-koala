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

pub mod auth;
pub mod notes;
pub mod protocol;
pub mod task_add;
pub mod task_list;
pub mod timer;

pub use auth::{AuthField, AuthMode, AuthState};
pub use notes::{NoteForm, NotesMode, NotesState};
pub use protocol::{ClientRequest, ClientResponse, Outcome, RequestId};
pub use task_add::{AddTaskState, EditTaskState};
pub use task_list::TaskListState;
pub use timer::{DurationEditState, Timer};

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
    /// Toggle the auth screen between login and register modes.
    ToggleAuthMode,
    /// Begin the add-task input flow (task list only).
    BeginAddTask,
    /// Begin editing the selected task's title/description (task list only).
    BeginEditTask,
    /// Toggle the selected task between done and open (task list only): a done task is reopened,
    /// an open task is marked done.
    ToggleDone,
    /// Begin (or, when already armed, confirm) deletion of the selected task (task list only). A
    /// two-step confirm affordance: the first press arms, a second confirms.
    DeleteSelected,
    /// Open the notes view for the active profile (task list only).
    OpenNotes,
    /// Return from the notes view to the task list (notes list, when idle).
    Back,
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
    /// Cancel the current sub-flow (e.g. abandon the add-task input) or abandon an in-flight
    /// request.
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

/// The session held in memory for the process lifetime: the JWT and the active profile id.
/// Never persisted (hard-constraint #1).
#[derive(Debug, Clone)]
pub struct Session {
    /// The bearer token returned by register/login.
    pub token: String,
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
    /// The task-list screen for the active profile.
    TaskList(TaskListState),
    /// The notes screen for the active profile.
    Notes(NotesState),
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
        matches!(self, Screen::TaskList(_) | Screen::Notes(_))
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

    /// The id of the request currently awaited on the active screen, if any.
    #[must_use]
    fn screen_pending_id(&self) -> Option<RequestId> {
        match &self.screen {
            Screen::Auth(auth) => auth.pending,
            Screen::TaskList(list) => list.pending,
            Screen::Notes(notes) => notes.pending,
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

    /// Dispatch a non-edit event to the active screen, stamping the screen's in-flight marker.
    fn handle_screen_event(&mut self, event: Event) -> Option<Dispatch> {
        // Cross-screen navigation between the two post-auth views (Assumption A7). Both issue a
        // fresh server load so the destination derives from a response (#1).
        match (&self.screen, &event) {
            (Screen::TaskList(list), Event::OpenNotes) if list.adding.is_none() => {
                return self.navigate_to_notes();
            }
            (Screen::Notes(notes), Event::Back) if !notes.in_sub_flow() => {
                return self.navigate_to_tasks();
            }
            _ => {}
        }
        let request = match &mut self.screen {
            Screen::Auth(auth) => auth.handle_event(event),
            Screen::TaskList(list) => list.handle_event(event, self.session.as_ref()),
            Screen::Notes(notes) => notes.handle_event(event, self.session.as_ref()),
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

    /// Switch to the notes view and dispatch its initial list load. The screen becomes an empty
    /// `Notes` placeholder carrying the in-flight marker; `apply_response` replaces it with the
    /// populated list (so it derives entirely from the server response, #1).
    fn navigate_to_notes(&mut self) -> Option<Dispatch> {
        let session = self.session.clone()?;
        self.screen = Screen::Notes(NotesState::new(Vec::new()));
        Some(self.dispatch_screen(ClientRequest::ListNotes {
            token: session.token,
            profile_id: session.profile_id,
        }))
    }

    /// Switch back to the task list and dispatch its initial list load (mirror of
    /// [`Self::navigate_to_notes`]).
    fn navigate_to_tasks(&mut self) -> Option<Dispatch> {
        let session = self.session.clone()?;
        self.screen = Screen::TaskList(TaskListState::new(Vec::new()));
        Some(self.dispatch_screen(ClientRequest::ListTasks {
            token: session.token,
            profile_id: session.profile_id,
        }))
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
            Screen::TaskList(list) => list.pending = id,
            Screen::Notes(notes) => notes.pending = id,
            Screen::Offline { pending, .. } => *pending = id,
        }
    }

    /// Abandon the in-flight request on the active screen (user pressed cancel): clear the marker
    /// so the screen is interactive again. The worker still runs the abandoned request to
    /// completion, but its response will be dropped by [`apply_response`] on id mismatch.
    fn cancel_in_flight(&mut self) {
        match &mut self.screen {
            Screen::Auth(auth) => auth.pending = None,
            Screen::TaskList(list) => {
                list.pending = None;
                if let Some(add) = &mut list.adding {
                    add.error = None;
                }
                if let Some(edit) = &mut list.editing {
                    edit.error = None;
                }
            }
            Screen::Notes(notes) => {
                notes.pending = None;
                match &mut notes.mode {
                    NotesMode::Creating(form) | NotesMode::Editing { form, .. } => {
                        form.error = None;
                    }
                    _ => {}
                }
            }
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
                    Outcome::ListTasks(result) => self.apply_tasks(result),
                    Outcome::CreateTask(result) => self.apply_create(result),
                    Outcome::UpdateTask(result) => self.apply_update(result),
                    Outcome::DeleteTask(result) => self.apply_delete(result),
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
                let token = session.token;
                Some(self.dispatch_screen(ClientRequest::ListProfiles { token }))
            }
            Err(err) => {
                self.set_screen_pending(None);
                self.handle_auth_error(err);
                None
            }
        }
    }

    fn apply_profiles(
        &mut self,
        token: String,
        result: crate::client::ClientResult<Vec<contract::Profile>>,
    ) -> Option<Dispatch> {
        match result {
            Ok(profiles) => {
                let Some(profile) = profiles.into_iter().next() else {
                    self.set_screen_pending(None);
                    if let Screen::Auth(auth) = &mut self.screen {
                        auth.error = Some("account has no profile".to_owned());
                    }
                    return None;
                };
                self.session = Some(Session {
                    token: token.clone(),
                    profile_id: profile.id.clone(),
                    profile_name: profile.name,
                });
                // The auth screen still holds the in-flight marker; carry it forward by
                // re-dispatching the task load (the new id replaces it on the auth screen until
                // the task list materialises in `apply_tasks`).
                Some(self.dispatch_screen(ClientRequest::ListTasks {
                    token,
                    profile_id: profile.id,
                }))
            }
            Err(err) => {
                self.set_screen_pending(None);
                self.handle_post_auth_error(err);
                None
            }
        }
    }

    fn apply_tasks(
        &mut self,
        result: crate::client::ClientResult<Vec<contract::Task>>,
    ) -> Option<Dispatch> {
        match result {
            Ok(tasks) => {
                let preserved = if let Screen::TaskList(list) = &self.screen {
                    list.adding.clone()
                } else {
                    None
                };
                let mut state = TaskListState::new(tasks);
                state.adding = preserved;
                self.screen = Screen::TaskList(state);
            }
            Err(err) => {
                self.set_screen_pending(None);
                self.handle_post_auth_error(err);
            }
        }
        None
    }

    fn apply_create(
        &mut self,
        result: crate::client::ClientResult<contract::Task>,
    ) -> Option<Dispatch> {
        match result {
            Ok(_) => {
                if let Screen::TaskList(list) = &mut self.screen {
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
            Ok(_) => {
                if let Screen::TaskList(list) = &mut self.screen {
                    list.editing = None;
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
                if let Screen::TaskList(list) = &mut self.screen {
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
        }))
    }

    fn apply_notes(
        &mut self,
        result: crate::client::ClientResult<Vec<contract::Note>>,
    ) -> Option<Dispatch> {
        match result {
            Ok(notes) => {
                // Preserve an in-progress create/edit sub-flow across a refresh (mirror of
                // `apply_tasks` preserving the add sub-flow).
                let preserved = if let Screen::Notes(state) = &self.screen {
                    Some(state.mode.clone())
                } else {
                    None
                };
                let mut state = NotesState::new(notes);
                if let Some(mode) = preserved
                    && matches!(mode, NotesMode::Creating(_) | NotesMode::Editing { .. })
                {
                    state.mode = mode;
                }
                self.screen = Screen::Notes(state);
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
                if let Screen::Notes(notes) = &mut self.screen {
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
                if let Screen::Notes(notes) = &mut self.screen {
                    notes.mode = NotesMode::Viewing(note);
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
            Ok(_) => {
                if let Screen::Notes(notes) = &mut self.screen {
                    notes.mode = NotesMode::List;
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
                if let Screen::Notes(notes) = &mut self.screen {
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
        if let Screen::Notes(notes) = &mut self.screen {
            match &mut notes.mode {
                NotesMode::Creating(form) | NotesMode::Editing { form, .. } => {
                    form.error = Some(err.to_string());
                }
                _ => notes.message = Some(err.to_string()),
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
    fn apply_timer_session(
        &mut self,
        result: crate::client::ClientResult<contract::TimerSession>,
    ) -> Option<Dispatch> {
        self.timer.pending = None;
        match result {
            Ok(session) => {
                self.timer.session = session;
                self.timer.applied_at = Some(std::time::Instant::now());
            }
            Err(err) => self.handle_timer_error(err),
        }
        None
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
            Screen::TaskList(list) => list.message = Some(message),
            Screen::Notes(notes) => notes.message = Some(message),
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
        if let Screen::TaskList(list) = &mut self.screen
            && let Some(add) = &mut list.adding
        {
            add.error = Some(err.to_string());
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
        if let Screen::TaskList(list) = &mut self.screen {
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
        let mut auth = AuthState::new();
        auth.error = Some("session expired — please log in again".to_owned());
        self.screen = Screen::Auth(auth);
    }
}
