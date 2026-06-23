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
    CreateTaskRequest, LoginRequest, Profile, RegisterRequest, SessionResponse, Task, TimerConfig,
    TimerSession, UpdateTimerConfigRequest,
};

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
        token: String,
    },
    /// `GET /api/profiles/{profile_id}/tasks`.
    ListTasks {
        /// The bearer token to authenticate with.
        token: String,
        /// The profile namespace to list.
        profile_id: String,
    },
    /// `POST /api/profiles/{profile_id}/tasks`.
    CreateTask {
        /// The bearer token to authenticate with.
        token: String,
        /// The profile namespace to create the task in.
        profile_id: String,
        /// The task to create.
        req: CreateTaskRequest,
    },
    /// `POST /api/profiles/{profile_id}/tasks/{task_id}/close`.
    CloseTask {
        /// The bearer token to authenticate with.
        token: String,
        /// The profile namespace owning the task.
        profile_id: String,
        /// The task to close.
        task_id: String,
    },
    /// `GET /api/timer/config` — read the account-global duration config.
    GetTimerConfig {
        /// The bearer token to authenticate with.
        token: String,
    },
    /// `PUT /api/timer/config` — update the global session duration.
    UpdateTimerConfig {
        /// The bearer token to authenticate with.
        token: String,
        /// The new duration to set.
        req: UpdateTimerConfigRequest,
    },
    /// `GET /api/timer/session` — read the current focus session.
    GetTimerSession {
        /// The bearer token to authenticate with.
        token: String,
    },
    /// `POST /api/timer/session/start` — start (or restart) a focus session.
    StartTimerSession {
        /// The bearer token to authenticate with.
        token: String,
    },
    /// `POST /api/timer/session/stop` — stop the active session (resets to idle).
    StopTimerSession {
        /// The bearer token to authenticate with.
        token: String,
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
        token: String,
        /// The profiles returned (or the error).
        result: ClientResult<Vec<Profile>>,
    },
    /// Result of a [`ClientRequest::ListTasks`] call.
    ListTasks(ClientResult<Vec<Task>>),
    /// Result of a [`ClientRequest::CreateTask`] call.
    CreateTask(ClientResult<Task>),
    /// Result of a [`ClientRequest::CloseTask`] call.
    CloseTask(ClientResult<Task>),
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
