---
id: 0002
title: Contract crate + workspace restructure (slice 1 of 0001)
status: awaiting-merge   # inbox → planned → ready → working → review → awaiting-merge → merged | blocked
priority: high       # high | medium | low
parent: 0001
depends-on: []
branch: feature/0002-contract-crate
worktree: .claude/worktrees/0002-contract-crate
created: 2026-06-11
updated: 2026-06-12
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
  `feature/0002-contract-crate` cut from `main`; status `ready` → `working`. (This branch copy
  is the authoritative live record per the board-on-branch model; `main` holds a frozen claim
  snapshot until merge.)
- 2026-06-11 [drive] build complete. `contract-owner` removed the `crates/organized-koala`
  placeholder and authored `crates/contract` (ADR-0005 DTOs + `ErrorCode` with lossless
  `Unknown` forward-compat + redacting `Password` newtype). `tester` added 37 serde/wire-format
  integration tests (+ 12 doctests = 49, all green). build/lint/fmt clean. Branch is code-only
  and linear atop `main`; the secret-scan hook fix that unblocked the ADR-mandated `password`
  field was relocated to `main` (shared infra, commit `d34570c`) and the branch rebased onto it,
  so the duplicate branch commit dropped. Commits: `7ca3e25` contract crate, `56833a6` tests.
  status `working` → `review`.
- 2026-06-11 [reviewer] cold re-review at rebased head. Gate green from worktree root: build
  clean; test 37 integration + 12 doctests, all pass; lint clean (deny-warnings, only sanctioned
  test-cfg allow); fmt clean. ADR-0005 conformance exact (all 8 DTOs, snake_case, UUID/RFC3339
  strings, lowercase `open`/`done`, nullable-emitted `closed_at`, `ErrorBody{code?,message}`
  code-omitted-when-None, bare task array); full 7-code error set + lossless `Unknown` forward-
  compat (tested); `Password` redacting Debug + transparent serialize, no leak; pure-DTO (deps
  `serde` + dev `serde_json`), no I/O/scope creep; `[workspace.lints]` intact. Confirmed
  `832a0c9` is board-only (no code follows `56833a6`). No fix-now findings.
  REVIEW-STATUS: approved 56833a6
- 2026-06-11 [verifier] VERIFIED at head `832a0c9` (last code `56833a6`). Pure-DTO seam, no
  stack to boot — live-stack E2E correctly deferred to 0003/0004 per ADR-0003. No hidden runtime
  surface (deps `serde` + dev `serde_json`; greps for net/fs/io/sqlx/tokio/axum/process found
  none). `./ok.sh test` green: 9 auth + 12 error + 3 profile + 13 task = 37 integration + 12
  doctests, non-vacuous (asserts real JSON bytes, `code`-omission, lowercase enums, `Password`
  redaction, `Unknown` round-trip). `./ok.sh build` clean rebuild of the `contract` rlib. Output
  matches ADR-0005 via the committed round-trip suite. Worktree clean, no scratch files.
- 2026-06-11 [eng-manager] cycle tail. Process learnings captured on `main` (the corrected
  three-home state model: shared/cross-cutting on `main` and never on a branch; feature-local
  board item travels with its code on the branch; derived dashboard regenerated on `main`;
  reviewer/verifier read-only and report verdicts the orchestrator commits on-branch). Handoff
  entry written; dashboards regenerated; `contract-owner` already owns `crates/contract` (no new
  agent). Summary filled; status `review` → `awaiting-merge`. Definition of done holds.
