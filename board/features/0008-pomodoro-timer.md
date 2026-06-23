---
id: 0008
title: Pomodoro focus timer — global duration config + start/stop session
type: feature      # feature | chore
status: merged          # inbox → planned → ready → working → review → awaiting-merge → merged | blocked
priority: medium    # high | medium | low
parent: null
depends-on: []      # ADR-0002 (timer authority) is on `main`; no in-flight Board item gates this
branch: feature/0008-pomodoro-timer
worktree: .claude/worktrees/0008-pomodoro-timer
created: 2026-06-23
updated: 2026-06-23
---

## Feature request

**Goal:** Implement the Pomodoro focus timer end-to-end, the first feature of the **Focus**
phase. A user can set the session duration, start a focus session, see a live countdown in the
TUI, and stop it. Authority and the rendering model are settled in
[ADR-0002][adr-0002] — this card implements that decision; it does not reopen it.

**Shape (deliberately flat — [ADR-0001][adr-0001] decision 3, hard-constraint #3):**

- **Config is global to the app, duration is the only knob** (default 30 minutes). Per
  [ADR-0002][adr-0002] this is account-global, **not** profile-scoped (#4 namespaces TODOs and
  Notes only).
- **No pause; stop resets.** There is no paused state — stopping clears the active session.
- **Server owns the timer; the TUI renders it** ([ADR-0002][adr-0002]). The server is the sole
  authority for both the duration config and the active session, and both persist in Postgres.
- **A session is an absolute end-instant, not a pushed counter.** The session response carries
  `ends_at` plus the server's current instant; the TUI computes `remaining` once and ticks it
  down locally on its render loop — **no** per-second polling, **no** server tick stream (stays
  inside [ADR-0006][adr-0006]). Completion (`now ≥ ends_at`) is decided by the server.

**Surface to build (final shapes pinned in the `architect` plan under [ADR-0002][adr-0002]):**

- `contract` — a new timer module: DTOs for the global config and the session state (the
  session DTO carries `ends_at` + a server-instant field), reusing the `{ code, message }` error
  contract.
- `server` — endpoints to read/update the global duration config, read the current session
  (idle / running-with-`ends_at` / completed), start a session, and stop a session; a reversible
  migration (up/down) for the config + session tables.
- `tui` — a focus/timer view that reads the config and session, renders the live countdown per
  the model above, and offers start / stop and duration adjustment. Stateless (#1): every view
  derives from a server response.

**Acceptance criteria:**

- [ ] A user can set the global session duration and have it persist across server restarts.
- [ ] Start → the TUI shows a live countdown derived from the server's `ends_at` + server-now;
      no per-second polling and no UI freeze (consistent with [ADR-0006][adr-0006]).
- [ ] Stop resets the session (no paused state); a session that reaches `ends_at` is reported
      completed by the server.
- [ ] The timer is account-global, **not** profile-scoped — switching profiles does not change
      the active session or duration ([ADR-0002][adr-0002], #4).
- [ ] Full `feature` Definition of done: `./ok.sh test | lint | fmt --check` green; `reviewer`
      approved (pinned to `./ok.sh code-hash`); live `verifier` pass exercising the server API +
      reqwest path (shapes, status codes, error contract, OTel spans); the `tui`-touching change
      is covered by the `ratatui` `TestBackend` suite ([ADR-0003][adr-0003]).
- [ ] The `contract` change is governed by [ADR-0002][adr-0002] (already accepted); any wire
      detail beyond what that ADR fixes is recorded in the plan.

**Out of scope (each would need a new ADR — #3 flatness):** pause/resume; per-profile or
multiple concurrent timers; break/long-break cycles or session history; notifications/sound;
any timer config knob other than duration.

<!-- feature: needs an `architect` plan (`plan` skill) writing a `## Plan(s)` block before code. -->

<!-- ─────────────────────────────  ARCHITECT PLAN  ───────────────────────────── -->
## Plan(s)

Planned by `architect` under [ADR-0002][adr-0002] (timer authority — accepted on `main`). This
plan **implements** that ADR and pins the exact DTO field names, endpoint paths, and table
shapes it left to the feature plan (ADR-0002 §6). **No new or amended ADR is required:** every
wire/domain decision below is already authorized by ADR-0002 (account-global config + session,
`ends_at` + server-now render model, persisted in Postgres, no pause / stop-resets). The TUI
loop stays inside [ADR-0006][adr-0006] (worker-thread + polled render loop, no per-second
polling, no tick stream). The error contract is reused verbatim (`{ code?, message }`).

### Design summary

- The timer is **account-global**, keyed on the authenticated `user_id` (the `AuthUser`
  extractor), **never** on `profile_id` — it is not under `/api/profiles/{id}/…`. This is the
  concrete realization of #4 (profiles namespace TODOs/Notes only) and ADR-0002 §5.
- **Config row**: at most one per user (one duration knob, default 30 min). Lazily
  upserted/defaulted so a user who never set it reads the default.
- **Session row**: at most one *active* session per user (ADR-0002 §5). A session is
  `started_at` + the `duration_minutes` snapshot taken at start → server derives `ends_at`.
  Stop deletes/clears the active session (no paused state). The server decides completion
  (`now ≥ ends_at`).
- **The session read carries `ends_at` + `server_now`** so the TUI computes `remaining` once and
  ticks locally (ADR-0002 §2–3). The TUI holds the displayed countdown as transient
  process-lifetime render state — the same #1-compatible category as the in-flight spinner
  marker (ADR-0006 §5), **not** persisted, **not** authoritative.

### `contract` wire shapes — new `timer` module (`crates/contract/src/timer/mod.rs`)

All `DateTime<Utc>` fields serialize RFC 3339 `Z` exactly as `Task::created_at` does. New public
items re-exported from `crates/contract/src/lib.rs`. Owner: **`contract-owner`**.

```rust
/// Global Pomodoro config (account-global; ADR-0002 §5). The only knob is duration.
pub struct TimerConfig {
    pub duration_minutes: u32,   // default 30; > 0 (server-enforced)
}

/// Request body for updating the global config. Duration is the only adjustable parameter (#3).
pub struct UpdateTimerConfigRequest {
    pub duration_minutes: u32,   // must be >= 1 and <= a sane cap (e.g. 1440); else validation_failed
}

/// The current focus session state (ADR-0002 §2–3). Tagged enum on `state`.
///   { "state": "idle" }
///   { "state": "running", "started_at": <rfc3339>, "ends_at": <rfc3339>,
///     "duration_minutes": <u32>, "server_now": <rfc3339> }
///   { "state": "completed", "started_at": <rfc3339>, "ends_at": <rfc3339>,
///     "duration_minutes": <u32>, "server_now": <rfc3339> }
/// `server_now` neutralizes client clock skew (ADR-0002 §3); `running` vs `completed` is the
/// server's verdict (`server_now >= ends_at`).
#[serde(tag = "state", rename_all = "lowercase")]
pub enum TimerSession {
    Idle,
    Running  { started_at, ends_at, duration_minutes, server_now },  // all carried on the wire
    Completed{ started_at, ends_at, duration_minutes, server_now },
}
```

- **Read config** → `TimerConfig`. **Update config** → `UpdateTimerConfigRequest` → `TimerConfig`.
- **Read session** → `TimerSession`. **Start** → (no body) → `TimerSession::Running`.
  **Stop** → (no body) → `TimerSession::Idle`.
- `duration_minutes` is `u32` (matches existing `clippy::as_conversions` discipline — store as
  `INT` in PG, map without lossy `as`; use `i32`↔`u32` `try_from` at the DB boundary, validated
  `>= 1`).
- Every new public type derives `Debug, Clone, PartialEq, Eq, Serialize, Deserialize`, carries
  rustdoc + a serialization doctest (the `contract` public-API + doctest layout, learned 0002).
  No secrets — nothing wraps `Password`/`Secret`.

**Assumption A1 (recorded below): the session DTO is a `#[serde(tag = "state")]` enum** rather
than three flat nullable fields, because it makes the idle/running/completed trichotomy
illegal-states-unrepresentable and mirrors the existing `TaskStatus` lowercase-string idiom.

### `server` — endpoints, handler module, migration

Owner: **`server-dev`** (handlers, error mapping, SQL); migration files in
`crates/server/migrations/`. New handler module `crates/server/src/handlers/timer.rs`, declared
in `crates/server/src/handlers/mod.rs`, wired in `crates/server/src/app.rs`. All routes take the
`AuthUser` extractor and key on `user_id` — **no `profile_id` in any path** (account-global).

Routes (mirrors the existing `get(...).post(...)` table style in `app.rs`):

| Method + path | Handler | Success | Notes |
| --- | --- | --- | --- |
| `GET  /api/timer/config`  | `get_config`    | `200 TimerConfig` | defaults to 30 if no row |
| `PUT  /api/timer/config`  | `update_config` | `200 TimerConfig` | upsert; `duration_minutes < 1` or over cap → `400 validation_failed` |
| `GET  /api/timer/session` | `get_session`   | `200 TimerSession` | idle / running / completed computed from `now` |
| `POST /api/timer/session/start` | `start_session` | `200 TimerSession::Running` | snapshots current `duration_minutes`; starting while one is active **replaces** it (single active session, ADR-0002 §5) |
| `POST /api/timer/session/stop`  | `stop_session`  | `200 TimerSession::Idle` | clears the active session (no pause); idempotent when already idle |

- `server_now` in every session response is the handler's `Utc::now()` (skew-neutralizing,
  ADR-0002 §3). `ends_at = started_at + duration_minutes`.
- Completion is read-time: `get_session` returns `Completed` when `now >= ends_at` for the
  active row (the row is not deleted on completion — stop is the only clear; this lets a
  reconnecting TUI still see the completed verdict per ADR-0002 §4).
- Reuse `ApiError`/`ApiResult` and the existing error contract; add **no** new `ErrorCode`
  (validation reuses `ValidationFailed`; absence of a session is the `Idle` variant, **not** a
  404). `#[tracing::instrument(skip_all, fields(user_id = %user.user_id))]` on each handler for
  OTel spans (DoD clause 4 checks spans).

**Migration** `crates/server/migrations/<ts>_timer.up.sql` / `.down.sql` (reversible — a missing
`down` is review-blocking). One timestamp after the existing `…163047_tasks`:

```sql
-- up: account-global timer config + at-most-one active session, keyed on the user (ADR-0002 §5).
CREATE TABLE timer_configs (
    user_id          UUID PRIMARY KEY REFERENCES users (id) ON DELETE CASCADE,
    duration_minutes INT  NOT NULL DEFAULT 30 CHECK (duration_minutes >= 1),
    updated_at       TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE TABLE timer_sessions (
    user_id          UUID PRIMARY KEY REFERENCES users (id) ON DELETE CASCADE,  -- one active per user
    started_at       TIMESTAMPTZ NOT NULL,
    duration_minutes INT NOT NULL CHECK (duration_minutes >= 1)                 -- snapshot at start
);
-- down: DROP TABLE timer_sessions; DROP TABLE timer_configs;
```

- `user_id` PRIMARY KEY enforces "at most one config / one active session per account" in the
  schema (ADR-0002 §5) — no app-level race. `ends_at` is **derived** (`started_at +
  duration_minutes`), not stored, so the absolute end-instant has a single source of truth.
- After authoring SQL queries, refresh the offline cache via `./ok.sh prepare` (sqlx offline
  mode is committed `.sqlx/`; queries won't compile in CI otherwise).

### `tui` — focus/timer view (ADR-0006 render model)

Owner: **`tui-dev`**. New screen state `crates/tui/src/app/timer.rs` (declared in
`crates/tui/src/app/mod.rs`), new draw fn in `crates/tui/src/ui/mod.rs`, protocol additions in
`crates/tui/src/app/protocol.rs`, client-trait additions in `crates/tui/src/client/mod.rs`, and
the worker match arm in `crates/tui/src/client/worker.rs`.

1. **`Client` trait** gains five methods mirroring the endpoints:
   `get_timer_config(token)`, `update_timer_config(token, &req)`, `get_timer_session(token)`,
   `start_timer_session(token)`, `stop_timer_session(token)` — each `ClientResult<…>` over the
   new DTOs. `HttpClient` implements them following the exact `bearer_auth` + status-branch +
   `decode`/`api_error` pattern already in `client/mod.rs`. The `FakeClient` in
   `crates/tui/tests/common/mod.rs` gains scripted impls (owner: **`tester`**, the sanctioned
   external-service mock).
2. **Protocol**: new `ClientRequest` variants (`GetTimerConfig`, `UpdateTimerConfig`,
   `GetTimerSession`, `StartTimerSession`, `StopTimerSession`) each carrying `token`, matching
   `Outcome` variants, and worker `run` arms — exactly the shape of the existing task arms.
3. **Screen**: add `Screen::Timer(TimerState)`. `TimerState` holds the last server-returned
   `TimerConfig` + `TimerSession`, the in-flight `pending: Option<RequestId>` marker, an optional
   inline `message`, and an optional duration-edit sub-flow buffer (same category as
   `AddTaskState`). **No countdown integer is stored authoritatively** — the live `remaining` is
   computed each draw from `ends_at − (server_now + elapsed_since_response)`, where
   `elapsed_since_response` comes from a monotonic `Instant` captured when the response was
   applied (ADR-0002 §3: render, not state; #1-safe).
4. **Render** (`draw_timer`): show the current duration, the session state, and — when running —
   the live `MM:SS` countdown recomputed every tick from the absolute `ends_at`; the existing
   poll loop already redraws each ~80 ms tick (`terminal::run`), so the countdown animates with
   **no per-second polling and no tick stream** (ADR-0002 §3, ADR-0006). On reaching `ends_at`
   locally, the view shows "completed"; the **server's** `Completed` verdict is authoritative and
   arrives on the next coarse refresh.
5. **Coarse refresh**: the timer view re-`GetTimerSession` on entry, on user action
   (start/stop/set-duration), and on a **coarse cadence** (Assumption A3) — never per second.
6. **Navigation** (Assumption A2): add a key on the task-list screen to open the timer view and a
   key to return, plus `map_key` arms in `terminal/mod.rs`. Switching profiles must **not** change
   the timer (account-global) — verified by `tester`/`verifier`.
7. **Error routing unchanged** (ADR-0006 §6): `unauthenticated` → login, offline → blocking
   screen, other `Api` codes → inline message, all via the existing `apply_response` branching
   pattern.

### Task breakdown (dependency order)

| # | Slice | Agent | Owns / touches |
| --- | --- | --- | --- |
| 1 | `contract` timer module: `TimerConfig`, `UpdateTimerConfigRequest`, `TimerSession`; re-exports; rustdoc + serialization doctests | `contract-owner` | `crates/contract/src/timer/mod.rs`, `crates/contract/src/lib.rs` |
| 1t | `contract` public-API tests for the new DTOs (round-trip, tag-enum, `Z` offsets) | `tester` | `crates/contract/tests/timer.rs` |
| 2 | Migration (up/down) + `timer.rs` handlers + route wiring + error mapping; `./ok.sh prepare` to refresh `.sqlx/` | `server-dev` | `crates/server/migrations/<ts>_timer.{up,down}.sql`, `crates/server/src/handlers/timer.rs`, `…/handlers/mod.rs`, `…/app.rs` |
| 2t | Server integration tests: config default+persist, start→running (`ends_at`/`server_now`), completion at `ends_at`, stop→idle, account-global (two profiles, same session), auth required | `tester` | `crates/server/tests/timer.rs`, `crates/server/tests/common/mod.rs` |
| 3 | TUI: `Client` trait + `HttpClient` methods; protocol variants + `Outcome` + worker arms | `tui-dev` | `crates/tui/src/client/mod.rs`, `…/client/worker.rs`, `…/app/protocol.rs` |
| 4 | TUI: `Screen::Timer` + `TimerState` (monotonic-render countdown), `draw_timer`, navigation + `map_key`, `apply_response` arms | `tui-dev` | `crates/tui/src/app/timer.rs`, `…/app/mod.rs`, `…/ui/mod.rs`, `…/terminal/mod.rs` |
| 4t | TUI `TestBackend`/core tests: start shows running countdown, stop→idle, set-duration, completed render, in-flight spinner, cancel/stale-id drop, profile-switch leaves timer unchanged; `FakeClient` timer impls | `tester` | `crates/tui/tests/timer.rs`, `crates/tui/tests/common/mod.rs` |

Dependency edges: **1 → 2 → 3 → 4** (each depends on the contract/protocol below it); tests
(`Nt`) land alongside their slice (tracer-bullet: thin end-to-end slice first, then widen —
`coding-standards`). `1` must merge into the working branch before `2`/`3` compile.

### Assumptions (human is AFK — smallest change satisfying acceptance)

- **A1 — `TimerSession` is a `#[serde(tag = "state")]` enum** (`idle`/`running`/`completed`),
  not flat nullable fields. Makes the trichotomy illegal-states-unrepresentable and mirrors
  `TaskStatus`. Smallest correct shape carrying ADR-0002 §2–3's `ends_at` + `server_now`.
- **A2 — Navigation: a single key toggles the task-list ↔ timer view** (e.g. `t` to open the
  timer, `Esc`/a back key to return), reusing the existing `map_key`/`Screen` pattern. The
  acceptance criteria require "a focus/timer view" but do not specify how it is reached; this is
  the smallest addition. If a full nav model is wanted later, that is a separate item.
- **A3 — Coarse session refresh cadence ≈ every 5 s** while the timer view is open (plus on
  entry and on every user action), well above the ~80 ms render tick and far from per-second
  polling — satisfies "no per-second polling" (ADR-0002 §3, ADR-0006) while keeping the
  server's running/completed verdict reasonably fresh. `tui-dev` picks the exact constant.
- **A4 — Duration validation bounds: `1 ≤ duration_minutes ≤ 1440`** (1 min .. 24 h),
  `400 validation_failed` outside. ADR-0002 fixes the default (30) and the single knob; the
  bound is the smallest sane guard and adds no new `ErrorCode`.
- **A5 — Start while a session is active replaces it** (re-derives `started_at`/`ends_at` from
  current config); "at most one active session" (ADR-0002 §5) read as upsert-on-start. No new
  state, no pause.
- **A6 — A completed session row is left in place until stop** (not auto-deleted), so a
  reconnecting/second TUI still reads the `Completed` verdict (ADR-0002 §4 persistence). `stop`
  is the only clear.

### Risks

- **Clock skew / sub-second drift** — mitigated by ADR-0002 §3 (`server_now` + monotonic local
  delta); drift is cosmetic and corrected on the next coarse refresh. The server is always the
  completion authority. (ADR-0002 Consequences already accepts this.)
- **`as`-conversion lint on `u32`↔`i32` at the DB boundary** — use `i32::try_from`/`u32::try_from`
  with explicit error handling, never `as` (`clippy::as_conversions` is denied). The `CHECK
  (duration_minutes >= 1)` plus the validated `INT` keeps values well within `i32`.
- **sqlx offline cache staleness** — new queries require `./ok.sh prepare`; an un-refreshed
  `.sqlx/` fails the offline build. Server-dev runs it as part of slice 2.
- **#1 leak risk in the TUI** — the live countdown must be *recomputed from a server response +
  monotonic clock*, never stored as authoritative remaining-seconds or persisted. Reviewer guards
  this exactly as it guards the in-memory session/spinner today (ADR-0006 §5).
- **Account-global regression** — the highest-value test is "switch profile, timer/session
  unchanged"; both `tester` (TUI + server) and the live `verifier` must exercise it (acceptance
  criterion + #4 boundary).

### Definition of done (feature track — all 7 clauses)

1. `./ok.sh test` green — public-API coverage: `contract` round-trips, server integration
   (config persist, start/running/completion/stop, account-global, auth-required), TUI
   `TestBackend` flows (countdown render, start/stop/set-duration, completed, spinner, cancel,
   profile-switch-unchanged). Mocks only the sanctioned `Client` trait.
2. `./ok.sh lint` clean — no unjustified `#[allow]`; no `as`-conversions at the DB boundary.
3. `./ok.sh fmt --check` clean.
4. **Live `verifier`**: boot the stack, exercise the five timer endpoints + the reqwest client
   path — shapes, status codes, the `{ code?, message }` error contract on bad duration,
   account-global (same session across two profiles), persistence across a server restart, and
   the OTel spans on the timer handlers. TUI interactive behaviour is `tester`'s `TestBackend`
   suite (ADR-0003); the verifier confirms it exists and is green.
5. The `contract` change is governed by **[ADR-0002][adr-0002]** (accepted) — this plan pins the
   shapes under it; **no new/amended ADR needed**. Any new gotcha is recorded in `CLAUDE.md`.
6. `reviewer` posts `REVIEW-STATUS: approved` pinned to `./ok.sh code-hash`.
7. Branch rebased current on `main` (step-7 freshen; verdict pins to the code-tree hash).

### Re-entry plan — 0008-R1 (feedback 2026-06-23, governed by [ADR-0006][adr-0006] §8 amendment)

Two `[human]` UI-feedback lines (global timer widget + `p` toggle + help-menu entry;
append-spinner instead of caption-replacement + ~1/min cadence) re-enter on the existing branch.
**TUI-only** — no `contract`, no server, no migration, no new wire/protocol shape. Governed by
the ADR-0006 §8 amendment (landed on `main`; branch rebased onto it before this work). The
[ADR-0002][adr-0002] authority/render model is unchanged (server owns the countdown; TUI renders
from `ends_at + server_now`). Reuses the existing 0008 client methods, protocol variants
(`GetTimerConfig`/`GetTimerSession`/`StartTimerSession`/`StopTimerSession`/`UpdateTimerConfig`),
and worker arms verbatim — **none of those change.**

Owner: **`tui-dev`** (source); **`tester`** (TestBackend/core suite). Files touched (all `tui`):
`crates/tui/src/app/mod.rs`, `crates/tui/src/app/timer.rs`, `crates/tui/src/ui/mod.rs`,
`crates/tui/src/terminal/mod.rs`; tests `crates/tui/tests/timer.rs`,
`crates/tui/tests/keybindings.rs`, `crates/tui/tests/common/mod.rs`.

#### Concrete changes

1. **Remove the dedicated timer screen.** Delete the `Screen::Timer(TimerState)` variant and its
   navigation (`t` to open, `Esc` to back, the `OpenTimer` event and its `map_key` arm). The
   per-screen `TimerState` no longer holds *navigation* state.
2. **Promote timer state to an app-level global field (ADR-0006 §8.1).** Move the last
   `TimerConfig` + `TimerSession`, the monotonic `applied_at: Option<Instant>`, the toggle's
   in-flight `Option<RequestId>` marker, and the optional duration-edit sub-flow onto `App` as a
   single field. Still derived-from-server, never persisted, no stored remaining-seconds integer
   (#1; reviewer guards exactly as today). On reaching a logged-in screen, an initial
   config→session load is issued; thereafter the coarse refresh and `p` toggle keep it current.
3. **Render the global corner widget (ADR-0006 §8.1).** `ui::draw` renders the timer widget in
   the **bottom-right** on every post-auth screen (auth excluded), beside the bottom-left hotkey
   caption — a small bottom row split left (caption) / right (timer). Shows `idle` + configured
   duration, the live `MM:SS` countdown when running (reusing the existing pure
   `countdown_label`), or `completed`. The countdown is recomputed each ~80 ms render tick.
4. **Global `p` toggle (ADR-0006 §8.2).** Add `Event::ToggleTimer`; `map_key` maps `p` to it on
   every post-auth screen (not while a text-entry sub-flow owns the keystroke). In the pure core,
   `ToggleTimer` resolves to `StartTimerSession` when idle/completed and `StopTimerSession` when
   running, stamping the app-level in-flight marker. At most one timer request in flight; a `p`
   while the toggle is already pending is a no-op.
5. **Add `p` to the hotkey caption (ADR-0006 §8.2 — the "help menu").** Append a `p: start/stop
   timer` entry to the bottom-left caption on every post-auth screen.
6. **Append-spinner in-flight indicator (ADR-0006 §8.3).** Change the caption draw so an
   outstanding request **appends** a trailing animated spinner glyph to the end of the existing
   caption rather than replacing it with the "working… (Esc to cancel)" string. The "Esc to
   cancel" affordance stays present. Applies to **every** screen's caption — removes the flicker
   class generally, not just for the timer.
7. **Coarse cadence ~1 min (ADR-0006 §8.4).** Raise `TIMER_REFRESH_TICKS` from `63` (~5 s) to
   ~`750` (~1 min) in `crates/tui/src/terminal/mod.rs`; exact constant is `tui-dev`'s to pin.
   `timer_refresh_due` now fires whenever a logged-in screen is shown (the timer is global), not
   only when a dedicated timer screen is open.
8. **Duration editing without a dedicated screen (Assumption B2).** Keep the existing duration
   edit sub-flow, reached by a key from any post-auth screen (e.g. `d`), issuing the existing
   `UpdateTimerConfig`. Smallest change — no new screen, no new wire shape.

#### Acceptance criteria (re-entry)

- [ ] The timer is **always visible in the bottom-right** on every post-auth screen; there is
      **no** dedicated timer page and no `t`/`Esc` navigation to one.
- [ ] **`p`** starts the session when idle/completed and stops it when running, from any
      post-auth screen; `p` is listed in the bottom-left hotkey caption.
- [ ] An in-flight request **appends a trailing spinner** to the hotkey caption — the caption text
      is **not** replaced (no flicker); verified in the `TestBackend` buffer.
- [ ] The coarse session refresh is **~1 minute** (`TIMER_REFRESH_TICKS ≈ 750`), not ~5 s; the
      local countdown still animates each render tick with no per-second polling.
- [ ] Account-global unchanged: no `profile_id` on any timer request; switching profiles does not
      change the timer (#4 / ADR-0002 §5).
- [ ] Full `feature` DoD re-run: `./ok.sh test | lint | fmt --check` green; `reviewer`
      re-approved (pinned to the new `./ok.sh code-hash`); the TUI `TestBackend` suite green
      (ADR-0003); the live `verifier` re-confirms server API + reqwest path unaffected.

#### Assumptions (human is AFK — smallest change satisfying both feedback items)

- **B1** — Both feedback items are co-located in `tui::ui` + the loop, so they re-enter together
  on the existing branch in a single pass, not as two items.
- **B2** — Duration editing keeps the existing edit sub-flow, reached by a key (e.g. `d`) from any
  post-auth screen rather than a dedicated screen.
- **B3** — The widget is rendered on every post-auth screen only (auth excluded — no
  session/token before login).
- **B4** — `p` is suppressed while a text-entry sub-flow owns keystrokes, so a literal `p` typed
  into a field is not hijacked by the global toggle.
- **B5** — The exact `TIMER_REFRESH_TICKS` constant (~750 for ~1 min at ~80 ms/tick) is
  `tui-dev`'s to pin; "~1 minute" is the requirement.

## Summary

The account-global Pomodoro focus timer, end-to-end across all three crates — the first feature
of the **Focus** phase, implementing [ADR-0002][adr-0002] (timer authority) without reopening or
amending it.

**What was built:**

- **`contract`** — a new `timer` module: `TimerConfig { duration_minutes }`,
  `UpdateTimerConfigRequest { duration_minutes }`, and a tagged `TimerSession` enum
  (`#[serde(tag = "state", rename_all = "lowercase")]`) with `Idle` / `Running` / `Completed`,
  the running/completed variants carrying `started_at`, `ends_at`, `duration_minutes`, and
  `server_now`. Datetimes serialize RFC 3339 `Z` exactly as `Task::created_at`; the established
  derive/rustdoc/doctest layout is followed and the three items are re-exported from `lib.rs`.
- **`server`** — five **account-global** routes keyed on `AuthUser.user_id` with **no
  `profile_id` in any path** (#4 / ADR-0002 §5): `GET`/`PUT /api/timer/config` (default 30 lazily,
  upsert, `[1, 1440]` bound → `400 ValidationFailed` outside, reusing the existing `{ code?,
  message }` contract and adding no new `ErrorCode`); `GET /api/timer/session` (idle / running /
  completed, completion decided read-time when `server_now >= ends_at`); `POST
  /api/timer/session/start` (snapshots the configured duration; start-while-active replaces);
  `POST /api/timer/session/stop` (clears the active row, idempotent when idle). A reversible
  paired `20260612163048_timer.{up,down}.sql` migration creates `timer_configs` and
  `timer_sessions`, both `user_id UUID PRIMARY KEY` (schema-enforced at-most-one config / one
  active session per account); `ends_at` is **derived** (`started_at + duration_minutes`), never
  stored. `#[tracing::instrument]` spans on every handler; `i32`↔`u32` at the DB boundary via
  `try_from`, never `as`.
- **`tui`** — the timer is an **always-visible global widget** (the 0008-R1 end state — see
  below). It renders in the bottom-right corner of every post-auth screen, beside the bottom-left
  hotkey caption; there is **no** dedicated timer page. The live `MM:SS` countdown is
  **render-only** (#1-safe): no authoritative remaining-seconds integer is stored; the label is
  recomputed every ~80 ms render tick from `ends_at − (server_now + elapsed_since_response)`, where
  `elapsed_since_response` comes from a monotonic `Instant` captured when the response landed.
  Coarse session re-reads are ~1 min (`TIMER_REFRESH_TICKS = 750`) — never per-second, no tick
  stream (stays inside [ADR-0006][adr-0006]). On reaching `00:00` locally the widget shows
  "completed" until the server's authoritative `Completed` verdict arrives.

**0008-R1 (feedback re-entry, TUI-only — governed by [ADR-0006][adr-0006] §8; authority/render
model still [ADR-0002][adr-0002]):** two `[human]` UI-feedback lines re-entered the
already-`awaiting-merge` item and reshaped only the TUI presentation — **no `contract`, server, or
migration change** (reviewer + verifier independently confirmed that `crates/contract`,
`crates/server`, and the `tui` client/protocol are byte-identical). The dedicated `Screen::Timer`
(and its `t`/`Esc`
navigation) was **removed**; the timer became an **app-level global widget** rendered bottom-right
on every post-auth screen (auth/offline excluded). A global **`p`** key toggles start/stop from any
post-auth screen (new `Event::ToggleTimer`, resolved in the pure core to `StartTimerSession` when
idle/completed and `StopTimerSession` when running, reusing the existing client/protocol/worker
shapes verbatim); `p` and `d: set duration` are listed in the bottom-left hotkey caption (the help
menu), and `p`/`d` are suppressed while a text-entry sub-flow owns keystrokes. The in-flight
indicator now **appends a trailing spinner** (+ "Esc to cancel") to the end of the stable caption
instead of **replacing** it with "working…", removing the flicker class on every screen. The coarse
session cadence loosened ~5 s → ~1 min (`TIMER_REFRESH_TICKS` 63 → 750); the local countdown still
animates each render tick with no per-second polling. The duration edit is preserved as a global
text-entry sub-flow (`d`). Account-global unchanged (no `profile_id` on any timer request).

**Key decisions:** account-global keying on `user_id` (not profile-scoped, the concrete
realization of #4 + ADR-0002 §5); render-only countdown from an absolute server end-instant +
monotonic clock (#1-safe, no stored counter); reversible up/down migration with a derived
`ends_at`; the `{ code?, message }` error contract reused verbatim with no new `ErrorCode`. The
**contract/domain surface** carries **no new or amended ADR** — [ADR-0002][adr-0002] governs it
end-to-end (the plan only pinned the exact DTO field names, endpoint paths, and table shapes under
it). The **TUI presentation** (global widget + `p` toggle + append-spinner + cadence) is governed
by the [ADR-0006][adr-0006] **§8 amendment** added during the 0008-R1 re-entry — a
TUI-structure/in-flight-indicator/poll-cadence decision, **not** a contract/domain one, so it
touched no wire shape.

**Verified outcome (end state, including the 0008-R1 re-entry):** the original live `verifier`
pass (against the booted `./ok.sh up` stack) **directly observed** the running→`completed`
transition at `ends_at` (a 1-min session polled every 5 s flipped when `server_now >= ends_at`; the
row was kept until `stop`), config + running-session **persistence across a `docker compose restart
server`** (state lives in Postgres), the account-global boundary (routes carry no `profile_id`; a
second account is independent), the `{ code?, message }` contract on bad duration, and OTel spans
for all five handlers. The **0008-R1** re-entry re-ran the full feature track on the new code-tree:
its reviewer + verifier each **independently confirmed the wire surface byte-identical**
(`crates/contract` + `crates/server` + the `tui` client/protocol unchanged vs the pre-re-entry
base), the live `./ok.sh up` wire pass was re-performed (config get/update, session
start/stop, error contract, OTel spans on all five handlers), and the ADR-0003 layer-2 `TestBackend`
suite asserts the re-entry behaviour by name (global widget render, `p` start/stop/when-completed,
second-`p`-while-pending no-op, `p` suppressed-while-editing, append-spinner-no-flicker; tui
keybindings 19 / rendering 11 / timer 17 + flows/in_flight/error_branches). Reviewer **approved**
and verifier **verified** at the 0008-R1 end state, both pinned to code-hash
`3fa0adefce8cd6d67ae716dae7a24ce6dbf9defd` (the original 0008 build was approved + verified at
code-hash `708ee8d0085ce9b3af68eb7e1b76dbe56a6185da`, voided when the re-entry moved the tree).

<!-- append-only; dated, AUTHORED entries. Human feedback = an UNCHECKED box: [ ] unhandled, [x] addressed + re-reviewed (the ONLY re-entry signal). -->
## Log / comments

- 2026-06-23 [orchestrator] minted the Pomodoro feature card now that [ADR-0002][adr-0002]
  (timer authority) is accepted on `main`, unblocking the Focus phase. This is the
  `## Feature request` only — as a `feature` it next goes to `architect` (`plan` skill) to write
  the `## Plan(s)` block (task breakdown, agent assignments, file ownership, the concrete
  `contract` wire shape under ADR-0002) before any code. No new ADR is needed — ADR-0002 already
  governs the contract surface; the plan pins the exact DTO/endpoint shapes under it.

- 2026-06-23 [architect] ran the `plan` skill and wrote the `## Plan(s)` block: concrete
  `contract` timer DTOs (`TimerConfig`, `UpdateTimerConfigRequest`, tagged `TimerSession`
  carrying `ends_at` + `server_now`), five account-global `/api/timer/...` endpoints keyed on
  `user_id` (not profile-scoped, #4 / [ADR-0002][adr-0002] §5), a reversible up/down migration
  for `timer_configs` + `timer_sessions`, and the TUI focus view rendering the countdown from
  the absolute end-instant inside [ADR-0006][adr-0006]. Task breakdown is dependency-ordered
  (`contract-owner` → `server-dev` → `tui-dev`, `tester` alongside) with explicit file
  ownership. **No new or amended ADR needed** — [ADR-0002][adr-0002] already governs the whole
  contract/domain surface; the plan only pins the exact shapes under it. Assumptions A1–A6
  recorded (tagged-enum session DTO, single key to reach the view, ~5 s coarse refresh, 1–1440
  min duration bound, start-replaces-active, completed-row-kept-until-stop). Status → `ready`.

- 2026-06-23 [contract-owner] built **slice 1** — the `contract` timer module
  (`crates/contract/src/timer/mod.rs`): `TimerConfig { duration_minutes: u32 }`,
  `UpdateTimerConfigRequest { duration_minutes: u32 }`, and the tagged session enum
  `TimerSession` (`#[serde(tag = "state", rename_all = "lowercase")]`) with `Idle`,
  `Running`, and `Completed { started_at, ends_at, duration_minutes, server_now }` — datetimes
  are `DateTime<Utc>` serializing RFC 3339 `Z` exactly as `Task::created_at`, mirroring the
  `TaskStatus` lowercase-tag idiom and the established derive/rustdoc/doctest layout. Re-exported
  the three public items from `crates/contract/src/lib.rs`. No new `ErrorCode`, no secrets, no
  fields beyond the ADR-0002 / plan shapes (#3 flat). Gates green from the worktree:
  `./ok.sh build`, `./ok.sh lint`, `./ok.sh fmt --check`, `./ok.sh test` (15 contract doctests
  pass, incl. the 3 new). Tests for the DTOs (slice 1t, `crates/contract/tests/timer.rs`) are
  `tester`'s; not written here.

- 2026-06-23 [orchestrator] claimed `ready` → `working`: cut worktree
  `.claude/worktrees/0008-pomodoro-timer` + branch `feature/0008-pomodoro-timer` from `main`
  @ 04926d4 (the commit carrying the plan; ADR-0002 already on `main`, verified present in the
  worktree). Branch copy is now authoritative; `main`'s copy frozen at the claim snapshot.
  Session: drive cycle, build slices next in plan dependency order (1 contract → 2 server →
  3 TUI client/protocol → 4 TUI view), tests alongside.

- 2026-06-23 [tester] built **slice 1t** — `contract` public-API tests for the new timer DTOs
  (`crates/contract/tests/timer.rs`, 19 tests), mirroring the established `task.rs` idiom.
  Covers: JSON round-trip (serialize→deserialize→equal) and exact-shape serialization for
  `TimerConfig` and `UpdateTimerConfigRequest`; the `TimerSession` tagged-enum wire shape —
  `{"state":"idle"}` plus `running`/`completed` carrying all four fields (`started_at`,
  `ends_at`, `duration_minutes`, `server_now`) with round-trip for each variant; RFC 3339 `Z`
  datetime serialization including offset-normalization to UTC (`+01:00` → `Z`) as
  `Task::created_at` is tested; deserialization of a known-good JSON literal per variant
  (wire-compatibility guards); and closure guards (unknown `state` tag rejected, malformed
  datetime rejected). Mocks nothing — pure DTO crate. Gates green from the worktree:
  `./ok.sh test` (19 timer tests + all 15 contract doctests, incl. the 3 new), `./ok.sh lint`,
  `./ok.sh fmt --check`. No source under `crates/*/src/` touched (tests only).

- 2026-06-23 [server-dev] built **slice 2** — the server endpoints, handler module, and
  reversible migration. New migration `20260612163048_timer.{up,down}.sql` (one timestamp after
  `…163047_tasks`): `timer_configs` and `timer_sessions`, both keyed `user_id UUID PRIMARY KEY
  REFERENCES users(id) ON DELETE CASCADE` (schema-enforced at-most-one config / one active
  session per account, ADR-0002 §5), `duration_minutes INT NOT NULL CHECK (>= 1)`; `ends_at` is
  derived (`started_at + duration_minutes`), never stored. The `down` drops both tables (a
  missing `down` is review-blocking). New handler module `crates/server/src/handlers/timer.rs`
  (declared in `handlers/mod.rs`, wired in `app.rs`) with five **account-global** routes keyed on
  `AuthUser.user_id`, **no `profile_id` in any path** (#4 / ADR-0002 §5): `GET /api/timer/config`
  (defaults to 30 lazily, no row written on read); `PUT /api/timer/config` (upsert; outside
  `[1, 1440]` → `400` reusing `ValidationFailed`, no new `ErrorCode`); `GET /api/timer/session`
  (idle/running/completed, completion read-time `server_now >= ends_at`, row kept until stop —
  A6); `POST /api/timer/session/start` (snapshots current `duration_minutes`, upsert so starting
  while active replaces — A5); `POST /api/timer/session/stop` (deletes the active row, idempotent
  when idle). `server_now = Utc::now()`; `ends_at = started_at + Duration::minutes(...)`.
  `#[tracing::instrument(skip_all, fields(user_id = %user.user_id))]` on each handler (OTel spans,
  DoD clause 4). `i32`↔`u32` at the DB boundary via `try_from` with explicit error handling —
  never `as` (`clippy::as_conversions` denied). Refreshed the committed `.sqlx/` cache via
  `./ok.sh prepare` against a **live throwaway test Postgres** (the project's own
  `deploy/docker-compose.test.yml` `postgres:16-alpine`, migrations applied via the sqlx CLI,
  torn down after) — 5 new query files, none orphaned. Gates green from the worktree:
  `./ok.sh build`, `./ok.sh lint`, `./ok.sh fmt --check`, `./ok.sh test` (full suite, 0 failures;
  contract + server-integration + tui suites). Integration tests (`crates/server/tests/timer.rs`,
  slice 2t) are `tester`'s; not written here.

- 2026-06-23 [tester] built **slice 2t** — server HTTP integration tests for the
  account-global timer surface (`crates/server/tests/timer.rs`, 21 `#[sqlx::test]` tests),
  mirroring the existing `tasks.rs`/`auth.rs` in-process `oneshot`-over-per-test-DB harness (no
  new boot path). Added four shared request helpers to `tests/common/mod.rs` (`put_json_auth`,
  `post_auth`, `post`, `put_json`) since the timer surface uses PUT and bodyless POST. Coverage
  by acceptance case: **config** — default 30 with no row (`config_defaults_to_30`), PUT→GET
  persistence round-trip + overwrite (`config_put_then_get_round_trips`, `config_put_overwrites`),
  `0` and `1441` → `400 ValidationFailed` (`config_put_zero_is_400`, `config_put_over_cap_is_400`),
  boundaries `1` and `1440` accepted (`config_put_lower_bound_1_accepted`,
  `config_put_upper_bound_1440_accepted`); **session** — idle from cold (`session_starts_idle`),
  start→running carrying all four fields with `ends_at == started_at + duration` and
  `server_now < ends_at` (`start_returns_running_with_consistent_instants`), duration snapshot
  (`start_snapshots_configured_duration`), running re-read (`get_session_after_start_is_running`),
  stop→idle clears the row (`stop_clears_running_session`), idempotent stop-when-idle
  (`stop_when_idle_is_idempotent`), start-replaces-active A5 (`start_while_active_replaces`);
  **account-global #4/ADR-0002 §5** — `timer_is_account_global_not_profile_scoped` (highest
  value): asserts the routes carry no profile segment, the same token observes the same session,
  and a second account has an independent (default/idle) timer; **auth** — each of the five
  routes without a bearer token → `401 unauthenticated` (`*_requires_auth`). **Partial coverage
  (noted inline in `shortest_session_reads_running_not_completed`):** the read-time `completed`
  verdict (`server_now >= ends_at`, A5/A6) is **not** positively driven here — the min duration
  is 1 minute and the public API exposes no clock/`started_at` control, so forcing
  `now >= ends_at` would need a real ~60 s sleep, which this suite deliberately avoids. The
  reachable assertion is the negative: a just-started 1-minute session reads `running`, not
  `completed`. The positive transition at `ends_at` is for the live `verifier` (DoD clause 4).
  Gates green from the worktree against the sanctioned test Postgres: `./ok.sh test` (21 timer
  integration tests pass, full suite 0 failures), `./ok.sh lint` clean, `./ok.sh fmt --check`
  clean. No source under `crates/*/src/` touched (tests only).

- 2026-06-23 [tui-dev] built **slice 3** — the TUI client/protocol additions for the timer
  surface. Added five `Client` trait methods (`crates/tui/src/client/mod.rs`):
  `get_timer_config`, `update_timer_config`, `get_timer_session`, `start_timer_session`,
  `stop_timer_session`, each `ClientResult<…>` over the new DTOs, with `HttpClient` impls
  following the existing `bearer_auth` + status-branch + `decode`/`api_error` pattern verbatim
  (`PUT /api/timer/config` carries the body; the session start/stop are bodyless POSTs; all are
  account-global, no `profile_id`). Added matching `ClientRequest` variants (`GetTimerConfig`,
  `UpdateTimerConfig`, `GetTimerSession`, `StartTimerSession`, `StopTimerSession`, each carrying
  `token`) and `Outcome` variants in `crates/tui/src/app/protocol.rs`, and the worker `run` arms
  in `crates/tui/src/client/worker.rs` — exact shape of the existing task arms. Gates on the
  source: `./ok.sh build` clean, `cargo clippy --lib --bins -p tui` clean, `./ok.sh fmt --check`
  clean. **Test-compile dependency flagged:** the `FakeClient` is in `crates/tui/tests/common/mod.rs`
  (tester-owned, slice 4t); the new trait methods mean the tui integration-test binaries do not
  compile until 4t adds the `FakeClient` timer impls + the `screen_name` `Screen::Timer` arm. Per
  the slice brief, that file was left untouched. `contract` + `server` suites and the tui lib/bin
  build stay green.

- 2026-06-23 [tui-dev] built **slice 4** — the focus/timer view. Added `Screen::Timer(TimerState)`
  with a new `crates/tui/src/app/timer.rs`: `TimerState` holds the last server-returned
  `TimerConfig` + `TimerSession`, the in-flight `pending: Option<RequestId>` marker, an optional
  inline `message`, an optional `DurationEditState` edit buffer (same category as `AddTaskState`),
  and a monotonic `applied_at: Option<Instant>` captured when the session response landed — **no
  authoritative remaining-seconds integer is stored** (#1). `draw_timer` (`crates/tui/src/ui/mod.rs`)
  shows the duration, the session state, and — when running — a live `MM:SS` countdown via the
  pure `countdown_label(ends_at_secs, server_now_secs, since_response)`, recomputed every ~80 ms
  render tick from `ends_at − (server_now + elapsed_since_response)`; on reaching `00:00` locally
  it shows "Completed (awaiting server confirmation)" until the server's authoritative `Completed`
  verdict arrives. `countdown_label` takes epoch seconds (not a `chrono` type) so the `tui` crate
  keeps its no-direct-`chrono` invariant; the caller derives them from the DTO's `DateTime` via
  `timestamp()`. Coarse session refresh is `TIMER_REFRESH_TICKS = 63` ticks ≈ 5 s (A3) in the
  `terminal::run` loop while the timer view is open and idle (plus on entry and on each user
  action) — never per-second; the ~80 ms tick only animates the local countdown. Navigation (A2):
  `t` on the task list opens the timer (`Event::OpenTimer`, which loads config→session from the
  server); `Esc` on the timer is `Event::Cancel` (the core resolves it to abandon-edit /
  cancel-in-flight / back-to-task-list, re-listing tasks on return); in the timer `s`=start,
  `x`=stop, `d`=set duration, `r`=refresh, `Ctrl+C`=quit — added the `map_key` arms and the
  `Screen::Timer` cases across `handle_event`/`set_pending`/`cancel_in_flight`/`apply_response`.
  Switching profiles does not touch the account-global timer (the view derives only from the
  timer endpoints, not the profile). Error routing unchanged (ADR-0006 §6): `unauthenticated` →
  login, offline → blocking screen, a duration-update validation error surfaces inline in the edit
  sub-flow, other `Api` codes → inline `message`. Gates on the source green: `./ok.sh build`,
  `cargo clippy --lib --bins -p tui`, `./ok.sh fmt --check`. The interactive `TestBackend` suite
  (`crates/tui/tests/timer.rs`) is slice 4t, `tester`'s — not written here; `./ok.sh test` /
  full-target `./ok.sh lint` go green once 4t lands (see the slice-3 test-compile note).

- 2026-06-23 [tester] built **slice 4t** — the TUI `TestBackend`/core timer suite and the
  `FakeClient` timer impls that make the test build compile. **Fixed the RED test build first**
  (`crates/tui/tests/common/mod.rs`, tester-owned): added the five `FakeClient` `Client` trait
  impls (`get_timer_config`/`update_timer_config`/`get_timer_session`/`start_timer_session`/
  `stop_timer_session`, scripted via per-endpoint `VecDeque` queues + `push_*` exactly as the
  task methods, recording new `Call` variants), the five `run_request` executor arms (mapping each
  new `ClientRequest` variant through the fake to its `Outcome` — the synchronous worker-analogue,
  the only mock being the sanctioned `Client` trait), and the `Screen::Timer` arm in `screen_name`.
  Added timer test helpers: `timer_config`, `running_session`/`completed_session` (built from
  canonical wire JSON so `chrono` instants are parsed by the `contract` derive — the suite keeps
  no direct `chrono` dep), and the `timer_screen`/`timer_screen_pending`/`timer_screen_editing`
  builders for the pure `map_key` tests. New `crates/tui/tests/timer.rs` (14 tests) driven through
  the public two-step `App` API + the synchronous executor, mapped to acceptance: **navigation** —
  `t` opens the view loading config→session, back key (`Cancel`) returns to the task list and
  re-lists (`open_timer_loads_config_then_session_from_server`, `back_key_returns_to_task_list_and_relists`);
  **start→running countdown** — the `MM:SS` label is derived from the server's `ends_at`+`server_now`
  via `countdown_label` and shown in the rendered buffer (`start_renders_running_countdown_from_ends_at_and_server_now`);
  **stop→idle** (`stop_returns_to_idle_render`); **set-duration** — `d`+input+submit issues
  `UpdateTimerConfig` with the typed minutes and reflects the new duration, validation error
  surfaces inline (`set_duration_issues_update_and_reflects_new_value`,
  `set_duration_validation_error_surfaces_inline_in_edit`); **completed render**
  (`completed_session_renders_completed_state`); **in-flight spinner**
  (`start_shows_in_flight_spinner_until_response`, `request_triggering_event_while_pending_is_a_no_op`);
  **cancel / stale-id drop** (`cancel_while_pending_clears_in_flight`,
  `stale_response_after_cancel_is_dropped`, `superseded_response_after_new_request_is_dropped`);
  **account-global #4/ADR-0002 §5** — every timer `Call` carries only the token (no `profile_id`,
  unlike `ListTasks`/`CreateTask`/`CloseTask`), and a leave-then-re-enter re-reads the same
  server-side session (`timer_requests_are_account_global_not_profile_scoped`,
  `timer_session_unaffected_by_returning_through_the_task_list`). Also added 5 timer keybinding
  tests to `crates/tui/tests/keybindings.rs` pinning `t`/`s`/`x`/`d`/`r`/`Esc` and the
  duration-edit text-entry context. Mocks only the sanctioned `Client` trait; no source under
  `crates/*/src/` touched. Gates green from the worktree: `./ok.sh test` (tui timer 14, server
  timer 21, contract timer 19; full suite 0 failures), `./ok.sh lint` (full `--all-targets`, now
  green — was red on the test build), `./ok.sh fmt --check`.

- 2026-06-23 [reviewer] cold pre-merge review at HEAD `fc894ce`, code-hash
  `708ee8d0085ce9b3af68eb7e1b76dbe56a6185da`. Mechanical gates green (`./ok.sh test` — contract
  19 / server 21 / tui 14 + 5 keybinding, 0 failures; `./ok.sh lint --all-targets` clean;
  `./ok.sh fmt --check` clean). Risk-surface checks all HOLD: **#1** stateless TUI (no stored
  remaining-seconds counter; countdown recomputed each draw from `ends_at` + `server_now` + a
  monotonic `Instant`, nothing persisted); **#4 / ADR-0002 §5** account-global (every route +
  client method keys on `user_id`, no `profile_id`; tables `user_id PRIMARY KEY`); **#3** flat
  (duration the only knob, no pause); **#2 / ADR-0002** contract is single source of truth, no
  new/amended ADR needed; reversible migration (paired up/down, `ends_at` derived not stored);
  `{ code?, message }` reused, bad duration → `400 ValidationFailed`, no new ErrorCode; no `as`
  at the DB boundary; `#[tracing::instrument]` spans on all five handlers; no secret leak; the
  three `#[allow]` are the sanctioned test-only exception. Deferred positive completion-at-
  `ends_at` verdict confirmed genuinely live-only (left to the verifier). No blocking/fix-now
  findings; no nits requiring change.

  `REVIEW-STATUS: approved   fc894ce   code-hash=708ee8d0085ce9b3af68eb7e1b76dbe56a6185da`

- 2026-06-23 [verifier] live pass against the booted stack (`./ok.sh up`; docker present,
  migrate one-shot exited 0, both timer tables created). Confirmed `./ok.sh code-hash HEAD` =
  `708ee8d0…` before booting. **All acceptance areas RAN live** (curl against `:8080`):
  config default 30, PUT→GET round-trip, `0`/`1441` → `400 validation_failed` with `{code,
  message}`, boundaries 1/1440 accepted; session idle→start(`ends_at == started_at + dur`,
  `server_now < ends_at`, all four fields)→stop, idempotent stop. **Completion DIRECTLY
  OBSERVED** (not inferred): 1-min session polled every 5 s flipped running→`completed` when
  `server_now >= ends_at`; row kept (`count=1`, re-read still `completed`) until `stop`
  (`count=0`, `idle`). **Persistence across `docker compose restart server`**: config `77` +
  running session survived (only `server_now` advanced) → state lives in Postgres. **Account-
  global**: routes carry no `profile_id`; a second account independent. **Auth**: no-bearer →
  `401 unauthenticated`. **OTel spans** observed in collector stdout for all five handlers with
  `code.namespace: server::handlers::timer` + the `user_id` attribute. TestBackend handshake
  (ADR-0003): `crates/tui/tests/timer.rs` 14 + `keybindings.rs` 17 green. reqwest client path
  cross-checked against live shapes (no client harness; TUI out of verifier scope per ADR-0003).
  Stack torn down cleanly. No blocking gaps.

  `VERIFY-STATUS: verified   fc894ce   code-hash=708ee8d0085ce9b3af68eb7e1b76dbe56a6185da`

- 2026-06-23 [eng-manager] post-cycle learn + summarise (drive step 6). Filled the `## Summary`
  above. Cross-cutting/derived outputs landed on `main` (homes #1/#3): the `docs/handoff.md`
  cycle entry + refreshed "What works right now" snapshot, `docs/build-plan.md` (0008 → in-flight,
  0006 corrected to merged), and the regenerated `board/README.md`. **No durable
  `CLAUDE.md`/standards-skill edit earned this cycle** — the render-only countdown pattern is
  already fully captured by ADR-0006 §5 + the existing #1 invariant + the pure-core/effectful-shell
  rule in `rust-standards` (it specializes those, it does not generalize beyond them), so it is
  recorded in the handoff rather than manufactured into a skill. **No new crate** (the timer is a
  module inside the existing `contract`/`server`/`tui` crates) → no new dev agent to register. **No
  new/amended ADR** — ADR-0002 governs. The orchestrator owns the `status` flip to
  `awaiting-merge`; not touched here.

- 2026-06-23 [orchestrator] step-7 freshen: rebased `feature/0008-pomodoro-timer` onto `main`
  @ `aee92a2` (main had advanced only in `docs/` + `board/` via the eng-manager step). The sole
  conflict was the feature-local Board file (main's frozen pointer vs. the branch's authoritative
  copy) — resolved in favour of the branch (frozen note dropped). `./ok.sh code-hash` at the
  rebased head is unchanged at `708ee8d0085ce9b3af68eb7e1b76dbe56a6185da` = the attested hash, so
  the `approved`/`verified` verdicts carry forward untouched (no relabel; the commit-sha pointers
  `fc894ce` may now be stale — the binding key is the hash). Re-ran the gates on the rebased tree:
  `./ok.sh test` green (exit 0), `./ok.sh lint` clean, `./ok.sh fmt --check` clean. Board-only
  freshen — does not retrigger review. Status → `awaiting-merge`.

- [x] 2026-06-23 [human] suggestion(ui): We don't need a dedicated page for the Timer, specially
  because it is a global concept, it should be always visible across pages. My suggestion is that
  we position it in the bottom right corner; user presses "p" to start and stop. Don't forget to
  add it to the help menu in the bottom left.
  → Resolved on-branch by **0008-R1** (ADR-0006 §8.1–8.2): `Screen::Timer` removed; the timer is
  an always-visible bottom-right global widget on every post-auth screen; global `p` toggles
  start/stop; `p` + `d: set duration` added to the bottom-left hotkey caption. Reviewer-approved +
  verifier-verified @ code-hash `3fa0adef`.

- [x] 2026-06-23 [human] issue(ui): The constant refresh of the UI for the Timer is causing the UI
  to flick due to the (working...) text replacing the hotkeys caption. My suggestion is that
  instead of replacing the text, we just add the spinner at the end of it whenever the state is
  updating. I also feel like we don't need to check the state of the timer that frequently — looks
  like every 5s now; once every minute should be more than enough.
  → Resolved on-branch by **0008-R1** (ADR-0006 §8.3–8.4): the in-flight indicator now **appends**
  a trailing spinner (+ "Esc to cancel") to the stable caption instead of replacing it with
  "working…" (flicker fix, every screen); coarse `GetTimerSession` cadence loosened ~5 s → ~1 min
  (`TIMER_REFRESH_TICKS` 63 → 750). Reviewer-approved + verifier-verified @ code-hash `3fa0adef`.

- 2026-06-23 [architect] feedback re-entry triage (drive step 0): the two `[human]` UI-feedback
  lines (global always-visible timer widget + `p` start/stop toggle + help-menu entry;
  append-spinner instead of caption-replacement + ~1/min coarse cadence) are scope/approach TUI
  changes, so they require an ADR before re-implementation. **Amended [ADR-0006][adr-0006] (§8)** —
  the natural home (TUI structure + the §5 in-flight indicator + the polled-loop coarse cadence);
  **not** ADR-0002 (timer authority/render model unchanged — server still owns the countdown) and
  not a new ADR. The amendment + the `docs/decisions.md` row update landed on **`main`** (home #1,
  commit `af582e6`); the branch was rebased onto it before `tui-dev` cites §8. Appended the
  **0008-R1 re-entry plan** above (TUI-only: remove `Screen::Timer`, promote timer state to an
  app-level global widget rendered bottom-right on every post-auth screen, add
  `Event::ToggleTimer`/`p`, add `p` to the caption, append-spinner indicator, `TIMER_REFRESH_TICKS`
  ~63 → ~750; reuse the existing client/protocol/worker shapes verbatim — **no
  `contract`/server/migration change**). Owners: `tui-dev` (source), `tester` (TestBackend suite).
  Assumptions B1–B5 recorded. The prior `approved`/`verified` verdicts are **void** once the TUI
  source changes (the workspace code-tree hash moves), so the item re-enters the full feature
  track. Status → `working`.

- 2026-06-23 [tui-dev] built **0008-R1 src** — the TUI-only feedback re-entry (ADR-0006 §8).
  **Removed the dedicated timer screen**: deleted `Screen::Timer(TimerState)`, the `OpenTimer`/
  `StartTimer`/`StopTimer` events, the `t`/`Esc` navigation, and `draw_timer`. **Promoted timer
  state to an app-level global field**: the per-screen `TimerState` became `app::timer::Timer`
  (renamed; same fields — last `TimerConfig` + `TimerSession`, monotonic `applied_at`, the
  duration-edit sub-flow, the in-flight `pending` marker — plus a `loaded` flag), held on `App`
  and rendered on every post-auth screen. No stored remaining-seconds integer (#1); the countdown
  is recomputed each render tick via the unchanged pure `countdown_label`. **Global `p` toggle**:
  added `Event::ToggleTimer`; the pure core resolves it to `StartTimerSession` when idle/completed
  and `StopTimerSession` when running, stamping the timer's own in-flight marker (independent of
  the screen marker, so a screen request and the timer coexist); a `p` while the toggle is pending
  is a no-op (`Timer::toggle` returns `None`). The duration edit (`d` → `BeginEditDuration`) is now
  a global text-entry sub-flow overlaying the active screen; while it owns keystrokes, `p`/`d` are
  suppressed (B4). **Global corner widget**: `ui::draw` renders the timer label bottom-right beside
  the bottom-left caption on every post-auth screen (`draw_bottom_row` splits the bottom row
  left/right); auth/offline excluded (B3, no session before login). **Append-spinner indicator**:
  replaced `working_hint` (which substituted the caption) with `caption_with_spinner(base, pending,
  tick)`, which **appends** a trailing animated spinner + "(Esc to cancel)" to the END of the
  stable caption — applied to **every** screen's caption (auth/task-list/offline), removing the
  flicker class generally. **Added `p: start/stop timer` + `d: set duration`** to the task-list
  caption (the help menu). **Coarse cadence**: raised `TIMER_REFRESH_TICKS` from `63` (~5 s) to
  **`750`** (~60 s at ~80 ms/tick, ADR-0006 §8.4); the refresh + the initial config→session load
  now fire whenever a post-auth screen is shown (via `App::load_timer_if_needed` /
  `App::refresh_timer`), not only on a dedicated timer screen. `map_key` now takes an
  `editing_duration: bool` (the duration-edit sub-flow is app-level, not per-screen) — the edge
  passes `app.is_editing_duration()`. **No `contract`/protocol/client-trait/worker shape changed**
  — reuses the existing `GetTimerConfig`/`UpdateTimerConfig`/`GetTimerSession`/`StartTimerSession`/
  `StopTimerSession` variants and worker arms verbatim; account-global (no `profile_id` on any
  timer request, #4 / ADR-0002 §5). Source gates green from the worktree: `./ok.sh build`,
  `cargo clippy --lib --bins -p tui` (no issues), `./ok.sh fmt --check`. **Expected test-build
  breakage for `tester` (slice next):** `crates/tui/tests/**` still reference removed/renamed
  symbols — `Screen::Timer`, `TimerState` (now `Timer`), `Event::OpenTimer`/`StartTimer`/
  `StopTimer`, the old `map_key(screen, key)` two-arg signature (now `map_key(screen,
  editing_duration, key)`), and `working_hint`/`draw_timer`. Affected files:
  `crates/tui/tests/timer.rs`, `crates/tui/tests/keybindings.rs`, `crates/tui/tests/common/mod.rs`
  (the `screen_name` `Screen::Timer` arm, the `timer_screen*` builders, the synchronous executor is
  unaffected since the protocol is unchanged). `./ok.sh test` / full-target `./ok.sh lint` go green
  once `tester` adapts the suite to the global-widget model.

- 2026-06-23 [tester] **0008-R1 tests** — adapted the `tui` `TestBackend`/core suite to the
  global-timer-widget model and extended coverage for the re-entry acceptance criteria. `./ok.sh
  test` GREEN (full workspace), `./ok.sh lint` (all-targets) clean, `./ok.sh fmt --check` clean.
  Changes: `crates/tui/tests/common/mod.rs` — dropped the removed `Screen::Timer`/`TimerState`/
  `DurationEditState` imports and the `screen_name` `Screen::Timer` arm and the `timer_screen*`
  builders; added `load_timer` / `refresh_timer` helpers that drive the edge hooks
  (`App::load_timer_if_needed` / `App::refresh_timer`) the way the real poll loop does (the timer
  no longer loads off an `Event`). `crates/tui/tests/keybindings.rs` — moved every call to the new
  `map_key(screen, editing_duration, key)` signature (`map`/`map_editing` shims); pinned the global
  `p`→`ToggleTimer` / `d`→`BeginEditDuration` bindings on the post-auth screen, their inactivity off
  it (auth: literal text; offline: unbound), `p` suppression in add-task + duration-edit text-entry
  (B4), and the duration-edit overlay as a global text-entry context; added the `t`-opens-nothing
  regression guard (no dedicated timer page). `crates/tui/tests/timer.rs` — rewritten to drive the
  global widget: `p` start-when-idle / stop-when-running / start-when-completed, second-`p`-while-
  pending no-op, append-spinner-no-flicker regression guard, idle-vs-pending caption contrast, the
  preserved set-duration sub-flow (success + validation-error inline), `p` suppressed-while-editing
  end-to-end, running countdown via `countdown_label`, superseded-RequestId drop on the timer's own
  marker, coarse refresh picking up the server verdict, and the account-global call-shape sweep (no
  `profile_id`, #4 / ADR-0002 §5). `crates/tui/tests/rendering.rs` — flipped the three in-flight
  tests from the old caption-replacement (`working…`) assertion to the ADR-0006 §8.3 append-spinner
  contract (caption text stays present, `working…` gone, trailing spinner + "Esc to cancel"
  appended). Counts: tui — keybindings 19, rendering 11, timer 17, error_branches 10, flows 9,
  in_flight 5; full workspace all green. Maps re-entry criteria 1–7 + criterion 8 (the new `map_key`
  signature). No source touched.

- 2026-06-23 [reviewer] cold review of the 0008-R1 re-entry (drive step 4), scoped to the two code
  commits `97b2b32` (source) + `67e40af` (tests) against pre-re-entry base `3f2bcbe`. Gates green:
  `./ok.sh test` (tui keybindings 19 / rendering 11 / timer 17 + flows/in_flight/error_branches),
  `./ok.sh lint` clean, `./ok.sh fmt --check` clean. **#1 holds** (app-level `Timer` is transient
  render state — no stored remaining-seconds; countdown recomputed each draw). **#2 holds and is
  byte-identical** — `git diff 3f2bcbe..67e40af` over `crates/contract/**` + `crates/server/**` +
  `tui` protocol/client is empty; the re-entry reuses the existing timer wire/protocol/client/worker
  shapes verbatim (this bounds the verifier to the TUI surface). **#4 holds** (no `profile_id` on any
  timer request). ADR-0006 §8 fidelity confirmed (global widget bottom-right post-auth only; `p`
  toggle + text-entry suppression; append-spinner not caption-replacement; `TIMER_REFRESH_TICKS =
  750`). Test coverage maps all re-entry acceptance criteria incl. the append-spinner-no-flicker
  regression guard. No unjustified `#[allow]`, `as`-free conversions, error contract intact. No
  blocking findings; no out-of-scope nits worth a chore. **REVIEW-STATUS: approved** ·
  CODE-HASH `3fa0adefce8cd6d67ae716dae7a24ce6dbf9defd` · COMMIT `67e40af`.

- 2026-06-23 [eng-manager] post-cycle learn + summarise for the **0008-R1 re-entry** (drive
  step 6). Extended the `## Summary` above to the shipped end state (always-visible bottom-right
  global widget, global `p` start/stop toggle, `p` + `d` in the help caption, append-spinner
  in-flight indicator, ~1-min coarse cadence) and recorded that ADR-0006 §8 governs the TUI
  presentation while ADR-0002 still governs authority/render — TUI-only, no contract/server/
  migration change. **Checked off the two `[human]` UI-feedback boxes `[x]`** with resolution
  pointers (resolved on-branch by 0008-R1, ADR-0006 §8, reviewer-approved + verifier-verified @
  code-hash `3fa0adef`). Cross-cutting/derived outputs landed on `main` (homes #1/#3): the
  `docs/handoff.md` 0008-R1 entry + refreshed "What works right now" snapshot, and the regenerated
  `board/README.md`. **No durable `CLAUDE.md`/standards-skill edit earned** — the existing
  "Feedback re-entry" + "Verdict pinning" text already covers this re-entry's mechanics exactly
  (ADR amendment lands on `main` first → branch rebased onto it → `awaiting-merge` → `working` →
  prior verdicts void because the code-tree hash moved → full feature track re-run); recorded as a
  worked example in the handoff, not manufactured into a new rule. **No new crate** → no new dev
  agent. The orchestrator owns the `status` flip back to `awaiting-merge`; not touched here.

- 2026-06-23 [verifier] live verify of the 0008-R1 re-entry (drive step 5), bounded per ADR-0003
  (TUI render/interaction re-entry; wire surface unchanged). **#2 byte-identity independently
  confirmed:** `git diff 3f2bcbe..HEAD` over `crates/contract` + `crates/server` + `tui`
  client/protocol is EMPTY — the full delta is confined to `crates/tui/src/{app,terminal,ui}` +
  `crates/tui/tests/**` + the Board file. **TestBackend suite green and asserts the re-entry
  behaviour by name** (`global_timer_widget_renders_on_the_task_list`, `p_starts/_stops/_when_
  completed`, `second_p_while_the_toggle_is_pending_is_a_no_op`, `p_is_suppressed_while_editing_
  duration_end_to_end`, `in_flight_appends_a_spinner_without_replacing_the_caption`): tui keybindings
  19 / rendering 11 / timer 17 / flows 9 / in_flight 5 / error_branches 10; full workspace green.
  **Docker present + sanctioned (as in the original 0008 pass — installed nothing), so the live
  wire pass was performed, not deferred:** `./ok.sh up` (postgres healthy → migrate one-shot exit 0
  → server on :8080); live round-trip of `GET/PUT /api/timer/config` (30→25), `GET/POST
  …/session{,/start,/stop}` (idle→running with `started_at`/`ends_at`/`duration_minutes`/`server_now`
  →idle), error contract `{code,message}` (duration 0 → 400 `validation_failed`; unauth → 401
  `unauthenticated`), OTel spans `get_config`/`update_config`/`get_session`/`start_session`/
  `stop_session` observed in the collector; `./ok.sh down` clean. `./ok.sh build|lint|fmt --check`
  all clean. code-hash at HEAD `3fa0adefce8cd6d67ae716dae7a24ce6dbf9defd` matches the reviewed hash.
  **VERIFY-STATUS: verified** · CODE-HASH `3fa0adefce8cd6d67ae716dae7a24ce6dbf9defd` · COMMIT
  `09470e9`.

- 2026-06-23 [orchestrator] step-7 freshen: rebased `feature/0008-pomodoro-timer` onto `main`
  @ `a07eba5` (main had advanced only in `docs/` + `board/README.md` via the eng-manager step).
  `./ok.sh code-hash` at the rebased head is unchanged at `3fa0adefce8cd6d67ae716dae7a24ce6dbf9defd`
  = the attested hash, so the `approved`/`verified` verdicts carry forward untouched (no relabel;
  the commit-sha pointers may now be stale — the binding key is the hash). Re-ran the gates on the
  rebased tree: `./ok.sh test` green (exit 0), `./ok.sh lint` clean, `./ok.sh fmt --check` clean.
  Board-only freshen — does not retrigger review. Status → `awaiting-merge`.

- 2026-06-23 [human] closed the item: fast-forward merged `feature/0008-pomodoro-timer`
  (`a07eba5..c32f0ad`, linear — single-parent, no merge commit) into `main` at the operator's
  explicit direction, bringing the finished feature (original 0008 build + the 0008-R1 feedback
  re-entry) onto `main` atomically with its Board item. Verdicts held at code-hash `3fa0adef`.
  Status → `merged`. Worktree + branch cleaned up.

[adr-0001]: ../../docs/adr/0001-foundational-architecture.md
[adr-0002]: ../../docs/adr/0002-pomodoro-timer-authority.md
[adr-0003]: ../../docs/adr/0003-verification-layering.md
[adr-0006]: ../../docs/adr/0006-tui-concurrency-and-responsiveness.md
