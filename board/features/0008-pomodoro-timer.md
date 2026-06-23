---
id: 0008
title: Pomodoro focus timer — global duration config + start/stop session
type: feature      # feature | chore
status: working          # inbox → planned → ready → working → review → awaiting-merge → merged | blocked
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

[adr-0001]: ../../docs/adr/0001-foundational-architecture.md
[adr-0002]: ../../docs/adr/0002-pomodoro-timer-authority.md
[adr-0003]: ../../docs/adr/0003-verification-layering.md
[adr-0006]: ../../docs/adr/0006-tui-concurrency-and-responsiveness.md
