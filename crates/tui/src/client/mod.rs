//! The HTTP boundary: a typed [`Client`] over the server's endpoints, and the `reqwest`
//! implementation [`HttpClient`].
//!
//! The trait abstracts the transport so the app core can be driven against a fake in tests
//! while the binary uses the real `reqwest` client. Every method consumes and returns
//! [`contract`] DTOs, and every failure is a typed [`ClientError`] that preserves the
//! server's machine-matchable [`ErrorCode`](contract::ErrorCode).

pub mod notify;
pub mod worker;

pub use notify::{DesktopNotifier, Notifier};

use contract::{
    CreateNoteRequest, CreateProfileRequest, CreateTaskRequest, ErrorBody, ErrorCode, LoginRequest,
    Note, Profile, RegisterRequest, SessionResponse, Task, TimerConfig, TimerSession,
    UpdateNoteRequest, UpdateProfileRequest, UpdateTaskRequest, UpdateTimerConfigRequest,
};

/// A failure from a client call.
///
/// The two cases the app core branches on are distinct: [`ClientError::Api`] is a structured
/// error the server returned (status + the standard `{ code, message }` body), while
/// [`ClientError::Offline`] means the request never reached a healthy server (connection
/// refused, DNS failure, timeout, or a non-JSON/garbled response). The app maps `Api` by its
/// [`ErrorCode`] and `Offline` to the blocking "server unreachable" screen.
#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    /// The server responded with a non-success status and the standard error body. The
    /// `code` (when present) is the machine-matchable identifier the app branches on.
    #[error("{message}")]
    Api {
        /// Optional machine-matchable error code from the server.
        code: Option<ErrorCode>,
        /// Human-readable message from the server.
        message: String,
    },
    /// The server could not be reached or returned an unintelligible response. Surfaced to
    /// the user as a blocking "server offline" message with a manual retry; never cached.
    #[error("the server is unreachable: {0}")]
    Offline(String),
}

impl ClientError {
    /// The error code, if the server returned a structured API error with one.
    ///
    /// # Examples
    ///
    /// ```
    /// use contract::ErrorCode;
    /// use tui::client::ClientError;
    ///
    /// let err = ClientError::Api {
    ///     code: Some(ErrorCode::Unauthenticated),
    ///     message: "token expired".to_owned(),
    /// };
    /// assert_eq!(err.code(), Some(&ErrorCode::Unauthenticated));
    ///
    /// let offline = ClientError::Offline("connection refused".to_owned());
    /// assert_eq!(offline.code(), None);
    /// ```
    #[must_use]
    pub fn code(&self) -> Option<&ErrorCode> {
        match self {
            Self::Api { code, .. } => code.as_ref(),
            Self::Offline(_) => None,
        }
    }

    /// Whether this is an [`ClientError::Offline`] failure (server unreachable).
    ///
    /// # Examples
    ///
    /// ```
    /// use tui::client::ClientError;
    ///
    /// assert!(ClientError::Offline("refused".to_owned()).is_offline());
    /// ```
    #[must_use]
    pub fn is_offline(&self) -> bool {
        matches!(self, Self::Offline(_))
    }
}

/// Result alias for client calls.
pub type ClientResult<T> = Result<T, ClientError>;

/// The server endpoints the TUI consumes, as a typed, injectable boundary.
///
/// Each method is a single round-trip to one endpoint. Authenticated calls take the session
/// `token` explicitly (the TUI holds it in memory only — never on disk). Implementations map
/// the standard error body to [`ClientError::Api`] preserving the
/// [`ErrorCode`](contract::ErrorCode), and any transport failure to [`ClientError::Offline`].
pub trait Client {
    /// `GET /healthz` — probe whether the server is online. `Ok(())` means a healthy
    /// response; an unreachable server is [`ClientError::Offline`].
    fn health(&self) -> ClientResult<()>;

    /// `POST /api/auth/register` — create an account and its default profile atomically,
    /// returning the session token.
    fn register(&self, req: &RegisterRequest) -> ClientResult<SessionResponse>;

    /// `POST /api/auth/login` — exchange credentials for a session token.
    fn login(&self, req: &LoginRequest) -> ClientResult<SessionResponse>;

