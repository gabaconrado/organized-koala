//! The task-list screen for the active profile: the task vector, the profile's sub-tasks, the
//! visible-row selection, the optional add/edit sub-flows, the in-flight marker, and the pure
//! event handler producing [`ClientRequest`]s.
//!
//! The list interleaves task rows and (indented) sub-task rows. A parent's sub-tasks are grouped
//! under it by `task_id` (defensively — an orphan sub-task whose parent is absent is ignored,
//! never panics; ADR-0013 Risk R3) and are shown expanded or collapsed. **Collapse is derived,
//! transient presentation state** (#1 / ADR-0012 §5): the initial state derives from the parent
//! task's status *each render* (open → expanded, done → collapsed); the user's `x` toggle records
//! an in-session, process-lifetime override keyed by task id, never persisted, dropped on a fresh
//! load for a task no longer present.

use std::collections::HashMap;

use contract::{
    CreateSubtaskRequest, CreateTaskRequest, Subtask, Task, TaskStatus, UpdateSubtaskRequest,
    UpdateTaskRequest,
};

use super::Session;
use super::protocol::{ClientRequest, RequestId};
use super::task_add::{AddSubtaskState, AddTaskState, EditSubtaskState, EditTaskState};
use super::task_detail::{TaskDetail, TaskPane};
use crate::app::Event;

/// A row in the rendered task list: either a top-level task or one of its sub-tasks. Selection
/// traverses **only visible rows** (sub-tasks under a collapsed parent are absent here), so the
/// cursor never lands on a hidden row (ADR-0013 Risk R2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VisibleRow {
    /// A top-level task at this index in `tasks`.
    Task {
        /// Index into [`TaskListState::tasks`].
        task_idx: usize,
    },
    /// A sub-task at this index in `subtasks`, rendered indented under its parent task.
    Subtask {
        /// Index into [`TaskListState::subtasks`].
        subtask_idx: usize,
    },
}

/// State of the task-list screen for the active profile.
#[derive(Debug, Clone)]
pub struct TaskListState {
    /// Tasks as returned by the server, newest-first.
    pub tasks: Vec<Task>,
    /// The profile's sub-tasks as returned by the server (the two-call tree load, ADR-0013 §3),
    /// grouped under their parent task by `task_id` when rendering.
    pub subtasks: Vec<Subtask>,
    /// Index of the selected **visible row** (task or sub-task), if any. Indexes into the row list
    /// produced by [`Self::visible_rows`], not directly into `tasks`/`subtasks`.
    pub selected: Option<usize>,
    /// Per-parent in-session collapse override, keyed by task id: `true` collapses that parent's
    /// sub-tasks, `false` expands them — overriding the status-derived default until a fresh load
    /// drops the entry for an absent task (ADR-0012 §5, A4). Transient process-lifetime UI state
    /// (#1); never persisted.
    pub collapse_overrides: HashMap<String, bool>,
    /// Active add-task sub-flow, if open.
    pub adding: Option<AddTaskState>,
    /// Active edit-task sub-flow, if open.
    pub editing: Option<EditTaskState>,
    /// Active add-sub-task sub-flow, if open (the `A` key; carries the parent task id).
    pub adding_subtask: Option<AddSubtaskState>,
    /// Active edit-sub-task-title sub-flow, if open (the `e` key on a sub-task row).
    pub editing_subtask: Option<EditSubtaskState>,
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
            subtasks: Vec::new(),
            selected,
            collapse_overrides: HashMap::new(),
            adding: None,
            editing: None,
            adding_subtask: None,
            editing_subtask: None,
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

    /// Whether the task pane is in an input-capturing sub-flow (add/edit task **or** add/edit
    /// sub-task, **or** an open detail view). Drives the terminal layer's overlay suppression and
    /// the `Tab`/`Esc` routing in [`crate::app::App`].
    #[must_use]
    pub fn in_sub_flow(&self) -> bool {
        self.adding.is_some()
            || self.editing.is_some()
            || self.adding_subtask.is_some()
            || self.editing_subtask.is_some()
            || self.detail.is_some()
    }

    /// Whether a sub-task add/edit form is the active text-entry context (drives `is_text_entry`
    /// at the terminal layer so letters are typed, not interpreted as commands).
    #[must_use]
    pub fn subtask_text_entry(&self) -> bool {
        self.adding_subtask.is_some() || self.editing_subtask.is_some()
    }

    /// Whether collapse for `task` resolves to collapsed: the in-session override if present, else
    /// the status-derived default (a **done** parent collapses, an **open** parent expands;
    /// ADR-0012 §5).
    #[must_use]
    pub fn is_collapsed(&self, task: &Task) -> bool {
        self.collapse_overrides
            .get(&task.id)
            .copied()
            .unwrap_or(matches!(task.status, TaskStatus::Done))
    }

    /// Whether `task` has at least one sub-task in the loaded set (groups defensively by `task_id`).
    #[must_use]
    pub fn has_subtasks(&self, task: &Task) -> bool {
        self.subtasks.iter().any(|s| s.task_id == task.id)
    }

