//! Shared test scaffolding for the `TestBackend` suite: a scripted fake [`Client`] (the
//! sanctioned external-service mock — the HTTP server), DTO builders that parse the canonical
//! wire JSON through the `contract` derives, a synchronous request executor that is the test-side
//! analogue of the worker thread, and a `TestBackend` render helper.
//!
//! The fake records the calls it received and returns scripted responses, so a test can both
//! drive the app's update path and assert what crossed the (mocked) wire — proving every view
//! derives from a server response (hard-constraint #1) with no internal collaborator mocked.
//!
//! The app core is two pure steps (ADR-0006): [`App::handle_event`] turns an [`Event`] into an
//! optional [`Dispatch`], and [`App::apply_response`] folds a completed [`ClientResponse`] back
//! into state (possibly returning a chained follow-up [`Dispatch`]). The effectful worker thread
//! that maps a [`ClientRequest`] through the real client and ships back a [`ClientResponse`] is
//! edge code, untestable in-process; [`execute`] / [`drive`] / [`submit`] below are its
//! **synchronous test-side analogue** — they run a `ClientRequest` through the `FakeClient` (the
//! sanctioned external-service mock) and feed the response back into `apply_response`, looping on
//! follow-ups until none. This is *not* mocking an internal collaborator: the only mock is the
//! `Client` trait (the server), exactly as the worker uses it.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]
// This module is `mod`-included by several integration-test binaries; not every one exercises
// every helper, so unused-warnings here are expected and benign for a shared test fixture.
#![allow(dead_code)]

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

use contract::{
    CreateNoteRequest, CreateTaskRequest, LoginRequest, Note, Profile, RegisterRequest,
    SessionResponse, Task, TimerConfig, TimerSession, UpdateNoteRequest, UpdateTimerConfigRequest,
};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use tui::app::{
    AddTaskState, App, AuthField, AuthMode, AuthState, ClientRequest, ClientResponse, Dispatch,
    Event, Outcome, RequestId, Screen, TaskListState,
};
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
    ListNotes {
        token: String,
        profile_id: String,
    },
    CreateNote {
        token: String,
        profile_id: String,
        title: String,
        content: String,
    },
    GetNote {
        token: String,
        profile_id: String,
        note_id: String,
    },
    UpdateNote {
        token: String,
        profile_id: String,
        note_id: String,
        title: String,
        content: String,
    },
    DeleteNote {
        token: String,
        profile_id: String,
        note_id: String,
    },
    GetTimerConfig {
        token: String,
    },
    UpdateTimerConfig {
        token: String,
        duration_minutes: u32,
    },
    GetTimerSession {
        token: String,
    },
    StartTimerSession {
        token: String,
    },
    StopTimerSession {
        token: String,
    },
}

/// The shared interior of a [`FakeClient`]: scripted response queues plus the recorded call
/// log. Held behind `Rc` so a test can keep a handle to script later responses and inspect
/// recorded calls while the executor borrows the same client.
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
    notes: RefCell<VecDeque<ClientResult<Vec<Note>>>>,
    create_note: RefCell<VecDeque<ClientResult<Note>>>,
    get_note: RefCell<VecDeque<ClientResult<Note>>>,
    update_note: RefCell<VecDeque<ClientResult<Note>>>,
    delete_note: RefCell<VecDeque<ClientResult<()>>>,
    timer_config: RefCell<VecDeque<ClientResult<TimerConfig>>>,
    update_timer_config: RefCell<VecDeque<ClientResult<TimerConfig>>>,
    timer_session: RefCell<VecDeque<ClientResult<TimerSession>>>,
    start_timer: RefCell<VecDeque<ClientResult<TimerSession>>>,
    stop_timer: RefCell<VecDeque<ClientResult<TimerSession>>>,
}