- [x] 2026-06-12 [human] **suggestion**: Can we use chrono::Datetime directly in the contract
  types so applications don't need to care about date formats?
  - 2026-06-12 [contract-owner] Done on-branch and re-reviewed (approved `98d1a85`). Timestamps
    are now `chrono::DateTime<Utc>` (`Task.created_at`/`closed_at`, `Profile.created_at`);
    consumers get a typed timestamp instead of an opaque string, and malformed dates now fail to
    parse. Wire bytes are unchanged (RFC 3339 `…Z`, `closed_at: null` still emitted), so it sits
    inside ADR-0005's frozen wire format — no ADR. `chrono` added pure-DTO (`default-features =
    false, features = ["std","serde"]`, no clock/IO).
- [x] 2026-06-12 [human] **thought**: Tests in the contracts crate are in the integrated tests
  directory (crate-root/tests) instead of along with the modules (crate-root/module/tests.rs).
  This seems to go against the testing directives
  - 2026-06-12 [drive] Resolved (clarification, no code change). `rust-standards` defines two
    layers: unit tests in `module/tests.rs` for *internal/private* logic, and integration tests
    in the crate-root `tests/` exercising the *public API*. The `contract` crate is pure-DTO —
    its entire surface is public and the suite locks the serde wire format as an external
    consumer sees it, so `tests/` + doctests is the correct, complete layout (and what the plan's
    slice 3 specified). There is no private logic for `module/tests.rs` to cover. `eng-manager`
    adds a clarifying line to the `rust-standards` skill on `main` so this is a durable rule.
- 2026-06-12 [drive] feedback sweep re-entry. `architect` triaged both items: (1) chrono ⇒
  behaviour tweak, no ADR (Rust-representation change inside ADR-0005's frozen wire format,
  delegated to `contract-owner`); (2) test layout ⇒ no conflict, the public-API `tests/` suite
  plus doctests is the correct layout for a pure-DTO crate (clarification, and a `rust-standards`
  note on `main`). Status `awaiting-merge` → `working` to implement (1) on-branch before merge
  (zero blast radius — 0003/0004 not yet built). Re-review + re-verify to follow.
- 2026-06-12 [drive] re-build complete. `contract-owner` typed the timestamps as
  `chrono::DateTime<Utc>` (`Task.created_at`/`closed_at`, `Profile.created_at`), adding `chrono`
  with `default-features = false, features = ["std","serde"]` (no clock/IO surface) — wire bytes
  unchanged (RFC 3339 `…Z`, `closed_at: null` still emitted), verified. `tester` adapted the
  integration suite and added contract-hardening cases (malformed `created_at`/`closed_at`
  rejected; offset-bearing input normalized to UTC). Gate green from worktree root: 41
  integration + 12 doctests pass, lint clean (`--all-targets`), fmt clean. Commits: `bc61626`
  contract, `98d1a85` tests. status `working` → `review`.
- 2026-06-12 [reviewer] cold re-review of the chrono delta (`56833a6..98d1a85`). Gate green from
  worktree root: build clean; 41 integration + 12 doctests pass; lint clean (`--all-targets`, only
  sanctioned test-cfg allow); fmt clean. Wire-format invariance HOLDS — exact-byte assertions
  (`…Z` suffix, `closed_at: null` emitted, no `skip_serializing_if`/`rename` on timestamps) not
  weakened; doctest confirms chrono emits `Z` not `+00:00`. No ADR needed — ADR-0005 delegates
  Rust representation to `contract-owner`; wire format unchanged. Crate stays pure-DTO — chrono
  `["std","serde"]` only, transitive deps `num-traits` + `serde`, no `iana-time-zone`/clock/IO.
  Hardening tests non-vacuous. Confirmed `dbcd85d` is board-only (no code follows `98d1a85`). No
  fix-now findings. REVIEW-STATUS: approved 98d1a85
- 2026-06-12 [verifier] VERIFIED the chrono delta at head `5ed575b` (last code `98d1a85`). Pure-DTO
  seam, no stack to boot — live-stack E2E correctly deferred to 0003/0004 per ADR-0003. No hidden
  runtime surface: chrono `["std","serde"]` only, transitive deps `chrono`+`num-traits`+`serde`,
  greps for `iana-time-zone`/`tokio`/`reqwest`/`libc`/clock found none. `./ok.sh test` green and
  non-vacuous: 9 auth + 12 error + 4 profile + 16 task = 41 integration + 12 doctests = 53 passed.
  Spot-checked: `…Z` suffix + `closed_at: null` emitted asserted on real JSON bytes; malformed
  `created_at`/`closed_at` rejected; `+01:00` input normalized to `10:00:00Z` on the wire. `./ok.sh
  build` clean (contract rlib). Wire format unchanged vs the prior `String` representation per the
  round-trip suite. Worktree clean, no scratch files.

<!-- written at end of cycle; what the human reviews -->
## Summary

Restructured the workspace to the target crate layout and stood up `crates/contract` as the
single source of truth for the foundational wire shapes ([ADR-0005][adr-0005]). This is slice
1 of 3 of the foundational vertical slice ([0001][feat-0001]); it ships **no** I/O, HTTP, or
DB code — a pure-DTO seam that 0003 (server) and 0004 (TUI) build against.

**What shipped:**

- Removed the `crates/organized-koala` placeholder; updated the root workspace members and
  lockfile. New crate scaffolded via the `new-crate` skill (`[lints] workspace = true`,
  README-as-rustdoc).
- The ADR-0005 DTOs: `RegisterRequest`, `LoginRequest`, `SessionResponse`, `Profile`, `Task`,
  `TaskStatus` (`open`/`done`), `CreateTaskRequest`, and `ErrorBody { code?, message }` with the
  stable error-code identifiers (`validation_failed`, `invalid_credentials`, `unauthenticated`,
  `not_found`, `username_taken`, `email_taken`, `internal`) plus a lossless `Unknown` catch-all
  for forward compatibility.
- A `Password` newtype: transparent serialize, `[REDACTED]` `Debug` so a secret cannot leak
  through derived formatting.
- Wire format locked to ADR-0005: snake_case fields, UUID-string ids, RFC 3339 UTC timestamps,
  lowercase enums, `closed_at` nullable-and-emitted (not omitted), `code` omitted when `None`,
  bare task arrays.

**Human-feedback follow-up (timestamps typed):** after this item first reached
`awaiting-merge`, human feedback asked the contract to carry chrono dates directly so consumers
need not parse strings. Timestamps (`Task.created_at`/`closed_at`, `Profile.created_at`) are now
`chrono::DateTime<Utc>` rather than opaque strings — consumers get a typed value and malformed
dates now **fail to parse**. The **wire bytes are unchanged** (RFC 3339 `…Z`, `closed_at: null`
still emitted), so this stays inside ADR-0005's frozen wire format — no wire change, no ADR
(ADR-0005 delegates the Rust representation to `contract-owner`). `chrono` is added pure-DTO
(`default-features = false, features = ["std","serde"]` — no clock/IO surface). Both `[human]`
feedback items are resolved: the chrono change was implemented, re-reviewed (approved `98d1a85`)
and re-verified; the second item — questioning the crate-root `tests/` layout — was a
clarification (no code change): a pure-DTO crate whose whole surface is public is correctly and
completely covered by the public-API `tests/` suite plus doctests, captured as a durable
`rust-standards` rule.

**Validation:** serde/wire-format integration tests + 12 doctests, all green; `./ok.sh`
build/lint/fmt clean (deny-warnings, `missing-docs` enforced). Reviewer approved at code head
`56833a6` (initial cycle); after the chrono follow-up the suite grew to 41 integration + 12
doctests = 53, re-reviewed (approved `98d1a85`) and re-verified. Verifier confirmed the pure-DTO
seam with no hidden runtime surface — live-stack E2E correctly deferred to 0003/0004 per
[ADR-0003][adr-0003].

**For the human merging:** the branch `feature/0002-contract-crate` (head `832a0c9`, last code
`56833a6`) is linear atop `main` and is a fast-forward. `main` already carries everything this
branch's code depends on — ADR-0005 (`1a2540c`), the `.githooks/secret-scan.sh` fix relocated
as shared infra (`d34570c`), and the corrected board-on-branch workflow docs (`ed9510e`). The
branch itself is **code-only** plus its own board record; merging it fast-forwards `main` and
brings this item to its final state. (The secret-scan now requires an assigned credential value
so the ADR-mandated `password` field no longer false-positives — see CLAUDE.md / bash-standards
for the current scan shape.)

[adr-0003]: ../../docs/adr/0003-verification-layering.md
[adr-0005]: ../../docs/adr/0005-foundational-wire-contract.md
[feat-0001]: ./0001-foundational-slice.md