    /// `GET /api/profiles` — the authenticated user's profiles, newest-first.
    fn list_profiles(&self, token: &str) -> ClientResult<Vec<Profile>>;

    /// `POST /api/profiles` — create a profile, returning the created [`Profile`] (`201`). A
    /// duplicate per-account name is a `profile_name_taken` [`ClientError::Api`]; an empty (after
    /// trim) name is `validation_failed`.
    fn create_profile(&self, token: &str, req: &CreateProfileRequest) -> ClientResult<Profile>;

    /// `PATCH /api/profiles/{profile_id}` — rename a profile, returning the updated [`Profile`]
    /// (`200`). A duplicate per-account name is `profile_name_taken`; an unowned/missing profile is
    /// `not_found`.
    fn rename_profile(
        &self,
        token: &str,
        profile_id: &str,
        req: &UpdateProfileRequest,
    ) -> ClientResult<Profile>;

    /// `DELETE /api/profiles/{profile_id}` — delete a profile (`204`, no body); cascades its tasks
    /// and notes server-side. Deleting the account's only remaining profile is a `last_profile`
    /// [`ClientError::Api`]; an unowned/missing profile is `not_found`.
    fn delete_profile(&self, token: &str, profile_id: &str) -> ClientResult<()>;

    /// `GET /api/profiles/{profile_id}/tasks` — the profile's tasks, newest-first.
    fn list_tasks(&self, token: &str, profile_id: &str) -> ClientResult<Vec<Task>>;

    /// `POST /api/profiles/{profile_id}/tasks` — create a task, returning the created [`Task`].
    fn create_task(
        &self,
        token: &str,
        profile_id: &str,
        req: &CreateTaskRequest,
    ) -> ClientResult<Task>;

    /// `PATCH /api/profiles/{profile_id}/tasks/{task_id}` — apply a partial update (title,
    /// description, and/or status) to a task, returning the updated [`Task`]. Setting
    /// `status: done` sets `closed_at`; `status: open` (reopen) clears it. A blank title is a
    /// `validation_failed` [`ClientError::Api`]; a missing/unowned task is `not_found`.
    fn update_task(
        &self,
        token: &str,
        profile_id: &str,
        task_id: &str,
        req: &UpdateTaskRequest,
    ) -> ClientResult<Task>;

    /// `DELETE /api/profiles/{profile_id}/tasks/{task_id}` — delete a task (`204`, no body). A
    /// missing/unowned task is a `not_found` [`ClientError::Api`].
    fn delete_task(&self, token: &str, profile_id: &str, task_id: &str) -> ClientResult<()>;

    /// `GET /api/profiles/{profile_id}/notes` — the profile's notes, newest-first.
    fn list_notes(&self, token: &str, profile_id: &str) -> ClientResult<Vec<Note>>;

    /// `POST /api/profiles/{profile_id}/notes` — create a note, returning the created [`Note`].
    /// An empty (after trim) title is a `validation_failed` [`ClientError::Api`].
    fn create_note(
        &self,
        token: &str,
        profile_id: &str,
        req: &CreateNoteRequest,
    ) -> ClientResult<Note>;

    /// `GET /api/profiles/{profile_id}/notes/{note_id}` — read one note. An unowned or missing
    /// note is a `not_found` [`ClientError::Api`].
    fn get_note(&self, token: &str, profile_id: &str, note_id: &str) -> ClientResult<Note>;

    /// `PATCH /api/profiles/{profile_id}/notes/{note_id}` — replace title+content in place,
    /// returning the updated [`Note`]. An unowned/missing note is `not_found`; an empty title is
    /// `validation_failed`.
    fn update_note(
        &self,
        token: &str,
        profile_id: &str,
        note_id: &str,
        req: &UpdateNoteRequest,
    ) -> ClientResult<Note>;

    /// `DELETE /api/profiles/{profile_id}/notes/{note_id}` — delete a note (`204`). An unowned or
    /// missing note is a `not_found` [`ClientError::Api`].
    fn delete_note(&self, token: &str, profile_id: &str, note_id: &str) -> ClientResult<()>;