/// A scripted, recording fake [`Client`] — the only mock in the suite, standing in for the
/// external HTTP server (ADR-0003 layer 2). Each endpoint pops its next scripted response from a
/// queue and records the call. Cloning yields another handle to the *same* shared state, so a
/// test holds one handle (to script responses and read back recorded calls) while the executor
/// runs requests through another.
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
    pub fn push_notes(&self, r: ClientResult<Vec<Note>>) {
        self.inner.notes.borrow_mut().push_back(r);
    }
    pub fn push_create_note(&self, r: ClientResult<Note>) {
        self.inner.create_note.borrow_mut().push_back(r);
    }
    pub fn push_get_note(&self, r: ClientResult<Note>) {
        self.inner.get_note.borrow_mut().push_back(r);
    }
    pub fn push_update_note(&self, r: ClientResult<Note>) {
        self.inner.update_note.borrow_mut().push_back(r);
    }
    pub fn push_delete_note(&self, r: ClientResult<()>) {
        self.inner.delete_note.borrow_mut().push_back(r);
    }
    pub fn push_timer_config(&self, r: ClientResult<TimerConfig>) {
        self.inner.timer_config.borrow_mut().push_back(r);
    }
    pub fn push_update_timer_config(&self, r: ClientResult<TimerConfig>) {
        self.inner.update_timer_config.borrow_mut().push_back(r);
    }
    pub fn push_timer_session(&self, r: ClientResult<TimerSession>) {
        self.inner.timer_session.borrow_mut().push_back(r);
    }
    pub fn push_start_timer(&self, r: ClientResult<TimerSession>) {
        self.inner.start_timer.borrow_mut().push_back(r);
    }
    pub fn push_stop_timer(&self, r: ClientResult<TimerSession>) {
        self.inner.stop_timer.borrow_mut().push_back(r);
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

    fn list_notes(&self, token: &str, profile_id: &str) -> ClientResult<Vec<Note>> {
        self.inner.calls.borrow_mut().push(Call::ListNotes {
            token: token.to_owned(),
            profile_id: profile_id.to_owned(),
        });
        pop(&self.inner.notes, "list_notes")
    }

    fn create_note(
        &self,
        token: &str,
        profile_id: &str,
        req: &CreateNoteRequest,
    ) -> ClientResult<Note> {
        self.inner.calls.borrow_mut().push(Call::CreateNote {
            token: token.to_owned(),
            profile_id: profile_id.to_owned(),
            title: req.title.clone(),
            content: req.content.clone(),
        });
        pop(&self.inner.create_note, "create_note")
    }

    fn get_note(&self, token: &str, profile_id: &str, note_id: &str) -> ClientResult<Note> {
        self.inner.calls.borrow_mut().push(Call::GetNote {
            token: token.to_owned(),
            profile_id: profile_id.to_owned(),
            note_id: note_id.to_owned(),
        });
        pop(&self.inner.get_note, "get_note")
    }

    fn update_note(
        &self,
        token: &str,
        profile_id: &str,
        note_id: &str,
        req: &UpdateNoteRequest,
    ) -> ClientResult<Note> {
        self.inner.calls.borrow_mut().push(Call::UpdateNote {
            token: token.to_owned(),
            profile_id: profile_id.to_owned(),
            note_id: note_id.to_owned(),
            title: req.title.clone(),
            content: req.content.clone(),
        });
        pop(&self.inner.update_note, "update_note")
    }

    fn delete_note(&self, token: &str, profile_id: &str, note_id: &str) -> ClientResult<()> {
        self.inner.calls.borrow_mut().push(Call::DeleteNote {
            token: token.to_owned(),
            profile_id: profile_id.to_owned(),
            note_id: note_id.to_owned(),
        });
        pop(&self.inner.delete_note, "delete_note")
    }

    fn get_timer_config(&self, token: &str) -> ClientResult<TimerConfig> {
        self.inner.calls.borrow_mut().push(Call::GetTimerConfig {
            token: token.to_owned(),
        });
        pop(&self.inner.timer_config, "get_timer_config")
    }

    fn update_timer_config(
        &self,
        token: &str,
        req: &UpdateTimerConfigRequest,
    ) -> ClientResult<TimerConfig> {
        self.inner.calls.borrow_mut().push(Call::UpdateTimerConfig {
            token: token.to_owned(),
            duration_minutes: req.duration_minutes,
        });
        pop(&self.inner.update_timer_config, "update_timer_config")
    }

    fn get_timer_session(&self, token: &str) -> ClientResult<TimerSession> {
        self.inner.calls.borrow_mut().push(Call::GetTimerSession {
            token: token.to_owned(),
        });
        pop(&self.inner.timer_session, "get_timer_session")
    }

    fn start_timer_session(&self, token: &str) -> ClientResult<TimerSession> {
        self.inner.calls.borrow_mut().push(Call::StartTimerSession {
            token: token.to_owned(),
        });
        pop(&self.inner.start_timer, "start_timer_session")
    }

    fn stop_timer_session(&self, token: &str) -> ClientResult<TimerSession> {
        self.inner.calls.borrow_mut().push(Call::StopTimerSession {
            token: token.to_owned(),
        });
        pop(&self.inner.stop_timer, "stop_timer_session")
    }
}

// ---- Synchronous request executor: the test-side analogue of the worker thread ----

