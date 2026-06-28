//! The task detail view: a per-field pane view of a single task, opened with `Enter` from the
//! task list (ADR-0010 §4). Each field is its own pane; `Tab`/`Shift+Tab` cycle panes, `e` opens
//! an in-place edit buffer on the focused editable pane, `Enter` commits that one field, and `Esc`
//! is two-tiered (cancel an in-progress edit, else exit to the list).
//!
//! All state here is transient process-lifetime UI state (hard-constraint #1): the snapshot
//! re-derives from the server after every commit (the task list refresh re-selects the same task),
//! and the edit buffer never outlives the edit.

use contract::Task;

/// The panes of the task detail view, in display + cycle order. `Title`/`Description` are editable;
/// `Status`/`Created`/`Closed` are read-only. `Closed` is only present when the task is done
/// (`closed_at` set), so the pane vector is built per snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskPane {
    /// The task title (editable).
    Title,
    /// The task description (editable).
    Description,
    /// The task status (read-only).
    Status,
    /// The creation timestamp (read-only).
    Created,
    /// The close timestamp (read-only); present only when the task is done.
    Closed,
}

impl TaskPane {
    /// Whether this pane is editable (`e` opens an edit buffer on it). Read-only panes are inert.
    #[must_use]
    pub fn is_editable(self) -> bool {
        matches!(self, TaskPane::Title | TaskPane::Description)
    }
}

/// The open task detail view: the task snapshot (re-derived from the server on every commit, #1),
/// the panes present for that snapshot, the focused pane index, and the optional in-progress edit
/// buffer. The buffer's presence is the two-tier `Esc` discriminant: `Some` ⇒ editing the focused
/// pane (Esc cancels the edit), `None` ⇒ viewing (Esc exits to the list).
#[derive(Debug, Clone)]
pub struct TaskDetail {
    /// The task being viewed, derived from the server.
    pub task: Task,
    /// The panes present for this snapshot, in cycle order.
    pub panes: Vec<TaskPane>,
    /// Index into `panes` of the focused pane.
    pub focused: usize,
    /// The in-progress edit buffer for the focused (editable) pane; `None` when not editing.
    pub edit: Option<String>,
}

impl TaskDetail {
    /// Open the detail view for `task` with the first pane focused and no edit in progress.
    #[must_use]
    pub fn new(task: Task) -> Self {
        let panes = Self::panes_for(&task);
        Self {
            task,
            panes,
            focused: 0,
            edit: None,
        }
    }

    /// The panes present for a task snapshot: the two editable fields, status, created, and — only
    /// when the task is closed — the close timestamp.
    fn panes_for(task: &Task) -> Vec<TaskPane> {
        let mut panes = vec![
            TaskPane::Title,
            TaskPane::Description,
            TaskPane::Status,
            TaskPane::Created,
        ];
        if task.closed_at.is_some() {
            panes.push(TaskPane::Closed);
        }
        panes
    }

    /// Re-derive the snapshot from a refreshed server task, preserving the focused pane where it
    /// still exists (the pane set can change if status flipped) and dropping any edit buffer.
    pub fn refresh_from(&mut self, task: Task) {
        let prev = self.focused_pane();
        self.panes = Self::panes_for(&task);
        self.task = task;
        self.edit = None;
        self.focused = prev
            .and_then(|p| self.panes.iter().position(|q| *q == p))
            .unwrap_or(0);
    }

    /// The currently-focused pane, if the pane vector is non-empty.
    #[must_use]
    pub fn focused_pane(&self) -> Option<TaskPane> {
        self.panes.get(self.focused).copied()
    }

    /// Cycle the focused pane forward (`true`) or backward, wrapping. A no-op while editing (the
    /// caller suppresses pane cycling during a field edit).
    pub fn cycle(&mut self, forward: bool) {
        let len = self.panes.len();
        if len == 0 {
            return;
        }
        self.focused = if forward {
            (self.focused + 1) % len
        } else {
            (self.focused + len - 1) % len
        };
    }

    /// Begin an in-place edit of the focused pane, seeding the buffer from its current value. A
    /// no-op on a read-only pane (`e` is inert there, Assumption A6).
    pub fn begin_edit(&mut self) {
        match self.focused_pane() {
            Some(TaskPane::Title) => self.edit = Some(self.task.title.clone()),
            Some(TaskPane::Description) => self.edit = Some(self.task.description.clone()),
            _ => {}
        }
    }

    /// Type a character into the edit buffer (no-op when not editing).
    pub fn push_char(&mut self, c: char) {
        if let Some(buf) = &mut self.edit {
            buf.push(c);
        }
    }

    /// Delete the last character of the edit buffer (no-op when not editing).
    pub fn backspace(&mut self) {
        if let Some(buf) = &mut self.edit {
            let _ = buf.pop();
        }
    }

    /// Cancel the in-progress edit, dropping the buffer (the pane reverts to the snapshot value).
    pub fn cancel_edit(&mut self) {
        self.edit = None;
    }

    /// Whether a field edit is in progress (the two-tier `Esc` discriminant).
    #[must_use]
    pub fn is_editing(&self) -> bool {
        self.edit.is_some()
    }
}
