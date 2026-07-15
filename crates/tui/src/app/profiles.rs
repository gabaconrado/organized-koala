//! The profile switcher for the account: the profile vector, selection, the create / rename /
//! delete sub-flows, the in-flight marker, and the pure event handler producing
//! [`ClientRequest`]s.
//!
//! Mirrors [`NotesState`](super::notes::NotesState) — a profile has only a `name`, so the form
//! is a single field, and there is no read-only "view" sub-flow. **Picking** a profile is not a
//! server call: it rebinds the in-memory active-profile id (handled by [`App`](super::App)) and
//! re-issues the scoped reads. All state here is transient process-lifetime UI state derived from
//! server responses (hard-constraint #1); nothing is persisted, and there is no server "switch"
//! endpoint (ADR-0009 §5, Assumption A7).

use contract::{CreateProfileRequest, Profile, UpdateProfileRequest};

use super::Session;
use super::protocol::{ClientRequest, RequestId};
use crate::app::Event;

/// A single-field profile-name form shared by the create and rename sub-flows.
#[derive(Debug, Clone)]
pub struct ProfileForm {
    /// Entered profile name.
    pub name: String,
    /// Inline error (e.g. a duplicate name rejected by the server), if any.
    pub error: Option<String>,
}

impl ProfileForm {
    fn empty() -> Self {
        Self {
            name: String::new(),
            error: None,
        }
    }

    fn from_profile(profile: &Profile) -> Self {
        Self {
            name: profile.name.clone(),
            error: None,
        }
    }

    /// Type a character into the name field.
    pub(crate) fn push_char(&mut self, c: char) {
        self.name.push(c);
    }

    /// Delete the last character of the name field.
    pub(crate) fn backspace(&mut self) {
        let _ = self.name.pop();
    }
}

/// Which sub-flow (if any) overlays the profile switcher list.
#[derive(Debug, Clone)]
pub enum ProfilesMode {
    /// The bare list: navigate, pick-active, begin create / rename / delete.
    List,
    /// The create sub-flow: a fresh [`ProfileForm`].
    Creating(ProfileForm),
    /// The rename sub-flow: the id being renamed plus the form prefilled from it.
    Renaming {
        /// The id of the profile being renamed.
        profile_id: String,
        /// The editable name form.
        form: ProfileForm,
    },
    /// The delete-confirmation sub-flow for the named profile.
    ConfirmingDelete {
        /// The id of the profile pending deletion.
        profile_id: String,
        /// The name shown in the confirmation prompt.
        name: String,
    },
}

/// State of the profile switcher for the account.
#[derive(Debug, Clone)]
pub struct ProfilesState {
    /// Profiles in the exact order the server returns them (oldest-first, ascending insertion
    /// order); the TUI does no client-side sort (hard-constraint #1: server is authoritative).
    pub profiles: Vec<Profile>,
    /// Index of the selected profile in `profiles`, if any.
    pub selected: Option<usize>,
    /// The active sub-flow overlaying the list.
    pub mode: ProfilesMode,
    /// A transient status/error message shown to the user, if any.
    pub message: Option<String>,
    /// The in-flight request id while a list/create/rename/delete call is outstanding; `None` when
    /// idle. Transient process-lifetime UI state (hard-constraint #1).
    pub pending: Option<RequestId>,
}

impl ProfilesState {
    /// Builds the switcher from a server list response, selecting the active profile if present in
    /// the list, else the first.
    #[must_use]
    pub fn new(profiles: Vec<Profile>, active_profile_id: &str) -> Self {
        let selected = profiles
            .iter()
            .position(|p| p.id == active_profile_id)
            .or(if profiles.is_empty() { None } else { Some(0) });
        Self {
            profiles,
            selected,
            mode: ProfilesMode::List,
            message: None,
            pending: None,
        }
    }

    /// Whether the switcher currently has a request outstanding.
    #[must_use]
    pub fn is_pending(&self) -> bool {
        self.pending.is_some()
    }

    /// Whether a text-entry sub-flow (create / rename) currently owns keystrokes.
    #[must_use]
    pub fn is_text_entry(&self) -> bool {
        matches!(
            self.mode,
            ProfilesMode::Creating(_) | ProfilesMode::Renaming { .. }
        )
    }

    /// Whether a sub-flow is open (any non-`List` mode). While true, `Esc` cancels the sub-flow
    /// and `Tab` switches the focused field rather than cycling the top-level tabs.
    #[must_use]
    pub fn in_sub_flow(&self) -> bool {
        !matches!(self.mode, ProfilesMode::List)
    }

    /// The selected profile, if any.
    #[must_use]
    pub(crate) fn selected_profile(&self) -> Option<&Profile> {
        self.selected.and_then(|i| self.profiles.get(i))
    }

