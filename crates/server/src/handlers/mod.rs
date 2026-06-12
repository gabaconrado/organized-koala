//! HTTP handlers for the ADR-0005 surface. Each is `tracing`-instrumented; mutations emit an
//! INFO event and errors are recorded by the boundary error type. All profile/task queries
//! are ownership-joined so a profile the caller does not own is `404 not_found` (never 403).

mod auth;
mod health;
mod profiles;
mod tasks;

pub use auth::{login, register};
pub use health::healthz;
pub use profiles::list_profiles;
pub use tasks::{close_task, create_task, list_tasks};