    /// The list of **visible rows**, in render order: each task followed by its sub-tasks (in
    /// `subtasks` order, i.e. creation order from the server) **unless** the task is collapsed. A
    /// sub-task whose parent task is absent from `tasks` is silently dropped (Risk R3) — it never
    /// appears as a row.
    #[must_use]
    pub fn visible_rows(&self) -> Vec<VisibleRow> {
        let mut rows = Vec::new();
        for (task_idx, task) in self.tasks.iter().enumerate() {
            rows.push(VisibleRow::Task { task_idx });
            if self.is_collapsed(task) {
                continue;
            }
            for (subtask_idx, subtask) in self.subtasks.iter().enumerate() {
                if subtask.task_id == task.id {
                    rows.push(VisibleRow::Subtask { subtask_idx });
                }
            }
        }
        rows
    }

    /// The currently-selected visible row, if any.
    #[must_use]
    pub fn selected_row(&self) -> Option<VisibleRow> {
        let rows = self.visible_rows();
        self.selected.and_then(|i| rows.get(i).copied())
    }

    /// The selected task, if a task row is selected.
    fn selected_task(&self) -> Option<&Task> {
        match self.selected_row()? {
            VisibleRow::Task { task_idx } => self.tasks.get(task_idx),
            VisibleRow::Subtask { .. } => None,
        }
    }

    /// The selected sub-task, if a sub-task row is selected.
    fn selected_subtask(&self) -> Option<&Subtask> {
        match self.selected_row()? {
            VisibleRow::Subtask { subtask_idx } => self.subtasks.get(subtask_idx),
            VisibleRow::Task { .. } => None,
        }
    }

    /// The id of the parent task of the current selection: the selected task itself, or the
    /// selected sub-task's parent. `A` always adds a sub-task to this task. `None` when nothing is
    /// selected.
    fn parent_task_id_of_selection(&self) -> Option<String> {
        match self.selected_row()? {
            VisibleRow::Task { task_idx } => self.tasks.get(task_idx).map(|t| t.id.clone()),
            VisibleRow::Subtask { subtask_idx } => {
                self.subtasks.get(subtask_idx).map(|s| s.task_id.clone())
            }
        }
    }

    fn move_selection(&mut self, forward: bool) {
        let len = self.visible_rows().len();
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
        if self.adding_subtask.is_some() {
            return self.handle_add_subtask_event(event, session);
        }
        if self.editing_subtask.is_some() {
            return self.handle_edit_subtask_event(event, session);
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
            Event::BeginAddSubtask => self.begin_add_subtask(),
            // `e` edits the selected sub-task's title when a sub-task row is selected, else the
            // selected task (the existing task edit sub-flow).
            Event::BeginEditTask => {
                if self.selected_subtask().is_some() {
                    self.begin_edit_subtask();
                } else {
                    self.begin_edit();
                }
            }
            // `Space` toggles the selected sub-task when a sub-task row is selected, else the task.
            Event::ToggleDone => {
                if self.selected_subtask().is_some() {
                    return self.toggle_subtask_done(session);
                }
                return self.toggle_done(session);
            }
            // `x` toggles collapse for the parent task of the current selection (ADR-0012 §5).
            Event::ToggleCollapse => self.toggle_collapse(),
            // `Enter` on a task row opens its detail view (chaining a per-task sub-task load for
            // the "Sub-tasks" section); on a sub-task row it is inert (a sub-task has no detail
            // view, ADR-0012 §1 / A8).
            Event::Submit => return self.open_detail(session),
            Event::DeleteSelected => self.arm_delete(),
            Event::Refresh => return self.refresh(session),
            _ => {}
        }
        None
    }

    /// Open the per-field detail view for the selected **task** (a no-op on a sub-task row — a
    /// sub-task has no detail view, ADR-0012 §1). The list is itself server-derived, so the detail
    /// opens from the already-loaded in-memory snapshot (Assumption A3); commits re-derive it from
    /// a fresh list refresh (#1). Chains a per-task `ListTaskSubtasks` so the detail's read-only
    /// "Sub-tasks" section reflects a server response (A6). A no-op with nothing selected.
    fn open_detail(&mut self, session: Option<&Session>) -> Option<ClientRequest> {
        let task = self.selected_task()?.clone();
        self.message = None;
        self.detail = Some(TaskDetail::new(task.clone()));
        let session = session?;
        Some(ClientRequest::ListTaskSubtasks {
            token: session.token.clone(),
            profile_id: session.profile_id.clone(),
            task_id: task.id,
        })
    }

    /// Begin the add-sub-task sub-flow for the parent task of the current selection (the selected
    /// task, or the selected sub-task's parent). A no-op with nothing selected.
    fn begin_add_subtask(&mut self) {
        if let Some(task_id) = self.parent_task_id_of_selection() {
            self.message = None;
            self.adding_subtask = Some(AddSubtaskState::new(task_id));
        }
    }

    /// Begin editing the selected sub-task's title, pre-filled from its current value.
    fn begin_edit_subtask(&mut self) {
        let Some(state) = self.selected_subtask().map(EditSubtaskState::new) else {
            return;
        };
        self.message = None;
        self.editing_subtask = Some(state);
    }

