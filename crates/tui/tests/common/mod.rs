//! Shared test scaffolding for the `TestBackend` suite: a scripted fake [`Client`] (the
//! sanctioned external-service mock — the HTTP server), DTO builders that parse the canonical
//! wire JSON through the `contract` derives, and a `TestBackend` render helper.
//!
//! The fake records the calls it received and returns scripted responses, so a test can both
//! drive the app's update path and assert what crossed the (mocked) wire — proving every view
//! derives from a server response (hard-constraint #1) with no internal collaborator mocked.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]
// This module is `mod`-included by several integration-test binaries; not every one exercises
// every helper, so unused-warnings here are expected and benign for a shared test fixture.
#![allow(dead_code)]

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

use contract::{CreateTaskRequest, LoginRequest, Profile, RegisterRequest, SessionResponse, Task};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use tui::app::{AddTaskState, App, AuthField, AuthMode, AuthState, Screen, TaskListState};
use tui::client::{Client, ClientError, ClientResult};

/// One recorded call against the fake client, for asserting what crossed the wire.
///
/// `RegisterRequest`/`LoginRequest` hold a `Password` that intentionally derives no `PartialEq`
/// (so the secret can never be compared/leaked), so this enum is not `PartialEq`; the auth
/// payloads are captured as their plain string fields instead, which the tests assert directly.
#[derive(Debug, Clone)]
pub enum Call {
    Health,
    /// Captured register fields (username, email, profile_name) — password omitted by design.
    Register {
        username: String,
        email: String,
        profile_name: String,
    },
    /// Captured login identifier — password omitted by design.
    Login {
        identifier: String,
    },
    ListProfiles {
        token: String,
    },
    ListTasks {
        token: String,
        profile_id: String,
    },
    CreateTask {
        token: String,
        profile_id: String,
        title: String,
        description: String,
    },
    CloseTask {
        token: String,
        profile_id: String,
        task_id: String,
    },
}

/// The shared interior of a [`FakeClient`]: scripted response queues plus the recorded call
/// log. Held behind `Rc` so a test can keep a handle to script later responses and inspect
/// recorded calls while the `App` owns its own clone of the same client.
#[derive(Debug, Default)]
struct Inner {
    calls: RefCell<Vec<Call>>,
    health: RefCell<VecDeque<ClientResult<()>>>,
    register: RefCell<VecDeque<ClientResult<SessionResponse>>>,
    login: RefCell<VecDeque<ClientResult<SessionResponse>>>,
    profiles: RefCell<VecDeque<ClientResult<Vec<Profile>>>>,
    tasks: RefCell<VecDeque<ClientResult<Vec<Task>>>>,
    create: RefCell<VecDeque<ClientResult<Task>>>,
    close: RefCell<VecDeque<ClientResult<Task>>>,
}

/// A scripted, recording fake [`Client`] — the only mock in the suite, standing in for the
/// external HTTP server (ADR-0003 layer 2). Each endpoint pops its next scripted response from a
/// queue and records the call. Cloning yields another handle to the *same* shared state, so a
/// test holds one handle (to script responses and read back recorded calls) while the `App`
/// owns another.
#[derive(Debug, Clone, Default)]
pub struct FakeClient {
    inner: Rc<Inner>,
}

impl FakeClient {
    pub fn new() -> Self {
        Self::default()
    }

