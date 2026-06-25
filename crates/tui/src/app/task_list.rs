//! The task-list screen for the active profile: the task vector, selection, the optional
//! add-task sub-flow, the in-flight marker, and the pure event handler producing
//! [`ClientRequest`]s.

use contract::{CreateTaskRequest, Task, TaskStatus, UpdateTaskRequest};

use super::Session;
use super::protocol::{ClientRequest, RequestId};
use super::task_add::{AddTaskState, EditTaskState};
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
    /// Active edit-task sub-flow, if open.
    pub editing: Option<EditTaskState>,
    /// Id of the task awaiting a delete confirmation (the two-step delete affordance): set on the
    /// first delete key, cleared on confirm or on any other navigation. `None` when not armed.
    pub confirming_delete: Option<String>,
    /// A transient status/error message shown to the user, if any.
    pub message: Option<String>,
    /// The in-flight request id while a list/create/update/delete call is outstanding; `None` when
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
            editing: None,
            confirming_delete: None,
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
    /// event produces (add submit, edit submit, toggle-done, delete-confirm, refresh), or `None`
    /// for a local edit or any event while a request is outstanding. `Cancel`/`Quit` are handled
    /// by the caller before reaching here. The `session` supplies the token + profile namespace
    /// for the request payloads.
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
        if self.editing.is_some() {
            return self.handle_edit_event(event, session);
        }
        // A delete confirmation is armed only across consecutive delete keystrokes; any other
        // action disarms it so a stray keypress can never delete.
        if !matches!(event, Event::DeleteSelected) {
            self.confirming_delete = None;
        }
        match event {
            Event::Next => self.move_selection(true),
            Event::Prev => self.move_selection(false),
            Event::BeginAddTask => {
                self.message = None;
                self.adding = Some(AddTaskState::new());
            }
            Event::BeginEditTask => self.begin_edit(),
            Event::ToggleDone => return self.toggle_done(session),
            Event::DeleteSelected => return self.delete_selected(session),
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
            Event::Next | Event::Prev => edit.toggle_field(),
            Event::Cancel => self.editing = None,
            Event::Submit => return self.submit_edit(session),
            _ => {}
        }
        None
    }

    /// Open the edit sub-flow for the selected task, pre-filled from its current values.
    fn begin_edit(&mut self) {
        let Some(task) = self.selected.and_then(|idx| self.tasks.get(idx)) else {
            return;
        };
        self.message = None;
        self.editing = Some(EditTaskState::new(task));
    }

    /// Submit the edit sub-flow as a title+description [`UpdateTaskRequest`]. Mirrors add-task's
    /// inline validation: a blank title (after trimming) is rejected locally without a round-trip.
    fn submit_edit(&mut self, session: Option<&Session>) -> Option<ClientRequest> {
        let session = session?;
        let edit = self.editing.as_mut()?;
        if edit.title.trim().is_empty() {
            edit.error = Some("title must not be empty".to_owned());
            return None;
        }
        edit.error = None;
        let req = UpdateTaskRequest {
            title: Some(edit.title.trim().to_owned()),
            description: Some(edit.description.clone()),
            status: None,
        };
        Some(ClientRequest::UpdateTask {
            token: session.token.clone(),
            profile_id: session.profile_id.clone(),
            task_id: edit.task_id.clone(),
            req,
        })
    }

    /// Toggle the selected task's status: a done task is reopened (`status: open`, clears
    /// `closed_at` server-side), an open task is marked done (`status: done`).
    fn toggle_done(&mut self, session: Option<&Session>) -> Option<ClientRequest> {
        let session = session?;
        let idx = self.selected?;
        let task = self.tasks.get(idx)?;
        let next = match task.status {
            TaskStatus::Open => TaskStatus::Done,
            TaskStatus::Done => TaskStatus::Open,
        };
        let req = UpdateTaskRequest {
            title: None,
            description: None,
            status: Some(next),
        };
        Some(ClientRequest::UpdateTask {
            token: session.token.clone(),
            profile_id: session.profile_id.clone(),
            task_id: task.id.clone(),
            req,
        })
    }

    /// Delete the selected task behind a two-step confirm: the first press arms the confirmation
    /// for that task id; the second (same task still selected) issues the delete. Selecting a
    /// different task re-arms for the new id.
    fn delete_selected(&mut self, session: Option<&Session>) -> Option<ClientRequest> {
        let session = session?;
        let idx = self.selected?;
        let task = self.tasks.get(idx)?;
        if self.confirming_delete.as_deref() == Some(task.id.as_str()) {
            return Some(ClientRequest::DeleteTask {
                token: session.token.clone(),
                profile_id: session.profile_id.clone(),
                task_id: task.id.clone(),
            });
        }
        self.message = None;
        self.confirming_delete = Some(task.id.clone());
        None
    }

    fn refresh(&mut self, session: Option<&Session>) -> Option<ClientRequest> {
        let session = session?;
        Some(ClientRequest::ListTasks {
            token: session.token.clone(),
            profile_id: session.profile_id.clone(),
        })
    }
}
