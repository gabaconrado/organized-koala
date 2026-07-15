//! The account-global focus timer: the last server-returned config + session, the optional
//! duration-edit sub-flow, the timer's own in-flight marker, and the monotonic reference for
//! rendering the live countdown.
//!
//! The timer is a **global widget**, not a navigable screen (ADR-0006 §8.1): its state lives on
//! [`App`](super::App) and is rendered in the bottom-right corner of every post-auth screen. The
//! displayed countdown is **render state, never authoritative** (hard-constraint #1, ADR-0002
//! §2–3): no remaining-seconds integer is stored. The timer keeps the absolute `ends_at` +
//! `server_now` from the last session response plus the [`Instant`] when that response was
//! applied, and recomputes `remaining = ends_at − (server_now + elapsed)` afresh on each draw. The
//! server is the sole authority for the running-vs-completed verdict, which arrives on the coarse
//! refresh.

use std::time::Instant;

use contract::{TimerConfig, TimerSession, UpdateTimerConfigRequest};

use super::Session;
use super::protocol::{ClientRequest, RequestId};
use super::text_input::{self, TextInput};
use crate::app::Event;

/// The duration-edit sub-flow: a single numeric input buffer for the new duration in minutes.
/// The same transient text-entry sub-flow category as [`AddTaskState`](super::AddTaskState).
#[derive(Debug, Clone)]
pub struct DurationEditState {
    /// The entered duration text (digits only; parsed on submit).
    pub buffer: TextInput,
    /// Inline error (e.g. an out-of-range duration rejected by the server), if any.
    pub error: Option<String>,
}

impl DurationEditState {
    fn new(current: u32) -> Self {
        Self {
            buffer: TextInput::new(current.to_string()),
            error: None,
        }
    }

    fn push_char(&mut self, c: char) {
        // Digit filtering stays here (numeric buffer); the caret still moves for editing.
        if c.is_ascii_digit() {
            self.buffer.insert_char(c);
        }
    }

    fn backspace(&mut self) {
        self.buffer.backspace();
    }

    fn motion(&mut self, event: &Event) -> bool {
        text_input::apply_motion(&mut self.buffer, event)
    }
}

/// The account-global timer state, held on [`App`](super::App) and rendered as a persistent
/// widget on every post-auth screen (ADR-0006 §8.1).
///
/// `config` and `session` are the last values the server returned; `applied_at` is the monotonic
/// instant the current `session` was folded in, used only to advance the *rendered* countdown
/// between coarse refreshes. No countdown integer is stored (hard-constraint #1). `pending` is the
/// timer's own in-flight marker, independent of the active screen's request marker, so the global
/// `p` toggle and the duration edit coexist with screen-local requests.
#[derive(Debug, Clone)]
pub struct Timer {
    /// The last server-returned global duration config.
    pub config: TimerConfig,
    /// The last server-returned session state (idle / running / completed).
    pub session: TimerSession,
    /// When the current `session` was applied — the monotonic reference for the rendered
    /// countdown. `None` until the first session response lands.
    pub applied_at: Option<Instant>,
    /// Active duration-edit sub-flow, if open. While set, the timer owns keystrokes.
    pub editing: Option<DurationEditState>,
    /// A transient status/error message, if any.
    pub message: Option<String>,
    /// The timer's in-flight request id while a timer call is outstanding; `None` when idle.
    /// Transient process-lifetime UI state (hard-constraint #1).
    pub pending: Option<RequestId>,
    /// Whether the initial config→session load has been issued for the current session. Reset on
    /// logout so a fresh login re-loads.
    pub loaded: bool,
    /// Fire-once guard: whether the completion notification has already fired for the current
    /// session. Set when a Running→Completed edge fires (or when an initial `Completed` is folded,
    /// arming without firing), cleared when a new `Running`/`Idle` session is folded and by
    /// [`reset`](Self::reset). Transient process-lifetime UI state (hard-constraint #1).
    pub notified_for_session: bool,
    /// One-shot signal that the edge should fire a completion notification, drained by
    /// [`App::take_pending_notification`](super::App::take_pending_notification). Transient
    /// process-lifetime UI state (hard-constraint #1).
    pub notify_pending: bool,
}

impl Default for Timer {
    fn default() -> Self {
        Self::new()
    }
}