    /// `GET /api/timer/config` — the account-global Pomodoro config (duration). Account-global,
    /// not profile-scoped (ADR-0002 §5), so it takes no `profile_id`.
    fn get_timer_config(&self, token: &str) -> ClientResult<TimerConfig>;

    /// `PUT /api/timer/config` — update the global session duration, returning the updated
    /// [`TimerConfig`]. An out-of-bounds duration is a `validation_failed` [`ClientError::Api`].
    fn update_timer_config(
        &self,
        token: &str,
        req: &UpdateTimerConfigRequest,
    ) -> ClientResult<TimerConfig>;

    /// `GET /api/timer/session` — the current focus session (idle / running / completed). The
    /// server is the authority for the running-vs-completed verdict (ADR-0002 §3).
    fn get_timer_session(&self, token: &str) -> ClientResult<TimerSession>;

    /// `POST /api/timer/session/start` — start (or restart) a focus session, returning the
    /// running session with its absolute `ends_at` and `server_now`.
    fn start_timer_session(&self, token: &str) -> ClientResult<TimerSession>;

    /// `POST /api/timer/session/stop` — stop the active session (no pause; resets to idle).
    /// Idempotent server-side.
    fn stop_timer_session(&self, token: &str) -> ClientResult<TimerSession>;
}

/// A blocking `reqwest` implementation of [`Client`] against a single server base URL.
///
/// Synchronous by design: the app core runs in a plain terminal loop, so no async runtime is
/// needed. Construct with [`HttpClient::new`] pointing at the server's base URL (no trailing
/// path), e.g. `http://127.0.0.1:8080`.
#[derive(Debug, Clone)]
pub struct HttpClient {
    base_url: String,
    http: reqwest::blocking::Client,
}

/// Client-side request timeout. Bounds how long an abandoned (user-cancelled) request occupies
/// the worker thread before its connection is torn down (ADR-0006 §4); a cancelled request's
/// response is dropped by `RequestId` mismatch regardless, this just frees the worker.
const REQUEST_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

impl HttpClient {
    /// Builds a client targeting `base_url` (scheme + host + port, no trailing slash).
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying `reqwest` client cannot be constructed.
    pub fn new(base_url: impl Into<String>) -> anyhow::Result<Self> {
        let http = reqwest::blocking::Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .build()?;
        Ok(Self {
            base_url: base_url.into(),
            http,
        })
    }

    fn url(&self, path: &str) -> String {
        format!("{}{path}", self.base_url)
    }
}

/// Map a transport-level `reqwest` error to [`ClientError::Offline`].
fn offline(err: reqwest::Error) -> ClientError {
    ClientError::Offline(err.to_string())
}

/// Read a non-success response into [`ClientError::Api`], parsing the standard error body and
/// falling back to a generic message if the body is missing or malformed.
fn api_error(status: reqwest::StatusCode, resp: reqwest::blocking::Response) -> ClientError {
    match resp.json::<ErrorBody>() {
        Ok(body) => ClientError::Api {
            code: body.code,
            message: body.message,
        },
        Err(_) => ClientError::Api {
            code: None,
            message: format!("server returned {status} with no error body"),
        },
    }
}

/// Decode a success response body as JSON, mapping a decode failure to [`ClientError::Offline`]
/// (an unintelligible body means we cannot trust the server's response).
fn decode<T: serde::de::DeserializeOwned>(resp: reqwest::blocking::Response) -> ClientResult<T> {
    resp.json::<T>().map_err(offline)
}

impl Client for HttpClient {
    fn health(&self) -> ClientResult<()> {
        let resp = self
            .http
            .get(self.url("/healthz"))
            .send()
            .map_err(offline)?;
        if resp.status().is_success() {
            Ok(())
        } else {
            Err(ClientError::Offline(format!(
                "health probe returned {}",
                resp.status()
            )))
        }
    }

    fn register(&self, req: &RegisterRequest) -> ClientResult<SessionResponse> {
        let resp = self
            .http
            .post(self.url("/api/auth/register"))
            .json(req)
            .send()
            .map_err(offline)?;
        let status = resp.status();
        if status.is_success() {
            decode(resp)
        } else {
            Err(api_error(status, resp))
        }
    }

