# Organized Koala

A small, deliberately-simple suite of personal-productivity tools. It does a few things and
does them plainly — no folders, no tags, no nesting, no configuration sprawl.

## What it does

- **To-do list** — capture tasks with a title, a description, and an open/closed status. That
  is the whole model: no subtasks, categories, or labels.
- **Pomodoro timer** — start and stop focus sessions. There is no pause; stopping resets. The
  only setting is the session length (30 minutes by default).
- **Notes** — jot free-form notes, each with a title and some content. No folders, no tags.
- **Profiles** — keep separate spaces (for example *work* and *personal*) under one account.
  Each profile has its own to-do list and notes; nothing leaks between them.

## How it is used

Organized Koala comes in two parts:

- A **server** that holds all your data and is the source of truth.
- A **terminal app (TUI)** that you run on your own machine to read and change that data.

The terminal app keeps nothing locally — it always talks to the server — so you can run it
from anywhere and see the same up-to-date information. You sign in with a username or email and
a password.

## Running it

All common operations go through the `./ok.sh` script at the root of the repository (building,
running the server, launching the terminal app, and bringing up the full stack). Run `./ok.sh`
with no arguments to see the available commands.

## Setting up a development environment

Everything is driven through `./ok.sh`, so the software below is what those verbs need.

### Required

- **Rust toolchain** — install via [rustup][rustup]. The exact version is pinned in
  `rust-toolchain.toml` (currently 1.96.0) and installed automatically on the first `cargo`
  invocation, along with the `rustfmt`, `clippy`, `rust-analyzer`, and `rust-src` components.
  `cargo` comes with it.
- **Docker** with the **Compose v2** plugin ([install][docker]). Needed both for `./ok.sh up` /
  `./ok.sh down` (the full stack: Postgres + server + OTel collector) **and** for `./ok.sh test`,
  which boots a throwaway test Postgres via Compose (unless you provide your own `DATABASE_URL`).

### Per-verb tools (install as needed)

- **[`cargo-llvm-cov`][llvm-cov]** — for `./ok.sh coverage`, the reported-only coverage verb.
  Install with `cargo install cargo-llvm-cov`.
- **[`sqlx-cli`][sqlx-cli]** — only for `./ok.sh prepare`, which regenerates the committed
  `.sqlx/` offline query cache against a live database. A normal build/test does **not** need it
  (the cache is committed and the workspace compiles in sqlx offline mode). Install with
  `cargo install sqlx-cli`.
- **[`rumdl`][rumdl]** (Markdown) and **[ShellCheck][shellcheck]** (shell) — used by the
  editor's lint-on-save hook. Optional for a manual workflow, but install them to match the
  linting the repo expects.

### Runtime: desktop notifications (Linux)

The terminal app fires a desktop notification when a focus timer ends. **Nothing is needed to
build or run** — no build-time apt package is required (the notification crate uses a pure-Rust
D-Bus backend). For the notification to actually **appear** on Linux you need a **notification
daemon** running on your session D-Bus; one is present on Ubuntu's default GNOME desktop out of
the box. In a bare TTY, headless, or SSH-without-a-graphical-session context there may be no
daemon — delivery then degrades **silently and non-fatally** (the app never crashes or blocks).

### One-time repository setup

- Enable the committed git hooks (the pre-commit secret scan):
  `git config core.hooksPath .githooks`.
- Sanity-check the toolchain end-to-end: `./ok.sh build`, then `./ok.sh check` (test + lint +
  format check).

[rustup]: https://rustup.rs
[docker]: https://docs.docker.com/get-docker/
[llvm-cov]: https://github.com/taiki-e/cargo-llvm-cov
[sqlx-cli]: https://crates.io/crates/sqlx-cli
[rumdl]: https://crates.io/crates/rumdl
[shellcheck]: https://www.shellcheck.net
