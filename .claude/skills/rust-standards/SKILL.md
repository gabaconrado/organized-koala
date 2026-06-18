---
name: rust-standards
description: Rust-specific standards for organized-koala. Extended over time via learnings + human feedback.
audience: dev
---

# Rust standards

## When to invoke

- Before writing or reviewing Rust in any crate.

## The standards

### Errors

- **Library crates** (`contract`, shared crates): errors are **strongly typed** with
  `thiserror`. Callers match on variants.
- **Binary crates** (`server`/`organized-koalad`, `tui`/`organized-koala`): use `anyhow` for
  application-level error propagation at the top.
- The HTTP error contract (status + `{ code?, message }`) is mapped at the server boundary
  from typed errors; the `code` is the stable, machine-matchable identifier.

### Tests

Idiomatic Rust layout (see [Rust by Example — Testing](https://doc.rust-lang.org/rust-by-example/testing.html)):

- **Unit tests** live right next to the module they test, in a sibling file named `tests.rs`,
  declared from the module with `#[cfg(test)] mod tests;`. The declaration is the *only*
  test-related line in the source file — the test code itself stays out of source. This lets
  agents parallelize and keeps source focused. (Module-directory / `mod.rs` layout only —
  self-named files like `foo.rs` are denied by `clippy::self_named_module_files`. So a module
  `foo` is defined by `foo/mod.rs` and its tests sit alongside it in `foo/tests.rs`, keeping
  the test file close to the module it covers.)
- **Integration tests** live in the crate's top-level `tests/` directory, exercising the
  crate's **public API**; mock only external services. "Hard to test" ⇒ bubble up to
  architecture review (do not bend source).
- **A binary crate that will be integration-tested needs a `[lib]` target — scaffold it
  lib+bin from the start (learned 0003).** A crate's `tests/` directory links against the
  crate's **library**, not its binary, so a binary-only crate (`main.rs` with no `[lib]`)
  cannot expose anything — `app::router`, `AppState`, config types — for `tests/` to drive
  in-process; the suite simply will not link. The fix is a `[lib] name = "<crate>"` target plus
  a thin `src/lib.rs` that declares the module tree and re-exports the test seams, with
  `main.rs` reduced to a shell over the lib (CLI parsing + a call into the library). Do this at
  scaffold time for any binary crate expected to carry integration tests (e.g. `server`/
  `organized-koalad`), not as a retrofit mid-cycle — 0003 had `tester` blocked until `server-dev`
  added the split. Same shape applies to `tui`/`organized-koala` when its non-`TestBackend`
  surface needs in-process tests.
- For a crate whose entire public surface *is* its API — a pure-DTO crate like `contract`,
  with no private/internal logic — the crate-root `tests/` public-API suite plus doctests is
  the correct and complete layout; `module/tests.rs` unit tests apply only where there is
  private/internal logic to cover (learned 0002).
- In **test code**, `unwrap`/`expect`/`panic` are acceptable. If the clippy `*_used` /
  `panic` denies fire there, a crate-root `#![cfg_attr(test, allow(clippy::unwrap_used,
  clippy::expect_used, clippy::panic))]` is the sanctioned, documented exception.
- **Make an interactive / IO-driven surface testable by separating the pure core from the
  effectful shell (learned 0004).** The TUI's whole interactive surface was driven through
  `ratatui`'s `TestBackend` with **no live server and no real terminal** because the crate was
  built as three pure layers behind one injected effect: (a) a **pure update function**
  (`App::handle_event` — a state machine over a transport-agnostic `Event` enum, returning the
  next state, never doing IO itself); (b) **pure draw functions** (`ui::*` that render a state
  into a `Frame`/buffer); (c) a **pure key-mapping** (`map_key`: `crossterm::KeyEvent` →
  `Event`, with no side effects); and the one external service — the server — reached only
  through an **injected `Client` trait**, so tests swap in a scripted fake while production uses
  the `reqwest` impl. The effectful shell (the crossterm driver, raw-mode guard, the real HTTP
  client) is a thin rim around that core. This is the ADR-0003 layer-2 enabler: `map_key`,
  `handle_event`, and the draw fns are unit/`TestBackend`-testable directly, and the only mock
  is the sanctioned external-service trait — no internal collaborator is mocked. If view code
  and HTTP code intertwine (so the suite can't be written without a live server or TTY), that
  is the ADR-0003 architecture smell — bubble up, don't bend the test.

### Documentation

- Every crate carries a `crates/<crate>/README.md` imported at the crate root with
  `#![doc = include_str!("../README.md")]`, so the README **is** the crate's rustdoc landing
  page (and satisfies crate-level `missing_docs`).
- All public items are documented (`rust.missing_docs = "deny"`) and every public type
  derives/implements `Debug` (`rust.missing_debug_implementations = "deny"`). The main public
  items carry a **doc test** demonstrating usage.
- Scaffold new crates with the `new-crate` skill so this layout is correct from the start.

### Lints

- The lint gate lives in **`[workspace.lints]`** in the root `Cargo.toml` (rust + clippy, all
  at `deny`); every crate opts in with `[lints] workspace = true`, so `cargo clippy`,
  `cargo build`, and rust-analyzer enforce the same gate — not just `./ok.sh lint`.
- Notable denies to **write around, not silence**: `clippy::unwrap_used`,
  `clippy::expect_used`, `clippy::panic`, `clippy::indexing_slicing`,
  `clippy::as_conversions`, `clippy::todo`/`unimplemented`/`unreachable`,
  `rust::missing_docs`, `rust::missing_debug_implementations`, `rust::unused_results`.
- **Never add `#[allow(...)]` without a documented, genuinely-good reason** in a comment on
  the attribute. An unjustified `#[allow]` is a review-blocking finding.

### Sensitive data

- Wrap every secret — passwords, tokens, JWT/session keys, DB credentials — in
  `secrecy::SecretString` / `Secret<T>`. This zeroizes the value on drop (via `zeroize`) and
  its `Debug`/`Display` render as `[REDACTED]`, so the secret can never leak through a derived
  `Debug`, a log/trace line, or a span field. Expose the inner value only at the point of use
  with `expose_secret()`.
- **Never** `#[derive(Debug)]` a struct, error variant, or DTO that holds a bare secret —
  hold it as a `Secret<_>` (or hand-write a redacting `Debug`). A bare secret reachable from a
  `Debug` impl, a log, or auto-instrumentation is a review-blocking leak.

### General

- Prefer the standard `./ok.sh` verbs; keep sqlx in **offline mode** and refresh `.sqlx/`
  via `./ok.sh prepare` when queries change.

## Extending this skill

Living document — `eng-manager` appends durable Rust learnings here with a rationale.
