//! The task-list screen for the active profile: the task vector, selection, the optional
//! add-task sub-flow, the in-flight marker, and the pure event handler producing
//! [`ClientRequest`]s.

use contract::{CreateTaskRequest, Task, TaskStatus, UpdateTaskRequest};

use super::Session;
use super::protocol::{ClientRequest, RequestId};
use super::task_add::{AddTaskState, EditTaskState};
use super::task_detail::{TaskDetail, TaskPane};
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
    /// The open per-field detail view, if any (ADR-0010 §4). Transient process-lifetime UI state
    /// (#1); the snapshot re-derives from the server after every commit.
    pub detail: Option<TaskDetail>,
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
            detail: None,
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

    /// Whether the detail view is open with a field edit in progress (text-entry context).
    #[must_use]
    pub fn detail_editing(&self) -> bool {
        self.detail.as_ref().is_some_and(TaskDetail::is_editing)
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
        if self.detail.is_some() {
            return self.handle_detail_event(event, session);
        }
        // A delete confirmation captures input until confirmed (`Submit`) or cancelled; any other
        // list action disarms it so a stray keypress can never delete.
        if self.confirming_delete.is_some() {
            return self.handle_delete_confirm_event(event, session);
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
            // `Enter` on the idle list opens the per-field detail view for the selected task.
            Event::Submit => self.open_detail(),
            Event::DeleteSelected => self.arm_delete(),
            Event::Refresh => return self.refresh(session),
            _ => {}
        }
        None
    }

    /// Open the per-field detail view for the selected task. The list is itself server-derived, so
    /// the detail opens from the already-loaded in-memory snapshot (Assumption A3); commits
    /// re-derive it from a fresh list refresh (#1). A no-op with nothing selected.
    fn open_detail(&mut self) {
        if let Some(task) = self.selected.and_then(|idx| self.tasks.get(idx)) {
            self.message = None;
            self.detail = Some(TaskDetail::new(task.clone()));
        }
    }

    /// Arm the delete confirmation for the selected task (the first `d`), opening the confirm dialog
    /// (ADR-0010 §4, Assumption A5). The second key (`Enter`) confirms via
    /// [`Self::handle_delete_confirm_event`]. A no-op with nothing selected.
    fn arm_delete(&mut self) {
        if let Some(task) = self.selected.and_then(|idx| self.tasks.get(idx)) {
            self.message = None;
            self.confirming_delete = Some(task.id.clone());
        }
    }

    /// Handle a key while the delete-confirm dialog is armed: `Submit` (Enter) confirms the delete;
    /// `Cancel` (Esc, routed by the caller) disarms; everything else is inert.
    fn handle_delete_confirm_event(
        &mut self,
        event: Event,
        session: Option<&Session>,
    ) -> Option<ClientRequest> {
        match event {
            Event::Submit => self.confirm_delete(session),
            Event::Cancel => {
                self.confirming_delete = None;
                None
            }
            _ => None,
        }
    }

    /// Handle a key while the per-field detail view is open (ADR-0010 §4). Two-tiered `Esc`: while
    /// editing a field, `Cancel` reverts the edit; with no edit, `Cancel` exits to the list. `e`
    /// opens the edit buffer on the focused editable pane; `Next`/`Prev` cycle panes when not
    /// editing; `Char`/`Backspace` mutate the buffer; `Submit` commits the focused field.
    fn handle_detail_event(
        &mut self,
        event: Event,
        session: Option<&Session>,
    ) -> Option<ClientRequest> {
        let Some(detail) = &mut self.detail else {
            return None;
        };
        if detail.is_editing() {
            match event {
                Event::Char(c) => detail.push_char(c),
                Event::Backspace => detail.backspace(),
                Event::Cancel => detail.cancel_edit(),
                Event::Submit => return self.submit_field(session),
                _ => {}
            }
            return None;
        }
        match event {
            Event::Next => detail.cycle(true),
            Event::Prev => detail.cycle(false),
            Event::BeginEditTask => detail.begin_edit(),
            Event::Cancel => self.detail = None,
            _ => {}
        }
        None
    }

    /// Commit the focused detail field via [`UpdateTaskRequest`] with **only** the edited field set
    /// (the request's other fields stay `None`, ADR-0010 §4). A no-op if not editing a field. A
    /// blank title is rejected locally without a round-trip (mirrors the edit dialog).
    fn submit_field(&mut self, session: Option<&Session>) -> Option<ClientRequest> {
        let session = session?;
        let detail = self.detail.as_mut()?;
        let buffer = detail.edit.as_ref()?.clone();
        let req = match detail.focused_pane()? {
            TaskPane::Title => {
                if buffer.trim().is_empty() {
                    return None;
                }
                UpdateTaskRequest {
                    title: Some(buffer.trim().to_owned()),
                    description: None,
                    status: None,
                }
            }
            TaskPane::Description => UpdateTaskRequest {
                title: None,
                description: Some(buffer),
                status: None,
            },
            TaskPane::Status | TaskPane::Created | TaskPane::Closed => return None,
        };
        Some(ClientRequest::UpdateTask {
            token: session.token.clone(),
            profile_id: session.profile_id.clone(),
            task_id: detail.task.id.clone(),
            req,
        })
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

    /// Issue the delete for the armed task id (the confirm dialog's `Enter`). A no-op if nothing is
    /// armed or the session is gone.
    fn confirm_delete(&mut self, session: Option<&Session>) -> Option<ClientRequest> {
        let session = session?;
        let task_id = self.confirming_delete.clone()?;
        Some(ClientRequest::DeleteTask {
            token: session.token.clone(),
            profile_id: session.profile_id.clone(),
            task_id,
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
