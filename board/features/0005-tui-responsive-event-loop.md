---
id: 0005
title: TUI — responsive (non-blocking) event loop + tui::app submodule reorganization
status: awaiting-merge   # inbox → planned → ready → working → review → awaiting-merge → merged | blocked
priority: high      # high | medium | low
parent: null
depends-on: [0004]
branch: feature/0005-tui-responsive-event-loop
worktree: .claude/worktrees/0005-tui-responsive-event-loop
created: 2026-06-22
updated: 2026-06-22
---

## Feature request

**Goal:** The `organized-koala` TUI stays responsive while a server request is outstanding —
it keeps rendering, shows a loading/spinner indicator, and accepts cancel and quit — instead
of freezing inside the blocking `reqwest` call as it does today. Folded in: reorganize
`tui::app` into per-feature submodules (`auth`, `task_add`, `task_list`) with `mod.rs` reduced
to wiring/infrastructure.

**Why:** Human feedback on 0004: *"UI responsiveness is critical — the current TUI freezes
during every HTTP request."* The 0004 loop (`terminal::run` in `crates/tui/src/terminal/mod.rs`)
blocks the UI thread inside `reqwest` for the whole request, so there is no redraw, spinner, or
cancel. The reorganization is folded in because it restructures the same `tui::app` module the
loop change touches, so doing both at once avoids two passes over the same code.

**Acceptance criteria:**

- [ ] While a server request is outstanding, the UI **stays responsive**: it continues to
      render, shows a loading/spinner indicator, and accepts a cancel affordance (Esc) and quit
      (Ctrl+C/q) — the UI thread never blocks on IO.
- [ ] The concurrency model follows [ADR-0006][adr-0006]: synchronous `Client` on a worker
      thread, `mpsc` request/response protocol, polled render loop. **No** `tokio`/async
      `Client`; no new wire shape; `contract` unchanged.
- [ ] `App::handle_event` is pure (`Event → Option<ClientRequest>`) and `App::apply_response`
      is pure; the `App` core no longer owns or calls a `Client` (the worker does). Error-code
      branching (`unauthenticated` → login, `validation_failed` → inline, offline → blocking
      retry) is **unchanged** and routes asynchronously-arriving responses through the same path.
- [ ] At most one request in flight; request-triggering events during flight are no-ops;
      `Cancel`/`Quit` stay live; a stale (`RequestId`-mismatch) response after cancel is dropped.