    // Each `push_*` enqueues the next scripted response for its endpoint. They return `()`
    // (not `&Self`) so the deny-by-default `unused_results` lint stays satisfied at call sites.
    pub fn push_health(&self, r: ClientResult<()>) {
        self.inner.health.borrow_mut().push_back(r);
    }
    pub fn push_register(&self, r: ClientResult<SessionResponse>) {
        self.inner.register.borrow_mut().push_back(r);
    }
    pub fn push_login(&self, r: ClientResult<SessionResponse>) {
        self.inner.login.borrow_mut().push_back(r);
    }
    pub fn push_profiles(&self, r: ClientResult<Vec<Profile>>) {
        self.inner.profiles.borrow_mut().push_back(r);
    }
    pub fn push_tasks(&self, r: ClientResult<Vec<Task>>) {
        self.inner.tasks.borrow_mut().push_back(r);
    }
    pub fn push_create(&self, r: ClientResult<Task>) {
        self.inner.create.borrow_mut().push_back(r);
    }
    pub fn push_close(&self, r: ClientResult<Task>) {
        self.inner.close.borrow_mut().push_back(r);
    }

    /// The calls the app made, in order.
    pub fn calls(&self) -> Vec<Call> {
        self.inner.calls.borrow().clone()
    }
}

fn pop<T>(q: &RefCell<VecDeque<ClientResult<T>>>, what: &str) -> ClientResult<T> {
    q.borrow_mut()
        .pop_front()
        .unwrap_or_else(|| panic!("fake client: no scripted response for {what}"))
}

impl Client for FakeClient {
    fn health(&self) -> ClientResult<()> {
        self.inner.calls.borrow_mut().push(Call::Health);
        pop(&self.inner.health, "health")
    }

    fn register(&self, req: &RegisterRequest) -> ClientResult<SessionResponse> {
        self.inner.calls.borrow_mut().push(Call::Register {
            username: req.username.clone(),
            email: req.email.clone(),
            profile_name: req.profile_name.clone(),
        });
        pop(&self.inner.register, "register")
    }

    fn login(&self, req: &LoginRequest) -> ClientResult<SessionResponse> {
        self.inner.calls.borrow_mut().push(Call::Login {
            identifier: req.identifier.clone(),
        });
        pop(&self.inner.login, "login")
    }

    fn list_profiles(&self, token: &str) -> ClientResult<Vec<Profile>> {
        self.inner.calls.borrow_mut().push(Call::ListProfiles {
            token: token.to_owned(),
        });
        pop(&self.inner.profiles, "list_profiles")
    }

    fn list_tasks(&self, token: &str, profile_id: &str) -> ClientResult<Vec<Task>> {
        self.inner.calls.borrow_mut().push(Call::ListTasks {
            token: token.to_owned(),
            profile_id: profile_id.to_owned(),
        });
        pop(&self.inner.tasks, "list_tasks")
    }

    fn create_task(
        &self,
        token: &str,
        profile_id: &str,
        req: &CreateTaskRequest,
    ) -> ClientResult<Task> {
        self.inner.calls.borrow_mut().push(Call::CreateTask {
            token: token.to_owned(),
            profile_id: profile_id.to_owned(),
            title: req.title.clone(),
            description: req.description.clone(),
        });
        pop(&self.inner.create, "create_task")
    }

    fn close_task(&self, token: &str, profile_id: &str, task_id: &str) -> ClientResult<Task> {
        self.inner.calls.borrow_mut().push(Call::CloseTask {
            token: token.to_owned(),
            profile_id: profile_id.to_owned(),
            task_id: task_id.to_owned(),
        });
        pop(&self.inner.close, "close_task")
    }
}

/// A session-token response.
pub fn session(token: &str) -> SessionResponse {
    SessionResponse {
        token: token.to_owned(),
    }
}

/// Build a [`Profile`] from canonical wire JSON (so its `chrono` timestamp is parsed by the
/// `contract` derive, not constructed here).
pub fn profile(id: &str, name: &str) -> Profile {
    let json = serde_json::json!({
        "id": id,
        "name": name,
        "created_at": "2026-06-18T12:00:00Z",
    });
    serde_json::from_value(json).expect("valid profile json")
}

/// Build an open [`Task`] from canonical wire JSON. `created_at` is supplied so tests can pin
/// newest-first ordering deterministically.
pub fn open_task(id: &str, title: &str, created_at: &str) -> Task {
    let json = serde_json::json!({
        "id": id,
        "title": title,
        "description": "",
        "status": "open",
        "created_at": created_at,
        "closed_at": null,
    });
    serde_json::from_value(json).expect("valid task json")
}