/// Run one [`ClientRequest`] through the (mocked) `client`, producing its [`Outcome`]. This
/// mirrors the worker thread's request→outcome dispatch exactly, so the response the core sees in
/// tests is identical to what the real edge would feed it — the only "mock" is the `Client` trait
/// itself (the external HTTP server), never an internal collaborator.
fn run_request(client: &FakeClient, request: ClientRequest) -> Outcome {
    match request {
        ClientRequest::Health => Outcome::Health(client.health()),
        ClientRequest::Register(req) => Outcome::Register(client.register(&req)),
        ClientRequest::Login(req) => Outcome::Login(client.login(&req)),
        ClientRequest::ListProfiles { token } => {
            let result = client.list_profiles(&token);
            Outcome::ListProfiles { token, result }
        }
        ClientRequest::ListTasks { token, profile_id } => {
            Outcome::ListTasks(client.list_tasks(&token, &profile_id))
        }
        ClientRequest::CreateTask {
            token,
            profile_id,
            req,
        } => Outcome::CreateTask(client.create_task(&token, &profile_id, &req)),
        ClientRequest::CloseTask {
            token,
            profile_id,
            task_id,
        } => Outcome::CloseTask(client.close_task(&token, &profile_id, &task_id)),
        ClientRequest::ListNotes { token, profile_id } => {
            Outcome::ListNotes(client.list_notes(&token, &profile_id))
        }
        ClientRequest::CreateNote {
            token,
            profile_id,
            req,
        } => Outcome::CreateNote(client.create_note(&token, &profile_id, &req)),
        ClientRequest::GetNote {
            token,
            profile_id,
            note_id,
        } => Outcome::GetNote(client.get_note(&token, &profile_id, &note_id)),
        ClientRequest::UpdateNote {
            token,
            profile_id,
            note_id,
            req,
        } => Outcome::UpdateNote(client.update_note(&token, &profile_id, &note_id, &req)),
        ClientRequest::DeleteNote {
            token,
            profile_id,
            note_id,
        } => Outcome::DeleteNote(client.delete_note(&token, &profile_id, &note_id)),
        ClientRequest::GetTimerConfig { token } => {
            Outcome::GetTimerConfig(client.get_timer_config(&token))
        }
        ClientRequest::UpdateTimerConfig { token, req } => {
            Outcome::UpdateTimerConfig(client.update_timer_config(&token, &req))
        }
        ClientRequest::GetTimerSession { token } => {
            Outcome::GetTimerSession(client.get_timer_session(&token))
        }
        ClientRequest::StartTimerSession { token } => {
            Outcome::StartTimerSession(client.start_timer_session(&token))
        }
        ClientRequest::StopTimerSession { token } => {
            Outcome::StopTimerSession(client.stop_timer_session(&token))
        }
    }
}

/// Execute one [`Dispatch`] against the fake client and return the [`ClientResponse`] the edge
/// would ship back (echoing the dispatch's [`RequestId`]). Does **not** feed it to the app — use
/// this when a test wants to inspect or delay the response (e.g. the stale-response-after-cancel
/// case).
#[must_use]
pub fn execute(client: &FakeClient, dispatch: Dispatch) -> ClientResponse {
    let outcome = run_request(client, dispatch.request);
    ClientResponse {
        id: dispatch.id,
        outcome,
    }
}

/// Drive a [`Dispatch`] to completion through the two-step seam: execute it against the fake,
/// apply the response to `app`, and loop on any chained follow-up dispatch until the flow settles
/// with no request in flight. This is what the real poll loop does across ticks, collapsed to a
/// synchronous call for tests.
pub fn drive(app: &mut App, client: &FakeClient, mut dispatch: Dispatch) {
    loop {
        let response = execute(client, dispatch);
        match app.apply_response(response) {
            Some(next) => dispatch = next,
            None => break,
        }
    }
}

