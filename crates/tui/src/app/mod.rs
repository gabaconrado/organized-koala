//! The app core: a screen state machine advanced by two pure update functions over [`Event`]s
//! and server [`ClientResponse`]s.
//!
//! [`App`] owns the session ([`Session`]) and the current [`Screen`]. It performs **no** I/O and
//! holds **no** client: [`App::handle_event`] is pure and returns an [`Option<Dispatch>`]
//! describing the [`ClientRequest`] to execute (the worker thread runs it), and
//! [`App::apply_response`] folds a completed [`ClientResponse`] back into screen state. The whole
//! interactive surface is driveable through a `ratatui` `TestBackend` with no client and no
//! threads (ADR-0003 / ADR-0006). All state lives in memory for the process lifetime only
//! (hard-constraint #1) — there is no on-disk or cross-run persistence.

pub mod auth;
pub mod protocol;
pub mod task_add;
pub mod task_list;

pub use auth::{AuthField, AuthMode, AuthState};
pub use protocol::{ClientRequest, ClientResponse, Outcome, RequestId};
pub use task_add::AddTaskState;
pub use task_list::TaskListState;

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
    /// Close / mark-done the selected task (task list only).
    CloseSelected,
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
/// stamped it with (and recorded as the active screen's in-flight marker). The edge ships this
/// to the worker thread verbatim.
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

/// The current screen of the state machine.
#[derive(Debug, Clone)]
pub enum Screen {
    /// The auth screen (login or register).
    Auth(AuthState),
    /// The task-list screen for the active profile.
    TaskList(TaskListState),
    /// The blocking "server unreachable" screen. Carries the message and the in-flight marker
    /// while a retry probe is outstanding.
    Offline {
        /// Human-readable description of the connectivity failure.
        message: String,
        /// The in-flight request id while a retry health-probe is outstanding; `None` when idle.
        pending: Option<RequestId>,
    },
}