/// Build a done [`Task`] (status `done`, `closed_at` set) from canonical wire JSON.
pub fn done_task(id: &str, title: &str, created_at: &str, closed_at: &str) -> Task {
    let json = serde_json::json!({
        "id": id,
        "title": title,
        "description": "",
        "status": "done",
        "created_at": created_at,
        "closed_at": closed_at,
    });
    serde_json::from_value(json).expect("valid task json")
}

/// An [`ClientError::Api`] with a code.
pub fn api_err(code: contract::ErrorCode, message: &str) -> ClientError {
    ClientError::Api {
        code: Some(code),
        message: message.to_owned(),
    }
}

/// An [`ClientError::Api`] with no machine-matchable code (e.g. a malformed/empty error body).
pub fn api_err_no_code(message: &str) -> ClientError {
    ClientError::Api {
        code: None,
        message: message.to_owned(),
    }
}

/// An [`ClientError::Offline`] transport failure.
pub fn offline_err(message: &str) -> ClientError {
    ClientError::Offline(message.to_owned())
}

/// Render the app onto a `TestBackend` of the given size and return the flattened buffer text
/// (one string with `\n` between rows, trailing spaces trimmed per row).
pub fn render<C: Client>(app: &App<C>, width: u16, height: u16) -> String {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    let _completed = terminal
        .draw(|frame| tui::ui::draw(frame, app))
        .expect("draw");
    buffer_text(terminal.backend().buffer())
}

/// Flatten a `ratatui` buffer into newline-joined, right-trimmed rows of text.
fn buffer_text(buffer: &Buffer) -> String {
    let area = buffer.area();
    let mut out = String::new();
    for y in 0..area.height {
        let mut row = String::new();
        for x in 0..area.width {
            row.push_str(buffer[(x, y)].symbol());
        }
        out.push_str(row.trim_end());
        out.push('\n');
    }
    out
}

/// Convenience: assert the app is on the auth screen and return its name for diagnostics.
pub fn screen_name<C: Client>(app: &App<C>) -> &'static str {
    match app.screen() {
        Screen::Auth(_) => "auth",
        Screen::TaskList(_) => "task_list",
        Screen::Offline { .. } => "offline",
    }
}

// ---- Screen builders (for the pure `map_key` keybinding tests) ----
//
// `map_key` takes only `&Screen`, so these construct representative screens directly via the
// public struct literals — no client or driven flow needed to pin the keybinding contract.

/// The auth (login) screen.
pub fn auth_screen() -> Screen {
    Screen::Auth(AuthState {
        mode: AuthMode::Login,
        focus: AuthField::Identifier,
        identifier: String::new(),
        username: String::new(),
        email: String::new(),
        password: String::new(),
        profile_name: String::new(),
        error: None,
    })
}

/// A task-list screen with one open task and no add-task sub-flow open.
pub fn task_list_screen() -> Screen {
    Screen::TaskList(TaskListState {
        tasks: vec![open_task(
            "00000000-0000-0000-0000-000000000001",
            "a task",
            "2026-06-18T10:00:00Z",
        )],
        selected: Some(0),
        adding: None,
        message: None,
    })
}

/// A task-list screen with the add-task sub-flow open (a text-entry context).
pub fn task_list_screen_adding() -> Screen {
    Screen::TaskList(TaskListState {
        tasks: Vec::new(),
        selected: None,
        adding: Some(AddTaskState {
            on_title: true,
            title: String::new(),
            description: String::new(),
            error: None,
        }),
        message: None,
    })
}

/// The blocking offline screen.
pub fn offline_screen() -> Screen {
    Screen::Offline {
        message: "the server is unreachable: connection refused".to_owned(),
    }
}
