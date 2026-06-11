---
name: new-crate
description: Scaffold a new workspace crate that inherits all shared workspace settings (version, edition, authors, license, lints) and ships the project's README-as-docs and test-layout conventions from the start. Use whenever adding a crate under crates/.
audience: dev
---

# new-crate

## When to invoke

- Whenever you add a crate under `crates/`. Use this instead of a bare `cargo new` so the
  crate inherits workspace settings and carries the required README + test layout from day
  one, rather than being retrofitted later.

## Steps

1. **Create the crate** under `crates/<name>/` (`cargo new --lib crates/<name>`, or `--bin`
   for a binary). Pick `name` per the crate-layout table in `CLAUDE.md` (`contract`,
   `server` → binary `organized-koalad`, `tui` → binary `organized-koala`, narrow shared
   crates as needed).

2. **Inherit from the workspace** in `crates/<name>/Cargo.toml` — do NOT redefine anything
   the workspace owns:

   ```toml
   [package]
   name = "<name>"
   version.workspace = true
   edition.workspace = true
   authors.workspace = true
   description.workspace = true
   repository.workspace = true
   license.workspace = true
   keywords.workspace = true
   categories.workspace = true
   readme.workspace = true

   [lints]
   workspace = true

   [dependencies]
   ```

   `[lints] workspace = true` is mandatory — it pulls in the deny-level rust + clippy gate
   defined in the root `Cargo.toml`. A crate missing it is a review-blocking finding.

3. **Write `crates/<name>/README.md`** — this is the crate's public documentation. Import it
   at the crate root so prose and API docs never drift, and so the crate-level `missing_docs`
   lint is satisfied:

   ```rust
   #![doc = include_str!("../README.md")]
   ```

   Put it as the first line of `lib.rs` (libraries) or `main.rs` (binaries). Keep the README
   free of ```rust fences unless you intend them as doc tests.

4. **Document the public API.** Every public item needs a doc comment (`rust.missing_docs =
   "deny"`), and every public type needs `#[derive(Debug)]` or a manual impl
   (`rust.missing_debug_implementations = "deny"`). The main public items carry a **doc
   test** demonstrating usage:

   ```rust
   /// Adds two numbers.
   ///
   /// # Examples
   /// ```
   /// assert_eq!(my_crate::add(2, 2), 4);
   /// ```
   pub fn add(a: u64, b: u64) -> u64 { a + b }
   ```

5. **Lay out tests** (see the `rust-standards` skill):
   - **Unit tests** live next to the module they test, in a sibling `tests.rs`, wired from
     the module with `#[cfg(test)] mod tests;`. Module-directory / `mod.rs` layout only —
     self-named files like `foo.rs` are denied by `clippy::self_named_module_files`, so a
     module `foo` lives in `foo/mod.rs` with its tests alongside in `foo/tests.rs`.
   - **Integration tests** live in the crate's top-level `tests/` directory, exercising the
     public API.

6. **Register the crate.** A non-trivial crate gets its OWN dev agent, added by `eng-manager`
   at creation time (see "One agent per crate" in `CLAUDE.md`); a genuinely trivial crate is
   shared by all agents. A new crate that introduces or changes a wire shape is an ADR event
   (`contract` is the single source of truth).

7. **Verify**: `./ok.sh build && ./ok.sh lint && ./ok.sh test && ./ok.sh fmt --check`.

## Reference example

`crates/organized-koala/` is the seed crate and demonstrates every rule above — workspace
inheritance, README-as-doc via `include_str!`, a documented public fn with a doc test, and
unit tests in a sibling `tests.rs`.

## Extending this skill

Living document — `eng-manager` appends durable crate-scaffolding learnings here with a
rationale.
