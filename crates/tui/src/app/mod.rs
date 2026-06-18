//! The app core: a screen state machine advanced by pure update functions over [`Event`]s.
//!
//! [`App`] owns the session ([`Session`]) and the current [`Screen`], and holds the injected
//! [`Client`] used to talk to the server. It performs no terminal I/O and holds no terminal
//! or transport types, so the whole interactive surface can be driven through a `ratatui`
//! `TestBackend` with a fake client (ADR-0003). All state lives in memory for the process
//! lifetime only (hard-constraint #1) — there is no on-disk or cross-run persistence.

use contract::{CreateTaskRequest, ErrorCode, LoginRequest, Password, RegisterRequest, Task};

use crate::client::{Client, ClientError};

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
    /// Cancel the current sub-flow (e.g. abandon the add-task input).
    Cancel,
    /// Request to quit the application.
    Quit,
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

/// Which auth form is showing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthMode {
    /// Login form: identifier + password.
    Login,
    /// Register form: username + email + password + profile name.
    Register,
}

/// The focused field within the auth form. The variants double as the navigation order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthField {
    /// Login identifier (username or email).
    Identifier,
    /// Registration username.
    Username,
    /// Registration email.
    Email,
    /// Password (both forms).
    Password,
    /// Registration default-profile name.
    ProfileName,
}

/// State of the auth screen (login or register), including form fields and any inline error.
#[derive(Debug, Clone)]
pub struct AuthState {
    /// Whether the login or register form is active.
    pub mode: AuthMode,
    /// The currently focused field.
    pub focus: AuthField,
    /// Login identifier input.
    pub identifier: String,
    /// Register username input.
    pub username: String,
    /// Register email input.
    pub email: String,
    /// Password input (rendered masked).
    pub password: String,
    /// Register profile-name input.
    pub profile_name: String,
    /// Inline error message (e.g. validation or invalid credentials), if any.
    pub error: Option<String>,
}

impl AuthState {
    fn new() -> Self {
        Self {
            mode: AuthMode::Login,
            focus: AuthField::Identifier,
            identifier: String::new(),
            username: String::new(),
            email: String::new(),
            password: String::new(),
            profile_name: String::new(),
            error: None,
        }
    }

    /// The fields shown in the current mode, in navigation order.
    fn fields(&self) -> &'static [AuthField] {
        match self.mode {
            AuthMode::Login => &[AuthField::Identifier, AuthField::Password],
            AuthMode::Register => &[
                AuthField::Username,
                AuthField::Email,
                AuthField::Password,
                AuthField::ProfileName,
            ],
        }
    }

    fn field_mut(&mut self, field: AuthField) -> &mut String {
        match field {
            AuthField::Identifier => &mut self.identifier,
            AuthField::Username => &mut self.username,
            AuthField::Email => &mut self.email,
            AuthField::Password => &mut self.password,
            AuthField::ProfileName => &mut self.profile_name,
        }
    }

    fn move_focus(&mut self, forward: bool) {
        let fields = self.fields();
        let current = fields.iter().position(|f| *f == self.focus).unwrap_or(0);
        let len = fields.len();
        let next = if forward {
            (current + 1) % len
        } else {
            (current + len - 1) % len
        };
        if let Some(field) = fields.get(next) {
            self.focus = *field;
        }
    }

    fn toggle_mode(&mut self) {
        self.error = None;
        self.mode = match self.mode {
            AuthMode::Login => {
                self.focus = AuthField::Username;
                AuthMode::Register
            }
            AuthMode::Register => {
                self.focus = AuthField::Identifier;
                AuthMode::Login
            }
        };
    }
}

/// The add-task sub-flow: which field is focused and the entered title/description.
#[derive(Debug, Clone)]
pub struct AddTaskState {
    /// Whether the title (`true`) or description field is focused.
    pub on_title: bool,
    /// Entered task title.
    pub title: String,
    /// Entered task description.
    pub description: String,
    /// Inline error (e.g. empty title rejected by the server), if any.
    pub error: Option<String>,
}

impl AddTaskState {
    fn new() -> Self {
        Self {
            on_title: true,
            title: String::new(),
            description: String::new(),
            error: None,
        }
    }
}

/// State of the task-list screen for the active profile.
#[derive(Debug, Clone)]
pub struct TaskListState {
    /// Tasks as returned by the server, newest-first.
    pub tasks: Vec<Task>,
    /// Index of the selected task in `tasks`, if any.
    pub selected: Option<usize>,
    /// Active add-task sub-flow, if open.
    pub adding: Option<AddTaskState>,
    /// A transient status/error message shown to the user, if any.
    pub message: Option<String>,
}