impl Timer {
    /// A fresh, unloaded timer seeded with the defaults shown until the first responses land. The
    /// real values arrive from the initial `GetTimerConfig` / `GetTimerSession` calls.
    #[must_use]
    pub fn new() -> Self {
        Self {
            config: TimerConfig {
                duration_minutes: 0,
            },
            session: TimerSession::Idle,
            applied_at: None,
            editing: None,
            message: None,
            pending: None,
            loaded: false,
            notified_for_session: false,
            notify_pending: false,
        }
    }

    /// Whether the timer currently has a request outstanding.
    #[must_use]
    pub fn is_pending(&self) -> bool {
        self.pending.is_some()
    }

    /// Whether the duration-edit sub-flow is open (the timer owns keystrokes while it is).
    #[must_use]
    pub fn is_editing(&self) -> bool {
        self.editing.is_some()
    }

    /// Begin the duration-edit sub-flow, seeded with the current configured duration.
    pub(crate) fn begin_edit(&mut self) {
        if self.is_pending() {
            return;
        }
        self.message = None;
        self.editing = Some(DurationEditState::new(self.config.duration_minutes));
    }

    /// Feed a character into the open duration-edit buffer (digits only).
    pub(crate) fn edit_char(&mut self, c: char) {
        if let Some(edit) = &mut self.editing {
            edit.push_char(c);
        }
    }

    /// Backspace in the open duration-edit buffer.
    pub(crate) fn edit_backspace(&mut self) {
        if let Some(edit) = &mut self.editing {
            edit.backspace();
        }
    }

    /// Apply a caret movement / forward-delete to the open duration-edit buffer, returning whether
    /// the event was a text-motion event.
    pub(crate) fn edit_motion(&mut self, event: &Event) -> bool {
        if let Some(edit) = &mut self.editing {
            edit.motion(event)
        } else {
            false
        }
    }

    /// Cancel the duration-edit sub-flow without issuing a request.
    pub(crate) fn cancel_edit(&mut self) {
        self.editing = None;
    }

    /// Parse and submit the edited duration, producing an `UpdateTimerConfig` request. A
    /// non-numeric buffer surfaces an inline error and produces no request.
    pub(crate) fn submit_edit(&mut self, session: &Session) -> Option<ClientRequest> {
        let edit = self.editing.as_mut()?;
        edit.error = None;
        let Ok(duration_minutes) = edit.buffer.as_str().trim().parse::<u32>() else {
            edit.error = Some("duration must be a whole number of minutes".to_owned());
            return None;
        };
        Some(ClientRequest::UpdateTimerConfig {
            token: session.token.clone(),
            req: UpdateTimerConfigRequest { duration_minutes },
        })
    }

    /// Resolve the global `p` toggle to the right request: start when idle/completed, stop when
    /// running. A no-op (returns `None`) while a timer request is already in flight.
    pub(crate) fn toggle(&mut self, session: &Session) -> Option<ClientRequest> {
        if self.is_pending() {
            return None;
        }
        self.message = None;
        let request = match self.session {
            TimerSession::Running { .. } => ClientRequest::StopTimerSession {
                token: session.token.clone(),
            },
            TimerSession::Idle | TimerSession::Completed { .. } => {
                ClientRequest::StartTimerSession {
                    token: session.token.clone(),
                }
            }
        };
        Some(request)
    }

    /// The initial config→session load request, issued once a session exists.
    pub(crate) fn initial_load(&mut self, session: &Session) -> Option<ClientRequest> {
        if self.loaded || self.is_pending() {
            return None;
        }
        self.loaded = true;
        Some(ClientRequest::GetTimerConfig {
            token: session.token.clone(),
        })
    }

    /// The coarse session-refresh request (the ~1-minute cadence). A no-op while a request is in
    /// flight or the duration edit owns keystrokes.
    pub(crate) fn refresh(&mut self, session: &Session) -> Option<ClientRequest> {
        if self.is_pending() || self.is_editing() {
            return None;
        }
        Some(ClientRequest::GetTimerSession {
            token: session.token.clone(),
        })
    }

    /// Reset the timer to its fresh, unloaded state (on logout — the next login re-loads).
    pub(crate) fn reset(&mut self) {
        *self = Self::new();
    }
}
