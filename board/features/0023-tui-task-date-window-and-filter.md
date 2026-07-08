---
id: 0023
title: TUI task date-window (hide older than X days) + filter-by-day
type: feature      # feature | chore
status: review          # inbox → planned → ready → working → review → awaiting-merge → merged | blocked
priority: medium    # high | medium | low
parent: null
depends-on: [0020]  # builds on the today/older split, `h` hide toggle, and the 200-cap (merged)
branch: feature/0023-tui-task-date-window-and-filter
worktree: .claude/worktrees/0023-tui-task-date-window-and-filter
created: 2026-07-08
updated: 2026-07-08
---

## Feature request

Two related Tasks-pane capabilities, both driven by a UTC-civil-day window over `created_at`.

**Part 1 — hide tasks older than X days.**

- Tasks older than X days do not show in the task list.
- X is configurable **in the client**, **non-persistent** (resets on restart), **default 3**.
  Hotkey **`F`** (capital) opens a config dialog modelled on the timer-duration dialog.
- Example: on Monday 6 July with X = 3, the list shows Today, Sun 5th, Sat 4th, Fri 3rd — i.e.
  today plus the previous 3 days, a **4-day window** `[today − 3, today]`.
- The **"Older tasks"** separator label changes to **"Last X days"** (rendered with the numeric
  value, e.g. `Last 3 days`).

**Part 2 — filter tasks by a selected day.**

- Hotkey **`f`** (lowercase) opens a config dialog; date format **`DD/MM/YYYY`**.
- Opens with **today's date** selected.
- Input is limited to a single component (day, month, year); **Tab** cycles through them
  (individual form fields are acceptable if easier).
- **Arrow Up/Down** increment/decrement the selected component.
- **No roll-over** of month/year: pressing Down on month `1` goes to `12` without changing the
  year; same for the day/month relationship.
- **No calendar validation** (28/30/31): validate only day `1–31`, month `1–12`, year `≥ 1970`.
- Hour is ignored; **day** is the granularity.
- Selecting date D **re-anchors** the last-X-days window to `[D − 3, D]` — "older tasks (D − 3)
  still shows for the selected date."

### Acceptance

1. Default Tasks pane shows only `[today − 3, today]`; a task created ≥ 4 days ago is not shown.
2. `F` opens a numeric dialog; changing X to `n` re-fetches so the window becomes `[anchor − n,
   anchor]`; the older separator reads `Last n days`; X resets to 3 on restart.
3. `f` opens a `DD/MM/YYYY` dialog seeded with today; Tab cycles day→month→year; Up/Down adjust
   the focused component with **wrap-in-place, no carry** (`month 1 −1 → 12`, `day 1 −1 → 31`,
   `31 +1 → 1`); values are bounded (day 1–31, month 1–12, year ≥ 1970); submit re-fetches with
   the window anchored on the selected day and re-titles the date header to that day.
4. With a past date D selected, tasks dated after D are hidden and `[D − X, D]` shows; the `h`
   hide-older toggle still collapses/hides the older group within the fetched window.
5. Help overlay documents `F` and `f` without the reference line wrapping (≤ `HELP_DIALOG_WIDTH`).
6. Full DoD: `./ok.sh test | lint | fmt --check` green; `tester` TUI `TestBackend` suite green
   and un-strands `crates/tui/tests/common/mod.rs`; `verifier` exercises the live server
   date-window query path; `reviewer` approves pinned to `./ok.sh code-hash`.

## Plan(s)

### Plan: Server-side UTC date window + two client-only view knobs

**Approach:** Tracer-bullet the wire first — add the two optional epoch-second bounds to
`TaskListQuery`, apply them as a `created_at` range filter server-side, and thread the **default**
window (`X = 3`, anchor = today) through the existing `task_list_query()` so the default list is
already date-windowed end-to-end. Then widen on the TUI: re-anchor the today/older split on the
selected day, make the separator label dynamic, and add the two dialogs (`F` numeric like the
timer edit; `f` a three-component `DD/MM/YYYY` editor) that mutate the client-only, non-persistent
`hide_window_days` / `filter_date` view-state and trigger a re-fetch on submit. All date math is
UTC civil-day in the TUI (per ADR-0015); the server never reasons about "days."