    fn login(&self, req: &LoginRequest) -> ClientResult<SessionResponse> {
        let resp = self
            .http
            .post(self.url("/api/auth/login"))
            .json(req)
            .send()
            .map_err(offline)?;
        let status = resp.status();
        if status.is_success() {
            decode(resp)
        } else {
            Err(api_error(status, resp))
        }
    }

    fn list_profiles(&self, token: &str) -> ClientResult<Vec<Profile>> {
        let resp = self
            .http
            .get(self.url("/api/profiles"))
            .bearer_auth(token)
            .send()
            .map_err(offline)?;
        let status = resp.status();
        if status.is_success() {
            decode(resp)
        } else {
            Err(api_error(status, resp))
        }
    }

    fn create_profile(&self, token: &str, req: &CreateProfileRequest) -> ClientResult<Profile> {
        let resp = self
            .http
            .post(self.url("/api/profiles"))
            .bearer_auth(token)
            .json(req)
            .send()
            .map_err(offline)?;
        let status = resp.status();
        if status.is_success() {
            decode(resp)
        } else {
            Err(api_error(status, resp))
        }
    }

    fn rename_profile(
        &self,
        token: &str,
        profile_id: &str,
        req: &UpdateProfileRequest,
    ) -> ClientResult<Profile> {
        let resp = self
            .http
            .patch(self.url(&format!("/api/profiles/{profile_id}")))
            .bearer_auth(token)
            .json(req)
            .send()
            .map_err(offline)?;
        let status = resp.status();
        if status.is_success() {
            decode(resp)
        } else {
            Err(api_error(status, resp))
        }
    }

    fn delete_profile(&self, token: &str, profile_id: &str) -> ClientResult<()> {
        let resp = self
            .http
            .delete(self.url(&format!("/api/profiles/{profile_id}")))
            .bearer_auth(token)
            .send()
            .map_err(offline)?;
        let status = resp.status();
        if status.is_success() {
            // `204 No Content` carries no body; success is the status alone.
            Ok(())
        } else {
            Err(api_error(status, resp))
        }
    }

    fn list_tasks(&self, token: &str, profile_id: &str) -> ClientResult<Vec<Task>> {
        let resp = self
            .http
            .get(self.url(&format!("/api/profiles/{profile_id}/tasks")))
            .bearer_auth(token)
            .send()
            .map_err(offline)?;
        let status = resp.status();
        if status.is_success() {
            decode(resp)
        } else {
            Err(api_error(status, resp))
        }
    }

    fn create_task(
        &self,
        token: &str,
        profile_id: &str,
        req: &CreateTaskRequest,
    ) -> ClientResult<Task> {
        let resp = self
            .http
            .post(self.url(&format!("/api/profiles/{profile_id}/tasks")))
            .bearer_auth(token)
            .json(req)
            .send()
            .map_err(offline)?;
        let status = resp.status();
        if status.is_success() {
            decode(resp)
        } else {
            Err(api_error(status, resp))
        }
    }

    fn update_task(
        &self,
        token: &str,
        profile_id: &str,
        task_id: &str,
        req: &UpdateTaskRequest,
    ) -> ClientResult<Task> {
        let resp = self
            .http
            .patch(self.url(&format!("/api/profiles/{profile_id}/tasks/{task_id}")))
            .bearer_auth(token)
            .json(req)
            .send()
            .map_err(offline)?;
        let status = resp.status();
        if status.is_success() {
            decode(resp)
        } else {
            Err(api_error(status, resp))
        }
    }

    fn delete_task(&self, token: &str, profile_id: &str, task_id: &str) -> ClientResult<()> {
        let resp = self
            .http
            .delete(self.url(&format!("/api/profiles/{profile_id}/tasks/{task_id}")))
            .bearer_auth(token)
            .send()
            .map_err(offline)?;
        let status = resp.status();
        if status.is_success() {
            // `204 No Content`: success carries no body to decode.
            Ok(())
        } else {
            Err(api_error(status, resp))
        }
    }

