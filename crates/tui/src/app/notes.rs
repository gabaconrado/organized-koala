//! The notes screen for the active profile: the note vector, selection, the create / edit /
//! delete sub-flows, the in-flight marker, and the pure event handler producing
//! [`ClientRequest`]s.
//!
//! Mirrors [`TaskListState`](super::task_list::TaskListState) one-for-one — a note has no
//! status or lifecycle, carries `content` instead of `description`, and adds an in-place edit
//! and a delete sub-flow. All state here is transient process-lifetime UI state derived from
//! server responses (hard-constraint #1); nothing is persisted.

use contract::{CreateNoteRequest, Note, UpdateNoteRequest};

use super::Session;
use super::protocol::{ClientRequest, RequestId};
use crate::app::Event;

/// A two-field note form (title / content) shared by the create and edit sub-flows.
#[derive(Debug, Clone)]
pub struct NoteForm {
    /// Whether the title (`true`) or content field is focused.
    pub on_title: bool,
    /// Entered note title.
    pub title: String,
    /// Entered note content.
    pub content: String,
    /// Inline error (e.g. empty title rejected by the server), if any.
    pub error: Option<String>,
}

impl NoteForm {
    fn empty() -> Self {
        Self {
            on_title: true,
            title: String::new(),
            content: String::new(),
            error: None,
        }
    }

    fn from_note(note: &Note) -> Self {
        Self {
            on_title: true,
            title: note.title.clone(),
            content: note.content.clone(),
            error: None,
        }
    }

    /// Type a character into the focused field.
    pub(crate) fn push_char(&mut self, c: char) {
        if self.on_title {
            self.title.push(c);
        } else {
            self.content.push(c);
        }
    }

    /// Delete the last character of the focused field.
    pub(crate) fn backspace(&mut self) {
        let target = if self.on_title {
            &mut self.title
        } else {
            &mut self.content
        };
        let _ = target.pop();
    }

    /// Toggle focus between the title and content fields.
    pub(crate) fn toggle_field(&mut self) {
        self.on_title = !self.on_title;
    }
}

/// Which sub-flow (if any) overlays the notes list.
#[derive(Debug, Clone)]
pub enum NotesMode {
    /// The bare list: navigate, open, begin create / edit / delete.
    List,
    /// Reading a single note (title + content + created_at). No editing.
    Viewing(Note),
    /// The create sub-flow: a fresh [`NoteForm`].
    Creating(NoteForm),
    /// The edit sub-flow: the id being edited plus the form prefilled from it.
    Editing {
        /// The id of the note being edited.
        note_id: String,
        /// The editable title/content form.
        form: NoteForm,
    },
    /// The delete-confirmation sub-flow for the named note.
    ConfirmingDelete {
        /// The id of the note pending deletion.
        note_id: String,
        /// The title shown in the confirmation prompt.
        title: String,
    },
}

/// State of the notes screen for the active profile.
#[derive(Debug, Clone)]
pub struct NotesState {
    /// Notes as returned by the server, newest-first.
    pub notes: Vec<Note>,
    /// Index of the selected note in `notes`, if any.
    pub selected: Option<usize>,
    /// The active sub-flow overlaying the list.
    pub mode: NotesMode,
    /// A transient status/error message shown to the user, if any.
    pub message: Option<String>,
    /// The in-flight request id while a list/create/get/update/delete call is outstanding;
    /// `None` when idle. Transient process-lifetime UI state (hard-constraint #1).
    pub pending: Option<RequestId>,
}

impl NotesState {
    /// Builds the notes screen from a server list response, selecting the first note if any.
    #[must_use]
    pub fn new(notes: Vec<Note>) -> Self {
        let selected = if notes.is_empty() { None } else { Some(0) };
        Self {
            notes,
            selected,
            mode: NotesMode::List,
            message: None,
            pending: None,
        }
    }

    /// Whether the notes screen currently has a request outstanding.
    #[must_use]
    pub fn is_pending(&self) -> bool {
        self.pending.is_some()
    }

    /// Whether a text-entry sub-flow (create / edit) currently owns keystrokes.
    #[must_use]
    pub fn is_text_entry(&self) -> bool {
        matches!(
            self.mode,
            NotesMode::Creating(_) | NotesMode::Editing { .. }
        )
    }

    /// Whether a sub-flow is open (any non-`List` mode). While true, `Esc` cancels the sub-flow
    /// and `Tab` switches the focused field rather than cycling the top-level tabs.
    #[must_use]
    pub fn in_sub_flow(&self) -> bool {
        !matches!(self.mode, NotesMode::List)
    }

