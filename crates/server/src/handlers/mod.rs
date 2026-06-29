//! HTTP handlers for the ADR-0005 surface. Each is `tracing`-instrumented; mutations emit an
//! INFO event and errors are recorded by the boundary error type. All profile/task/note
//! queries are ownership-joined so a profile (or note) the caller does not own is
//! `404 not_found` (never 403).

mod auth;
mod health;
mod notes;
mod profiles;
mod subtasks;
mod tasks;
mod timer;

pub use auth::{login, register};
pub use health::healthz;
pub use notes::{create_note, delete_note, get_note, list_notes, update_note};
pub use profiles::{create_profile, delete_profile, list_profiles, rename_profile};
pub use subtasks::{
    create_subtask, delete_subtask, list_profile_subtasks, list_subtasks, patch_subtask,
};
pub use tasks::{create_task, delete_task, list_tasks, patch_task};
pub use timer::{get_config, get_session, start_session, stop_session, update_config};
