---
id: 0002
title: Contract crate + workspace restructure (slice 1 of 0001)
status: working      # inbox → planned → ready → working → review → awaiting-merge → merged | blocked
priority: high       # high | medium | low
parent: 0001
depends-on: []
branch: feature/0002-contract-crate
worktree: .claude/worktrees/0002-contract-crate
created: 2026-06-11
updated: 2026-06-11
---

## Feature request

**Goal:** The workspace is restructured to the target crate layout and `crates/contract`
exists as the single source of truth for every wire shape of the foundational slice, per
[ADR-0005][adr-0005].

**Why:** First slice of [0001][feat-0001] — the contract seam must exist
before server (0003) and TUI (0004) can build against it.

**Acceptance criteria:**

- [ ] The `crates/organized-koala` placeholder is removed; `crates/contract` exists
      (scaffolded via the `new-crate` skill, `[lints] workspace = true`, README-as-rustdoc).
- [ ] DTOs per ADR-0005: `RegisterRequest`, `LoginRequest`, `SessionResponse`, `Profile`,
      `Task`, `TaskStatus` (`open`/`done`), `CreateTaskRequest`, `ErrorBody { code?, message }`,
      and the stable error-code identifiers (`validation_failed`, `invalid_credentials`,
      `unauthenticated`, `not_found`, `username_taken`, `email_taken`, `internal`).
- [ ] Serde round-trips match the ADR-0005 wire conventions: snake_case fields, UUID-string
      ids, RFC 3339 UTC timestamps, lowercase enums, nullable `closed_at`.
- [ ] Public items documented with doc tests on the main types; `./ok.sh build`, `test`,
      `lint`, `fmt --check` all green.

**Out of scope:** server/TUI crates (0003/0004), any HTTP or DB code, Notes/Pomodoro DTOs.

<!-- written by `architect` via the `plan` skill -->
## Plan(s)

### Plan: contract crate (2026-06-11, architect)

**Approach:** Pure-DTO crate, no I/O. Remove the placeholder crate, scaffold
`crates/contract` with the `new-crate` skill, define the ADR-0005 shapes with `serde`
derives, and lock the wire format with serde round-trip tests so 0003/0004 consume a frozen
seam. This is the tracer bullet's first segment: everything downstream imports these types.

**ADR:** [ADR-0005][adr-0005] (accepted; shapes are
fixed — deviations re-enter via `architect`).

**Slices:**

1. [contract-owner] Remove `crates/organized-koala` placeholder; update root `Cargo.toml`
   workspace members — files: `Cargo.toml`, `crates/organized-koala/` (delete),
   `Cargo.lock`.
2. [contract-owner] Scaffold + author `crates/contract`: auth DTOs, `Profile`, task DTOs,
   `ErrorBody` + error-code identifiers, with rustdoc + doc tests — files:
   `crates/contract/**` (src, README.md, Cargo.toml).
3. [tester] Serde round-trip / wire-format tests against the public API (JSON fixtures
   asserting ADR-0005 conventions incl. unknown-code tolerance) — files:
   `crates/contract/tests/**`.

**Assumptions:**

- The error `code` is represented so the TUI can match known codes while tolerating unknown
  ones (forward compatibility); exact Rust representation (enum with catch-all vs. typed
  constants over `String`) is `contract-owner`'s call within ADR-0005's stable string values.
- Timestamp/UUID crate choice (`chrono` vs `time`, `uuid`) is `contract-owner`'s call; the
  wire format (RFC 3339 UTC strings, UUID strings) is fixed by ADR-0005.
- Validation *rules* (username `@` ban, non-empty title) are enforced server-side in 0003;
  the contract crate carries the shapes and documents the rules, it does not enforce them.
- No shared observability crate this slice; server-local tracing init lives in 0003.

**Risks:**

- Shape churn discovered during 0003/0004 ripples back here and to ADR-0005 — blast radius
  is both consumers; mitigated by freezing shapes in the ADR before code.
- Workspace-member edits touch the root `Cargo.toml` lint/lock setup; keep the
  `[workspace.lints]` gate intact (review-blocking if weakened).

<!-- append-only; dated, AUTHORED entries. Human feedback = an UNCHECKED box: [ ] unhandled, [x] addressed + re-reviewed (the ONLY re-entry signal). -->
## Log / comments

- 2026-06-11 [architect] item created as slice 1/3 of 0001 via the `plan` skill; plan
  authored; ADR-0005 accepted; status `planned` → `ready`. No upstream dependency — this is
  the first workable item.
- 2026-06-11 [drive] claimed; worktree `.claude/worktrees/0002-contract-crate` on branch
  `feature/0002-contract-crate` cut from `main`; status `ready` → `working`.

<!-- written at end of cycle; what the human reviews -->
## Summary

[adr-0005]: ../../docs/adr/0005-foundational-wire-contract.md
[feat-0001]: ./0001-foundational-slice.md
