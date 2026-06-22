---
id: 0004
title: TUI — register/login, default profile, task add/list/close (slice 3 of 0001)
status: awaiting-merge  # inbox → planned → ready → working → review → awaiting-merge → merged | blocked
priority: high       # high | medium | low
parent: 0001
depends-on: [0003]
branch: feature/0004-tui-foundational
worktree: .claude/worktrees/0004-tui-foundational
created: 2026-06-11
updated: 2026-06-18
---

## Feature request

**Goal:** The `organized-koala` TUI completes the loop: a user registers (choosing the
default profile name) or logs in, lands in their profile's task list, adds a task
(Title + Description), sees done/undone markers, and marks a task done — all against the
live server, holding no local persistence.

**Why:** Slice 3 of [0001][feat-0001]: closes the tracer bullet
TUI ↔ `contract` ↔ server ↔ Postgres; completing it satisfies the parent item's
acceptance criteria end-to-end.

**Acceptance criteria:**

- [ ] `crates/tui` (binary `organized-koala`) exists; `ratatui` + `crossterm` + `reqwest`;
      all wire types imported from `contract` (no local DTOs — hard-constraint #2).
- [ ] Auth screen: register (username, email, password, profile name) and login
      (identifier + password); on success the TUI fetches `GET /api/profiles` and enters
      the (single) profile's task list.
- [ ] Task list view: tasks newest-first with done/undone markers; add-task flow
      (Title + Description); mark-done sends `…/close` and re-renders from the server
      response (Status + Closed-at set).
- [ ] Error handling branches on the ADR-0005 `code`: `unauthenticated` → back to login;
      `validation_failed` → inline message; server offline/unreachable → clear blocking
      message with retry (no cached data — hard-constraint #1).
- [ ] No on-disk or cross-run state; JWT + active profile id live in process memory only.
- [ ] `tester`'s `ratatui` `TestBackend` suite covers view/update, keybindings, and
      error-code branching per [ADR-0003][adr-0003];
      `./ok.sh test|lint|fmt --check` green.

**Out of scope:** profile switching/creation UX, task edit/delete, Notes, Pomodoro,
any persistence, theming polish.

<!-- written by `architect` via the `plan` skill -->
## Plan(s)

### Plan: TUI client (2026-06-11, architect)

**Approach:** A thin stateless client over the frozen contract: an HTTP client module
(reqwest, typed on `contract` DTOs, mapping `ErrorBody` to a typed client error) and an
app core (screen state machine: Auth → TaskList; pure update functions over events) kept
strictly separable so `tester` can drive the whole interactive surface through
`TestBackend` with the server mocked (ADR-0003 layer 2), while the verifier exercises the
reqwest path against the live 0003 stack (layer 1).

**ADR:** [ADR-0005][adr-0005] (accepted) +
[ADR-0003][adr-0003] verification routing; no new ADR.

**Slices:**

1. [tui-dev] Scaffold `crates/tui` (`new-crate` skill); reqwest client module over
   `contract` types (register/login/profiles/tasks/close, `healthz` probe, `ErrorBody` →
   typed error) — files: `crates/tui/**`, root `Cargo.toml` member.
2. [tui-dev] App core + views: auth screen (register/login forms), task-list screen
   (markers, add-task input flow, mark-done), event/keybinding mapping, error-code
   branching, server-offline handling — files: `crates/tui/src/**`.
3. [tester] `TestBackend` suite with the server mocked: keybinding → action mapping,
   buffer-snapshot rendering of auth + task list, error-code branches
   (`unauthenticated` → login, `validation_failed` inline, offline message), statelessness
   (every view derives from a (mock) server response) — files: `crates/tui/tests/**` and
   module-sibling `tests.rs` files.
4. [verifier] Per ADR-0003: live pass over the reqwest client path against `./ok.sh up`,
   plus the delegation handshake — confirm the `TestBackend` suite exists and is green
   under `./ok.sh test`, quoting results. On completion, parent 0001's acceptance criteria
   are checked off and 0001 advances with this item.

**Assumptions:**

- This slice's accounts have exactly one profile (created at registration), so the TUI
  auto-selects the first profile from `GET /api/profiles`; a profile picker is the
  backlog "multiple profiles UX" item.
- Concrete keybindings (e.g. `a` add, `d`/`space` toggle-done, `q` quit) are `tui-dev`'s
  call; the tester suite pins whatever is chosen.
- After a mutation the TUI re-renders from the mutation response and/or a fresh list fetch
  — either satisfies statelessness; `tui-dev` picks the simpler.
- TUI-side tracing/OTel is **not** required by 0001's criteria (server-side audit traces
  satisfy them); deferred to the backlog observability item.
- The server-offline probe uses `healthz` and/or request-error mapping; no retry daemon —
  a manual retry key is the smallest UX.
- Mock mechanism for the client in tests (e.g. `wiremock`/local stub server vs trait-level
  fake) is `tester`'s call — server mocking is the sanctioned external-service mock.

**Risks:**

- If view code and HTTP code intertwine, the `TestBackend` suite becomes hard to write —
  that is the ADR-0003 architecture smell; bubble up instead of bending tests.
- Terminal raw-mode/teardown bugs are invisible to `TestBackend` — accepted residual risk;
  covered by the `docs/manual-smoke.md` checklist (ADR-0003 §3), not a gate here.
- Latency on every keystroke-triggered request: keep mutations request-per-action (no
  optimistic state — that would be client-side state); acceptable at personal scale.

<!-- append-only; dated, AUTHORED entries. Human feedback = an UNCHECKED box: [ ] unhandled, [x] addressed + re-reviewed (the ONLY re-entry signal). -->
## Log / comments

- 2026-06-11 [architect] item created as slice 3/3 of 0001 via the `plan` skill; plan
  authored; governed by ADR-0005 with ADR-0003 verification routing; status `planned` →
  `ready`. **Dependency gate:** do not start until
  [0003][feat-0003] is `awaiting-merge`/`merged` (the verifier
  needs the live stack); compile-time work only needs 0002, but the gate stays on 0003 to
  keep the cycle linear.
- 2026-06-18 [orchestrator] claimed via `/drive`; cut worktree
  `.claude/worktrees/0004-tui-foundational` on branch `feature/0004-tui-foundational` from
  `main`@8a7981b (carries ADR-0005 and the contract/server crates merged from 0002/0003);
  `ready` → `working`. Dependency gate satisfied: 0003 `merged`. Session: drive-2026-06-18.
- 2026-06-18 [tui-dev] slice 1 — scaffolded `crates/tui` (binary `organized-koala`,
  lib+bin split) via the `new-crate` skill; deps `ratatui` 0.29, `crossterm` 0.28, blocking
  `reqwest` 0.12 (rustls). Built `src/client/mod.rs`: a `Client` trait over health, register,
  login, list-profiles, list-tasks, create-task, close-task (all typed on `contract` DTOs,
  no local DTOs), with the `reqwest` impl `HttpClient`. The standard `ErrorBody` (code plus
  message) maps to a typed `ClientError`: `Api` (preserving the `ErrorCode` for branching)
  and `Offline` for any transport failure or unintelligible body. Crate auto-discovered by
  the existing `members = ["crates/*"]`; no root `Cargo.toml` edit needed.
- 2026-06-18 [tui-dev] slice 2 — app core + views. `src/app/mod.rs` is a pure screen state
  machine (`Auth` to `TaskList`, plus a blocking `Offline` screen) advanced by
  `App::handle_event` over a transport-agnostic `Event` enum, with the `Client` injected, so
  `tester` can drive it through `TestBackend` with a fake client (ADR-0003). Auth: login
  (identifier plus password) and register (username, email, password, profile-name); on
  success fetches `GET /api/profiles` and auto-selects the first profile (per Assumptions),
  then loads its task list. Task list: newest-first with done/undone markers, add-task
  sub-flow (Title plus Description), mark-done sends the close request and replaces the task
  in place from the server response, refresh re-fetches. Error-code branching per ADR-0005:
  `unauthenticated` drops the in-memory session and returns to login; `validation_failed`
  (and other Api errors) surface inline; offline goes to the blocking screen with a manual
  retry. `src/ui/mod.rs` holds pure draw fns; `src/terminal/mod.rs` is the crossterm driver
  with a pure `map_key` mapping and a raw-mode guard restoring the terminal on drop. JWT plus
  active profile id live in process memory only (hard-constraint #1).
- 2026-06-18 [tui-dev] keybindings chosen (pin these in the `TestBackend` suite): global
  `Esc` or `Ctrl+C` quit, except in the add-task sub-flow where `Esc` cancels instead;
  `Enter` submit/confirm; `Tab` or `Down` next field/item, `Shift+Tab` or `Up` previous;
  `Backspace` delete in the focused field. Auth screen: `F2` toggles login/register. Task
  list when not entering text: `a` add task, `c` mark selected done, `r` refresh, `q` quit.
  Offline screen: `r` retry. In text-entry contexts (auth forms, the add-task fields)
  printable keys are typed literally rather than treated as commands.
- 2026-06-18 [tester] slice 3 — `TestBackend` suite added under `crates/tui/tests/`, the only
  mock being a held, recording fake `Client` (ADR-0003 layer 2; no binary, no live DB). Shared
  scaffolding lives in `tests/common/mod.rs`: a clone-shared `FakeClient` (interior `Rc`,
  scripted per-endpoint response queues, a recorded call log), DTO builders that parse the
  canonical wire JSON through the `contract` derives (so the suite needs no direct `chrono`
  dep), and a `TestBackend` buffer-text render helper. Coverage across four files:
  - `keybindings.rs` (11 tests) pins the whole `map_key` contract — `Esc`/`Ctrl+C` quit (and
    `Esc` = cancel only in the add-task flow), `Enter` submit, `Tab`/`Down` = next and
    `BackTab`/`Up` = prev, `Backspace`, auth-only `F2` toggle, task-list `a`/`c`/`r`/`q`
    commands, offline `r` retry, and the context-sensitivity (`a`/`c`/`r`/`q` typed literally
    in the auth form and the open add-task flow; `r` literal on auth).
  - `rendering.rs` (7 tests) buffer-snapshots the login and register screens (field labels,
    masked password — plaintext never rendered), the task list (newest-first ordering,
    `[ ]`/`[x]` markers, profile in the header), the add-task panel, and the blocking offline
    screen.
  - `error_branches.rs` (9 tests) drives the ADR-0005 `code` branches through the fake:
    `validation_failed`/`invalid_credentials` surface inline and keep the session;
    `unauthenticated` (on refresh and on close) returns to login with the in-memory session
    dropped; transport failure goes to the blocking offline screen, and a manual `r` retry
    recovers (or stays offline while still down).
  - `flows.rs` (8 tests) covers login/register reaching the auto-selected profile's list (the
    exact login -> profiles -> list-tasks call sequence), the add-task flow posting Title +
    Description then re-rendering from the server's fresh list, mark-done sending `…/close` and
    replacing the row from the returned `Task`, and statelessness (the rendered list equals the
    server response, refresh drops stale data, a new app holds no session).
  Gates from the worktree root: `./ok.sh test` green (35 new tui tests; whole workspace
  passes), `./ok.sh fmt --check` clean, `./ok.sh lint` clean (no `#[allow]` beyond the
  sanctioned test-only `unwrap`/`expect`/`panic` exception plus a documented `dead_code` allow
  on the shared `common` fixture). No source touched.
- 2026-06-18 [reviewer] cold review of `3954cce..6d09213` (`crates/`, `Cargo.lock`,
  `Cargo.toml`). All four gates green at HEAD (`build`/`test`/`lint`/`fmt --check`). Verified:
  hard-constraint #2 (no local wire DTOs; every shape from `contract`), #1 (no file I/O,
  no persistence, no logging; JWT + active profile id in process memory only; offline path
  fabricates no cached data — statelessness tests assert this), the ADR-0005 error contract
  (`code`-preserving typed error; `unauthenticated`→login, `validation_failed`→inline,
  offline→blocking+retry all wired and tested), the ADR-0003 layer-2 `TestBackend` suite
  (exists, green, 35 tests, only mock is the injected `Client`), and no contract/migration/
  shared-state drift on the branch. `#[allow]` audit clean — only the sanctioned test-only
  exceptions, none leaked into source. No fix-now findings. One non-blocking nit: the
  orchestrator's board-claim commit `846ba2a` used a `noreply@anthropic.com` co-author
  trailer instead of the project `<agent>@organized-koala.local` form (board-only, outside
  reviewed code). Verdict: **REVIEW-STATUS: approved 6d09213**. Status `working` → `review`.
