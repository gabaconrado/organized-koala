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

/// The panes of the note detail view, in display + cycle order. `Title`/`Content` are editable;
/// `Created` is read-only (ADR-0010 §4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotePane {
    /// The note title (editable).
    Title,
    /// The note content (editable).
    Content,
    /// The creation timestamp (read-only).
    Created,
}

impl NotePane {
    /// The fixed pane order of the note detail view: the read-only `Created` sits between the
    /// single-line `Title` and the multiline `Content` that fills the remaining height (ADR-0011).
    pub const ALL: [NotePane; 3] = [NotePane::Title, NotePane::Created, NotePane::Content];

    /// Whether this pane is editable (`e` opens an edit buffer on it). `Created` is inert.
    #[must_use]
    pub fn is_editable(self) -> bool {
        matches!(self, NotePane::Title | NotePane::Content)
    }

    /// The index in [`NotePane::ALL`] of the first editable pane, or `0` if none is editable (kept
    /// total; a note always carries the editable Title/Content, so the fallback is unreachable in
    /// practice but guards against an empty editable set).
    fn first_editable() -> usize {
        NotePane::ALL
            .iter()
            .position(|p| p.is_editable())
            .unwrap_or(0)
    }
}

/// The open note detail view: the note snapshot (re-derived from the server on every commit, #1),
/// the focused pane index, and the optional in-progress edit buffer (its presence is the two-tier
/// `Esc` discriminant). Per-field commit re-sends the unedited field from the snapshot since
/// [`UpdateNoteRequest`] has no optional fields (ADR-0010 §4, plan R5).
#[derive(Debug, Clone)]
pub struct NoteDetail {
    /// The note being viewed, derived from the server.
    pub note: Note,
    /// Index into [`NotePane::ALL`] of the focused pane.
    pub focused: usize,
    /// The in-progress edit buffer for the focused (editable) pane; `None` when not editing.
    pub edit: Option<String>,
}

impl NoteDetail {
    /// Open the detail view for `note` with the first editable pane (`Title`) focused and no edit
    /// in progress.
    #[must_use]
    pub fn new(note: Note) -> Self {
        Self {
            note,
            focused: NotePane::first_editable(),
            edit: None,
        }
    }

    /// Re-derive the snapshot from a refreshed server note, preserving the focused pane and
    /// dropping any edit buffer (#1).
    pub fn refresh_from(&mut self, note: Note) {
        self.note = note;
        self.edit = None;
    }

    /// The currently-focused pane. Falls back to the first editable pane (`Title`) if the index is
    /// somehow out of range, never to a read-only pane.
    #[must_use]
    pub fn focused_pane(&self) -> NotePane {
        NotePane::ALL
            .get(self.focused)
            .copied()
            .unwrap_or(NotePane::Title)
    }

    /// Force focus onto `pane`. A testing seam (ADR-0003 layer 2) so a test can construct a
    /// read-only-focused state directly — `cycle` no longer lands focus on the read-only `Created`
    /// pane, so the inert-`e` (A6) guard cannot be reached by key events alone. Production focus
    /// moves only through [`Self::new`]/[`Self::cycle`].
    pub fn focus_pane(&mut self, pane: NotePane) {
        if let Some(i) = NotePane::ALL.iter().position(|p| *p == pane) {
            self.focused = i;
        }
    }

    /// Cycle the focused pane forward (`true`) or backward to the next/previous **editable** pane,
    /// wrapping among editable panes only — the read-only `Created` pane is skipped and never landed
    /// on. A no-op if no pane is editable (kept total against an empty editable set).
    pub fn cycle(&mut self, forward: bool) {
        let len = NotePane::ALL.len();
        if len == 0 {
            return;
        }
        let step = if forward { 1 } else { len - 1 };
        let mut next = self.focused;
        for _ in 0..len {
            next = (next + step) % len;
            if NotePane::ALL.get(next).is_some_and(|p| p.is_editable()) {
                self.focused = next;
                return;
            }
        }
        // No editable pane found: leave focus unchanged (no-op).
    }