    fn move_selection(&mut self, forward: bool) {
        let len = self.notes.len();
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

    /// Pure update for the notes screen. Returns the [`ClientRequest`] a request-triggering event
    /// produces (create/edit submit, delete confirm, refresh), or `None` for a local edit or any
    /// event while a request is outstanding. `Cancel`/`Quit` and tab-switching are handled by the
    /// caller before reaching here. The `session` supplies the token + profile namespace for the
    /// payloads.
    pub(crate) fn handle_event(
        &mut self,
        event: Event,
        session: Option<&Session>,
    ) -> Option<ClientRequest> {
        if self.is_pending() {
            // One request in flight: ignore request-triggering and edit events alike.
            return None;
        }
        match &self.mode {
            NotesMode::List => self.handle_list_event(event, session),
            NotesMode::Viewing(_) => self.handle_view_event(event),
            NotesMode::Creating(_) => self.handle_create_event(event, session),
            NotesMode::Editing { .. } => self.handle_edit_event(event, session),
            NotesMode::ConfirmingDelete { .. } => self.handle_delete_event(event, session),
        }
    }

    fn handle_list_event(
        &mut self,
        event: Event,
        session: Option<&Session>,
    ) -> Option<ClientRequest> {
        match event {
            Event::Next => self.move_selection(true),
            Event::Prev => self.move_selection(false),
            Event::BeginAddNote => {
                self.message = None;
                self.mode = NotesMode::Creating(NoteForm::empty());
            }
            Event::Submit => return self.open_selected(session),
            Event::BeginEditNote => self.begin_edit(),
            Event::BeginDeleteNote => self.begin_delete(),
            Event::Refresh => return self.refresh(session),
            _ => {}
        }
        None
    }

    /// Open the selected note: issue a fresh `GetNote` so the read-only view derives from a
    /// server response (#1) rather than the cached list entry. `apply_response` folds the result
    /// into [`NotesMode::Viewing`].
    fn open_selected(&mut self, session: Option<&Session>) -> Option<ClientRequest> {
        let session = session?;
        let note = self.selected.and_then(|i| self.notes.get(i))?;
        Some(ClientRequest::GetNote {
            token: session.token.clone(),
            profile_id: session.profile_id.clone(),
            note_id: note.id.clone(),
        })
    }

    fn begin_edit(&mut self) {
        if let Some(note) = self.selected.and_then(|i| self.notes.get(i)) {
            self.message = None;
            self.mode = NotesMode::Editing {
                note_id: note.id.clone(),
                form: NoteForm::from_note(note),
            };
        }
    }

    fn begin_delete(&mut self) {
        if let Some(note) = self.selected.and_then(|i| self.notes.get(i)) {
            self.message = None;
            self.mode = NotesMode::ConfirmingDelete {
                note_id: note.id.clone(),
                title: note.title.clone(),
            };
        }
    }

    fn handle_view_event(&mut self, event: Event) -> Option<ClientRequest> {
        // The view is read-only: any cancel-equivalent returns to the list (the caller routes
        // `Esc` to `Cancel`); the rest are inert.
        if matches!(event, Event::Cancel) {
            self.mode = NotesMode::List;
        }
        None
    }

    fn handle_create_event(
        &mut self,
        event: Event,
        session: Option<&Session>,
    ) -> Option<ClientRequest> {
        let NotesMode::Creating(form) = &mut self.mode else {
            return None;
        };
        match event {
            Event::Char(c) => form.push_char(c),
            Event::Backspace => form.backspace(),
            Event::Next | Event::Prev => form.toggle_field(),
            Event::Submit => return self.submit_create(session),
            _ => {}
        }
        None
    }

    fn handle_edit_event(
        &mut self,
        event: Event,
        session: Option<&Session>,
    ) -> Option<ClientRequest> {
        let NotesMode::Editing { form, .. } = &mut self.mode else {
            return None;
        };
        match event {
            Event::Char(c) => form.push_char(c),
            Event::Backspace => form.backspace(),
            Event::Next | Event::Prev => form.toggle_field(),
            Event::Submit => return self.submit_edit(session),
            _ => {}
        }
        None
    }

    fn handle_delete_event(
        &mut self,
        event: Event,
        session: Option<&Session>,
    ) -> Option<ClientRequest> {
        // Confirm with `Submit`; `Cancel` (Esc) is handled by the caller's cancel path.
        if matches!(event, Event::Submit) {
            return self.submit_delete(session);
        }
        None
    }

    fn submit_create(&mut self, session: Option<&Session>) -> Option<ClientRequest> {
        let session = session?;
        let NotesMode::Creating(form) = &mut self.mode else {
            return None;
        };
        form.error = None;
        let req = CreateNoteRequest {
            title: form.title.trim().to_owned(),
            content: form.content.clone(),
        };
        Some(ClientRequest::CreateNote {
            token: session.token.clone(),
            profile_id: session.profile_id.clone(),
            req,
        })
    }

    fn submit_edit(&mut self, session: Option<&Session>) -> Option<ClientRequest> {
        let session = session?;
        let NotesMode::Editing { note_id, form } = &mut self.mode else {
            return None;
        };
        form.error = None;
        let req = UpdateNoteRequest {
            title: form.title.trim().to_owned(),
            content: form.content.clone(),
        };
        Some(ClientRequest::UpdateNote {
            token: session.token.clone(),
            profile_id: session.profile_id.clone(),
            note_id: note_id.clone(),
            req,
        })
    }

    fn submit_delete(&mut self, session: Option<&Session>) -> Option<ClientRequest> {
        let session = session?;
        let NotesMode::ConfirmingDelete { note_id, .. } = &self.mode else {
            return None;
        };
        Some(ClientRequest::DeleteNote {
            token: session.token.clone(),
            profile_id: session.profile_id.clone(),
            note_id: note_id.clone(),
        })
    }

    fn refresh(&mut self, session: Option<&Session>) -> Option<ClientRequest> {
        let session = session?;
        Some(ClientRequest::ListNotes {
            token: session.token.clone(),
            profile_id: session.profile_id.clone(),
        })
    }
}