    /// Toggle collapse/expand for the parent task of the current selection: records an in-session
    /// override that is the inverse of the current resolved collapse state (ADR-0012 §5, A4). A
    /// no-op with nothing selected.
    fn toggle_collapse(&mut self) {
        let Some(task_id) = self.parent_task_id_of_selection() else {
            return;
        };
        let Some(task) = self.tasks.iter().find(|t| t.id == task_id) else {
            return;
        };
        let next = !self.is_collapsed(task);
        let _ = self.collapse_overrides.insert(task_id, next);
    }

    /// Arm the delete confirmation for the selected **task** (the first `d`), opening the confirm
    /// dialog (ADR-0010 §4, Assumption A5). The second key (`Enter`) confirms via
    /// [`Self::handle_delete_confirm_event`]. A no-op on a sub-task row or with nothing selected.
    fn arm_delete(&mut self) {
        let Some(task_id) = self.selected_task().map(|t| t.id.clone()) else {
            return;
        };
        self.message = None;
        self.confirming_delete = Some(task_id);
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

    /// Handle a key while the add-sub-task form is open: a single title field. `Submit` issues the
    /// create; `Cancel` abandons it; `Char`/`Backspace` edit the title.
    fn handle_add_subtask_event(
        &mut self,
        event: Event,
        session: Option<&Session>,
    ) -> Option<ClientRequest> {
        let Some(add) = &mut self.adding_subtask else {
            return None;
        };
        match event {
            Event::Char(c) => add.push_char(c),
            Event::Backspace => add.backspace(),
            Event::Cancel => self.adding_subtask = None,
            Event::Submit => return self.submit_add_subtask(session),
            _ => {}
        }
        None
    }

    /// Submit the add-sub-task form as a [`CreateSubtaskRequest`]. A blank title (after trimming)
    /// is rejected locally without a round-trip (mirrors add-task).
    fn submit_add_subtask(&mut self, session: Option<&Session>) -> Option<ClientRequest> {
        let session = session?;
        let add = self.adding_subtask.as_mut()?;
        if add.title.trim().is_empty() {
            add.error = Some("title must not be empty".to_owned());
            return None;
        }
        add.error = None;
        let req = CreateSubtaskRequest {
            title: add.title.trim().to_owned(),
        };
        Some(ClientRequest::CreateSubtask {
            token: session.token.clone(),
            profile_id: session.profile_id.clone(),
            task_id: add.task_id.clone(),
            req,
        })
    }

    /// Handle a key while the edit-sub-task-title form is open: a single title field. `Submit`
    /// issues the patch; `Cancel` abandons it; `Char`/`Backspace` edit the title.
    fn handle_edit_subtask_event(
        &mut self,
        event: Event,
        session: Option<&Session>,
    ) -> Option<ClientRequest> {
        let Some(edit) = &mut self.editing_subtask else {
            return None;
        };
        match event {
            Event::Char(c) => edit.push_char(c),
            Event::Backspace => edit.backspace(),
            Event::Cancel => self.editing_subtask = None,
            Event::Submit => return self.submit_edit_subtask(session),
            _ => {}
        }
        None
    }

    /// Submit the edit-sub-task form as a title-only [`UpdateSubtaskRequest`]. A blank title (after
    /// trimming) is rejected locally without a round-trip.
    fn submit_edit_subtask(&mut self, session: Option<&Session>) -> Option<ClientRequest> {
        let session = session?;
        let edit = self.editing_subtask.as_mut()?;
        if edit.title.trim().is_empty() {
            edit.error = Some("title must not be empty".to_owned());
            return None;
        }
        edit.error = None;
        let req = UpdateSubtaskRequest {
            title: Some(edit.title.trim().to_owned()),
            status: None,
        };
        Some(ClientRequest::UpdateSubtask {
            token: session.token.clone(),
            profile_id: session.profile_id.clone(),
            task_id: edit.task_id.clone(),
            subtask_id: edit.subtask_id.clone(),
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
        let Some(state) = self.selected_task().map(EditTaskState::new) else {
            return;
        };
        self.message = None;
        self.editing = Some(state);
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
        let task = self.selected_task()?;
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

    /// Toggle the selected sub-task's status: a done sub-task is reopened, an open one is marked
    /// done (a plain status flip — a sub-task has no `closed_at`).
    fn toggle_subtask_done(&mut self, session: Option<&Session>) -> Option<ClientRequest> {
        let session = session?;
        let subtask = self.selected_subtask()?;
        let next = match subtask.status {
            TaskStatus::Open => TaskStatus::Done,
            TaskStatus::Done => TaskStatus::Open,
        };
        let req = UpdateSubtaskRequest {
            title: None,
            status: Some(next),
        };
        Some(ClientRequest::UpdateSubtask {
            token: session.token.clone(),
            profile_id: session.profile_id.clone(),
            task_id: subtask.task_id.clone(),
            subtask_id: subtask.id.clone(),
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