    /// Begin an in-place edit of the focused pane, seeding the buffer from its current value. A
    /// no-op on the read-only `Created` pane (`e` is inert there).
    pub fn begin_edit(&mut self) {
        match self.focused_pane() {
            NotePane::Title => self.edit = Some(self.note.title.clone()),
            NotePane::Content => self.edit = Some(self.note.content.clone()),
            NotePane::Created => {}
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

/// Which sub-flow (if any) overlays the notes list.
#[derive(Debug, Clone)]
pub enum NotesMode {
    /// The bare list: navigate, open, begin create / edit / delete.
    List,
    /// The per-field detail view of a single note (ADR-0010 §4): Title/Content editable, Created
    /// read-only. Derived from a fresh `GetNote` response (#1).
    Detail(NoteDetail),
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

    /// Whether a text-entry sub-flow (create / edit, or a detail-view field edit) currently owns
    /// keystrokes.
    #[must_use]
    pub fn is_text_entry(&self) -> bool {
        match &self.mode {
            NotesMode::Creating(_) | NotesMode::Editing { .. } => true,
            NotesMode::Detail(detail) => detail.is_editing(),
            NotesMode::List | NotesMode::ConfirmingDelete { .. } => false,
        }
    }

    /// Whether the per-field detail view is open (regardless of an in-progress field edit).
    #[must_use]
    pub fn detail_open(&self) -> bool {
        matches!(self.mode, NotesMode::Detail(_))
    }

    /// Whether the detail view is open with a field edit in progress.
    #[must_use]
    pub fn detail_editing(&self) -> bool {
        matches!(&self.mode, NotesMode::Detail(detail) if detail.is_editing())
    }

    /// Whether the detail view is editing the multiline `Content` pane — the discriminant the
    /// keymap uses to route `Enter` to a newline and `Ctrl+S` to commit (ADR-0011 §2). `false`
    /// while editing the single-line `Title`, or when not editing.
    #[must_use]
    pub fn editing_content_pane(&self) -> bool {
        matches!(
            &self.mode,
            NotesMode::Detail(detail)
                if detail.is_editing() && detail.focused_pane() == NotePane::Content
        )
    }

    /// Whether a sub-flow is open (any non-`List`, non-`Detail` mode). While true, `Esc` cancels
    /// the sub-flow and `Tab` switches the focused field rather than cycling the top-level tabs.
    /// The detail view is **not** counted here — it has its own input-capturing handling
    /// ([`Self::detail_open`]) so that `?` stays reachable over it (Assumption A7).
    #[must_use]
    pub fn in_sub_flow(&self) -> bool {
        matches!(
            self.mode,
            NotesMode::Creating(_) | NotesMode::Editing { .. } | NotesMode::ConfirmingDelete { .. }
        )
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
            NotesMode::Detail(_) => self.handle_detail_event(event, session),
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

    /// Handle a key while the per-field detail view is open (ADR-0010 §4). Two-tiered `Esc`:
    /// while editing a field, `Cancel` reverts the edit; with no edit, `Cancel` exits to the list.
    /// `e` opens the edit buffer on the focused editable pane; `Next`/`Prev` cycle panes when not
    /// editing; `Char`/`Backspace` mutate the buffer. `Submit` and `Commit` both commit the focused
    /// field (Title on Enter, multiline Content on Ctrl+S); `Newline` inserts a line break into the
    /// edit buffer (the multiline Content pane's Enter) — ADR-0011 §2.
    fn handle_detail_event(
        &mut self,
        event: Event,
        session: Option<&Session>,
    ) -> Option<ClientRequest> {
        let NotesMode::Detail(detail) = &mut self.mode else {
            return None;
        };
        if detail.is_editing() {
            match event {
                Event::Char(c) => detail.push_char(c),
                Event::Backspace => detail.backspace(),
                Event::Newline => detail.push_char('\n'),
                Event::Cancel => detail.cancel_edit(),
                // Both commit the focused field: the single-line Title commits on `Submit`
                // (Enter), the multiline Content on `Commit` (Ctrl+S) — ADR-0011 §2.
                Event::Submit | Event::Commit => return self.submit_field(session),
                _ => {}
            }
            return None;
        }
        match event {
            Event::Next => detail.cycle(true),
            Event::Prev => detail.cycle(false),
            Event::BeginEditNote => detail.begin_edit(),
            Event::Cancel => self.mode = NotesMode::List,
            _ => {}
        }
        None
    }

    /// Commit the focused detail field via [`UpdateNoteRequest`], re-sending the unedited field from
    /// the snapshot (the request has no optional fields, plan R5). A no-op if not editing.
    fn submit_field(&mut self, session: Option<&Session>) -> Option<ClientRequest> {
        let session = session?;
        let NotesMode::Detail(detail) = &mut self.mode else {
            return None;
        };
        let buffer = detail.edit.as_ref()?.clone();
        let (title, content) = match detail.focused_pane() {
            NotePane::Title => (buffer.trim().to_owned(), detail.note.content.clone()),
            NotePane::Content => (detail.note.title.clone(), buffer),
            NotePane::Created => return None,
        };
        let req = UpdateNoteRequest { title, content };
        Some(ClientRequest::UpdateNote {
            token: session.token.clone(),
            profile_id: session.profile_id.clone(),
            note_id: detail.note.id.clone(),
            req,
        })
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
            Event::Cancel => self.mode = NotesMode::List,
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
            Event::Cancel => self.mode = NotesMode::List,
            _ => {}
        }
        None
    }

    fn handle_delete_event(
        &mut self,
        event: Event,
        session: Option<&Session>,
    ) -> Option<ClientRequest> {
        // Confirm with `Submit`; `Cancel` (Esc) resets to the list, mirroring the detail/Tasks
        // handlers.
        match event {
            Event::Submit => return self.submit_delete(session),
            Event::Cancel => self.mode = NotesMode::List,
            _ => {}
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