- 2026-06-18 [verifier] **VERIFIED** at code sha `6d09213`. Capabilities present (Docker
  29.5.3, Compose v5.1.4) — no gap. Booted `./ok.sh up` in the worktree (postgres healthy →
  `migrate` exited 0 → server healthy → otel-collector up) and exercised the live reqwest
  client path (ADR-0003 layer 1): every endpoint the `tui` `Client` consumes round-tripped
  with shapes matching `contract` — `register` 201, `login` 200, `GET /api/profiles` 200,
  task list/create/close (open→done, `closed_at` set). Error contract verified live with
  exact wire strings: `unauthenticated`/`invalid_credentials` 401, `username_taken`/
  `email_taken` 409, `validation_failed` 400, `not_found` 404. Profile-scoping (#4) verified
  with a second account — cross-profile reads/writes return 404 with no existence leak.
  Persistence verified across a server restart. OTel spans verified end-to-end: the
  collector received exported OTLP/gRPC spans (`list_profiles`/`list_tasks` with
  `user_id`/`profile_id` attrs). Layer-2 handshake: the `TestBackend` suite exists and is
  green under `./ok.sh test` (error_branches 9, flows 8, keybindings 11, rendering 7; whole
  workspace 0 failed). Only un-driven items: interactive crossterm on a real TTY (routed to
  the ungated manual smoke check per ADR-0003 §3) and the out-of-scope timer endpoint —
  neither blocks. Stack torn down; no edits, no scratch files.
- 2026-06-22 [orchestrator] freshened the branch against `main` (drive step 7) so the human
  reviews exactly what will merge. Rebased across this session as `main` advanced: original
  base `main`@8a7981b → `bd3f797` → `3954cce` (the last hop is 0005's planning — ADR-0006 +
  the planned 0005 item — committed to `main`). Each rebase rewrote shas, so the verdict
  citations above are relabelled to the current last-code sha along the chain
  `8fb0505` → `53da791` → **`6d09213`**, and the reviewer's review range to `3954cce..6d09213`.
  **Safe because the code tree is byte-identical to what was reviewed**:
  `git diff 53da791 6d09213 -- crates/ Cargo.toml Cargo.lock` is empty (`main` moved only in
  `docs/`/`.claude/`/`board/`, never `crates/`), so the approved+verified attestation carries
  forward unchanged — no re-review needed (DoD clause 7, code-identical branch). Gates re-run
  green on the rebased tree (`fmt --check`/`lint`/`test`); no code commit follows `6d09213`.
- [x] 2026-06-22 [human] suggestion: in `tui::app`, organize the methods/structs by
  submodule based on the feature (`auth`, `task_add`, `task_list`), leaving in `mod.rs`
  only the wiring/infrastructure code.
  - 2026-06-22 [orchestrator] triaged per the human's chosen sequencing: **re-homed to
    follow-up item 0005** (responsive TUI). The async/responsive-UI rework restructures
    `tui::app` anyway, so this submodule split is folded into 0005's scope rather than done
    twice under the soon-to-be-replaced synchronous model. Resolved-by-routing so 0004
    (verified, closes 0001) stays mergeable; not implemented on this branch by design.
    Board-only edit — review/verify verdicts unaffected.

<!-- written at end of cycle; what the human reviews -->
## Summary

**What was built.** `crates/tui` (binary `organized-koala`, lib+bin split; `ratatui` 0.29,
`crossterm` 0.28, blocking `reqwest` 0.12/rustls), the stateless TUI client that closes the
0001 tracer bullet: TUI ↔ `contract` ↔ server ↔ Postgres. The crate is structured as a pure
core behind one injected effect — a `Client` trait over health/register/login/list-profiles/
list-tasks/create-task/close-task (typed entirely on `contract` DTOs, no local wire shapes),
with the `reqwest` impl `HttpClient` and `ErrorBody` → a `code`-preserving typed `ClientError`;
a pure screen state machine `App::handle_event` (`Auth` → `TaskList` + a blocking `Offline`
screen); pure `ui` draw fns; and a crossterm driver with a pure `map_key` and a raw-mode guard.
The auth screen registers (username, email, password, profile-name) or logs in, then fetches
`GET /api/profiles` and auto-selects the single default profile into its task list (newest-
first, done/undone markers, add-task Title+Description sub-flow, mark-done via `…/close`
re-rendering from the server response). Keybindings (now pinned by tests): `Esc`/`Ctrl+C` quit
(`Esc` = cancel in add-task), `Enter` submit, `Tab`/`Down` next, `Shift+Tab`/`Up` prev,
`Backspace`, auth `F2` login/register toggle, task-list `a`/`c`/`r`/`q`, offline `r` retry.

**Acceptance criteria — all satisfied.** `crates/tui` exists with the required deps and imports
every wire shape from `contract` (#2); the auth→profile→task-list flow works; the task-list view
renders newest-first with markers, the add-task flow, and server-driven mark-done; error
handling branches on the ADR-0005 `code` (`unauthenticated`→login, `validation_failed`→inline,
offline→clear blocking message with manual retry, no cached data — #1); no on-disk/cross-run
state (JWT + active profile id in process memory only); and `tester`'s `ratatui` `TestBackend`
suite (35 tests across keybindings/rendering/error-branches/flows, ADR-0003 layer 2) covers
view/update, keybindings, and error-code branching, with `./ok.sh test|lint|fmt --check` green.

**Verdicts.** Reviewer **`REVIEW-STATUS: approved 6d09213`** (all gates green; #1/#2 held; error
contract + layer-2 suite verified; no fix-now findings — one board-only co-author-trailer nit).
Verifier **`VERIFY-STATUS: verified 6d09213`** — live over the full reqwest client path
(Docker 29.5.3 / Compose v5.1.4): every `Client` endpoint round-tripped with `contract`-matching
shapes, the ADR-0005 error contract with exact wire strings, profile-scoping (#4) with a second
account (404, no leak), persistence across a server restart, and OTel spans received end-to-end;
the `TestBackend` suite confirmed green. No contract change, no migration, no new ADR.

**Slice 0001 closeable.** This is slice 3 of 3 of the 0001 umbrella; merging it puts all three
children (0002 contract, 0003 server, 0004 TUI) on `main`, so parent 0001's end-to-end
acceptance — register/login → profile → task add/list/close across TUI ↔ contract ↔ server ↔
Postgres — is now closeable with this slice.

[adr-0003]: ../../docs/adr/0003-verification-layering.md
[adr-0005]: ../../docs/adr/0005-foundational-wire-contract.md
[feat-0001]: ./0001-foundational-slice.md
[feat-0003]: ./0003-server-auth-profile-tasks.md
