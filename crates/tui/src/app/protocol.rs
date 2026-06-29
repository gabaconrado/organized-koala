//! The request/response protocol between the pure [`App`](super::App) core and the effectful
//! worker thread (ADR-0006).
//!
//! [`App::handle_event`](super::App::handle_event) returns an [`Option<ClientRequest>`]: a
//! request-triggering event yields the [`ClientRequest`] to execute, never calling the client
//! itself. The edge stamps it with a [`RequestId`], runs it on the worker thread, and feeds the
//! [`ClientResponse`] back to [`App::apply_response`](super::App::apply_response). These types
//! are the core's transport-agnostic request language — they carry owned [`contract`] payloads
//! and the bearer token, never a live connection.

use contract::{
    CreateNoteRequest, CreateProfileRequest, CreateSubtaskRequest, CreateTaskRequest, LoginRequest,
    Note, Profile, RegisterRequest, SessionResponse, Subtask, Task, TimerConfig, TimerSession,
    UpdateNoteRequest, UpdateProfileRequest, UpdateSubtaskRequest, UpdateTaskRequest,
    UpdateTimerConfigRequest,
};

use crate::app::token::SessionToken;
use crate::client::ClientResult;

/// A monotonically increasing identifier the edge stamps onto each in-flight request, so a
/// stale response (one whose request was cancelled or superseded) can be dropped on arrival.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RequestId(pub u64);

