#![doc = include_str!("../README.md")]

mod auth;
mod error;
mod note;
mod profile;
mod task;
mod timer;

pub use auth::{LoginRequest, Password, RegisterRequest, SessionResponse};
pub use error::{ErrorBody, ErrorCode};
pub use note::{CreateNoteRequest, Note, UpdateNoteRequest};
pub use profile::{CreateProfileRequest, Profile, UpdateProfileRequest};
pub use task::{
    CreateSubtaskRequest, CreateTaskRequest, MAX_TASK_LIST_LIMIT, Subtask, Task, TaskListQuery,
    TaskStatus, UpdateSubtaskRequest, UpdateTaskRequest,
};
pub use timer::{TimerConfig, TimerSession, UpdateTimerConfigRequest};