    fn move_selection(&mut self, forward: bool) {
        let len = self.profiles.len();
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

    /// Pure update for the switcher. Returns the [`ClientRequest`] a request-triggering event
    /// produces (create/rename submit, delete confirm, refresh), or `None` for a local edit or any
    /// event while a request is outstanding. `Cancel`/`Quit`, tab-switching, and pick-active
    /// (`Submit` on the list) are handled by the caller before reaching here. The `session`
    /// supplies the token for the payloads.
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
            ProfilesMode::List => self.handle_list_event(event, session),
            ProfilesMode::Creating(_) => self.handle_create_event(event, session),
            ProfilesMode::Renaming { .. } => self.handle_rename_event(event, session),
            ProfilesMode::ConfirmingDelete { .. } => self.handle_delete_event(event, session),
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
            Event::BeginAddProfile => {
                self.message = None;
                self.mode = ProfilesMode::Creating(ProfileForm::empty());
            }
            Event::BeginRenameProfile => self.begin_rename(),
            Event::BeginDeleteProfile => self.begin_delete(),
            Event::Refresh => return self.refresh(session),
            _ => {}
        }
        None
    }

    fn begin_rename(&mut self) {
        let Some(profile) = self.selected_profile() else {
            return;
        };
        let mode = ProfilesMode::Renaming {
            profile_id: profile.id.clone(),
            form: ProfileForm::from_profile(profile),
        };
        self.message = None;
        self.mode = mode;
    }

    fn begin_delete(&mut self) {
        let Some(profile) = self.selected_profile() else {
            return;
        };
        let mode = ProfilesMode::ConfirmingDelete {
            profile_id: profile.id.clone(),
            name: profile.name.clone(),
        };
        self.message = None;
        self.mode = mode;
    }

    fn handle_create_event(
        &mut self,
        event: Event,
        session: Option<&Session>,
    ) -> Option<ClientRequest> {
        let ProfilesMode::Creating(form) = &mut self.mode else {
            return None;
        };
        match event {
            Event::Char(c) => form.push_char(c),
            Event::Backspace => form.backspace(),
            Event::Submit => return self.submit_create(session),
            Event::Cancel => self.mode = ProfilesMode::List,
            _ => {}
        }
        None
    }

    fn handle_rename_event(
        &mut self,
        event: Event,
        session: Option<&Session>,
    ) -> Option<ClientRequest> {
        let ProfilesMode::Renaming { form, .. } = &mut self.mode else {
            return None;
        };
        match event {
            Event::Char(c) => form.push_char(c),
            Event::Backspace => form.backspace(),
            Event::Submit => return self.submit_rename(session),
            Event::Cancel => self.mode = ProfilesMode::List,
            _ => {}
        }
        None
    }

    fn handle_delete_event(
        &mut self,
        event: Event,
        session: Option<&Session>,
    ) -> Option<ClientRequest> {
        // Confirm with `Submit`; `Cancel` (Esc) resets to the list, mirroring the notes/Tasks
        // handlers.
        match event {
            Event::Submit => return self.submit_delete(session),
            Event::Cancel => self.mode = ProfilesMode::List,
            _ => {}
        }
        None
    }

    fn submit_create(&mut self, session: Option<&Session>) -> Option<ClientRequest> {
        let session = session?;
        let ProfilesMode::Creating(form) = &mut self.mode else {
            return None;
        };
        form.error = None;
        let req = CreateProfileRequest {
            name: form.name.trim().to_owned(),
        };
        Some(ClientRequest::CreateProfile {
            token: session.token.clone(),
            req,
        })
    }

    fn submit_rename(&mut self, session: Option<&Session>) -> Option<ClientRequest> {
        let session = session?;
        let ProfilesMode::Renaming { profile_id, form } = &mut self.mode else {
            return None;
        };
        form.error = None;
        let req = UpdateProfileRequest {
            name: form.name.trim().to_owned(),
        };
        Some(ClientRequest::UpdateProfile {
            token: session.token.clone(),
            profile_id: profile_id.clone(),
            req,
        })
    }

    fn submit_delete(&mut self, session: Option<&Session>) -> Option<ClientRequest> {
        let session = session?;
        let ProfilesMode::ConfirmingDelete { profile_id, .. } = &self.mode else {
            return None;
        };
        Some(ClientRequest::DeleteProfile {
            token: session.token.clone(),
            profile_id: profile_id.clone(),
        })
    }

    fn refresh(&mut self, session: Option<&Session>) -> Option<ClientRequest> {
        let session = session?;
        Some(ClientRequest::ListProfiles {
            token: session.token.clone(),
        })
    }
}
