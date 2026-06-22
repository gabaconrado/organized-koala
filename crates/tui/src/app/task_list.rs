//! The task-list screen for the active profile: the task vector, selection, the optional
//! add-task sub-flow, the in-flight marker, and the pure event handler producing
//! [`ClientRequest`]s.

use contract::{CreateTaskRequest, Task};

use super::Session;
use super::protocol::{ClientRequest, RequestId};
use super::task_add::AddTaskState;
use crate::app::Event;

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
    /// The in-flight request id while a list/create/close call is outstanding; `None` when
    /// idle. Transient process-lifetime UI state (hard-constraint #1).
    pub pending: Option<RequestId>,
}

impl TaskListState {
    pub(crate) fn new(tasks: Vec<Task>) -> Self {
        let selected = if tasks.is_empty() { None } else { Some(0) };
        Self {
            tasks,
            selected,
            adding: None,
            message: None,
            pending: None,
        }
    }

    /// Whether the task list currently has a request outstanding.
    #[must_use]
    pub fn is_pending(&self) -> bool {
        self.pending.is_some()
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

    /// Pure update for the task-list screen. Returns the [`ClientRequest`] a request-triggering
    /// event produces (add-task submit, close, refresh), or `None` for a local edit or any event
    /// while a request is outstanding. `Cancel`/`Quit` are handled by the caller before reaching
    /// here. The `session` supplies the token + profile namespace for the request payloads.
    pub(crate) fn handle_event(
        &mut self,
        event: Event,
        session: Option<&Session>,
    ) -> Option<ClientRequest> {
        if self.is_pending() {
            // One request in flight: ignore request-triggering and edit events alike.
            return None;
        }
        if self.adding.is_some() {
            return self.handle_add_event(event, session);
        }
        match event {
            Event::Next => self.move_selection(true),
            Event::Prev => self.move_selection(false),
            Event::BeginAddTask => {
                self.message = None;
                self.adding = Some(AddTaskState::new());
            }
            Event::CloseSelected => return self.close_selected(session),
            Event::Refresh => return self.refresh(session),
            _ => {}
        }
        None
    }

    fn handle_add_event(
        &mut self,
        event: Event,
        session: Option<&Session>,
    ) -> Option<ClientRequest> {
        let Some(add) = &mut self.adding else {
            return None;
        };
        match event {
            Event::Char(c) => add.push_char(c),
            Event::Backspace => add.backspace(),
            Event::Next | Event::Prev => add.toggle_field(),
            Event::Cancel => self.adding = None,
            Event::Submit => return self.submit_add(session),
            _ => {}
        }
        None
    }

    fn submit_add(&mut self, session: Option<&Session>) -> Option<ClientRequest> {
        let session = session?;
        let add = self.adding.as_mut()?;
        add.error = None;
        let req = CreateTaskRequest {
            title: add.title.trim().to_owned(),
            description: add.description.clone(),
        };
        Some(ClientRequest::CreateTask {
            token: session.token.clone(),
            profile_id: session.profile_id.clone(),
            req,
        })
    }

    fn close_selected(&mut self, session: Option<&Session>) -> Option<ClientRequest> {
        let session = session?;
        let idx = self.selected?;
        let task = self.tasks.get(idx)?;
        Some(ClientRequest::CloseTask {
            token: session.token.clone(),
            profile_id: session.profile_id.clone(),
            task_id: task.id.clone(),
        })
    }

    fn refresh(&mut self, session: Option<&Session>) -> Option<ClientRequest> {
        let session = session?;
        Some(ClientRequest::ListTasks {
            token: session.token.clone(),
            profile_id: session.profile_id.clone(),
        })
    }
}