- [ ] **Statelessness (hard-constraint #1) preserved:** the only new state is the transient
      in-flight marker + spinner tick (process-lifetime UI state, never persisted, never cached
      server data); no on-disk or cross-run state; JWT + active profile id remain in memory only.
- [ ] `tui::app` is reorganized into feature submodules `auth`, `task_add`, `task_list`;
      `app/mod.rs` keeps only the `App` struct, the screen enum, the `handle_event`/
      `apply_response` wiring, and shared infrastructure.
- [ ] `tester`'s `ratatui` `TestBackend` suite is still green and **extended** to cover the
      in-flight state (spinner/loading render, in-flight no-op, cancel + stale-response drop,
      `apply_response` error-code branching), per [ADR-0003][adr-0003].
- [ ] `./ok.sh test | lint | fmt --check` green.

**Out of scope:** any `contract`/wire change; an async runtime (`tokio`) — explicitly rejected
in ADR-0006; multiple concurrent in-flight requests; true mid-flight request abort (cancel is
user-perceived via stale-response drop); optimistic UI state; Notes, Pomodoro, profile
switching/creation, task edit/delete; theming polish.

<!-- written by `architect` via the `plan` skill -->
## Plan(s)

### Plan: responsive event loop + app reorg (2026-06-22, architect)

**Approach:** Per [ADR-0006][adr-0006], confine all concurrency to the edge. Split the `App`
update seam into two pure steps — `handle_event(Event) -> Option<ClientRequest>` and
`apply_response(ClientResponse)` — and remove the `Client` from the core (drop the `App<C>`
generic). The effectful shell gains a single worker thread owning the real `HttpClient`,
talking to the UI thread over `mpsc` (`ClientRequest`/`ClientResponse`, each stamped with a
`RequestId`). `terminal::run` becomes a poll loop (`event::poll(tick)` + drain responses +
redraw each tick) so a spinner animates and cancel/quit stay live in flight. Concurrently,
reorganize `tui::app` into `auth`/`task_add`/`task_list` submodules with `mod.rs` as wiring.
The pure core + synchronous `Client` trait keep the ADR-0003 `TestBackend` seam intact.

**ADR:** [ADR-0006][adr-0006] (Accepted; committed to `main` before the worktree is cut). No
`contract` change; no new ADR beyond 0006.

**Slices:**

1. [tui-dev] **Seam split + app reorg.** Refactor `crates/tui/src/app/` into submodules
   `auth/`, `task_add/`, `task_list/` (each `mod.rs` + sibling `tests.rs` where it has internal
   logic, per `rust-standards`); `app/mod.rs` keeps the `App` struct, `Screen`, and the
   `handle_event`/`apply_response` wiring. Make `handle_event` return `Option<ClientRequest>`
   and add pure `apply_response(ClientResponse)`; introduce `ClientRequest`/`ClientResponse`/
   `RequestId` types and the transient in-flight marker on the screen states. Drop the `App<C>`
   generic; the core no longer holds a `Client`. Files: `crates/tui/src/app/**`.
2. [tui-dev] **Edge: worker thread + poll loop.** Add the worker thread owning `HttpClient`,
   the two `mpsc` channels, and the `RequestId` stamping; rewrite `terminal::run` as a poll
   loop (input poll with tick timeout + response drain + per-tick redraw); add a `reqwest`
   client-side timeout to bound abandoned requests; wire `main.rs` to spawn the worker. Add
   spinner/loading rendering + "working… (Esc to cancel)" hint in `ui/`. Stale-response drop by
   `RequestId`. Files: `crates/tui/src/terminal/**`, `crates/tui/src/client/**` (worker +
   timeout only — trait unchanged), `crates/tui/src/ui/**`, `crates/tui/src/main.rs`.
3. [tester] **Extend the `TestBackend` suite.** Adapt existing flows to the
   `handle_event`→`apply_response` two-step (a thin synchronous test executor mapping a
   `ClientRequest` through the `FakeClient` to a `ClientResponse` is acceptable). Add coverage:
   in-flight render (spinner/loading + hint), request-triggering event in flight is a no-op,
   `Cancel` leaves the in-flight state and a stale `RequestId` response is dropped,
   `apply_response` error-code branching (`unauthenticated`/`validation_failed`/offline/other)
   matches pre-split behaviour, and statelessness (every view still derives from a server
   response). Re-point keybinding/render tests at the reorganized modules. Files:
   `crates/tui/tests/**`, module-sibling `tests.rs` files.
4. [verifier] Per [ADR-0003][adr-0003]: live pass over the reqwest client path against
   `./ok.sh up` (shapes, status codes, error contract, profile-scoping, OTel spans — unchanged
   by this item), plus the delegation handshake: confirm the extended `TestBackend` suite
   exists and is green under `./ok.sh test`, quoting results. (Spinner/in-flight/cancel are
   interactive behaviour → `tester`'s suite, not the verifier, per ADR-0003.)

**Assumptions:**

- One request in flight at a time is sufficient at personal scale (matches 0004's
  request-per-action design); multiplexing is out of scope.
- Cancel is **user-perceived**: the UI leaves the in-flight state immediately and drops the
  abandoned response by `RequestId`; the worker is not force-killed. A `reqwest` timeout bounds
  the abandoned request. (ADR-0006 §4.)
- The poll tick interval (spinner cadence / input latency) is `tui-dev`'s call; the tester suite
  pins observable in-flight behaviour, not the exact interval.
- `std::sync::mpsc` vs `crossbeam-channel` is `tui-dev`'s ergonomic call within model (A); both
  satisfy ADR-0006 and add no async runtime.
- The exact in-flight marker shape (`Option<RequestId>` vs a `Pending` sub-state per screen) is
  `tui-dev`'s call; tester pins the rendered/observable behaviour.
- The submodule boundary (`auth`/`task_add`/`task_list`) maps to the existing `AuthState` /
  `AddTaskState` / `TaskListState` handler groups; shared helpers stay in `mod.rs`.

**Risks:**

- The seam split + reorg + worker land together; if the `TestBackend` suite becomes *harder*
  to write (e.g. the two-step executor feels like mocking an internal collaborator), that is the
  ADR-0003 architecture smell — bubble up to `architect`, do not bend the test.
- Dropping the `App<C>` generic touches every test construction site and `ui::draw` signature;
  mechanical but broad — coordinate slices 1–3 so the suite is adapted in lockstep, not retrofit.
- Terminal raw-mode/teardown and real spinner repaint are invisible to `TestBackend` — accepted
  residual risk, covered by `docs/manual-smoke.md` (ADR-0003 §3), not gated here. The poll-loop
  redraw path is a new trigger candidate for that checklist.
- A late stale response arriving after the screen has moved on must be dropped, not applied — a
  correctness risk explicitly covered by a tester case (`RequestId` mismatch drop).

<!-- append-only; dated, AUTHORED entries. Human feedback = an UNCHECKED box: [ ] unhandled, [x] addressed + re-reviewed (the ONLY re-entry signal). -->
## Log / comments

- 2026-06-22 [architect] item created from 0004's re-homed responsiveness feedback via the
  `plan` skill; scope folds the responsive event loop (ADR-0006) with the requested `tui::app`
  submodule reorg (both restructure `tui::app`). Governed by [ADR-0006][adr-0006] with ADR-0003
  verification routing.
- 2026-06-22 [orchestrator] human accepted **Model A** (worker thread + `mpsc`, no async
  runtime); ADR-0006 moved to **Accepted** and committed to `main` with the decisions-index row
  and this item; `planned` → `ready`. **Claim gate:** depends-on 0004 — the 0005 worktree must
  be cut from a `main` commit that contains the 0004 TUI code this plan refactors, so 0005 is
  not claimable until 0004 has merged to `main`.
- 2026-06-22 [orchestrator] claimed: 0004 confirmed merged to `main` (TUI code + ADR-0006
  present at base `f0204fd`); worktree `.claude/worktrees/0005-tui-responsive-event-loop`
  cut from `f0204fd`, branch `feature/0005-tui-responsive-event-loop`; `ready` → `working`.
  Session `ea39f44a`. Branch copy is now authoritative.
- 2026-06-22 [tui-dev] slice 1 — seam split + app reorg. Split `app/` into submodules
  `auth`/`task_add`/`task_list` (each owning its screen state); `app/mod.rs` keeps `App`,
  `Screen`, `Session`, `Event`, and the `handle_event`/`apply_response` wiring. Added pure
  protocol types (`ClientRequest`/`ClientResponse`/`Outcome`/`RequestId`/`Dispatch`) in
  `app/protocol.rs`. `handle_event` is now pure `Event -> Option<Dispatch>` and `apply_response`
  folds a `ClientResponse` into state (chaining post-auth profile→task load and post-create
  refresh as follow-up dispatches). Dropped the `App<C>` generic — the core holds no client.
  In-flight marker is `pending: Option<RequestId>` per screen state; error-code branching
  preserved unchanged.
- 2026-06-22 [tui-dev] slice 2 — edge: worker thread + poll loop. Added `client/worker.rs`: a
  single thread owning the real `HttpClient`, mapping `ClientRequest`→`Outcome` over two
  `std::sync::mpsc` channels (no new dep, no async). Rewrote `terminal::run` as a poll loop
  (`event::poll(80ms)` for input + `try_recv` drain of responses + per-tick redraw) so the UI
  never blocks on IO; the spinner animates and `Esc` (cancel) / `Ctrl+C`,`q` (quit) stay live in
  flight. At most one request in flight (enforced in the pure core); a stale `RequestId`-mismatch
  response is dropped in `apply_response`. `map_key` now maps `Esc` to `Cancel` while pending.
  Added a 30s `reqwest` client-side timeout to bound abandoned requests (trait unchanged). Added
  spinner glyph + "working… (Esc to cancel)" hint rendering in `ui/`; `ui::draw` takes a tick.
  Wired `main.rs` to spawn the worker and pass the channels to `terminal::run`. `./ok.sh build`
  green; lint/test failures remaining are solely in `crates/tui/tests/**` (old `App<C>` API) —
  slice 3 (tester) adapts them.
- 2026-06-22 [tester] slice 3 — adapted + extended the `TestBackend` suite to the two-step
  seam. Added a synchronous request executor in `tests/common/mod.rs` (`execute`/`drive`/
  `submit`) — the test-side analogue of the worker thread: it maps a `Dispatch`'s
  `ClientRequest` through the `FakeClient` (the sole external-service mock) to a
  `ClientResponse` and feeds it back into `apply_response`, looping on chained follow-ups until
  the flow settles. No internal collaborator is mocked. Dropped all `App<C>`/`App::new(client)`;
  the core is built clientless and `ui::draw` now takes a tick (`render`/`render_at` helpers).
  **Adapted:** `flows.rs` (auth/register/add-task/mark-done/statelessness), `error_branches.rs`
  (`unauthenticated`→login, `validation_failed`/`invalid_credentials`→inline, offline→blocking
  retry, coded-less→generic), `rendering.rs`, `keybindings.rs` (pending screen builders).
  **Added:** in-flight render (spinner glyph + "working… (Esc to cancel)" hint on auth/task-list/
  offline; `spinner_glyph_advances_with_the_tick`), in-flight no-op (`refresh`/`submit` while
  pending dispatch nothing — `in_flight.rs`), `Cancel` clears in-flight + restores
  interactivity, stale/superseded `RequestId`-mismatch response dropped by `apply_response`,
  at-most-one-chained-request, `other_api_error_after_auth`, and Esc→Cancel / Ctrl+C→Quit
  keybindings while pending. `./ok.sh test` green (tui: error_branches 10, flows 9, in_flight 5,
  keybindings 13, rendering 11); `./ok.sh lint` clean; `./ok.sh fmt --check` clean.
- 2026-06-22 [reviewer] cold review of `f0204fd..HEAD` (code commits 804849a/2630d03/a4f99fd).
  Gates green (test/lint/fmt). Confirmed: `handle_event`/`apply_response` pure, `App<C>` generic
  gone; one-in-flight invariant holds (pending short-circuit + same-screen re-dispatch on chains);
  stale/superseded `RequestId`-mismatch drop correct (`in_flight.rs:118,157`); error-code branching
  preserved through the shared handlers; `contract` diff empty; no tokio/async (reqwest::blocking +
  std::thread + std::mpsc only); 30s timeout bounds abandoned requests; worker teardown clean; no
  secret-leak path; tests are public-API with only the `Client` trait mocked. One non-blocking,
  **pre-existing** nit (stale doc comment `main.rs:4` about an initial health probe — already stale
  at base `f0204fd`, out of scope). **REVIEW-STATUS: approved** — pinned to code-hash
  `bc89672d4be5cdecd0bb54b340a24a5b8741cf21` (last code commit `a4f99fd`; reported by reviewer,
  committed here by orchestrator on-branch).
- 2026-06-22 [verifier] live verify against `./ok.sh up` (docker 29.5.3 + compose; full stack:
  postgres → migrate one-shot → server → otel-collector). Confirmed `crates/server`+`crates/contract`
  diff vs `f0204fd` is empty (TUI-only change). **Live:** register/login (`201`/`200`, SessionResponse),
  `GET /api/profiles` (Profile shape), task create/list/close (Task shape, lowercase status, idempotent
  close); error contract `{code,message}` for `unauthenticated`/`invalid_credentials`/`validation_failed`/
  `username_taken`/`not_found` with correct statuses; profile-scoping isolation across two users (no
  cross-profile read/write, `404` no existence leak); OTel server spans observed for every client call.
  ADR-0003 delegation handshake: TestBackend suites present + green (flows 9, error_branches 10,
  in_flight 5, keybindings 13, rendering 11). Gates: test/lint/fmt all exit 0. **Inferred (code-read):**
  that `HttpClient` issues exactly these requests — drove equivalent endpoints over HTTP per the
  standard ADR-0003 verifier split (interactive TUI owned by the green TestBackend suites). Stack torn
  down clean. **VERIFY-STATUS: verified** — pinned to code-hash
  `bc89672d4be5cdecd0bb54b340a24a5b8741cf21` (reported by verifier, committed here by orchestrator).
- 2026-06-22 [orchestrator] step-7 freshen: rebased branch onto `main` (`d911315`; eng-manager's
  shared learnings + regenerated dashboard). Only conflict was this feature-local Board file —
  resolved in favour of the branch (dropped `main`'s frozen-pointer note). `./ok.sh code-hash` at
  the rebased head is unchanged at `bc89672d4be5cdecd0bb54b340a24a5b8741cf21` (= OLD_HASH), so the
  code is byte-identical and the `approved`/`verified` verdicts **carry forward untouched** — no
  relabelling. Re-ran gates on the rebased tree: test/lint/fmt all green. `working`→`awaiting-merge`.

<!-- written at end of cycle; what the human reviews -->
## Summary

The `organized-koala` TUI no longer freezes while a server request is outstanding. It keeps
rendering, animates a spinner with a "working… (Esc to cancel)" hint, and stays interactive
(Esc cancels, Ctrl+C/`q` quit) for the whole in-flight window. Folded in: `tui::app` is
reorganized into per-feature submodules. **TUI-only change — `crates/contract` and
`crates/server` are byte-identical to base `f0204fd` (no wire change, no ADR beyond 0006).**

**Design — [ADR-0006][adr-0006] Model A (synchronous client on a worker thread; no async).**
All concurrency is confined to the edge. The `App` core is now **client-free** and built from
two pure seams: `handle_event(Event) -> Option<Dispatch>` (a state machine over a
transport-agnostic `Event`, doing no IO) and `apply_response(ClientResponse) -> Option<Dispatch>`
(folds an arriving response into state, chaining follow-ups — post-auth profile→task load,
post-create refresh — as further dispatches). The `App<C>` generic is gone; the core holds no
`Client`. The effectful shell is a single worker thread (`client/worker.rs`) owning the real
`HttpClient`, mapping a `ClientRequest` to an `Outcome` over two `std::sync::mpsc` channels
(no new dependency, no `tokio`/async). `terminal::run` is now a poll loop: `event::poll(80ms)`
for input + `try_recv` drain of responses + a per-tick redraw, so the UI thread never blocks on
IO. A 30s `reqwest` client-side timeout bounds an abandoned request (the `Client` trait is
unchanged). `main.rs` spawns the worker and hands the channels to `terminal::run`.

**One-in-flight invariant + cancel.** Each screen state carries a transient `pending:
Option<RequestId>`. While set, request-triggering events are no-ops (the pure core
short-circuits); `Cancel`/`Quit` stay live. Cancel is **user-perceived**: the screen leaves
the in-flight state immediately and a later, now-superseded response is dropped by
`RequestId`-mismatch in `apply_response` — the worker is not force-killed. Error-code branching
(`unauthenticated`→login, `validation_failed`/`invalid_credentials`→inline, offline→blocking
retry, coded-less→generic) is preserved unchanged and routes asynchronously-arriving responses
through the same handlers.

**Module layout.** `crates/tui/src/app/` is now: `mod.rs` (the `App` struct, `Screen`,
`Session`, `Event`, and the `handle_event`/`apply_response` wiring + shared infrastructure);
`protocol.rs` (the pure `ClientRequest`/`ClientResponse`/`Outcome`/`RequestId`/`Dispatch`
types); and feature submodules `auth.rs`, `task_add.rs`, `task_list.rs` (each owning its screen
state and handlers).

**Test executor pattern.** `tester` added a synchronous request executor to
`tests/common/mod.rs` (`execute`/`drive`/`submit`) — the test-side analogue of the worker
thread: it maps a `Dispatch`'s `ClientRequest` through the `FakeClient` (the sole
external-service mock) to a `ClientResponse` and feeds it back into `apply_response`, looping on
chained follow-ups until the flow settles. This exercises the pure two-step seam end-to-end
with no async and no internal collaborator mocked. The `TestBackend` suite (ADR-0003 layer 2)
is green and extended: in-flight render (spinner glyph + hint, `spinner_glyph_advances_with_the_tick`),
in-flight no-op, `Cancel` clears in-flight + restores interactivity, stale/superseded
`RequestId`-drop, at-most-one-chained-request, and Esc→Cancel / Ctrl+C→Quit while pending —
tui totals: flows 9, error_branches 10, in_flight 5, keybindings 13, rendering 11.

**Verified vs. inferred.** Reviewer **approved** and verifier **verified**, both pinned to
code-hash `bc89672d4be5cdecd0bb54b340a24a5b8741cf21`. Gates green: `./ok.sh test`/`lint`/`fmt
--check` all exit 0. The verifier confirmed (per ADR-0003) `crates/server`+`crates/contract`
are unchanged vs base and ran the live reqwest client path against `./ok.sh up` (register/login,
profiles, task create/list/close, the `{code,message}` error contract with correct statuses,
two-user profile-scoping isolation with no existence leak, OTel server spans), and confirmed the
extended `TestBackend` suites green. **Inferred (code-read):** that `HttpClient` issues exactly
those requests — the standard ADR-0003 split, since interactive TUI behaviour (spinner,
in-flight no-op, cancel, error-code branching) is owned by the green `TestBackend` suite, not
the verifier. Spinner repaint and terminal raw-mode teardown remain invisible to `TestBackend`
(accepted residual risk, covered by `docs/manual-smoke.md`, not gated here).

**Hard constraints.** **#1 (statelessness) holds:** the only new state is the transient
in-flight marker + spinner tick — process-lifetime UI state, never persisted, never cached
server data; JWT + active profile id remain in memory only; every view still derives from a
server response. **#2 (contract is the single source of truth) holds:** `crates/contract` is
unchanged; the TUI defines no wire types. The new protocol types (`ClientRequest`/
`ClientResponse`/`Dispatch`/`RequestId`) are **internal TUI control messages**, not wire shapes
— they cross the in-process `mpsc` boundary, never the network.

[adr-0003]: ../../docs/adr/0003-verification-layering.md
[adr-0006]: ../../docs/adr/0006-tui-concurrency-and-responsiveness.md
