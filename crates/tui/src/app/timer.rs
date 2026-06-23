//! The focus/timer screen: the last server-returned config + session, the optional
//! duration-edit sub-flow, the in-flight marker, and the monotonic reference for rendering the
//! live countdown.
//!
//! The displayed countdown is **render state, never authoritative** (hard-constraint #1,
//! ADR-0002 §2–3): no remaining-seconds integer is stored. The screen keeps the absolute
//! `ends_at` + `server_now` from the last session response plus the [`Instant`] when that
//! response was applied, and recomputes `remaining = ends_at − (server_now + elapsed)` afresh on
//! each draw. The server is the sole authority for the running-vs-completed verdict, which
//! arrives on the coarse refresh.

use std::time::Instant;

use contract::{TimerConfig, TimerSession, UpdateTimerConfigRequest};

use super::Session;
use super::protocol::{ClientRequest, RequestId};
use crate::app::Event;

/// The duration-edit sub-flow: a single numeric input buffer for the new duration in minutes.
/// The same transient sub-flow category as [`AddTaskState`](super::AddTaskState).
#[derive(Debug, Clone)]
pub struct DurationEditState {
    /// The entered duration text (digits only; parsed on submit).
    pub buffer: String,
    /// Inline error (e.g. an out-of-range duration rejected by the server), if any.
    pub error: Option<String>,
}

impl DurationEditState {
    fn new(current: u32) -> Self {
        Self {
            buffer: current.to_string(),
            error: None,
        }
    }

    fn push_char(&mut self, c: char) {
        if c.is_ascii_digit() {
            self.buffer.push(c);
        }
    }

    fn backspace(&mut self) {
        let _ = self.buffer.pop();
    }
}

/// State of the focus/timer screen.
///
/// `config` and `session` are the last values the server returned; `applied_at` is the monotonic
/// instant the current `session` was folded in, used only to advance the *rendered* countdown
/// between coarse refreshes. No countdown integer is stored (hard-constraint #1).
#[derive(Debug, Clone)]
pub struct TimerState {
    /// The last server-returned global duration config.
    pub config: TimerConfig,
    /// The last server-returned session state (idle / running / completed).
    pub session: TimerSession,
    /// When the current `session` was applied — the monotonic reference for the rendered
    /// countdown. `None` until the first session response lands.
    pub applied_at: Option<Instant>,
    /// Active duration-edit sub-flow, if open.
    pub editing: Option<DurationEditState>,
    /// A transient status/error message shown to the user, if any.
    pub message: Option<String>,
    /// The in-flight request id while a timer call is outstanding; `None` when idle. Transient
    /// process-lifetime UI state (hard-constraint #1).
    pub pending: Option<RequestId>,
}

impl TimerState {
    /// A fresh timer screen seeded with the defaults shown until the first responses land. The
    /// real values arrive from the entry `GetTimerConfig` / `GetTimerSession` calls.
    pub(crate) fn new() -> Self {
        Self {
            config: TimerConfig {
                duration_minutes: 0,
            },
            session: TimerSession::Idle,
            applied_at: None,
            editing: None,
            message: None,
            pending: None,
        }
    }

    /// Whether the timer screen currently has a request outstanding.
    #[must_use]
    pub fn is_pending(&self) -> bool {
        self.pending.is_some()
    }

    /// Pure update for the timer screen. Returns the [`ClientRequest`] a request-triggering event
    /// produces (start / stop / set-duration / refresh), or `None` for a local edit or any event
    /// while a request is outstanding. `Cancel`/`Quit`/navigation are handled by the caller before
    /// reaching here. The `session` supplies the token for the request payloads.
    pub(crate) fn handle_event(
        &mut self,
        event: Event,
        session: Option<&Session>,
    ) -> Option<ClientRequest> {
        if self.is_pending() {
            // One request in flight: ignore request-triggering and edit events alike.
            return None;
        }
        if self.editing.is_some() {
            return self.handle_edit_event(event, session);
        }
        match event {
            Event::BeginEditDuration => {
                self.message = None;
                self.editing = Some(DurationEditState::new(self.config.duration_minutes));
            }
            Event::StartTimer => return self.start(session),
            Event::StopTimer => return self.stop(session),
            Event::Refresh => return self.refresh_session(session),
            _ => {}
        }
        None
    }

    fn handle_edit_event(
        &mut self,
        event: Event,
        session: Option<&Session>,
    ) -> Option<ClientRequest> {
        let Some(edit) = &mut self.editing else {
            return None;
        };
        match event {
            Event::Char(c) => edit.push_char(c),
            Event::Backspace => edit.backspace(),
            Event::Cancel => self.editing = None,
            Event::Submit => return self.submit_duration(session),
            _ => {}
        }
        None
    }

    fn submit_duration(&mut self, session: Option<&Session>) -> Option<ClientRequest> {
        let session = session?;
        let edit = self.editing.as_mut()?;
        edit.error = None;
        let Ok(duration_minutes) = edit.buffer.trim().parse::<u32>() else {
            edit.error = Some("duration must be a whole number of minutes".to_owned());
            return None;
        };
        Some(ClientRequest::UpdateTimerConfig {
            token: session.token.clone(),
            req: UpdateTimerConfigRequest { duration_minutes },
        })
    }

    fn start(&mut self, session: Option<&Session>) -> Option<ClientRequest> {
        let session = session?;
        self.message = None;
        Some(ClientRequest::StartTimerSession {
            token: session.token.clone(),
        })
    }

    fn stop(&mut self, session: Option<&Session>) -> Option<ClientRequest> {
        let session = session?;
        self.message = None;
        Some(ClientRequest::StopTimerSession {
            token: session.token.clone(),
        })
    }

    fn refresh_session(&mut self, session: Option<&Session>) -> Option<ClientRequest> {
        let session = session?;
        Some(ClientRequest::GetTimerSession {
            token: session.token.clone(),
        })
    }
}