    fn list_notes(&self, token: &str, profile_id: &str) -> ClientResult<Vec<Note>> {
        let resp = self
            .http
            .get(self.url(&format!("/api/profiles/{profile_id}/notes")))
            .bearer_auth(token)
            .send()
            .map_err(offline)?;
        let status = resp.status();
        if status.is_success() {
            decode(resp)
        } else {
            Err(api_error(status, resp))
        }
    }

    fn create_note(
        &self,
        token: &str,
        profile_id: &str,
        req: &CreateNoteRequest,
    ) -> ClientResult<Note> {
        let resp = self
            .http
            .post(self.url(&format!("/api/profiles/{profile_id}/notes")))
            .bearer_auth(token)
            .json(req)
            .send()
            .map_err(offline)?;
        let status = resp.status();
        if status.is_success() {
            decode(resp)
        } else {
            Err(api_error(status, resp))
        }
    }

    fn get_note(&self, token: &str, profile_id: &str, note_id: &str) -> ClientResult<Note> {
        let resp = self
            .http
            .get(self.url(&format!("/api/profiles/{profile_id}/notes/{note_id}")))
            .bearer_auth(token)
            .send()
            .map_err(offline)?;
        let status = resp.status();
        if status.is_success() {
            decode(resp)
        } else {
            Err(api_error(status, resp))
        }
    }

    fn update_note(
        &self,
        token: &str,
        profile_id: &str,
        note_id: &str,
        req: &UpdateNoteRequest,
    ) -> ClientResult<Note> {
        let resp = self
            .http
            .patch(self.url(&format!("/api/profiles/{profile_id}/notes/{note_id}")))
            .bearer_auth(token)
            .json(req)
            .send()
            .map_err(offline)?;
        let status = resp.status();
        if status.is_success() {
            decode(resp)
        } else {
            Err(api_error(status, resp))
        }
    }

    fn delete_note(&self, token: &str, profile_id: &str, note_id: &str) -> ClientResult<()> {
        let resp = self
            .http
            .delete(self.url(&format!("/api/profiles/{profile_id}/notes/{note_id}")))
            .bearer_auth(token)
            .send()
            .map_err(offline)?;
        let status = resp.status();
        if status.is_success() {
            // `204 No Content` carries no body; success is the status alone.
            Ok(())
        } else {
            Err(api_error(status, resp))
        }
    }

    fn get_timer_config(&self, token: &str) -> ClientResult<TimerConfig> {
        let resp = self
            .http
            .get(self.url("/api/timer/config"))
            .bearer_auth(token)
            .send()
            .map_err(offline)?;
        let status = resp.status();
        if status.is_success() {
            decode(resp)
        } else {
            Err(api_error(status, resp))
        }
    }

    fn update_timer_config(
        &self,
        token: &str,
        req: &UpdateTimerConfigRequest,
    ) -> ClientResult<TimerConfig> {
        let resp = self
            .http
            .put(self.url("/api/timer/config"))
            .bearer_auth(token)
            .json(req)
            .send()
            .map_err(offline)?;
        let status = resp.status();
        if status.is_success() {
            decode(resp)
        } else {
            Err(api_error(status, resp))
        }
    }

    fn get_timer_session(&self, token: &str) -> ClientResult<TimerSession> {
        let resp = self
            .http
            .get(self.url("/api/timer/session"))
            .bearer_auth(token)
            .send()
            .map_err(offline)?;
        let status = resp.status();
        if status.is_success() {
            decode(resp)
        } else {
            Err(api_error(status, resp))
        }
    }

    fn start_timer_session(&self, token: &str) -> ClientResult<TimerSession> {
        let resp = self
            .http
            .post(self.url("/api/timer/session/start"))
            .bearer_auth(token)
            .send()
            .map_err(offline)?;
        let status = resp.status();
        if status.is_success() {
            decode(resp)
        } else {
            Err(api_error(status, resp))
        }
    }

    fn stop_timer_session(&self, token: &str) -> ClientResult<TimerSession> {
        let resp = self
            .http
            .post(self.url("/api/timer/session/stop"))
            .bearer_auth(token)
            .send()
            .map_err(offline)?;
        let status = resp.status();
        if status.is_success() {
            decode(resp)
        } else {
            Err(api_error(status, resp))
        }
    }
}