/// A unit of work the core wants executed against the server. Mirrors the
/// [`Client`](crate::client::Client) trait's methods, carrying owned payloads and the bearer
/// token for authenticated calls. The core returns these from
/// [`handle_event`](super::App::handle_event); the worker thread maps each to the corresponding
/// synchronous client call and returns a [`ClientResponse`].
#[derive(Debug, Clone)]
pub enum ClientRequest {
    /// `GET /healthz` — re-probe connectivity (the offline-screen retry).
    Health,
    /// `POST /api/auth/register`.
    Register(RegisterRequest),
    /// `POST /api/auth/login`.
    Login(LoginRequest),
    /// `GET /api/profiles` — carries the freshly-issued token to chain the post-auth load.
    ListProfiles {
        /// The bearer token to authenticate with.
        token: SessionToken,
    },
    /// `POST /api/profiles` — create a profile (the switcher's create sub-flow).
    CreateProfile {
        /// The bearer token to authenticate with.
        token: SessionToken,
        /// The profile to create.
        req: CreateProfileRequest,
    },
    /// `PATCH /api/profiles/{profile_id}` — rename a profile (the switcher's rename sub-flow).
    UpdateProfile {
        /// The bearer token to authenticate with.
        token: SessionToken,
        /// The profile to rename.
        profile_id: String,
        /// The new name.
        req: UpdateProfileRequest,
    },
    /// `DELETE /api/profiles/{profile_id}` — delete a profile (the switcher's delete sub-flow).
    DeleteProfile {
        /// The bearer token to authenticate with.
        token: SessionToken,
        /// The profile to delete.
        profile_id: String,
    },
    /// `GET /api/profiles/{profile_id}/tasks`.
    ListTasks {
        /// The bearer token to authenticate with.
        token: SessionToken,
        /// The profile namespace to list.
        profile_id: String,
    },
    /// `POST /api/profiles/{profile_id}/tasks`.
    CreateTask {
        /// The bearer token to authenticate with.
        token: SessionToken,
        /// The profile namespace to create the task in.
        profile_id: String,
        /// The task to create.
        req: CreateTaskRequest,
    },
    /// `PATCH /api/profiles/{profile_id}/tasks/{task_id}` — partial update (edit, toggle-done,
    /// reopen).
    UpdateTask {
        /// The bearer token to authenticate with.
        token: SessionToken,
        /// The profile namespace owning the task.
        profile_id: String,
        /// The task to update.
        task_id: String,
        /// The fields to change (all-optional partial update).
        req: UpdateTaskRequest,
    },
    /// `DELETE /api/profiles/{profile_id}/tasks/{task_id}`.
    DeleteTask {
        /// The bearer token to authenticate with.
        token: SessionToken,
        /// The profile namespace owning the task.
        profile_id: String,
        /// The task to delete.
        task_id: String,
    },
    /// `GET /api/profiles/{profile_id}/subtasks` — every sub-task in the profile (the Tasks-tab
    /// tree load's second call; grouped under parents client-side).
    ListSubtasks {
        /// The bearer token to authenticate with.
        token: SessionToken,
        /// The profile namespace to list.
        profile_id: String,
    },
    /// `GET /api/profiles/{profile_id}/tasks/{task_id}/subtasks` — one parent task's sub-tasks
    /// (the Task Detail "Sub-tasks" section's load).
    ListTaskSubtasks {
        /// The bearer token to authenticate with.
        token: SessionToken,
        /// The profile namespace owning the task.
        profile_id: String,
        /// The parent task to list sub-tasks for.
        task_id: String,
    },
    /// `POST /api/profiles/{profile_id}/tasks/{task_id}/subtasks`.
    CreateSubtask {
        /// The bearer token to authenticate with.
        token: SessionToken,
        /// The profile namespace owning the parent task.
        profile_id: String,
        /// The parent task to create the sub-task under.
        task_id: String,
        /// The sub-task to create.
        req: CreateSubtaskRequest,
    },
    /// `PATCH /api/profiles/{profile_id}/tasks/{task_id}/subtasks/{subtask_id}` — partial update
    /// (edit title and/or toggle status).
    UpdateSubtask {
        /// The bearer token to authenticate with.
        token: SessionToken,
        /// The profile namespace owning the parent task.
        profile_id: String,
        /// The parent task owning the sub-task.
        task_id: String,
        /// The sub-task to update.
        subtask_id: String,
        /// The fields to change (all-optional partial update).
        req: UpdateSubtaskRequest,
    },
    /// `DELETE /api/profiles/{profile_id}/tasks/{task_id}/subtasks/{subtask_id}`.
    DeleteSubtask {
        /// The bearer token to authenticate with.
        token: SessionToken,
        /// The profile namespace owning the parent task.
        profile_id: String,
        /// The parent task owning the sub-task.
        task_id: String,
        /// The sub-task to delete.
        subtask_id: String,
    },
    /// `GET /api/profiles/{profile_id}/notes`.
    ListNotes {
        /// The bearer token to authenticate with.
        token: SessionToken,
        /// The profile namespace to list.
        profile_id: String,
    },
    /// `POST /api/profiles/{profile_id}/notes`.
    CreateNote {
        /// The bearer token to authenticate with.
        token: SessionToken,
        /// The profile namespace to create the note in.
        profile_id: String,
        /// The note to create.
        req: CreateNoteRequest,
    },
    /// `GET /api/profiles/{profile_id}/notes/{note_id}`.
    GetNote {
        /// The bearer token to authenticate with.
        token: SessionToken,
        /// The profile namespace owning the note.
        profile_id: String,
        /// The note to read.
        note_id: String,
    },
    /// `PATCH /api/profiles/{profile_id}/notes/{note_id}`.
    UpdateNote {
        /// The bearer token to authenticate with.
        token: SessionToken,
        /// The profile namespace owning the note.
        profile_id: String,
        /// The note to update.
        note_id: String,
        /// The replacement title+content.
        req: UpdateNoteRequest,
    },
    /// `DELETE /api/profiles/{profile_id}/notes/{note_id}`.
    DeleteNote {
        /// The bearer token to authenticate with.
        token: SessionToken,
        /// The profile namespace owning the note.
        profile_id: String,
        /// The note to delete.
        note_id: String,
    },
    /// `GET /api/timer/config` — read the account-global duration config.
    GetTimerConfig {
        /// The bearer token to authenticate with.
        token: SessionToken,
    },
    /// `PUT /api/timer/config` — update the global session duration.
    UpdateTimerConfig {
        /// The bearer token to authenticate with.
        token: SessionToken,
        /// The new duration to set.
        req: UpdateTimerConfigRequest,
    },
    /// `GET /api/timer/session` — read the current focus session.
    GetTimerSession {
        /// The bearer token to authenticate with.
        token: SessionToken,
    },
    /// `POST /api/timer/session/start` — start (or restart) a focus session.
    StartTimerSession {
        /// The bearer token to authenticate with.
        token: SessionToken,
    },
    /// `POST /api/timer/session/stop` — stop the active session (resets to idle).
    StopTimerSession {
        /// The bearer token to authenticate with.
        token: SessionToken,
    },
}

