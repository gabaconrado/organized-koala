//! The auth screen (login / register): form state, focus navigation, and the pure event
//! handler that turns a submit into a [`ClientRequest`].

use contract::{LoginRequest, Password, RegisterRequest};

use super::protocol::{ClientRequest, RequestId};
use super::text_input::{self, TextInput};
use crate::app::Event;

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
    pub identifier: TextInput,
    /// Register username input.
    pub username: TextInput,
    /// Register email input.
    pub email: TextInput,
    /// Password input (rendered masked).
    pub password: TextInput,
    /// Register profile-name input.
    pub profile_name: TextInput,
    /// The account identifier captured at submit time (the login identifier or registered
    /// username), carried into the in-memory [`Session`](super::Session) for the post-auth title.
    /// Client-side only, no new wire (ADR-0010 §2); empty until a successful auth submit.
    pub account: String,
    /// Inline error message (e.g. validation or invalid credentials), if any.
    pub error: Option<String>,
    /// The in-flight request id while an auth call (or its chained post-auth load) is
    /// outstanding; `None` when idle. Transient process-lifetime UI state (hard-constraint #1).
    pub pending: Option<RequestId>,
}

impl AuthState {
    pub(crate) fn new() -> Self {
        Self {
            mode: AuthMode::Login,
            focus: AuthField::Identifier,
            identifier: TextInput::default(),
            username: TextInput::default(),
            email: TextInput::default(),
            password: TextInput::default(),
            profile_name: TextInput::default(),
            account: String::new(),
            error: None,
            pending: None,
        }
    }

    /// Whether the auth screen currently has a request outstanding.
    #[must_use]
    pub fn is_pending(&self) -> bool {
        self.pending.is_some()
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

    fn field_mut(&mut self, field: AuthField) -> &mut TextInput {
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

    /// Pure update for the auth screen. Returns the [`ClientRequest`] a submit produces, or
    /// `None` for a local edit (typing, focus, mode toggle) or any event while a request is
    /// outstanding. While pending, only the in-flight no-op path applies — `Cancel`/`Quit` are
    /// handled by the caller before dispatch here.
    pub(crate) fn handle_event(&mut self, event: Event) -> Option<ClientRequest> {
        if self.is_pending() {
            // One request in flight: ignore request-triggering and edit events alike.
            return None;
        }
        match event {
            Event::Char(c) => self.field_mut(self.focus).insert_char(c),
            Event::Backspace => self.field_mut(self.focus).backspace(),
            Event::Next => self.move_focus(true),
            Event::Prev => self.move_focus(false),
            Event::ToggleAuthMode => self.toggle_mode(),
            Event::Submit => return Some(self.submit()),
            // Caret movement / forward-delete act on the focused field.
            other => {
                let _ = text_input::apply_motion(self.field_mut(self.focus), &other);
            }
        }
        None
    }

    /// Build the auth request for the active form. The caller stamps and dispatches it.
    fn submit(&mut self) -> ClientRequest {
        self.error = None;
        match self.mode {
            AuthMode::Login => ClientRequest::Login(LoginRequest {
                identifier: self.identifier.as_str().trim().to_owned(),
                password: Password::new(self.password.as_str().to_owned()),
            }),
            AuthMode::Register => ClientRequest::Register(RegisterRequest {
                username: self.username.as_str().trim().to_owned(),
                email: self.email.as_str().trim().to_owned(),
                password: Password::new(self.password.as_str().to_owned()),
                profile_name: self.profile_name.as_str().trim().to_owned(),
            }),
        }
    }
}
