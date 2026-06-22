//! The add-task sub-flow within the task list: a two-field form (title / description) and its
//! pure event handling.

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