impl TaskListState {
    fn new(tasks: Vec<Task>) -> Self {
        let selected = if tasks.is_empty() { None } else { Some(0) };
        Self {
            tasks,
            selected,
            adding: None,
            message: None,
        }
    }

    fn move_selection(&mut self, forward: bool) {
        let len = self.tasks.len();
        if len == 0 {
            self.selected = None;
            return;
        }
        let current = self.selected.unwrap_or(0);
        let next = if forward {
            (current + 1) % len
        } else {
            (current + len - 1) % len
        };
        self.selected = Some(next);
    }
}

/// The current screen of the state machine.
#[derive(Debug, Clone)]
pub enum Screen {
    /// The auth screen (login or register).
    Auth(AuthState),
    /// The task-list screen for the active profile.
    TaskList(TaskListState),
    /// The blocking "server unreachable" screen. Carries the message and the screen to
    /// return to once a retry succeeds.
    Offline {
        /// Human-readable description of the connectivity failure.
        message: String,
    },
}

/// The application: the screen state machine plus the injected client and in-memory session.
///
/// Advance it by feeding [`Event`]s to [`App::handle_event`]; render the current state with
/// the [`crate::ui`] draw functions. [`App::should_quit`] reports when a quit was requested.
#[derive(Debug)]
pub struct App<C: Client> {
    client: C,
    session: Option<Session>,
    screen: Screen,
    quit: bool,
}

