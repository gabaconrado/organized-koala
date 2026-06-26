//! The single post-auth tabbed view: a `Tasks | Notes | Profiles` tab bar over three list panes
//! that coexist behind one [`Screen`](super::Screen) (ADR-0010 §1).
//!
//! The three list states ([`TaskListState`], [`NotesState`], [`ProfilesState`]) are the *panes* of
//! this view rather than mutually-exclusive screens. The active tab is a **view selector**, not
//! cached server data (hard-constraint #1): each pane still derives from a server response for the
//! active profile, and per-tab list selection is transient process-lifetime UI state preserved
//! across switches. Tab switching is `Tab` / `Shift+Tab` only, cycling
//! `Tasks → Notes → Profiles → Tasks` and back; the per-pane action keys are unchanged.

use super::notes::NotesState;
use super::profiles::ProfilesState;
use super::task_list::TaskListState;

/// Which pane the post-auth tabbed view is showing. The variants double as the cycle order
/// (`Tasks → Notes → Profiles → Tasks`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    /// The task-list pane for the active profile.
    Tasks,
    /// The notes pane for the active profile.
    Notes,
    /// The profile switcher pane.
    Profiles,
}

impl Tab {
    /// The next tab in the cycle (`Tab` key): `Tasks → Notes → Profiles → Tasks`.
    #[must_use]
    pub fn next(self) -> Self {
        match self {
            Tab::Tasks => Tab::Notes,
            Tab::Notes => Tab::Profiles,
            Tab::Profiles => Tab::Tasks,
        }
    }

    /// The previous tab in the cycle (`Shift+Tab` key): the reverse of [`Self::next`].
    #[must_use]
    pub fn prev(self) -> Self {
        match self {
            Tab::Tasks => Tab::Profiles,
            Tab::Notes => Tab::Tasks,
            Tab::Profiles => Tab::Notes,
        }
    }
}

/// The post-auth tabbed view: the active tab plus the three list panes, all alive at once so
/// per-tab selection and any open sub-flow survive a tab switch. Only the active pane's request
/// marker is consulted by the surface-level in-flight logic.
#[derive(Debug, Clone)]
pub struct MainState {
    /// The pane currently shown and receiving per-pane action keys.
    pub active_tab: Tab,
    /// The task-list pane for the active profile.
    pub tasks: TaskListState,
    /// The notes pane for the active profile.
    pub notes: NotesState,
    /// The profile switcher pane.
    pub profiles: ProfilesState,
}

impl MainState {
    /// Builds the tabbed view with Tasks selected by default (ADR-0010 §1) and empty panes; each
    /// pane is then populated from its own server list response.
    #[must_use]
    pub fn new(tasks: TaskListState, notes: NotesState, profiles: ProfilesState) -> Self {
        Self {
            active_tab: Tab::Tasks,
            tasks,
            notes,
            profiles,
        }
    }
}