/// Feed an [`Event`] to the app and, if it triggers a [`Dispatch`], drive that to completion. The
/// common path for tests that don't care about observing the in-flight state. Returns nothing —
/// assert against `app.screen()` / `app.session()` / the fake's recorded calls afterwards.
pub fn submit(app: &mut App, client: &FakeClient, event: Event) {
    if let Some(dispatch) = app.handle_event(event) {
        drive(app, client, dispatch);
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

/// Build a [`Note`] from canonical wire JSON (so its `chrono` timestamp is parsed by the
/// `contract` derive). `created_at` is supplied so tests can pin newest-first ordering
/// deterministically.
pub fn note(id: &str, title: &str, content: &str, created_at: &str) -> Note {
    let json = serde_json::json!({
        "id": id,
        "title": title,
        "content": content,
        "created_at": created_at,
    });
    serde_json::from_value(json).expect("valid note json")
}

/// A [`TimerConfig`] with the given duration in minutes.
pub fn timer_config(duration_minutes: u32) -> TimerConfig {
    TimerConfig { duration_minutes }
}

/// Build a running [`TimerSession`] from canonical wire JSON, so its `chrono` timestamps are
/// parsed by the `contract` derive. `ends_at`/`server_now` are supplied so a test can pin the
/// `MM:SS` countdown the view derives (`ends_at − server_now`) deterministically.
pub fn running_session(
    started_at: &str,
    ends_at: &str,
    duration_minutes: u32,
    server_now: &str,
) -> TimerSession {
    let json = serde_json::json!({
        "state": "running",
        "started_at": started_at,
        "ends_at": ends_at,
        "duration_minutes": duration_minutes,
        "server_now": server_now,
    });
    serde_json::from_value(json).expect("valid running session json")
}

/// Build a completed [`TimerSession`] from canonical wire JSON (the server's `now >= ends_at`
/// verdict).
pub fn completed_session(
    started_at: &str,
    ends_at: &str,
    duration_minutes: u32,
    server_now: &str,
) -> TimerSession {
    let json = serde_json::json!({
        "state": "completed",
        "started_at": started_at,
        "ends_at": ends_at,
        "duration_minutes": duration_minutes,
        "server_now": server_now,
    });
    serde_json::from_value(json).expect("valid completed session json")
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

/// Render the app onto a `TestBackend` of the given size at the given spinner `tick`, returning
/// the flattened buffer text (one string with `\n` between rows, trailing spaces trimmed per
/// row). `tick` drives the in-flight spinner; it is ignored when no request is outstanding.
pub fn render_at(app: &App, width: u16, height: u16, tick: u64) -> String {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    let _completed = terminal
        .draw(|frame| tui::ui::draw(frame, app, tick))
        .expect("draw");
    buffer_text(terminal.backend().buffer())
}

/// Render at tick 0 (the common case for non-spinner assertions).
pub fn render(app: &App, width: u16, height: u16) -> String {
    render_at(app, width, height, 0)
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

/// Convenience: the active screen's name, for diagnostics.
pub fn screen_name(app: &App) -> &'static str {
    match app.screen() {
        Screen::Auth(_) => "auth",
        Screen::TaskList(_) => "task_list",
        Screen::Notes(_) => "notes",
        Screen::Offline { .. } => "offline",
    }
}

/// Drive the edge's "initial timer load" hook to completion: the real poll loop calls
/// [`App::load_timer_if_needed`] every frame, so a logged-in app issues its `GetTimerConfig` →
/// `GetTimerSession` chain off that hook, not off an [`Event`]. This is the test-side analogue —
/// it runs that hook once and drives the resulting chain through the fake. A no-op if the timer
/// already loaded (or no post-auth session exists yet).
pub fn load_timer(app: &mut App, client: &FakeClient) {
    if let Some(dispatch) = app.load_timer_if_needed() {
        drive(app, client, dispatch);
    }
}

/// Drive the edge's coarse timer-session refresh hook ([`App::refresh_timer`]) to completion — the
/// test-side analogue of the loop firing on the `TIMER_REFRESH_TICKS` boundary. A no-op when the
/// timer surface declines (in flight, editing, or not post-auth).
pub fn refresh_timer(app: &mut App, client: &FakeClient) {
    if let Some(dispatch) = app.refresh_timer() {
        drive(app, client, dispatch);
    }
}

// ---- Screen builders (for the pure `map_key` keybinding tests) ----
//
// `map_key` takes only `&Screen`, so these construct representative screens directly via the
// public struct literals — no client or driven flow needed to pin the keybinding contract. Each
// carries its in-flight marker (`pending`) so the same builders cover the idle and in-flight
// keybinding contexts.

/// The auth (login) screen, idle (no request in flight).
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
        pending: None,
    })
}

/// The auth (login) screen with a request outstanding.
pub fn auth_screen_pending() -> Screen {
    match auth_screen() {
        Screen::Auth(mut auth) => {
            auth.pending = Some(RequestId(0));
            Screen::Auth(auth)
        }
        other => other,
    }
}

/// A task-list screen with one open task and no add-task sub-flow open, idle.
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
        pending: None,
    })
}

/// A task-list screen with a request outstanding (no add-task sub-flow open).
pub fn task_list_screen_pending() -> Screen {
    match task_list_screen() {
        Screen::TaskList(mut list) => {
            list.pending = Some(RequestId(0));
            Screen::TaskList(list)
        }
        other => other,
    }
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
        pending: None,
    })
}

/// The blocking offline screen, idle.
pub fn offline_screen() -> Screen {
    Screen::Offline {
        message: "the server is unreachable: connection refused".to_owned(),
        pending: None,
    }
}

/// The blocking offline screen with a retry probe outstanding.
pub fn offline_screen_pending() -> Screen {
    Screen::Offline {
        message: "the server is unreachable: connection refused".to_owned(),
        pending: Some(RequestId(0)),
    }
}