impl<C: Client> App<C> {
    /// Creates a new app on the auth (login) screen with the given client.
    pub fn new(client: C) -> Self {
        Self {
            client,
            session: None,
            screen: Screen::Auth(AuthState::new()),
            quit: false,
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

    /// The single update entry point: apply one [`Event`] to the current screen.
    ///
    /// This is the pure-update seam (modulo the injected client's I/O): the same event on the
    /// same state always produces the same transition, so the whole interactive surface is
    /// driveable from tests with a fake client.
    pub fn handle_event(&mut self, event: Event) {
        if matches!(event, Event::Quit) {
            self.quit = true;
            return;
        }
        match &mut self.screen {
            Screen::Auth(_) => self.handle_auth_event(event),
            Screen::TaskList(_) => self.handle_task_list_event(event),
            Screen::Offline { .. } => self.handle_offline_event(event),
        }
    }

    fn handle_auth_event(&mut self, event: Event) {
        let Screen::Auth(auth) = &mut self.screen else {
            return;
        };
        match event {
            Event::Char(c) => auth.field_mut(auth.focus).push(c),
            Event::Backspace => {
                let _ = auth.field_mut(auth.focus).pop();
            }
            Event::Next => auth.move_focus(true),
            Event::Prev => auth.move_focus(false),
            Event::ToggleAuthMode => auth.toggle_mode(),
            Event::Submit => self.submit_auth(),
            _ => {}
        }
    }

    fn submit_auth(&mut self) {
        let Screen::Auth(auth) = &mut self.screen else {
            return;
        };
        let result = match auth.mode {
            AuthMode::Login => {
                let req = LoginRequest {
                    identifier: auth.identifier.trim().to_owned(),
                    password: Password::new(auth.password.clone()),
                };
                self.client.login(&req)
            }
            AuthMode::Register => {
                let req = RegisterRequest {
                    username: auth.username.trim().to_owned(),
                    email: auth.email.trim().to_owned(),
                    password: Password::new(auth.password.clone()),
                    profile_name: auth.profile_name.trim().to_owned(),
                };
                self.client.register(&req)
            }
        };
        match result {
            Ok(session) => self.enter_app(session.token),
            Err(err) => self.handle_auth_error(err),
        }
    }

    fn handle_auth_error(&mut self, err: ClientError) {
        if err.is_offline() {
            self.go_offline(err);
            return;
        }
        if let Screen::Auth(auth) = &mut self.screen {
            auth.error = Some(err.to_string());
        }
    }

    /// After a successful auth, fetch the profiles and auto-select the first (this slice's
    /// accounts have exactly one), then enter its task list.
    fn enter_app(&mut self, token: String) {
        match self.client.list_profiles(&token) {
            Ok(profiles) => {
                let Some(profile) = profiles.into_iter().next() else {
                    if let Screen::Auth(auth) = &mut self.screen {
                        auth.error = Some("account has no profile".to_owned());
                    }
                    return;
                };
                self.session = Some(Session {
                    token: token.clone(),
                    profile_id: profile.id.clone(),
                    profile_name: profile.name,
                });
                self.load_task_list(&token, &profile.id);
            }
            Err(err) => self.handle_auth_error(err),
        }
    }

    fn load_task_list(&mut self, token: &str, profile_id: &str) {
        match self.client.list_tasks(token, profile_id) {
            Ok(tasks) => self.screen = Screen::TaskList(TaskListState::new(tasks)),
            Err(err) => self.handle_post_auth_error(err),
        }
    }

    /// Map an error encountered after authentication: an `unauthenticated` code returns to
    /// login; offline goes to the blocking screen; anything else surfaces inline on the task
    /// list (or, if we are mid-auth, on the auth screen).
    fn handle_post_auth_error(&mut self, err: ClientError) {
        if err.is_offline() {
            self.go_offline(err);
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

    fn handle_task_list_event(&mut self, event: Event) {
        let Screen::TaskList(list) = &mut self.screen else {
            return;
        };
        if list.adding.is_some() {
            self.handle_add_task_event(event);
            return;
        }
        match event {
            Event::Next => list.move_selection(true),
            Event::Prev => list.move_selection(false),
            Event::BeginAddTask => {
                list.message = None;
                list.adding = Some(AddTaskState::new());
            }
            Event::CloseSelected => self.close_selected(),
            Event::Refresh => self.refresh_task_list(),
            _ => {}
        }
    }

    fn handle_add_task_event(&mut self, event: Event) {
        let Screen::TaskList(list) = &mut self.screen else {
            return;
        };
        let Some(add) = &mut list.adding else {
            return;
        };
        match event {
            Event::Char(c) => {
                if add.on_title {
                    add.title.push(c);
                } else {
                    add.description.push(c);
                }
            }
            Event::Backspace => {
                let target = if add.on_title {
                    &mut add.title
                } else {
                    &mut add.description
                };
                let _ = target.pop();
            }
            Event::Next | Event::Prev => add.on_title = !add.on_title,
            Event::Cancel => list.adding = None,
            Event::Submit => self.submit_add_task(),
            _ => {}
        }
    }

    fn submit_add_task(&mut self) {
        let Some(session) = self.session.clone() else {
            self.go_to_login();
            return;
        };
        let Screen::TaskList(list) = &mut self.screen else {
            return;
        };
        let Some(add) = &mut list.adding else {
            return;
        };
        let req = CreateTaskRequest {
            title: add.title.trim().to_owned(),
            description: add.description.clone(),
        };
        match self
            .client
            .create_task(&session.token, &session.profile_id, &req)
        {
            Ok(_) => {
                if let Screen::TaskList(list) = &mut self.screen {
                    list.adding = None;
                }
                self.refresh_task_list();
            }
            Err(err) => self.handle_add_task_error(err),
        }
    }

    fn handle_add_task_error(&mut self, err: ClientError) {
        if err.is_offline() {
            self.go_offline(err);
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

    fn close_selected(&mut self) {
        let Some(session) = self.session.clone() else {
            self.go_to_login();
            return;
        };
        let task_id = {
            let Screen::TaskList(list) = &self.screen else {
                return;
            };
            let Some(idx) = list.selected else {
                return;
            };
            let Some(task) = list.tasks.get(idx) else {
                return;
            };
            task.id.clone()
        };
        match self
            .client
            .close_task(&session.token, &session.profile_id, &task_id)
        {
            Ok(updated) => {
                if let Screen::TaskList(list) = &mut self.screen
                    && let Some(slot) = list.tasks.iter_mut().find(|t| t.id == updated.id)
                {
                    *slot = updated;
                }
            }
            Err(err) => self.handle_post_auth_error(err),
        }
    }

    fn refresh_task_list(&mut self) {
        let Some(session) = self.session.clone() else {
            self.go_to_login();
            return;
        };
        match self.client.list_tasks(&session.token, &session.profile_id) {
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
            Err(err) => self.handle_post_auth_error(err),
        }
    }

    fn handle_offline_event(&mut self, event: Event) {
        if matches!(event, Event::Refresh | Event::Submit) {
            self.retry_from_offline();
        }
    }

    /// Manual retry from the offline screen: re-probe health, then return to the right screen
    /// (the task list if a session exists, otherwise login).
    fn retry_from_offline(&mut self) {
        match self.client.health() {
            Ok(()) => {
                if let Some(session) = self.session.clone() {
                    self.load_task_list(&session.token, &session.profile_id);
                } else {
                    self.go_to_login();
                }
            }
            Err(err) => self.go_offline(err),
        }
    }

    fn go_offline(&mut self, err: ClientError) {
        self.screen = Screen::Offline {
            message: err.to_string(),
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
