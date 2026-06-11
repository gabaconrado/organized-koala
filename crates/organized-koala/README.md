# organized-koala (seed crate)

Placeholder workspace crate. It exists so the workspace compiles and so the project's
crate-level conventions have a concrete reference; it is **not** the target layout and is
removed/restructured into `contract` / `server` / `tui` during the first feature (see
`CLAUDE.md`).

It also serves as the reference example for the `new-crate` skill, demonstrating:

- inheriting shared metadata and the lint gate from the workspace `Cargo.toml`;
- importing this README at the crate root with `#![doc = include_str!("../README.md")]` so
  the README is the crate's rustdoc landing page;
- a documented public API with a doc test;
- unit tests in a sibling `tests.rs` wired with `#[cfg(test)] mod tests;`.