/// The application: the screen state machine plus the in-memory session and the request-id
/// counter.
///
/// Advance it by feeding [`Event`]s to [`App::handle_event`] (which may return a [`Dispatch`] to
/// run) and completed results to [`App::apply_response`]; render the current state with the
/// [`crate::ui`] draw functions. [`App::should_quit`] reports when a quit was requested.
#[derive(Debug)]
pub struct App {
    session: Option<Session>,
    screen: Screen,
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
            quit: false,
            next_id: 0,
        }
    }

    /// The current screen, for rendering.
    #[must_use]
    pub fn screen(&self) -> &Screen {
        &self.screen
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

    /// Whether a server request is currently outstanding (the active screen's in-flight marker
    /// is set). While true the UI renders a spinner and request-triggering events are no-ops.
    #[must_use]
    pub fn is_pending(&self) -> bool {
        self.pending_id().is_some()
    }

    /// The id of the currently-awaited request, if any.
    #[must_use]
    fn pending_id(&self) -> Option<RequestId> {
        match &self.screen {
            Screen::Auth(auth) => auth.pending,
            Screen::TaskList(list) => list.pending,
            Screen::Offline { pending, .. } => *pending,
        }
    }

    /// The pure update entry point: apply one [`Event`] to the current screen, returning a
    /// [`Dispatch`] if the event triggers a server request.
    ///
    /// This is purely a state transition (no I/O, no client). The same event on the same state
    /// always produces the same `(next state, Option<Dispatch>)`, so the whole interactive
    /// surface is driveable from tests with no client and no threads. At most one request is in
    /// flight: a request-triggering event while a request is outstanding is a no-op. `Quit` and
    /// `Cancel` stay live during a request.
    pub fn handle_event(&mut self, event: Event) -> Option<Dispatch> {
        match event {
            Event::Quit => {
                self.quit = true;
                return None;
            }
            Event::Cancel if self.is_pending() => {
                self.cancel_in_flight();
                return None;
            }
            _ => {}
        }
        let request = match &mut self.screen {
            Screen::Auth(auth) => auth.handle_event(event),
            Screen::TaskList(list) => list.handle_event(event, self.session.as_ref()),
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
        request.map(|request| self.dispatch(request))
    }

    /// Stamp a request with a fresh id, record it as the active screen's in-flight marker, and
    /// return the [`Dispatch`] for the edge to run.
    fn dispatch(&mut self, request: ClientRequest) -> Dispatch {
        let id = RequestId(self.next_id);
        self.next_id = self.next_id.wrapping_add(1);
        self.set_pending(Some(id));
        Dispatch { id, request }
    }

    fn set_pending(&mut self, id: Option<RequestId>) {
        match &mut self.screen {
            Screen::Auth(auth) => auth.pending = id,
            Screen::TaskList(list) => list.pending = id,
            Screen::Offline { pending, .. } => *pending = id,
        }
    }

    /// Abandon the in-flight request (user pressed cancel): clear the marker so the screen is
    /// interactive again. The worker still runs the abandoned request to completion, but its
    /// response will be dropped by [`apply_response`] on id mismatch.
    fn cancel_in_flight(&mut self) {
        match &mut self.screen {
            Screen::Auth(auth) => auth.pending = None,
            Screen::TaskList(list) => {
                list.pending = None;
                if let Some(add) = &mut list.adding {
                    add.error = None;
                }
            }
            Screen::Offline { pending, .. } => *pending = None,
        }
    }

    /// The pure response-folding seam: apply a completed [`ClientResponse`] to the in-flight
    /// state, running the same success / error-code branching the inline code ran pre-split.
    ///
    /// A response whose id does not match the currently-awaited request is **dropped** (it was
    /// cancelled or superseded). Returns a follow-up [`Dispatch`] when the response chains into
    /// the next request (post-auth profile/task load, or a refresh after create).
    pub fn apply_response(&mut self, response: ClientResponse) -> Option<Dispatch> {
        if self.pending_id() != Some(response.id) {
            // Stale: the request was cancelled or superseded — never mutate state.
            return None;
        }
        match response.outcome {
            Outcome::Health(result) => self.apply_health(result),
            Outcome::Register(result) | Outcome::Login(result) => self.apply_auth(result),
            Outcome::ListProfiles { token, result } => self.apply_profiles(token, result),
            Outcome::ListTasks(result) => self.apply_tasks(result),
            Outcome::CreateTask(result) => self.apply_create(result),
            Outcome::CloseTask(result) => self.apply_close(result),
        }
    }

    fn apply_health(&mut self, result: crate::client::ClientResult<()>) -> Option<Dispatch> {
        self.set_pending(None);
        match result {
            Ok(()) => {
                if let Some(session) = self.session.clone() {
                    Some(self.dispatch(ClientRequest::ListTasks {
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
                Some(self.dispatch(ClientRequest::ListProfiles { token }))
            }
            Err(err) => {
                self.set_pending(None);
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
                    self.set_pending(None);
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
                Some(self.dispatch(ClientRequest::ListTasks {
                    token,
                    profile_id: profile.id,
                }))
            }
            Err(err) => {
                self.set_pending(None);
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
                self.set_pending(None);
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
                    self.set_pending(None);
                    self.go_to_login();
                    return None;
                };
                Some(self.dispatch(ClientRequest::ListTasks {
                    token: session.token,
                    profile_id: session.profile_id,
                }))
            }
            Err(err) => {
                self.set_pending(None);
                self.handle_add_task_error(err);
                None
            }
        }
    }

    fn apply_close(
        &mut self,
        result: crate::client::ClientResult<contract::Task>,
    ) -> Option<Dispatch> {
        self.set_pending(None);
        match result {
            Ok(updated) => {
                if let Screen::TaskList(list) = &mut self.screen
                    && let Some(slot) = list.tasks.iter_mut().find(|t| t.id == updated.id)
                {
                    *slot = updated;
                }
            }
            Err(err) => self.handle_post_auth_error(err),
        }
        None
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

    fn go_offline(&mut self, err: &ClientError) {
        self.screen = Screen::Offline {
            message: err.to_string(),
            pending: None,
        };
    }

    fn go_to_login(&mut self) {
        // Expiry / unauthenticated drops the in-memory session and returns to login.
        self.session = None;
        let mut auth = AuthState::new();
        auth.error = Some("session expired — please log in again".to_owned());
        self.screen = Screen::Auth(auth);
    }
}
