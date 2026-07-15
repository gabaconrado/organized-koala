#![doc = include_str!("../README.md")]
// Source-owned unit tests (e.g. `app::text_input`) may use `unwrap`/`expect`/`panic` freely; the
// workspace denies these in production paths (rust-standards sanctioned exception).
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

pub mod app;
pub mod client;
pub mod terminal;
pub mod ui;