**ADR:** [ADR-0015][adr-0015] — required, authored before code (adds `created_from`/`created_until`
to `TaskListQuery`; resolves the idea-0009 date-basis fork as keep-UTC).

**Slices:**

1. **[contract-owner]** Add `created_from: Option<i64>` + `created_until: Option<i64>` (UTC epoch
   seconds; inclusive-from / exclusive-until) to `TaskListQuery`, both `skip_serializing_if`;
   doc the day-aligned-boundary convention and empty-query invariant. — files:
   `crates/contract/src/task/mod.rs`.
2. **[server-dev]** Apply the bounds in the task-list handler/SQL: `created_at >=
   to_timestamp($from)` / `created_at < to_timestamp($until)` when present; reject
   `from > until` as `400 validation_failed` (standard `{code,message}` body); preserve
   `created_at DESC`, profile-scoping (#4), `limit` clamp/`offset`. Refresh `.sqlx/`
   (`./ok.sh prepare`). — files: `crates/server/src/...` (task-list route + query), `.sqlx/`.
3. **[tui-dev]** (a) Add non-persistent `hide_window_days: u32` (default 3) and `filter_date:
   Option<i64>` (civil day-number) to `TaskListState`; anchor `A = filter_date.unwrap_or(current
   day)`. (b) Build `created_from = (A − X)·86400`, `created_until = (A + 1)·86400` into
   `task_list_query()`; re-issue `ListTasks` when X or D changes. (c) Re-anchor the today/older
   split and the date header on `A`; make `OLDER_SEPARATOR_LABEL` dynamic (`Last {X} days`).
   (d) Two dialogs cloned from the timer `DurationEditState` pattern (`Option<EditState>` field +
   `begin_edit` + short-circuit in `handle_event` + `draw_active_dialog` branch): `F` numeric
   window size; `f` three-component `DD/MM/YYYY` with Tab-cycle, Up/Down wrap-in-place (no carry),
   and bounds validation, seeded to today. (e) New `Event` variants + `map_key` arms (`F`/`f` under
   `on_tasks && globals_live`), `overlay_capturing_input()` update, help-overlay lines within
   `HELP_DIALOG_WIDTH`. — files: `crates/tui/src/app/task_list.rs`, `app/mod.rs`,
   `app/protocol.rs`, `terminal/mod.rs`, `ui/mod.rs`, `client/{mod.rs,worker.rs}`.
4. **[tester]** Contract serde round-trip (new fields absent → empty query; present → params).
   Server integration tests: window filter (inclusive-from/exclusive-until), `from > until` →
   400, profile-scoped, absent → whole list. TUI `TestBackend`: default window applied, `F`
   re-fetch + dynamic label, `f` Tab/Up-Down/no-carry/bounds/submit-re-fetch, past-date re-anchor
   with `h` interaction, help no-wrap regression. **Un-strand `crates/tui/tests/common/mod.rs`** (new
   query args + new `TaskListState` fields + any `ClientRequest`/`Outcome`/`Client` surface) per
   the 0019/0020 gotcha; default unscripted list calls to the natural window. — files:
   `crates/contract/tests/`, `crates/server/tests/`, `crates/tui/tests/`.

**Assumptions:**

- Date basis is **UTC civil-day** (operator, 2026-07-08); no timezone dependency — ADR-0015.
- Filter semantics = **anchor the window on D** (operator): selecting D shows `[D − X, D]`;
  today's tasks are hidden when D is in the past unless within that window.
- Filtering locus = **server-side date param** (operator): required so >200 total tasks does not
  strand older ones; the 200-cap still bounds a single window (future pagination unchanged).
- `hide_window_days` / `filter_date` are **ephemeral in-session** view-state (same class as
  `hide_older` and the timer edit buffer) — non-persistent, reset on restart; #1 preserved.
- Separator label renders the numeric value (`Last 3 days`), not the literal glyph `X`.
- `X` minimum is `1` (a 2-day window `[anchor − 1, anchor]`); `F` rejects `0`/non-numeric with an
  inline error like the timer dialog (operator, 2026-07-08). **Today-only is not an `X = 0` mode**
  — it is achieved with the existing `h` hide-older toggle (0020), which collapses the older group
  and leaves only the anchor-day group visible.
- idea 0009 is resolved keep-UTC by ADR-0015; the ADR-0014 §5 / 0020-plan "local date" wording
  reconciliation is a docs-on-`main` follow-up (eng-manager / this cycle's close).

**Risks:**

- **Harness re-strand (expected).** New always-runs query args + new `TaskListState` fields will
  break `crates/tui/tests/common/mod.rs`; the dev's `--lib --bins` gate looks green while
  `--all-targets` goes red until the tester slice lands (0019/0020 gotcha). Not mergeable until
  the tester slice lands in the same cycle.
- **Help-overlay overflow (recurring gotcha).** Two new Tasks hotkeys risk wrapping a reference
  line past `HELP_DIALOG_WIDTH = 72`; add a third Tasks line or widen, with a pinned no-wrap test.
- **Default-behaviour shift.** The default list now hides tasks >3 days old — intended, but a
  visible change; the `## Summary` should call it out.
- **Day-boundary edge (accepted).** UTC civil-day grouping near local midnight (idea 0009);
  accepted at single-user scale.

## Log / comments

- [x] 2026-07-08 [architect] Planned via `plan` skill. Operator settled the three design forks
  (date basis = keep UTC; filter semantics = anchor-on-D; locus = server-side date param).
  ADR-0015 authored. Status → ready. Planning artifacts (ADR-0015, decisions index, this item)
  committed to `main` before any worktree is cut.
- [x] 2026-07-08 [orchestrator] Claimed. Worktree + branch
  `feature/0023-tui-task-date-window-and-filter` cut from `main` @ f41c596 (carries ADR-0015 +
  plan). Status → working. Branch copy now authoritative; `main` copy frozen at claim snapshot.
- [x] 2026-07-08 [contract-owner] Slice 1 (contract) done. Added two optional, additive fields
  to `contract::TaskListQuery` in `crates/contract/src/task/mod.rs`: `created_from: Option<i64>`
  (inclusive lower bound, UTC epoch seconds) and `created_until: Option<i64>` (exclusive upper
  bound), both `#[serde(skip_serializing_if = "Option::is_none")]` so `TaskListQuery::default()`
  still serializes to an empty query string (absent-both === pre-0023, ADR-0015 §2). Matches the
  ADR-0015 Decision block shape; `limit`/`offset` unchanged. Rustdoc documents the
  day-aligned-boundary convention (`(anchor − X)·86400` / `(anchor + 1)·86400`; server = plain
  `timestamptz` range filter, no civil-day math), inclusive-from / exclusive-until semantics, and
  the empty-query invariant, with a window doctest. No `Task`/`Subtask` touched (#3 preserved).
  Contract crate builds (`cargo build -p contract`), 26 doctests pass, `clippy -p contract --lib`
  clean, `./ok.sh fmt --check` clean. Workspace `build`/`lint`/`test` go red only on the not-yet-
  written server/tui/tester slices (expected).
- [x] 2026-07-08 [server-dev] Slice 2 (server) done. Applied the two ADR-0015 bounds in the
  task-list query (`crates/server/src/handlers/tasks.rs`): a single static, sqlx-offline-checkable
  `SELECT` gains two NULL-guarded predicates `($4::bigint IS NULL OR created_at >=
  to_timestamp($4::bigint))` and `($5::bigint IS NULL OR created_at < to_timestamp($5::bigint))`,
  binding `query.created_from` / `query.created_until` (`Option<i64>`). An absent bound is a NULL
  param that drops out, so **absent-both is byte-identical to pre-0023** (whole list within
  `limit`); `created_from` inclusive / `created_until` exclusive; the server does no civil-day
  math. Validation added before the query: **both** bounds present and `created_from >
  created_until` → `ApiError::Validation` = `400 {code: "validation_failed", message}` (same
  construction as the existing `limit`/title validations); `created_from == created_until` is a
  valid empty window (upper exclusive) → `200 []`. `created_at DESC`, profile-scoping (#4, the
  `assert_owned` gate + `profile_id = $1`), and the `limit` clamp / `offset` semantics (ADR-0014)
  are unchanged and compose with the filter. Refreshed `.sqlx/` via a server-scoped
  `cargo sqlx prepare` against the sanctioned throwaway test Postgres (`./ok.sh prepare`'s
  `--workspace` cannot complete until slice 3 lands — the not-yet-built `tui` fails to construct
  `TaskListQuery` with the new fields; `tui` carries no SQL, so a server-scoped prepare yields
  identical cache content): one query file replaced, no others touched. Gates: `./ok.sh fmt
  --check` clean; server crate `cargo build -p server` + `cargo clippy -p server --all-targets`
  clean (offline). Workspace `build`/`lint`/`test` remain red only on the not-yet-written tui +
  tester slices (expected, 0019/0020 harness gotcha). Files: `crates/server/src/handlers/tasks.rs`,
  `.sqlx/`.
- [x] 2026-07-08 [tui-dev] Slice 3 (TUI) done. **(a)** Added two ephemeral in-session fields to
  `TaskListState` (`crates/tui/src/app/task_list.rs`): `hide_window_days: u32` (default
  `DEFAULT_HIDE_WINDOW_DAYS = 3`) and `filter_date: Option<i64>` (civil day-number, default `None`),
  plus a `pub anchor_day(today_day) = filter_date.unwrap_or(today_day)` helper; both reset on restart,
  never persisted (#1). **(b)** New free fn `windowed_task_list_query(anchor, X)` in `app/mod.rs`
  builds `created_from = (anchor − X)·86400` / `created_until = (anchor + 1)·86400` (SECS_PER_DAY made
  `pub(crate)`); `App::task_list_query(&self)` reads the current tasks pane (else today + default
  during bootstrap), threaded through all six `ListTasks` sites; `TaskListState::refresh` re-fetches
  on any `F`/`f` change. `apply_tasks` now **preserves** `hide_window_days`/`filter_date` across the
  reload (else the label + later fetches would revert). **(c)** Re-anchored the today/older split, the
  date header, and the collapse indicators on `A` (render + `apply_subtasks` selection clamp); the
  older separator label is now dynamic `Last {X} days` (`older_separator_label`). **(d)** Two dialogs
  cloned from the timer `DurationEditState` pattern: `F` (`WindowEditState`) numeric editor — rejects
  `0`/non-numeric inline (min X = 1; today-only stays via `h`); `f` (`DateFilterState`) three-component
  `DD/MM/YYYY` editor seeded to today — Tab/Shift+Tab cycle day↔month↔year, Up/Down adjust the focused
  component with **wrap-in-place, no carry** (`month 1 −1 → 12`, `day 1 −1 → 31`, `day 31 +1 → 1`;
  year clamped ≥ 1970), no calendar validation; submit maps `(d,m,y)` → day-number via the new pure
  `ui::days_from_civil` (Hinnant inverse of the existing `civil_from_days`), sets `filter_date`,
  re-anchors, re-fetches. **(e)** New `Event` variants `BeginEditWindow`/`BeginFilterDate` (F/f, gated
  `on_tasks && globals_live`) + `IncrementField`/`DecrementField` (Up/Down while the `f` editor is
  open); `overlay_capturing_input` updated via `in_sub_flow` (both dialogs suppress globals + tab
  switch); `is_text_entry` includes the `F` numeric editor. Help overlay: added a **third Tasks
  reference line** `F window size · f filter by date` (41 cols) rather than lengthening the existing
  lines (64/66 cols) — no line exceeds the ~70-col inner width, no wrap (0015/0019 gotcha).
  **Surface for tester:** no `Client`/`ClientRequest`/`Outcome` change (reuses `ListTasks` with the
  new `TaskListQuery` window params); the re-strand is the four new `TaskListState` fields
  (`hide_window_days`/`filter_date`/`editing_window`/`filtering_date`) + every `ListTasks` query now
  carrying `created_from`/`created_until`, plus the four new `Event` variants. Gates: `./ok.sh fmt
  --check` clean; `cargo clippy --lib --bins -p tui` clean; lib+bins build. `--all-targets` is red only
  on `crates/tui/tests/common/mod.rs` (expected 0019/0020 harness re-strand — tester slice 4). Files:
  `crates/tui/src/app/{task_list.rs,mod.rs}`, `terminal/mod.rs`, `ui/mod.rs`.
- [x] 2026-07-08 [tester] Slice 4 (tests) done. **Un-stranded the harness**
  (`crates/tui/tests/common/mod.rs`, 0019/0020 gotcha): added the four new `TaskListState` fields to
  both struct initializers (`hide_window_days: DEFAULT_HIDE_WINDOW_DAYS`, `filter_date: None`,
  `editing_window: None`, `filtering_date: None`); extended `Call::ListTasks` + the fake `list_tasks`
  to capture `created_from`/`created_until` so a test can assert the wire window; added
  wall-clock-aware builders `iso_at_day` / `open_task_on_day` (place a fixture on any civil day
  relative to today). No `Event` match existed in the harness and no `Client`/`ClientRequest`/`Outcome`
  change (slice 3 reused `ListTasks`), so those were the only re-strands. **Contract tests were also
  stranded** — the existing `TaskListQuery { limit, offset }` literals in
  `crates/contract/tests/task.rs` miss the two new fields; updated them and added window round-trip
  tests (absent → omitted; present → epoch-second params; window-only omits pagination; negative lower
  bound; full round-trip). **Server** (`crates/server/tests/tasks.rs`): 8 live-DB tests via a
  direct-SQL `insert_task_at` fixture (controlled `created_at` — the `POST` path can't):
  inclusive-lower/exclusive-upper at the exact boundary second, `from`-only / `until`-only bounds,
  `from > until` → 400 `validation_failed`, `from == until` → 200 `[]`, absent-both whole list,
  profile-scoped (#4), `created_at DESC` preserved + composes with `limit`. **TUI**
  (`crates/tui/tests/date_window.rs`, 10 tests + a help no-wrap test in `dialogs.rs`): default
  `[today−3, today]` window on bootstrap; `F` re-fetch + dynamic `Last 5 days` label + `0`/empty-buffer
  inline reject with no re-fetch; `f` seeded to today, Tab cycles day→month→year, Up/Down
  wrap-in-place no-carry (`month 1→12`, `day 1→31`, `31→1`, no year carry), year clamped ≥ 1970;
  past-date submit re-anchors window+header and `h` still hides older within the window; the third
  Tasks help line (`F window size · f filter by date`) pinned no-wrap. **Also updated 3 pre-existing
  `tasks.rs` tests** whose render assertions used the retired static "Older tasks" label (slice 3
  made the rendered separator the dynamic `Last {X} days`) → local
  `DEFAULT_OLDER_SEPARATOR = "Last 3 days"`. Gates all green: `./ok.sh fmt --check`, `./ok.sh lint`,
  `./ok.sh test` (contract task 33, server tasks 35, tui date_window 10 / dialogs 26 / tasks 35; full
  workspace green). No source bug found. Files: `crates/contract/tests/task.rs`,
  `crates/server/tests/tasks.rs`,
  `crates/tui/tests/{common/mod.rs,date_window.rs,dialogs.rs,tasks.rs}`.
- [x] 2026-07-08 [reviewer] Cold review — **REVIEW-STATUS: approved**, pinned to code-hash
  `700e3b535c587fd309e4de0a5f973867a577fc02` (last code commit `5ee5bda`; branch head `582b67e`
  is a Board-only flip). Gates all green (`test`/`lint`/`fmt --check`). Verified: #2 contract drift
  clean (only `created_from`/`created_until`, both `skip_serializing_if`, empty-query invariant
  test-pinned); ADR-0015 fidelity (epoch-second bounds, server = plain `to_timestamp` range filter,
  inclusive-from/exclusive-until, `from > until` → 400 `validation_failed`, `from == until` → 200 `[]`,
  ordering/scoping/limit compose unchanged); #1 stateless (ephemeral view-state), #3 flat (no `Task`
  field), #4 profile-scoping preserved; `.sqlx/` consistent (old query cache replaced, no drift,
  builds under `SQLX_OFFLINE`); no migration needed/added; help no-wrap third Tasks line pinned by
  regression test. No fix-now findings. One out-of-scope observation (`f` accepts calendar-impossible
  dates → `days_from_civil` normalizes deterministically) is **explicitly by-design per ADR-0015**
  ("no calendar validation") — no action.
- [x] 2026-07-08 [verifier] Live verify — **VERIFY-STATUS: verified**, pinned to code-hash
  `700e3b535c587fd309e4de0a5f973867a577fc02` (HEAD `be80f5c`). `./ok.sh up` booted the stack
  healthy (one-shot `migrate` exit 0, no migration-history conflict — query-only change, no new
  migration); `./ok.sh down` cleaned up. RAN against the live server: register/login + profile;
  created 3 tasks at integer second 1783540757 giving a clean boundary demo — `created_from=…757`
  → 200 all 3 (inclusive-lower), `created_until=…757` → 200 `[]` (exclusive-upper), `…758` → all 3;
  wide window → 3, entirely-before/after → `[]`; `from > until` → **400
  `{code:"validation_failed", message:"created_from must not be after created_until"}`**;
  `from == until` → **200 `[]`**; absent-both → whole list as bare `[Task]` array, `created_at DESC`
  (ADR-0005 §5 unchanged). Profile-scoping (#4) RAN: 2nd profile's task not returned for the 1st —
  no cross-profile leakage. Error contract `{code,message}` confirmed (also `401 unauthenticated`).
  OTel `list_tasks` span observed live (scope `organized-koalad`, attrs `user_id`/`profile_id`).
  TUI TestBackend suites green (`date_window.rs` 10, `dialogs.rs` 26, tui `tasks.rs` 30, server
  `tasks.rs` 35). Inferred only: the shipped reqwest binary's own serialization (hit server via
  curl; wire shape test-pinned identical). No blocking gaps.

## Summary

coverage: 73.20%

The Tasks pane gains a **server-backed UTC-civil-day window** over `created_at` plus two
client-only, non-persistent view knobs, per [ADR-0015][adr-0015]:

- **Wire (additive, #2 via ADR-0015).** `contract::TaskListQuery` gains two optional epoch-second
  bounds — `created_from` (inclusive lower) / `created_until` (exclusive upper) — both
  `skip_serializing_if`, so `default()` still serializes to an empty query and **absent-both is
  byte-identical to pre-0023** (whole list within `limit`). No `Task`/`Subtask` field (#3), no
  migration.
- **Server (range filter + validation).** The task-list query applies two NULL-guarded predicates
  (`created_at >= to_timestamp($from)` / `< to_timestamp($until)`); no civil-day math server-side.
  Both present with `created_from > created_until` → `400 {code: "validation_failed", …}`;
  `from == until` is a valid empty window → `200 []`. `created_at DESC`, profile-scoping (#4), and
  the ADR-0014 `limit`/`offset` semantics are unchanged and compose with the filter; `.sqlx/` cache
  refreshed.
- **TUI knobs (ephemeral, #1).** `F` opens a numeric dialog for the **window size X** (default 3,
  min 1, rejects `0`/non-numeric inline, resets on restart); the "older" separator label is now the
  dynamic `Last {X} days`. `f` opens a **`DD/MM/YYYY`** dialog seeded to today — Tab/Shift+Tab cycle
  day↔month↔year, Up/Down adjust the focused component with wrap-in-place and **no carry** (`month
  1 → 12`, `day 1 → 31`, `31 → 1`; year clamped ≥ 1970), no calendar validation (ADR-0015). Both
  are `TaskListState` view-state (never persisted); selecting day D re-anchors the window to
  `[D − X, D]`, re-titles the date header, and re-fetches. A third Tasks help line
  (`F window size · f filter by date`) keeps every reference line inside `HELP_DIALOG_WIDTH`.

**Default-behaviour shift (flagged as a Risk in the plan).** With default `X = 3` and
`anchor = today`, the TUI now **always sends** the lower bound, so the **default Tasks list hides
tasks created more than 3 days ago** — a visible change from pre-0023 (which showed the whole list
up to the cap). This is intended: older tasks are reached by widening `F` or re-anchoring with `f`,
which is the operator's stated motivation (a profile with >200 tasks could not otherwise page back
to older ones). Absent-params wire behaviour is unchanged; only the TUI's *choice* to always send
`created_from` differs.

Reviewer **approved** + verifier **verified** live (boundary-second inclusive/exclusive demo,
`from > until` → 400, `from == until` → 200 `[]`, profile-scoping #4, error contract, OTel
`list_tasks` span; TUI `TestBackend` suites green), both pinned to code-hash
`700e3b535c587fd309e4de0a5f973867a577fc02`. No `Client`/`ClientRequest`/`Outcome` change (reuses
`ListTasks`); the expected harness re-strand (four new `TaskListState` fields + always-on window
query args) and help-overlay overflow both recurred exactly as predicted and were absorbed by the
tester slice.

[adr-0015]: ../../docs/adr/0015-task-list-date-window-query.md
