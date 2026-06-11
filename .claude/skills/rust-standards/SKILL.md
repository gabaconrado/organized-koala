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

- Tests live in **their own files** (e.g. `tests/` or a sibling `*_test.rs` module file),
  **not** inline `#[cfg(test)] mod tests` blocks in source — this lets agents parallelize and
  keeps source focused. (The placeholder `lib.rs` inline test is removed during restructure.)

### Lints

- Hard rules are enforced by clippy (`./ok.sh lint` runs `-D warnings`).
- **Never add `#[allow(...)]` without a documented, genuinely-good reason** in a comment on
  the attribute. An unjustified `#[allow]` is a review-blocking finding.

### General

- Prefer the standard `./ok.sh` verbs; keep sqlx in **offline mode** and refresh `.sqlx/`
  via `./ok.sh prepare` when queries change.

## Extending this skill

Living document — `eng-manager` appends durable Rust learnings here with a rationale.
