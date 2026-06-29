//! The add-task and edit-task sub-flows within the task list: a two-field form (title /
//! description) and its pure event handling. The edit flow reuses the same field machinery and
//! additionally carries the id of the task being edited.
//!
//! Sub-task add/edit are single-field (title only) flows ([`AddSubtaskState`] /
//! [`EditSubtaskState`]), each carrying the parent task id so the create/edit request targets the
//! nested route (ADR-0012 §1 — a sub-task carries only a title).

use contract::{Subtask, Task};

/// The add-task sub-flow: which field is focused and the entered title/description.
#[derive(Debug, Clone)]
pub struct AddTaskState {
    /// Whether the title (`true`) or description field is focused.
    pub on_title: bool,
    /// Entered task title.
    pub title: String,
    /// Entered task description.
    pub description: String,
    /// Inline error (e.g. empty title rejected by the server), if any.
    pub error: Option<String>,
}

impl AddTaskState {
    pub(crate) fn new() -> Self {
        Self {
            on_title: true,
            title: String::new(),
            description: String::new(),
            error: None,
        }
    }

    /// Type a character into the focused field.
    pub(crate) fn push_char(&mut self, c: char) {
        if self.on_title {
            self.title.push(c);
        } else {
            self.description.push(c);
        }
    }

    /// Delete the last character of the focused field.
    pub(crate) fn backspace(&mut self) {
        let target = if self.on_title {
            &mut self.title
        } else {
            &mut self.description
        };
        let _ = target.pop();
    }

    /// Toggle focus between the title and description fields.
    pub(crate) fn toggle_field(&mut self) {
        self.on_title = !self.on_title;
    }
}

/// The add-sub-task sub-flow: the parent task id the new sub-task is created under, plus a
/// single title field (a sub-task carries only a title; ADR-0012 §1).
#[derive(Debug, Clone)]
pub struct AddSubtaskState {
    /// Id of the parent task the sub-task is created under.
    pub task_id: String,
    /// Entered sub-task title.
    pub title: String,
    /// Inline error (e.g. blank title rejected by the server), if any.
    pub error: Option<String>,
}

impl AddSubtaskState {
    pub(crate) fn new(task_id: String) -> Self {
        Self {
            task_id,
            title: String::new(),
            error: None,
        }
    }

    /// Type a character into the title field.
    pub(crate) fn push_char(&mut self, c: char) {
        self.title.push(c);
    }

    /// Delete the last character of the title field.
    pub(crate) fn backspace(&mut self) {
        let _ = self.title.pop();
    }
}

/// The edit-sub-task sub-flow: the parent task id and the sub-task id being edited, plus a single
/// title field pre-filled from the sub-task's current title (a sub-task carries only a title).
#[derive(Debug, Clone)]
pub struct EditSubtaskState {
    /// Id of the parent task owning the sub-task.
    pub task_id: String,
    /// Id of the sub-task being edited.
    pub subtask_id: String,
    /// Edited sub-task title (pre-filled from the sub-task).
    pub title: String,
    /// Inline error (e.g. blank title rejected by the server), if any.
    pub error: Option<String>,
}

impl EditSubtaskState {
    /// Open the edit sub-flow for `subtask`, pre-filling the title from its current value.
    pub(crate) fn new(subtask: &Subtask) -> Self {
        Self {
            task_id: subtask.task_id.clone(),
            subtask_id: subtask.id.clone(),
            title: subtask.title.clone(),
            error: None,
        }
    }

    /// Type a character into the title field.
    pub(crate) fn push_char(&mut self, c: char) {
        self.title.push(c);
    }

    /// Delete the last character of the title field.
    pub(crate) fn backspace(&mut self) {
        let _ = self.title.pop();
    }
}

/// The edit-task sub-flow: the id of the task being edited plus the same two-field form as
/// [`AddTaskState`], pre-filled from the task's current title/description.
#[derive(Debug, Clone)]
pub struct EditTaskState {
    /// Id of the task being edited.
    pub task_id: String,
    /// Whether the title (`true`) or description field is focused.
    pub on_title: bool,
    /// Edited task title (pre-filled from the task).
    pub title: String,
    /// Edited task description (pre-filled from the task).
    pub description: String,
    /// Inline error (e.g. blank title rejected by the server), if any.
    pub error: Option<String>,
}

impl EditTaskState {
    /// Open the edit sub-flow for `task`, pre-filling the fields from its current values.
    pub(crate) fn new(task: &Task) -> Self {
        Self {
            task_id: task.id.clone(),
            on_title: true,
            title: task.title.clone(),
            description: task.description.clone(),
            error: None,
        }
    }

    /// Type a character into the focused field.
    pub(crate) fn push_char(&mut self, c: char) {
        if self.on_title {
            self.title.push(c);
        } else {
            self.description.push(c);
        }
    }

    /// Delete the last character of the focused field.
    pub(crate) fn backspace(&mut self) {
        let target = if self.on_title {
            &mut self.title
        } else {
            &mut self.description
        };
        let _ = target.pop();
    }

    /// Toggle focus between the title and description fields.
    pub(crate) fn toggle_field(&mut self) {
        self.on_title = !self.on_title;
    }
}