/// The outcome of a [`ClientRequest`], paired with the [`RequestId`] it was dispatched under so
/// the core can reject a stale response. Each variant carries the corresponding client result so
/// [`apply_response`](super::App::apply_response) runs the same success / error-code branching the
/// pre-split inline code ran.
#[derive(Debug)]
pub enum Outcome {
    /// Result of a [`ClientRequest::Health`] probe.
    Health(ClientResult<()>),
    /// Result of a [`ClientRequest::Register`] call.
    Register(ClientResult<SessionResponse>),
    /// Result of a [`ClientRequest::Login`] call.
    Login(ClientResult<SessionResponse>),
    /// Result of a [`ClientRequest::ListProfiles`] call, carrying back the token it used so the
    /// core can establish the session on success without re-deriving it.
    ListProfiles {
        /// The bearer token the request used.
        token: SessionToken,
        /// The profiles returned (or the error).
        result: ClientResult<Vec<Profile>>,
    },
    /// Result of a [`ClientRequest::CreateProfile`] call.
    CreateProfile(ClientResult<Profile>),
    /// Result of a [`ClientRequest::UpdateProfile`] call.
    UpdateProfile(ClientResult<Profile>),
    /// Result of a [`ClientRequest::DeleteProfile`] call (`204` carries no body).
    DeleteProfile(ClientResult<()>),
    /// Result of a [`ClientRequest::ListTasks`] call.
    ListTasks(ClientResult<Vec<Task>>),
    /// Result of a [`ClientRequest::CreateTask`] call.
    CreateTask(ClientResult<Task>),
    /// Result of a [`ClientRequest::UpdateTask`] call.
    UpdateTask(ClientResult<Task>),
    /// Result of a [`ClientRequest::DeleteTask`] call (`204` carries no body).
    DeleteTask(ClientResult<()>),
    /// Result of a [`ClientRequest::ListSubtasks`] call (the profile's whole sub-task set).
    ListSubtasks(ClientResult<Vec<Subtask>>),
    /// Result of a [`ClientRequest::ListTaskSubtasks`] call (one parent task's sub-tasks).
    ListTaskSubtasks(ClientResult<Vec<Subtask>>),
    /// Result of a [`ClientRequest::CreateSubtask`] call.
    CreateSubtask(ClientResult<Subtask>),
    /// Result of a [`ClientRequest::UpdateSubtask`] call.
    UpdateSubtask(ClientResult<Subtask>),
    /// Result of a [`ClientRequest::DeleteSubtask`] call (`204` carries no body).
    DeleteSubtask(ClientResult<()>),
    /// Result of a [`ClientRequest::ListNotes`] call.
    ListNotes(ClientResult<Vec<Note>>),
    /// Result of a [`ClientRequest::CreateNote`] call.
    CreateNote(ClientResult<Note>),
    /// Result of a [`ClientRequest::GetNote`] call.
    GetNote(ClientResult<Note>),
    /// Result of a [`ClientRequest::UpdateNote`] call.
    UpdateNote(ClientResult<Note>),
    /// Result of a [`ClientRequest::DeleteNote`] call.
    DeleteNote(ClientResult<()>),
    /// Result of a [`ClientRequest::GetTimerConfig`] call.
    GetTimerConfig(ClientResult<TimerConfig>),
    /// Result of a [`ClientRequest::UpdateTimerConfig`] call.
    UpdateTimerConfig(ClientResult<TimerConfig>),
    /// Result of a [`ClientRequest::GetTimerSession`] call.
    GetTimerSession(ClientResult<TimerSession>),
    /// Result of a [`ClientRequest::StartTimerSession`] call.
    StartTimerSession(ClientResult<TimerSession>),
    /// Result of a [`ClientRequest::StopTimerSession`] call.
    StopTimerSession(ClientResult<TimerSession>),
}

/// A completed [`ClientRequest`]: the [`RequestId`] it ran under plus its [`Outcome`]. The edge
/// feeds this to [`apply_response`](super::App::apply_response), which drops it if the id no
/// longer matches the awaited request (cancelled or superseded).
#[derive(Debug)]
pub struct ClientResponse {
    /// The id the request was dispatched under.
    pub id: RequestId,
    /// The result of running it.
    pub outcome: Outcome,
}
