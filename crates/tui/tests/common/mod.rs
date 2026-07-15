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
use std::collections::{HashMap, VecDeque};
use std::rc::Rc;

use contract::{
    CreateNoteRequest, CreateProfileRequest, CreateSubtaskRequest, CreateTaskRequest, LoginRequest,
    Note, Profile, RegisterRequest, SessionResponse, Subtask, Task, TaskStatus, TimerConfig,
    TimerSession, UpdateNoteRequest, UpdateProfileRequest, UpdateSubtaskRequest, UpdateTaskRequest,
    UpdateTimerConfigRequest,
};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use tui::app::{
    AddTaskState, App, AuthField, AuthMode, AuthState, ClientRequest, ClientResponse, DeleteTarget,
    Dispatch, EditTaskState, Event, MainState, NoteDetail, NoteForm, NotePane, NotesMode,
    NotesState, Outcome, ProfileForm, ProfilesMode, ProfilesState, RequestId, Screen, Tab,
    TaskListState, TextInput,
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
    CreateProfile {
        token: String,
        name: String,
    },
    UpdateProfile {
        token: String,
        profile_id: String,
        name: String,
    },
    DeleteProfile {
        token: String,
        profile_id: String,
    },
    /// Captured `GET …/tasks` with the pagination-ready query the caller sent (the TUI hard-codes
    /// `limit = TASK_LIST_LIMIT` / `offset = 0`; ADR-0014 §2), plus the ADR-0015 date-window bounds
    /// (`created_from` inclusive lower / `created_until` exclusive upper, UTC epoch seconds), so a
    /// test can assert the whole wire query — including the default `[anchor − X, anchor]` window.
    ListTasks {
        token: String,
        profile_id: String,
        limit: Option<u32>,
        offset: Option<u32>,
        created_from: Option<i64>,
        created_until: Option<i64>,
    },
    CreateTask {
        token: String,
        profile_id: String,
        title: String,
        description: String,
    },
    /// Captured `PATCH …/tasks/{id}` partial-update fields (edit / toggle-done / reopen), so a
    /// test can assert exactly what the issued patch carried.
    UpdateTask {
        token: String,
        profile_id: String,
        task_id: String,
        title: Option<String>,
        description: Option<String>,
        status: Option<TaskStatus>,
    },
    /// Captured `DELETE …/tasks/{id}` target.
    DeleteTask {
        token: String,
        profile_id: String,
        task_id: String,
    },
    /// Captured `GET …/subtasks` (the profile-wide tree-load list).
    ListSubtasks {
        token: String,
        profile_id: String,
    },
    /// Captured `GET …/tasks/{tid}/subtasks` (one parent's sub-tasks, for the detail section).
    ListTaskSubtasks {
        token: String,
        profile_id: String,
        task_id: String,
    },
    /// Captured `POST …/tasks/{tid}/subtasks` (the `A`-create).
    CreateSubtask {
        token: String,
        profile_id: String,
        task_id: String,
        title: String,
    },
    /// Captured `PATCH …/tasks/{tid}/subtasks/{sid}` partial-update fields (edit-title / toggle),
    /// so a test can assert exactly what the issued patch carried.
    UpdateSubtask {
        token: String,
        profile_id: String,
        task_id: String,
        subtask_id: String,
        title: Option<String>,
        status: Option<TaskStatus>,
    },
    /// Captured `DELETE …/tasks/{tid}/subtasks/{sid}` target.
    DeleteSubtask {
        token: String,
        profile_id: String,
        task_id: String,
        subtask_id: String,
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
    create_profile: RefCell<VecDeque<ClientResult<Profile>>>,
    update_profile: RefCell<VecDeque<ClientResult<Profile>>>,
    delete_profile: RefCell<VecDeque<ClientResult<()>>>,
    tasks: RefCell<VecDeque<ClientResult<Vec<Task>>>>,
    create: RefCell<VecDeque<ClientResult<Task>>>,
    update: RefCell<VecDeque<ClientResult<Task>>>,
    delete: RefCell<VecDeque<ClientResult<()>>>,
    list_subtasks: RefCell<VecDeque<ClientResult<Vec<Subtask>>>>,
    list_task_subtasks: RefCell<VecDeque<ClientResult<Vec<Subtask>>>>,
    create_subtask: RefCell<VecDeque<ClientResult<Subtask>>>,
    update_subtask: RefCell<VecDeque<ClientResult<Subtask>>>,
    delete_subtask: RefCell<VecDeque<ClientResult<()>>>,
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
    pub fn push_create_profile(&self, r: ClientResult<Profile>) {
        self.inner.create_profile.borrow_mut().push_back(r);
    }
    pub fn push_update_profile(&self, r: ClientResult<Profile>) {
        self.inner.update_profile.borrow_mut().push_back(r);
    }
    pub fn push_delete_profile(&self, r: ClientResult<()>) {
        self.inner.delete_profile.borrow_mut().push_back(r);
    }
    pub fn push_tasks(&self, r: ClientResult<Vec<Task>>) {
        self.inner.tasks.borrow_mut().push_back(r);
    }
    pub fn push_create(&self, r: ClientResult<Task>) {
        self.inner.create.borrow_mut().push_back(r);
    }
    pub fn push_update(&self, r: ClientResult<Task>) {
        self.inner.update.borrow_mut().push_back(r);
    }
    pub fn push_delete(&self, r: ClientResult<()>) {
        self.inner.delete.borrow_mut().push_back(r);
    }
    pub fn push_list_subtasks(&self, r: ClientResult<Vec<Subtask>>) {
        self.inner.list_subtasks.borrow_mut().push_back(r);
    }
    pub fn push_list_task_subtasks(&self, r: ClientResult<Vec<Subtask>>) {
        self.inner.list_task_subtasks.borrow_mut().push_back(r);
    }
    pub fn push_create_subtask(&self, r: ClientResult<Subtask>) {
        self.inner.create_subtask.borrow_mut().push_back(r);
    }
    pub fn push_update_subtask(&self, r: ClientResult<Subtask>) {
        self.inner.update_subtask.borrow_mut().push_back(r);
    }
    pub fn push_delete_subtask(&self, r: ClientResult<()>) {
        self.inner.delete_subtask.borrow_mut().push_back(r);
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

/// Pop the next scripted sub-task list, defaulting to an empty list when none is scripted.
///
/// The two-call Tasks-tab tree load chains `ListSubtasks` after every `ListTasks` (post-auth
/// bootstrap and every task-mutation refresh), and opening a task detail chains
/// `ListTaskSubtasks`. Those are core-issued infrastructure on flows that are usually not *about*
/// sub-tasks, so a test that scripts only `push_tasks` is implicitly asserting "this profile/task
/// has no sub-tasks" — the natural empty default. Tests that exercise sub-tasks script explicit
/// non-empty responses with `push_list_subtasks` / `push_list_task_subtasks`, which take
/// precedence (the queue is drained first). The *mutating* sub-task calls keep the strict
/// panic-on-empty safety net (a missing create/update/delete script is always a test bug).
fn pop_subtasks_or_empty(
    q: &RefCell<VecDeque<ClientResult<Vec<Subtask>>>>,
) -> ClientResult<Vec<Subtask>> {
    q.borrow_mut().pop_front().unwrap_or_else(|| Ok(Vec::new()))
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

    fn create_profile(&self, token: &str, req: &CreateProfileRequest) -> ClientResult<Profile> {
        self.inner.calls.borrow_mut().push(Call::CreateProfile {
            token: token.to_owned(),
            name: req.name.clone(),
        });
        pop(&self.inner.create_profile, "create_profile")
    }

    fn rename_profile(
        &self,
        token: &str,
        profile_id: &str,
        req: &UpdateProfileRequest,
    ) -> ClientResult<Profile> {
        self.inner.calls.borrow_mut().push(Call::UpdateProfile {
            token: token.to_owned(),
            profile_id: profile_id.to_owned(),
            name: req.name.clone(),
        });
        pop(&self.inner.update_profile, "rename_profile")
    }

    fn delete_profile(&self, token: &str, profile_id: &str) -> ClientResult<()> {
        self.inner.calls.borrow_mut().push(Call::DeleteProfile {
            token: token.to_owned(),
            profile_id: profile_id.to_owned(),
        });
        pop(&self.inner.delete_profile, "delete_profile")
    }

    fn list_tasks(
        &self,
        token: &str,
        profile_id: &str,
        query: &contract::TaskListQuery,
    ) -> ClientResult<Vec<Task>> {
        self.inner.calls.borrow_mut().push(Call::ListTasks {
            token: token.to_owned(),
            profile_id: profile_id.to_owned(),
            limit: query.limit,
            offset: query.offset,
            created_from: query.created_from,
            created_until: query.created_until,
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

    fn update_task(
        &self,
        token: &str,
        profile_id: &str,
        task_id: &str,
        req: &UpdateTaskRequest,
    ) -> ClientResult<Task> {
        self.inner.calls.borrow_mut().push(Call::UpdateTask {
            token: token.to_owned(),
            profile_id: profile_id.to_owned(),
            task_id: task_id.to_owned(),
            title: req.title.clone(),
            description: req.description.clone(),
            status: req.status,
        });
        pop(&self.inner.update, "update_task")
    }

    fn delete_task(&self, token: &str, profile_id: &str, task_id: &str) -> ClientResult<()> {
        self.inner.calls.borrow_mut().push(Call::DeleteTask {
            token: token.to_owned(),
            profile_id: profile_id.to_owned(),
            task_id: task_id.to_owned(),
        });
        pop(&self.inner.delete, "delete_task")
    }

    fn list_subtasks(&self, token: &str, profile_id: &str) -> ClientResult<Vec<Subtask>> {
        self.inner.calls.borrow_mut().push(Call::ListSubtasks {
            token: token.to_owned(),
            profile_id: profile_id.to_owned(),
        });
        pop_subtasks_or_empty(&self.inner.list_subtasks)
    }

    fn list_task_subtasks(
        &self,
        token: &str,
        profile_id: &str,
        task_id: &str,
    ) -> ClientResult<Vec<Subtask>> {
        self.inner.calls.borrow_mut().push(Call::ListTaskSubtasks {
            token: token.to_owned(),
            profile_id: profile_id.to_owned(),
            task_id: task_id.to_owned(),
        });
        pop_subtasks_or_empty(&self.inner.list_task_subtasks)
    }

    fn create_subtask(
        &self,
        token: &str,
        profile_id: &str,
        task_id: &str,
        req: &CreateSubtaskRequest,
    ) -> ClientResult<Subtask> {
        self.inner.calls.borrow_mut().push(Call::CreateSubtask {
            token: token.to_owned(),
            profile_id: profile_id.to_owned(),
            task_id: task_id.to_owned(),
            title: req.title.clone(),
        });
        pop(&self.inner.create_subtask, "create_subtask")
    }

    fn update_subtask(
        &self,
        token: &str,
        profile_id: &str,
        task_id: &str,
        subtask_id: &str,
        req: &UpdateSubtaskRequest,
    ) -> ClientResult<Subtask> {
        self.inner.calls.borrow_mut().push(Call::UpdateSubtask {
            token: token.to_owned(),
            profile_id: profile_id.to_owned(),
            task_id: task_id.to_owned(),
            subtask_id: subtask_id.to_owned(),
            title: req.title.clone(),
            status: req.status,
        });
        pop(&self.inner.update_subtask, "update_subtask")
    }

    fn delete_subtask(
        &self,
        token: &str,
        profile_id: &str,
        task_id: &str,
        subtask_id: &str,
    ) -> ClientResult<()> {
        self.inner.calls.borrow_mut().push(Call::DeleteSubtask {
            token: token.to_owned(),
            profile_id: profile_id.to_owned(),
            task_id: task_id.to_owned(),
            subtask_id: subtask_id.to_owned(),
        });
        pop(&self.inner.delete_subtask, "delete_subtask")
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
            let result = client.list_profiles(token.expose());
            Outcome::ListProfiles { token, result }
        }
        ClientRequest::CreateProfile { token, req } => {
            Outcome::CreateProfile(client.create_profile(token.expose(), &req))
        }
        ClientRequest::UpdateProfile {
            token,
            profile_id,
            req,
        } => Outcome::UpdateProfile(client.rename_profile(token.expose(), &profile_id, &req)),
        ClientRequest::DeleteProfile { token, profile_id } => {
            Outcome::DeleteProfile(client.delete_profile(token.expose(), &profile_id))
        }
        ClientRequest::ListTasks {
            token,
            profile_id,
            query,
        } => Outcome::ListTasks(client.list_tasks(token.expose(), &profile_id, &query)),
        ClientRequest::CreateTask {
            token,
            profile_id,
            req,
        } => Outcome::CreateTask(client.create_task(token.expose(), &profile_id, &req)),
        ClientRequest::UpdateTask {
            token,
            profile_id,
            task_id,
            req,
        } => Outcome::UpdateTask(client.update_task(token.expose(), &profile_id, &task_id, &req)),
        ClientRequest::DeleteTask {
            token,
            profile_id,
            task_id,
        } => Outcome::DeleteTask(client.delete_task(token.expose(), &profile_id, &task_id)),
        ClientRequest::ListSubtasks { token, profile_id } => {
            Outcome::ListSubtasks(client.list_subtasks(token.expose(), &profile_id))
        }
        ClientRequest::ListTaskSubtasks {
            token,
            profile_id,
            task_id,
        } => Outcome::ListTaskSubtasks(client.list_task_subtasks(
            token.expose(),
            &profile_id,
            &task_id,
        )),
        ClientRequest::CreateSubtask {
            token,
            profile_id,
            task_id,
            req,
        } => Outcome::CreateSubtask(client.create_subtask(
            token.expose(),
            &profile_id,
            &task_id,
            &req,
        )),
        ClientRequest::UpdateSubtask {
            token,
            profile_id,
            task_id,
            subtask_id,
            req,
        } => Outcome::UpdateSubtask(client.update_subtask(
            token.expose(),
            &profile_id,
            &task_id,
            &subtask_id,
            &req,
        )),
        ClientRequest::DeleteSubtask {
            token,
            profile_id,
            task_id,
            subtask_id,
        } => Outcome::DeleteSubtask(client.delete_subtask(
            token.expose(),
            &profile_id,
            &task_id,
            &subtask_id,
        )),
        ClientRequest::ListNotes { token, profile_id } => {
            Outcome::ListNotes(client.list_notes(token.expose(), &profile_id))
        }
        ClientRequest::CreateNote {
            token,
            profile_id,
            req,
        } => Outcome::CreateNote(client.create_note(token.expose(), &profile_id, &req)),
        ClientRequest::GetNote {
            token,
            profile_id,
            note_id,
        } => Outcome::GetNote(client.get_note(token.expose(), &profile_id, &note_id)),
        ClientRequest::UpdateNote {
            token,
            profile_id,
            note_id,
            req,
        } => Outcome::UpdateNote(client.update_note(token.expose(), &profile_id, &note_id, &req)),
        ClientRequest::DeleteNote {
            token,
            profile_id,
            note_id,
        } => Outcome::DeleteNote(client.delete_note(token.expose(), &profile_id, &note_id)),
        ClientRequest::GetTimerConfig { token } => {
            Outcome::GetTimerConfig(client.get_timer_config(token.expose()))
        }
        ClientRequest::UpdateTimerConfig { token, req } => {
            Outcome::UpdateTimerConfig(client.update_timer_config(token.expose(), &req))
        }
        ClientRequest::GetTimerSession { token } => {
            Outcome::GetTimerSession(client.get_timer_session(token.expose()))
        }
        ClientRequest::StartTimerSession { token } => {
            Outcome::StartTimerSession(client.start_timer_session(token.expose()))
        }
        ClientRequest::StopTimerSession { token } => {
            Outcome::StopTimerSession(client.stop_timer_session(token.expose()))
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

/// A UTC ISO-8601 timestamp on **today's** civil day (the day the driven flow's
/// [`current_day_number`](tui::app::current_day_number) reads from the wall clock), at the given
/// `HH:MM:SS`. The task-list today/older split (ADR-0014 §4) groups by civil day against that
/// wall-clock "today"; the pure seams take an injected `today_day`, but the `App`-driven path reads
/// the clock, so a driven test that wants its tasks in the *today* group builds their `created_at`
/// with this — deterministic to the day, independent of when the suite runs. The `HH:MM:SS` lets a
/// test still pin relative newest-first order within today (larger time = newer).
pub fn today_at(hms: &str) -> String {
    let today = tui::app::current_day_number();
    let (year, month, day) = tui::ui::civil_from_days(today);
    format!("{year:04}-{month:02}-{day:02}T{hms}Z")
}

/// An open [`Task`] created **today** (see [`today_at`]) at the given `HH:MM:SS`, so a driven flow
/// renders it in the today group (expanded, above any "Older tasks" separator).
pub fn today_open_task(id: &str, title: &str, hms: &str) -> Task {
    open_task(id, title, &today_at(hms))
}

/// A UTC ISO-8601 timestamp on the civil day `day_number` (days since the Unix epoch, [`day_number`
/// via `tui::app::current_day_number`]), at the given `HH:MM:SS`. The 0023 date-window and the
/// filter-by-day flows anchor on a civil day-number (which may be today or a chosen past day), so a
/// window/filter test builds fixtures relative to *now* — e.g. `today − 2` for an older-but-in-window
/// task, or a fixed past anchor day for the `f` filter — via this deterministic, chrono-free helper.
pub fn iso_at_day(day_number: i64, hms: &str) -> String {
    let (year, month, day) = tui::ui::civil_from_days(day_number);
    format!("{year:04}-{month:02}-{day:02}T{hms}Z")
}

/// An open [`Task`] created on the civil day `day_number` (see [`iso_at_day`]). Used by the 0023
/// window/filter suite to land a fixture on a precise day relative to today (e.g. `today − 4` to sit
/// outside the default 3-day window, or on the selected filter anchor day).
pub fn open_task_on_day(id: &str, title: &str, day_number: i64, hms: &str) -> Task {
    open_task(id, title, &iso_at_day(day_number, hms))
}

/// A done [`Task`] created **today** (see [`today_at`]); `closed_at` is fixed on the same day.
pub fn today_done_task(id: &str, title: &str, hms: &str) -> Task {
    let created = today_at(hms);
    done_task(id, title, &created, &created)
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

/// Build an open [`Subtask`] under parent task `task_id` from canonical wire JSON (so its status
/// is parsed by the `contract` derive). A sub-task carries only id/task_id/title/status.
pub fn open_subtask(id: &str, task_id: &str, title: &str) -> Subtask {
    subtask(id, task_id, title, "open")
}

/// Build a done [`Subtask`] under parent task `task_id` from canonical wire JSON.
pub fn done_subtask(id: &str, task_id: &str, title: &str) -> Subtask {
    subtask(id, task_id, title, "done")
}

/// Build a [`Subtask`] with the given status string (`open`/`done`) from canonical wire JSON.
fn subtask(id: &str, task_id: &str, title: &str, status: &str) -> Subtask {
    let json = serde_json::json!({
        "id": id,
        "task_id": task_id,
        "title": title,
        "status": status,
    });
    serde_json::from_value(json).expect("valid subtask json")
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

/// Render the app onto a `TestBackend` and return the on-screen caret cell the draw layer placed
/// via `frame.set_cursor_position` — the `(x, y)` column/row of the visible terminal cursor for the
/// focused text field (feature 0025). Rendered at tick 0.
///
/// A screen with no focused text field sets no cursor position, so the draw hides the cursor and
/// this returns the backend's default `(0, 0)` (a border corner, never a valid caret cell); use
/// this helper only where a caret is expected, and assert its exact cell.
#[must_use]
pub fn render_cursor(app: &App, width: u16, height: u16) -> (u16, u16) {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    let _completed = terminal
        .draw(|frame| tui::ui::draw(frame, app, 0))
        .expect("draw");
    let pos = terminal.get_cursor_position().expect("cursor position");
    (pos.x, pos.y)
}

/// Render the app onto a `TestBackend` and return the raw `ratatui` [`Buffer`], so a test can
/// inspect per-cell styling (e.g. the purple focus-border foreground) rather than only the flat
/// text. Rendered at tick 0.
#[must_use]
pub fn render_buffer(app: &App, width: u16, height: u16) -> Buffer {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    let _completed = terminal
        .draw(|frame| tui::ui::draw(frame, app, 0))
        .expect("draw");
    terminal.backend().buffer().clone()
}

/// The number of cells carrying `fg` as their foreground colour on the buffer row that contains
/// the first occurrence of `label`. A bordered field's title sits on the top border row, so when
/// the field is focused that whole row's border cells carry the purple border style — many `fg`
/// cells; a non-focused field's title row carries only the surrounding chrome's `fg` (e.g. the two
/// magenta edge columns of an enclosing dialog box), far fewer. Comparing the count between a
/// focused and a non-focused field's row distinguishes the focus cue robustly inside a dialog
/// (whose outer box border is itself magenta). Returns `0` if the label is not found.
#[must_use]
pub fn row_fg_count(buffer: &Buffer, label: &str, fg: ratatui::style::Color) -> usize {
    let area = buffer.area();
    let flat = buffer_text(buffer);
    let Some(row) = flat.lines().position(|line| line.contains(label)) else {
        return 0;
    };
    let Ok(y) = u16::try_from(row) else {
        return 0;
    };
    if y >= area.height {
        return 0;
    }
    (0..area.width).filter(|&x| buffer[(x, y)].fg == fg).count()
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

/// Convenience: the active screen's name, for diagnostics. The post-auth tabbed view names the
/// active tab so a failing assertion still pinpoints which pane was showing.
pub fn screen_name(app: &App) -> &'static str {
    match app.screen() {
        Screen::Auth(_) => "auth",
        Screen::Main(main) => match main.active_tab {
            Tab::Tasks => "main:tasks",
            Tab::Notes => "main:notes",
            Tab::Profiles => "main:profiles",
        },
        Screen::Offline { .. } => "offline",
    }
}

// ---- Post-auth tabbed-view accessors (ADR-0010 §1) ----
//
// The three list screens are gone: Tasks/Notes/Profiles are now panes of one
// `Screen::Main(Box<MainState>)`. These accessors read the active tab's pane so a suite can assert
// on it exactly as it used to assert on `Screen::TaskList(_)` etc. Each panics with a diagnostic if
// the app is not on the tabbed view (the suites use these only after reaching it).

/// The post-auth tabbed view, panicking if the app is not on it.
pub fn main_state(app: &App) -> &MainState {
    match app.screen() {
        Screen::Main(main) => main,
        other => panic!("expected the post-auth tabbed view, got {other:?}"),
    }
}

/// Whether the app is on the post-auth tabbed view with the given tab active.
#[must_use]
pub fn on_tab(app: &App, tab: Tab) -> bool {
    matches!(app.screen(), Screen::Main(main) if main.active_tab == tab)
}

/// The Tasks pane of the post-auth tabbed view, panicking if the view is not showing.
pub fn tasks_pane(app: &App) -> &TaskListState {
    &main_state(app).tasks
}

/// The Notes pane of the post-auth tabbed view, panicking if the view is not showing.
pub fn notes_pane(app: &App) -> &NotesState {
    &main_state(app).notes
}

/// The Profiles pane of the post-auth tabbed view, panicking if the view is not showing.
pub fn profiles_pane(app: &App) -> &ProfilesState {
    &main_state(app).profiles
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
//
// Post-auth (ADR-0010 §1) the three list screens are panes of one `Screen::Main(Box<MainState>)`.
// A builder for "the task list" therefore constructs the tabbed view with the Tasks tab active and
// the other panes empty; the `task_list_screen*` / `profiles_screen*` names are kept so the
// keybinding suite reads unchanged.

/// The screen-derivable part of [`App::overlay_capturing_input`](tui::app::App), for the pure
/// `map_key` keybinding tests. `map_key` takes the unified overlay-capturing predicate as a
/// parameter; the production value comes from `App`, but these tests build a bare `Screen`, so this
/// mirrors the predicate's screen-driven branch (`help_open` / timer-editing are passed separately
/// by the caller). It is `true` whenever the active pane has an add/edit/confirm-delete sub-flow
/// open; `false` on the auth/offline screens (the auth form is its own always-text-entry context,
/// not an overlay).
#[must_use]
pub fn screen_overlay_capturing(screen: &Screen) -> bool {
    match screen {
        Screen::Main(main) => match main.active_tab {
            Tab::Tasks => {
                main.tasks.adding.is_some()
                    || main.tasks.editing.is_some()
                    || main.tasks.confirming_delete.is_some()
                    || main.tasks.detail.is_some()
            }
            // An open note detail view counts as input-capturing too (it suppresses globals and
            // makes `Esc` two-tiered), mirroring `App::overlay_capturing_input`.
            Tab::Notes => main.notes.in_sub_flow() || main.notes.detail_open(),
            Tab::Profiles => main.profiles.in_sub_flow(),
        },
        Screen::Auth(_) | Screen::Offline { .. } => false,
    }
}

/// A bare empty task pane.
fn empty_tasks_pane() -> TaskListState {
    TaskListState {
        tasks: Vec::new(),
        subtasks: Vec::new(),
        selected: None,
        collapse_overrides: HashMap::new(),
        adding: None,
        editing: None,
        adding_subtask: None,
        editing_subtask: None,
        detail: None,
        confirming_delete: None,
        message: None,
        pending: None,
        hide_older: false,
        hide_window_days: tui::app::task_list::DEFAULT_HIDE_WINDOW_DAYS,
        filter_date: None,
        editing_window: None,
        filtering_date: None,
    }
}

/// A task pane with one open task selected, no sub-flow.
fn one_task_pane() -> TaskListState {
    TaskListState {
        tasks: vec![open_task(
            "00000000-0000-0000-0000-000000000001",
            "a task",
            "2026-06-18T10:00:00Z",
        )],
        subtasks: Vec::new(),
        selected: Some(0),
        collapse_overrides: HashMap::new(),
        adding: None,
        editing: None,
        adding_subtask: None,
        editing_subtask: None,
        detail: None,
        confirming_delete: None,
        message: None,
        pending: None,
        hide_older: false,
        hide_window_days: tui::app::task_list::DEFAULT_HIDE_WINDOW_DAYS,
        filter_date: None,
        editing_window: None,
        filtering_date: None,
    }
}

/// A profiles pane listing two profiles in the bare list mode, first selected.
fn two_profiles_pane() -> ProfilesState {
    ProfilesState {
        profiles: vec![profile("p1", "work"), profile("p2", "personal")],
        selected: Some(0),
        mode: ProfilesMode::List,
        message: None,
        pending: None,
    }
}

/// Wrap three panes into the post-auth tabbed view with `active` selected.
fn main_screen(active: Tab, tasks: TaskListState, profiles: ProfilesState) -> Screen {
    main_screen_full(active, tasks, NotesState::new(Vec::new()), profiles)
}

/// Wrap all three panes (including a populated notes pane) into the tabbed view with `active`
/// selected.
fn main_screen_full(
    active: Tab,
    tasks: TaskListState,
    notes: NotesState,
    profiles: ProfilesState,
) -> Screen {
    let mut main = MainState::new(tasks, notes, profiles);
    main.active_tab = active;
    Screen::Main(Box::new(main))
}

/// The auth (login) screen, idle (no request in flight).
pub fn auth_screen() -> Screen {
    Screen::Auth(AuthState {
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

/// The post-auth tabbed view on the Tasks tab with one open task and no sub-flow, idle.
pub fn task_list_screen() -> Screen {
    main_screen(Tab::Tasks, one_task_pane(), two_profiles_pane())
}

/// The tabbed view on the Tasks tab with the active pane's request outstanding.
pub fn task_list_screen_pending() -> Screen {
    let mut tasks = one_task_pane();
    tasks.pending = Some(RequestId(0));
    main_screen(Tab::Tasks, tasks, two_profiles_pane())
}

/// The tabbed view on the Tasks tab with the add-task sub-flow open (a text-entry context).
pub fn task_list_screen_adding() -> Screen {
    let mut tasks = empty_tasks_pane();
    tasks.adding = Some(AddTaskState {
        on_title: true,
        title: TextInput::default(),
        description: TextInput::default(),
        error: None,
    });
    main_screen(Tab::Tasks, tasks, two_profiles_pane())
}

/// The tabbed view on the Tasks tab with the edit-task sub-flow open (a text-entry context).
pub fn task_list_screen_editing() -> Screen {
    let mut tasks = one_task_pane();
    tasks.editing = Some(EditTaskState {
        task_id: "00000000-0000-0000-0000-000000000001".to_owned(),
        on_title: true,
        title: TextInput::new("a task"),
        description: TextInput::default(),
        error: None,
    });
    main_screen(Tab::Tasks, tasks, two_profiles_pane())
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

/// The tabbed view on the Profiles tab listing two profiles, bare list mode, idle.
pub fn profiles_screen() -> Screen {
    main_screen(Tab::Profiles, empty_tasks_pane(), two_profiles_pane())
}

/// The tabbed view on the Profiles tab with the active pane's request outstanding.
pub fn profiles_screen_pending() -> Screen {
    let mut profiles = two_profiles_pane();
    profiles.pending = Some(RequestId(0));
    main_screen(Tab::Profiles, empty_tasks_pane(), profiles)
}

/// The tabbed view on the Profiles tab with the create sub-flow open (a text-entry context).
pub fn profiles_screen_creating() -> Screen {
    let mut profiles = two_profiles_pane();
    profiles.mode = ProfilesMode::Creating(ProfileForm {
        name: TextInput::default(),
        error: None,
    });
    main_screen(Tab::Profiles, empty_tasks_pane(), profiles)
}

/// The tabbed view on the Profiles tab with the rename sub-flow open (a text-entry context).
pub fn profiles_screen_renaming() -> Screen {
    let mut profiles = two_profiles_pane();
    profiles.mode = ProfilesMode::Renaming {
        profile_id: "p1".to_owned(),
        form: ProfileForm {
            name: TextInput::new("work"),
            error: None,
        },
    };
    main_screen(Tab::Profiles, empty_tasks_pane(), profiles)
}

/// The tabbed view on the Notes tab, bare list mode, idle. Used by the keybinding suite to pin the
/// Notes-tab command keys.
pub fn notes_screen() -> Screen {
    main_screen(Tab::Notes, empty_tasks_pane(), two_profiles_pane())
}

/// The tabbed view on the Tasks tab with the delete-confirmation dialog armed against the selected
/// **task** (a non-text-entry overlay that still captures input — the two-step confirm affordance,
/// Assumption A5). The armed target is a [`DeleteTarget::Task`] (the sub-task-armed variant is
/// exercised by the driven delete-confirm flow in `tasks.rs`).
pub fn task_list_screen_confirming_delete() -> Screen {
    let mut tasks = one_task_pane();
    tasks.confirming_delete = Some(DeleteTarget::Task {
        task_id: "00000000-0000-0000-0000-000000000001".to_owned(),
    });
    main_screen(Tab::Tasks, tasks, two_profiles_pane())
}

/// A notes pane listing one note in the bare list mode, first selected.
fn one_note_pane() -> NotesState {
    let mut notes = NotesState::new(vec![note("n1", "a note", "body", "2026-06-18T10:00:00Z")]);
    notes.selected = Some(0);
    notes
}

/// The tabbed view on the Notes tab with the create sub-flow open (a text-entry overlay).
pub fn notes_screen_creating() -> Screen {
    let mut notes = one_note_pane();
    notes.mode = NotesMode::Creating(NoteForm {
        on_title: true,
        title: TextInput::default(),
        content: TextInput::default(),
        error: None,
    });
    main_screen_full(Tab::Notes, empty_tasks_pane(), notes, two_profiles_pane())
}

/// The tabbed view on the Notes tab with the edit sub-flow open (a text-entry overlay).
pub fn notes_screen_editing() -> Screen {
    let mut notes = one_note_pane();
    notes.mode = NotesMode::Editing {
        note_id: "n1".to_owned(),
        form: NoteForm {
            on_title: true,
            title: TextInput::new("a note"),
            content: TextInput::new("body"),
            error: None,
        },
    };
    main_screen_full(Tab::Notes, empty_tasks_pane(), notes, two_profiles_pane())
}

/// The tabbed view on the Notes tab with the delete-confirmation dialog open (a non-text-entry
/// overlay that still captures input — globals suppressed, `Enter` confirms, `Esc` cancels).
pub fn notes_screen_confirming_delete() -> Screen {
    let mut notes = one_note_pane();
    notes.mode = NotesMode::ConfirmingDelete {
        note_id: "n1".to_owned(),
        title: "a note".to_owned(),
    };
    main_screen_full(Tab::Notes, empty_tasks_pane(), notes, two_profiles_pane())
}

/// The tabbed view on the Notes tab with the per-field detail view open and no field edit in
/// progress (the idle detail; `Title` focused). Used by the keybinding suite to pin that `Enter`
/// stays `Submit` (commit/open) when no Content edit is active.
pub fn notes_screen_detail_idle() -> Screen {
    let detail = NoteDetail::new(note("n1", "a note", "body", "2026-06-18T10:00:00Z"));
    notes_detail_screen(detail)
}

/// The tabbed view on the Notes tab editing the single-line `Title` pane of the detail view.
/// `Enter` must stay `Submit` here (Title commits on Enter, ADR-0011 §2).
pub fn notes_screen_editing_title() -> Screen {
    let mut detail = NoteDetail::new(note("n1", "a note", "body", "2026-06-18T10:00:00Z"));
    detail.focus_pane(NotePane::Title);
    detail.begin_edit();
    notes_detail_screen(detail)
}

/// The tabbed view on the Notes tab editing the multiline `Content` pane of the detail view — the
/// sole context where `Enter` maps to `Newline` and `Ctrl+S` commits (ADR-0011 §2).
pub fn notes_screen_editing_content() -> Screen {
    let mut detail = NoteDetail::new(note("n1", "a note", "body", "2026-06-18T10:00:00Z"));
    detail.focus_pane(NotePane::Content);
    detail.begin_edit();
    notes_detail_screen(detail)
}

/// Wrap a [`NoteDetail`] into the Notes-tab tabbed view in `NotesMode::Detail`.
fn notes_detail_screen(detail: NoteDetail) -> Screen {
    let mut notes = one_note_pane();
    notes.mode = NotesMode::Detail(detail);
    main_screen_full(Tab::Notes, empty_tasks_pane(), notes, two_profiles_pane())
}

/// The tabbed view on the Profiles tab with the delete-confirmation dialog open (a non-text-entry
/// overlay that still captures input).
pub fn profiles_screen_confirming_delete() -> Screen {
    let mut profiles = two_profiles_pane();
    profiles.mode = ProfilesMode::ConfirmingDelete {
        profile_id: "p1".to_owned(),
        name: "work".to_owned(),
    };
    main_screen(Tab::Profiles, empty_tasks_pane(), profiles)
}
