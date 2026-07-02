---
id: 0020
title: Tasks-pane rendering overhaul — completed-last, today/older split, hide toggle, bounded 200-cap
type: feature      # feature | chore
status: working  # inbox → planned → ready → working → review → awaiting-merge → merged | blocked
priority: medium    # high | medium | low
parent: null
depends-on: [0019]  # builds on the task + sub-task list/collapse rendering (merged)
branch: feature/0020-tui-tasks-pane-rendering-overhaul
worktree: .claude/worktrees/0020-tui-tasks-pane-rendering-overhaul
created: 2026-07-02
updated: 2026-07-02
---

## Feature request

**Goal:** Improve how the **Tasks pane** renders its list: completed items sink below active
ones, a human-readable *today* date is shown, tasks split into a **today / older** grouping
with older collapsed, an **`h`** key hides the older group, and the fetch is **bounded to 200
tasks** via a limit that lives in the wire but is hard-coded by the TUI — designed so
pagination can be added later without a wire break.

**Context (current behaviour to change):** the Tasks pane renders the task tree (tasks +
one level of sub-tasks, per [ADR-0012][adr-0012]) in the server-returned order with no
today/older grouping, no completed-last ordering, no date header, and no fetch cap.

### Behaviour (acceptance)

1. **Completed rendered last.** Within the task list, tasks whose status is *completed* render
   **after** all non-completed tasks. The same rule applies to **sub-tasks within their parent**:
   completed sub-tasks render after non-completed sub-tasks. The ordering **re-sorts immediately
   whenever a task or sub-task changes state** (complete / reopen / toggle) — no manual refresh.
2. **Today date header (full-width separator row).** The current date is shown as a **full-width
   separator row at the top of the task list, inside the bordered Tasks pane** (styled like the
   "Older tasks" separator, spanning the pane's inner width), in human-readable form — e.g.
   `Tuesday, July 2nd, 2026` (weekday, month, ordinal day, year). It is a non-selectable header
   row (navigation skips it). **Not** shown for the Notes or Profiles panes.
   *(amended 2026-07-02 per human feedback — was "top-center", a short line above the pane.)*
3. **Today / older separator.** Tasks are grouped by *created-at* into **created-today** (above)
   and **created-before-today** (below), with a **full-width** separator line labelled **"Older
   tasks"** (spanning the pane inner width) between them. Older-group tasks **default to collapsed**
   (sub-tasks hidden) on load, but **`x` toggles their collapse state** like today's tasks — the
   per-task collapse override (`x`) takes precedence over the older-group default. (The override
   remains render-time-derived, never persisted; #1 / A7.)
   *(amended 2026-07-02 per human feedback — was a hard forced-collapse, `x` inert in the older
   group.)*
4. **`h` hides the older group.** Pressing **`h`** hides all the older tasks **along with the
   separator**; pressing `h` again shows them. **Default is shown.** Add `h` to the **shortcut
   help dialog**.
5. **200-task cap.** The maximum number of tasks rendered is **200 total** (today + older
   combined). This limit is a **configurable value carried in the contract + enforced by the
   server + accepted by the reqwest client**, but the **TUI caller hard-codes the value 200**
   (the limit is a wire capability; 200 is the TUI's choice of it).
6. **Pagination-ready (design only, not built).** No pagination in the TUI yet, but the
   contract/server shape must be designed so pagination can be added **without a wire break**
   (e.g. a limit + offset/cursor request shape and a response that can later carry a
   next-page marker). The TUI does not paginate in this feature; it just requests the first
   (and only) 200.
7. **`d` deletes the selected sub-task.** *(added 2026-07-02 per human feedback.)* On a sub-task
   row, `d` arms the same two-step delete confirmation as a task (`Enter` confirms, any navigation
   disarms) and, on confirm, issues `DeleteSubtask` for the selected sub-task. On a task row `d`
   deletes the task as before. Reuses the shipped `DeleteSubtask` wire (no contract change).

### Constraints / notes for the architect (planning starts here)

- **ADR (#2 — contract is an ADR event).** Adding a **limit** (and the pagination-ready request
  shape) to the task-list wire is a `contract` change → **the architect authors/updates an ADR
  first**. The pagination shape (limit+offset vs cursor) is the decision to settle there. The
  "configurable in contract/server, hard-coded in TUI" split is deliberate: the wire exposes the
  capability, the caller chooses the number.
- **Domain flatness (#3).** The today/older grouping, the "Older tasks" separator, the
  collapse-older behaviour, and the `h` hide are **TUI-render concepts derived from
  `created_at`** — they add **no** new domain structure and **no** per-item fields. Confirm the
  plan adds no `TaskStatus` variant / task field beyond the list limit + pagination request
  params.
- **Completed-last ordering — server vs TUI.** Decide whether completed-last is a server
  `ORDER BY` or a TUI-side sort of the returned list. Because the TUI must **re-sort locally on
  a state change** and holds no persistence, a TUI-side sort of the current snapshot is the
  natural fit (#1 stateless-TUI); the server may still order for a stable default. Architect to
  settle, keeping #1.
- **Stateless TUI (#1).** The `h` hide state and the collapse-older state are **ephemeral view
  state derived per render** (not persisted) — allowed. Every view still derives from a server
  response.
- **Gotchas to plan for.**
  - **tui protocol/state extension strands the tester harness (learned 0019).** Any new
    `Client` method / `ClientRequest`+`Outcome` variant / screen-state field (e.g. the limit
    plumbing, the hide flag) leaves `crates/tui/tests/common/mod.rs` (fake client, worker-
    analogue match, state initializers) non-compiling under `--all-targets`. The **tester slice
    must land the harness update in the same cycle** — the dev's `--lib --bins`-green slice is
    not mergeable alone.
  - **Help-overlay width re-wrap (learned 0015, recurred 0019).** Adding the `h` reference line
    to the `?` help overlay can overflow the fixed-width dialog and wrap flush-left. Check the
    help-reference line widths against the dialog inner width and pin a regression test in
    `crates/tui/tests/dialogs.rs`.
- **Interaction with 0019.** Completed-last and the today/older split both reorder the same
  task-tree render path that 0019 introduced (list nesting + collapse); this card intentionally
  owns all of that reordering so it lands in one render change rather than two dueling rebases.

[adr-0012]: ../../docs/adr/0012-subtasks-domain-exception.md
[adr-0014]: ../../docs/adr/0014-task-list-pagination-ready-limit.md

## Plan(s)

**ADR:** [ADR-0014][adr-0014] — task-list pagination-ready `limit`+`offset` request (query
params), bare-array response preserved, completed-last as a **TUI-side snapshot sort**, and a
confirmation that this adds **no** domain structure (#3). Read it before starting any slice.

**Decisions settled in ADR-0014 that constrain this plan:**

- Request shape = **`limit` + `offset` optional query params** (offset pagination), not cursor.
- Response = **unchanged bare `[Task]` array**; a next-page marker (if ever needed) arrives later
  as an **additive header** — a future ADR.
- **`contract`** owns the capability + ceiling: `TaskListQuery { limit: Option<u32>, offset:
  Option<u32> }` and `MAX_TASK_LIST_LIMIT: u32 = 500`. **Server** enforces (clamp/validate, default
  absent `limit` → ceiling, absent `offset` → 0). **TUI** hard-codes `limit = 200`, `offset = 0`
  (a `tui`-local constant — the caller's choice of the capability).
- **Completed-last = TUI-side stable sort of the snapshot**, re-applied every render (so it
  re-orders instantly on complete/reopen/toggle with no re-fetch; #1). Server keeps
  `ORDER BY created_at DESC`.
- No new `TaskStatus` variant, no new per-task/per-sub-task field. Today/older split, "Older
  tasks" separator, collapse-older, and `h`-hide are TUI-render concepts derived from `created_at`.

### Dependency order

`contract-owner` → `server-dev` → `tui-dev`, with `tester` landing the harness + regression tests
in the **same cycle** (the tui protocol/state extension strands
`crates/tui/tests/common/mod.rs` — learned 0019 — so the dev's `--lib --bins`-green slice is
**not** mergeable alone). Slices S3 (tui) and S4 (tester) form one non-separable unit at the DoD
gate.

### Slice S1 — `contract`: the query DTO + ceiling constant — owner `contract-owner`

- **Files (owns):** `crates/contract/src/task/mod.rs`; doc/README as needed.
- Add `pub const MAX_TASK_LIST_LIMIT: u32 = 500;` and
  `pub struct TaskListQuery { limit: Option<u32>, offset: Option<u32> }` with
  `#[serde(skip_serializing_if = "Option::is_none")]` on each field, `Default`, `Debug`, and the
  standard derives. It must (de)serialize as query params (both optional).
- Public-API doctests demonstrating: an all-`None` query serializes to `{}` / an empty query
  string; a `limit`-only query omits `offset`; the ceiling constant value.
- **Do NOT touch** `Task`, `Subtask`, `TaskStatus`, or any create/update DTO (#3 — confirmed
  no field/variant added).
- **Tests (contract):** `contract-owner` extends `crates/contract/tests/task.rs` (public-API,
  the pure-DTO crate layout — rust-standards) OR the doctests cover it; no `module/tests.rs`
  needed (no private logic).

### Slice S2 — `server`: enforce the limit on `GET …/tasks` — owner `server-dev`

- **Files (owns):** `crates/server/src/handlers/tasks.rs`; the changed static query's `.sqlx/`
  entry via `./ok.sh prepare`.
- Change `list_tasks` to extract `Query(TaskListQuery)` (axum `Query` extractor). Resolve
  effective `limit` = `min(query.limit.unwrap_or(MAX_TASK_LIST_LIMIT), MAX_TASK_LIST_LIMIT)`;
  **reject** a `limit` strictly above `MAX_TASK_LIST_LIMIT` with `400 validation_failed` (do not
  silently clamp an explicit over-ceiling value — the ADR calls it validation); `offset` =
  `query.offset.unwrap_or(0)`. Apply `LIMIT $2 OFFSET $3` to the existing
  `ORDER BY created_at DESC` query. Bind as `i64` for sqlx/Postgres (`as_conversions` is denied —
  use `i64::from(u32)`, never `as`).
- Response body **unchanged** (bare `[Task]` array). No migration (no schema change).
- **Do NOT** add status ordering to the query — completed-last is TUI-side (ADR §4).
- **Tests (server):** `tester` owns server integration tests — add cases to the server tasks
  suite: default (no params) returns whole list newest-first; `limit=N` caps; `offset=K` skips;
  `limit` above ceiling → `400 validation_failed`; profile-scoping still holds. (Live shapes /
  status codes are the `verifier`'s clause-4 pass.)

### Slice S3 — `tui`: limit plumbing + render overhaul — owner `tui-dev`

- **Files (owns):** `crates/tui/src/app/protocol.rs`, `crates/tui/src/client/mod.rs`,
  `crates/tui/src/client/worker.rs`, `crates/tui/src/app/task_list.rs`,
  `crates/tui/src/app/mod.rs`, `crates/tui/src/ui/mod.rs`.
- **Limit plumbing (wire capability, TUI value):**
  - Add a `tui`-local `const TASK_LIST_LIMIT: u32 = 200;` (NOT in `contract`).
  - Thread `limit`/`offset` onto the `ClientRequest::ListTasks` variant (add `limit`/`offset`
    fields) OR pass a `TaskListQuery` on it; update the `Client::list_tasks` trait method
    signature to accept the query, the `reqwest` impl to send `?limit=&offset=` (serialize
    `TaskListQuery` via `.query(&q)`), and the worker arm in `worker.rs`. Every existing
    `ClientRequest::ListTasks { … }` construction site (`app/mod.rs` lines ~474, ~706, ~784,
    ~1159, ~1349; `app/task_list.rs` ~658) passes `TASK_LIST_LIMIT` / offset 0.
- **Completed-last sort (ADR §4):** in the task-list render/row-assembly (`visible_rows` and its
  sub-task grouping in `task_list.rs`), apply a **stable** sort keyed on `status` so `open` tasks
  precede `done` tasks, and within each parent `open` sub-tasks precede `done` sub-tasks —
  preserving the server's `created_at DESC` order within each status group. It must re-derive per
  render so a complete/reopen/toggle re-orders on the next frame with no re-fetch (#1).
- **Today date header (acceptance #2):** render the current **local** date top-center **inside the
  Tasks pane only**, human-readable with weekday, month, **ordinal** day, year (e.g.
  `Tuesday, July 2nd, 2026`). Not on Notes/Profiles panes. (Ordinal suffix st/nd/rd/th computed
  TUI-side.)
- **Today / older split + separator (acceptance #3):** group tasks by `created_at` into
  created-today (local date == today) above and created-before-today below, with an **"Older
  tasks"** separator row between the groups. Older-group tasks render **collapsed regardless of
  status** and **independent of** the per-task `collapse_overrides` — this is a *render-time*
  forcing, not a mutation of the collapse map (keep the two concerns separate; do not write the
  older-forcing into `collapse_overrides`).
- **`h` hide (acceptance #4):** add an ephemeral `hide_older: bool` field to `TaskListState`
  (default `false` = shown; process-lifetime view state, #1, never persisted); an `Event`
  variant + key-map for `h` that toggles it; when `true`, the older group **and** the "Older
  tasks" separator are not rendered, and selection/`visible_rows` skips the hidden rows. Add `h`
  to the `?` help overlay reference (see the help-width note below).
- **Help overlay `h` line:** add `h` to the Tasks reference block in `draw_help`
  (`crates/tui/src/ui/mod.rs`, ~line 462–463). The Tasks block already spans two lines and 0019's
  additions pushed the first line to the 64-char edge under `HELP_DIALOG_WIDTH = 72` (inner ~70).
  **Place the new `h hide older` pair on the second Tasks line** (`x collapse/expand sub-tasks ·
  Enter detail`) — do **not** lengthen the already-tight first line — and **verify the resulting
  line width against the dialog inner width** so it does not re-wrap flush-left (learned 0015,
  recurred 0019).
- **Scope guard:** no `Task`/`Subtask`/`TaskStatus`/DTO change; no new domain field. The dev's
  gate is `--lib --bins` green — the crate's `--all-targets` will be **red** until S4 lands (this
  is expected, learned 0019; do NOT read `--lib --bins` green as DoD clause-1/2 pass).

### Slice S4 — `tester`: un-strand the harness + pin the new behaviour — owner `tester`

- **Files (owns):** `crates/tui/tests/common/mod.rs`, `crates/tui/tests/dialogs.rs`,
  `crates/tui/tests/tasks.rs` (and the server tasks suite for S2's cases).
- **Un-strand `common/mod.rs` (learned 0019):** update the fake `Client::list_tasks` signature to
  match S3's new query arg, the worker-analogue `ClientRequest::ListTasks` match arm, and any
  `TaskListState` initializer for the new `hide_older` field. Follow the 0019 pattern: keep the
  strict panic-on-empty net for mutating calls; a `list_tasks` call with a query arg still pops a
  scripted tasks response (the query does not change the fake's return shape).
- **Pin the new behaviour (tasks.rs / rendering):** completed-last ordering (open-before-done at
  both levels; re-sorts after a toggle with no re-fetch); today/older split + "Older tasks"
  separator; older group forced collapsed regardless of status; the today date header renders in
  the Tasks pane and NOT in Notes/Profiles; `h` toggles the older group + separator visibility
  (default shown); `ListTasks` requests carry `limit=200`.
- **Help-overlay regression (dialogs.rs, learned 0015/0019):** assert the Tasks help reference
  lines (now including `h`) do **not** re-wrap flush-left — pin against the dialog inner width,
  mirroring the existing Global-block and Tasks-line pins.

### Risks

- **R1 — render reordering collides with 0019's tree/collapse path.** Completed-last + today/older
  both reshape `visible_rows` and sub-task grouping. Mitigation: 0020 owns all the reordering in
  one change (per the card); keep the older-group forced-collapse **separate** from
  `collapse_overrides` (render-time only) so the `x` per-task toggle semantics are unchanged.
- **R2 — harness stranding (learned 0019).** S3 alone leaves `--all-targets` red. Mitigation: S3
  and S4 are one non-separable unit at the DoD gate; do not merge on `--lib --bins` green.
- **R3 — help-overlay re-wrap (learned 0015, recurred 0019).** The `h` line can overflow.
  Mitigation: put `h` on the second Tasks line, verify width, pin a regression test in dialogs.rs.
- **R4 — `as_conversions` denied in server.** The `u32`→`i64` bind for `LIMIT`/`OFFSET` must use
  `i64::from(...)`, never `as`. Mitigation: called out in S2.
- **R5 — ordinal date formatting.** No stdlib ordinal suffix; compute st/nd/rd/th TUI-side (11–13
  → th). Mitigation: unit-testable pure helper; pin a couple of cases in tasks.rs/rendering.

### Assumptions

- **A1 — offset over cursor** (ADR §1): personal-scale single-user list; offset+limit is the
  smallest pagination-ready shape and additive query params never break existing callers.
- **A2 — `MAX_TASK_LIST_LIMIT = 500`**: comfortable headroom over the TUI's 200; a single ceiling
  the server clamps/validates to. Chosen as the smallest safe bound above the caller's value.
- **A3 — over-ceiling `limit` is `400 validation_failed`, not silent clamp** (ADR §2): an explicit
  bad value is a client error; an *absent* value defaults to the ceiling (preserving ADR-0005 §5
  whole-list behaviour). Smallest change consistent with the ADR.
- **A4 — completed-last is TUI-side** (ADR §4): #1 requires instant re-sort on state change with
  no re-fetch, which only a snapshot sort delivers.
- **A5 — "today" is the local date** vs each task's `created_at` (rendered from a UTC timestamp).
  Smallest interpretation of the operator's `Tuesday, July 2nd, 2026` example; matches the
  top-center "current date" header.
- **A6 — the `A/e/Space/d/x` semantics from 0019 are unchanged**; `h` is a new, non-colliding
  binding in the Tasks pane only (checked against the existing keymap).
- **A7 — the older group's forced-collapse does not mutate `collapse_overrides`**; it is a
  render-time forcing, so leaving the older group (or pressing `x`) does not corrupt a task's
  in-session collapse override.

### Amendment 2026-07-02 (human feedback re-entry, `working`)

Three operator adjustments triaged by `architect`; **no ADR** (no #2/#3 change — `DeleteSubtask`
wire already shipped in 0019; ADR-0014 unamended). All re-enter at `working` (TUI-only).

- **S3-a (item 1 — `ui/mod.rs` only):** move the today date INTO the Tasks list as a full-width,
  non-selectable separator **header row** (drop the above-border `Constraint::Length(1)`
  `Paragraph` slot; give the whole area to the bordered list). Render both the date row and the
  "Older tasks" separator via a shared full-inner-width `separator_line(label, inner)` helper
  (border-aware inner width = pane width − 2). Selection must skip the date row — prepend-at-draw
  with a `ListState` selected-index offset preferred (keeps `visible_rows`/selection indices
  untouched). No new hotkey → help overlay untouched (skip the learned-0015/0019 width check).
- **S3-b (item 3 — `task_list.rs`):** stop force-collapsing the older group in `visible_rows`;
  emit its sub-task rows when expanded. Older rows resolve collapse as
  `collapse_overrides.get(&id).copied().unwrap_or(true)` (default collapsed, `x`-toggleable).
  Keep `is_older` for the `+` indicator; do **not** write `collapse_overrides` from the older
  path (A7 preserved). Grouping / completed-last / `h`-hide unchanged.
- **S3-c (item 2 — `task_list.rs`):** replace `confirming_delete: Option<String>` with
  `Option<DeleteTarget>` (`Task { task_id }` | `Subtask { task_id, subtask_id }`); `arm_delete`
  arms by selected-row kind (separator → no-op); `confirm_delete` dispatches
  `DeleteTask`/`DeleteSubtask` (both already wired). Two-step confirm affordance unchanged.
- **S4 (tester):** un-strand `common/mod.rs` for the `confirming_delete` type change; **update the
  assertions that pinned the OLD forced-collapse + the above-pane centered header** (now wrong);
  add pins for older-group `x`-toggle emitting sub-task rows, sub-task delete confirm+issue, and
  the full-width separators. No `Client`/`ClientRequest` surface change this cycle.

## Log / comments

- [ ] 2026-07-02 [human] Filed from an operator interface-improvements request; see acceptance above.
- 2026-07-02 [orchestrator] Claimed → `working`. Worktree cut from `main` @ b059865 (carries
  ADR-0014 + plan). Branch `feature/0020-tui-tasks-pane-rendering-overhaul`. Session drive-0020.
- 2026-07-02 [contract-owner] S1 done. Added `MAX_TASK_LIST_LIMIT: u32 = 500` and
  `TaskListQuery { limit, offset }` (both `Option<u32>`, `skip_serializing_if`, `Default`) to
  `contract::task`; re-exported at the crate root. Query-param (de)serialization via
  `serde_urlencoded` (dev-dep, already in lock via reqwest); doctests + `tests/task.rs` cover
  all-`None`→empty, `limit`-only omits `offset`, round-trip, ceiling value. No `Task`/`Subtask`/
  `TaskStatus`/create/update DTO touched (#3). Gates: fmt/build/lint/test all green.
- 2026-07-02 [server-dev] S2 done. `list_tasks` extracts `Query(TaskListQuery)`; absent `limit`→
  ceiling, `offset`→0, over-ceiling `limit`→`400 validation_failed` (no clamp); `LIMIT $2 OFFSET
  $3` bound via `i64::from` (no `as`). Response unchanged; no migration; no status ORDER BY.
  `.sqlx/` regenerated via `./ok.sh prepare` (docker DB up). Gates: fmt/build/lint/test green.
- 2026-07-02 [tui-dev] S3 done. `TASK_LIST_LIMIT=200` (tui-local); threaded `TaskListQuery` onto
  `ClientRequest::ListTasks` + `Client::list_tasks` + reqwest `.query()` + worker; all 6 `ListTasks`
  sites send limit=200/offset=0. Completed-last stable sort (open<done) at both levels; today/older
  split by UTC day (`day_number`, epoch-secs — no chrono dep, A8) with "Older tasks" separator;
  older forced-collapsed render-time (not `collapse_overrides`); `h`→`ToggleHideOlder` + map_key +
  help 2nd Tasks line (66≤70 inner, no re-wrap); today date header (weekday/month/ordinal/year)
  top-center Tasks pane only. Gates: fmt + `clippy --lib --bins` + `./ok.sh build` green; doctests
  pass. `--all-targets` (tests) expected-RED pending tester S4 (harness stranding, learned 0019) —
  NOT a clause-1/2 pass. Assumption: today = UTC civil day (A5-note, keeps tui chrono-free).
- 2026-07-02 [tester] S4 done. Un-stranded `tests/common/mod.rs`: fake `list_tasks(query)`,
  worker-arm, `Call::ListTasks{limit,offset}`, `hide_older:false` initializers; added `today_at`
  plus `today_open_task`/`today_done_task` builders so wall-clock-driven flows land tasks in the
  today group (existing suites switched off fixed past dates). New tui tasks.rs coverage:
  completed-last (task + sub-task levels; re-sorts after a toggle with no extra fetch), today/older
  split + separator, older forced-collapsed regardless of status, `h` toggle + `visible_rows`/
  selection skip, today header present in Tasks / absent in Notes+Profiles, ordinal_suffix (incl
  11-13→th) + today_header formatting, `limit=200`/offset 0 on the wire. dialogs.rs: 2nd Tasks
  help line pins `h hide older` against flush-left re-wrap (learned 0015/0019). Server tasks.rs
  (live throwaway Postgres, docker confirmed): default whole-list newest-first, limit caps, offset
  skips, limit+offset window, over-ceiling→400 validation_failed, at-ceiling ok, profile-scoping
  under limit. Gates: `./ok.sh fmt --check` + `lint` (--all-targets, now GREEN) + `test` all green.
- 2026-07-02 [reviewer] Cold review → `REVIEW-STATUS: approved`. All gates verified green
  independently (fmt/build/lint/test, --all-targets). Hard constraints #1–#4 upheld; full ADR-0014
  conformance (additive limit/offset, bare-array response, over-ceiling→400, TUI-side stable
  completed-last sort, `i64::from` no `as`). UTC-civil-day "today" deviation (no tui timezone dep,
  A5/A8) judged within the AFK smallest-change + recorded-assumption policy — not a blocker. Help
  line 66≤70 (no re-wrap); harness genuinely un-stranded. Verdict pinned to code-tree hash
  `25ed4351d5beedb2d4f0cc517e3bdd867389cedc` (head `ec79791`). Two out-of-scope ideas filed on
  `main` for human triage (local-date grouping + ADR/plan doc-consistency; standing secret-Debug
  note).
- 2026-07-02 [verifier] Live clause-4 pass → `VERIFY-STATUS: verified`. Booted the stack
  (`./ok.sh up`); migrate one-shot a no-op (0020 adds no migration; no history conflict). Live API:
  default list newest-first, `limit` caps, `offset` skips, window works, `limit=500` ok, `limit=501`
  → `400 {"code":"validation_failed"}`, cross-profile → `404` (scoping #4 holds). Drove the shipped
  reqwest `HttpClient` end-to-end (`list_tasks(200,0)` ok; `limit=501` decodes to
  `ErrorCode::ValidationFailed`). OTel `list_tasks` spans emitted with user_id/profile_id. TUI
  `TestBackend` suite exists + green (ADR-0003). Teardown `./ok.sh down` (volume preserved). Pinned
  to code-tree hash `25ed4351d5beedb2d4f0cc517e3bdd867389cedc` (head `c1099b0`). Minor inferred edge
  (empty-string `?limit=` → plain-text 400, unreachable by shipped client) filed as an idea.
- 2026-07-02 [orchestrator] Step-7 freshen: rebased onto `main` @ bb9e260 (docs/`.claude`/board
  only — eng-manager learnings + dashboard + ideas 0009/0010). One expected conflict on this Board
  file resolved in favour of the branch. `./ok.sh code-hash` **unchanged** at
  `25ed4351d5beedb2d4f0cc517e3bdd867389cedc` — code byte-identical, so the `approved`+`verified`
  verdicts carry forward untouched (no relabel). Gates re-run green on the rebased tree
  (fmt/lint/test). → `awaiting-merge`.
- [ ] 2026-07-02 [human] Adjustment 1 (render): show the today date as a **full-width separator
  row inside** the Tasks pane (like the "Older tasks" one), not a short left-aligned line above
  it — and make the separators span the whole pane width (currently left-aligned and short).
- [ ] 2026-07-02 [human] Adjustment 2 (bug): `d` does nothing on a sub-task row — pressing it
  should delete the selected sub-task (the `DeleteSubtask` wire path already exists).
- [ ] 2026-07-02 [human] Adjustment 3 (behaviour): `x` does not expand/collapse tasks in the
  "Older" group — it should toggle there too (amends acceptance #3's forced-collapse).
- 2026-07-02 [architect] Triaged the 3 adjustments → all re-enter at `working`, **no ADR** (none
  touches the wire (#2) or domain (#3): item 2 reuses the shipped `DeleteSubtask` wire; items 1/3
  are TUI render/interaction). Amended acceptance #2 (date = full-width in-pane separator row), #3
  (older defaults collapsed but `x`-toggleable), and added #7 (`d` deletes sub-task). Slice plan
  S3-a/b/c + S4 recorded under `## Plan(s)` → Amendment. Verdicts from the prior `awaiting-merge`
  are void (code will change); item re-enters review + verify after the build.
- 2026-07-02 [tui-dev] S3 re-entry done (TUI-only, `task_list.rs` + `ui/mod.rs`; no
  contract/wire/DTO touched). **Item 2 (`d` deletes sub-task):** added `enum DeleteTarget { Task |
  Subtask }`, changed `confirming_delete` to `Option<DeleteTarget>`; `arm_delete` now arms by
  selected-row kind (task → `DeleteTarget::Task`, sub-task → `DeleteTarget::Subtask{parent,
  subtask}`, separator → no-op); `confirm_delete` dispatches `DeleteTask`/`DeleteSubtask` (both
  pre-wired). Exported `DeleteTarget`; `app/mod.rs` `.is_some()`/`= None` sites unchanged.
  **Item 3 (`x` toggles older group):** replaced `is_collapsed` with a group-aware
  `resolve_collapsed(task, today_day)` (`override.unwrap_or(is_older || Done)` — today default
  status-derived, older default `true`, both `x`-overridable); factored a `push_task_rows` helper so
  both today/older loops emit sub-task rows when expanded; `toggle_collapse` and the `+`/`>` render
  indicator both use `resolve_collapsed`, so rows + render never diverge. Older path never writes
  `collapse_overrides` (A7 preserved). **Item 1 (full-width date separator row):** dropped the
  above-border `Length(1)` `Paragraph` header, gave the whole area to the bordered list, prepended
  the today date as a bold non-selectable first `ListItem` spanning the block inner width, offset the
  `ListState` selection by +1 (keeps `visible_rows`/selection untouched). Added a shared
  `separator_line(label, inner_width)` (centers label, `─`-fills to inner width) used for BOTH the
  date row and the "Older tasks" separator (`OLDER_SEPARATOR_LABEL` now the bare `"Older tasks"`,
  padded at draw). No new hotkey → help overlay untouched. Gates: `./ok.sh fmt --check` +
  `clippy --lib --bins` + `./ok.sh build` green; doctests (3) pass. `--all-targets`/`./ok.sh test`
  expected-RED until tester S4 un-strands `common/mod.rs` for the `confirming_delete` type change
  (learned 0019) — NOT a clause-1/2 pass of this slice.
- 2026-07-02 [tester] S4 re-entry done. **Un-stranded `tests/common/mod.rs`** for the S3
  `confirming_delete: Option<DeleteTarget>` type change: imported `DeleteTarget`, updated the
  `task_list_screen_confirming_delete()` builder to arm a `DeleteTarget::Task`. No
  `Client`/`ClientRequest`/`Outcome` surface change this cycle (`DeleteSubtask` already existed), so
  the fake client + worker-analogue match were untouched. **Updated the now-wrong assertions** in
  `tests/tasks.rs`: (a) the `d`-arms-confirm test now matches `DeleteTarget::Task { task_id }` instead
  of `confirming_delete.as_deref()`; (b) replaced `older_tasks_are_forced_collapsed_regardless_of_status`
  (pinned the retired hard-forced-collapse) with the amended toggle behaviour; (c) rewrote the
  today-date-header test from a bare centered-line check to the full-width in-pane separator row.
  **Added new-behaviour pins** (all via the wall-clock `today_*` builders + `TestBackend`): item 3 —
  older `x` toggle emits/re-hides `VisibleRow::Subtask` rows, today-group collapse + completed-last
  unchanged, and `x` on one older task does not bleed onto another (A7); item 2 — sub-task-row `d` arms
  `DeleteTarget::Subtask{task_id,subtask_id}`, `Enter` issues `DeleteSubtask{parent,subtask}`, a
  task-row `d`→Enter still issues `DeleteTask` (not `DeleteSubtask`), a non-confirm key while armed
  issues no delete, and `d` after skipping the separator arms the older task not the separator;
  item 1 — the today date + "Older tasks" rows render as full-width `─`-filled in-pane separators
  (inner span = a task row's inner width) and the date row carries no selection highlight (REVERSED
  lands on the first task). dialogs.rs untouched (the `?` help overlay was not changed this cycle —
  no re-wrap check needed). **Note on the prompt's "navigation disarms" wording:** the shipped
  `handle_delete_confirm_event` captures input (only `Enter` confirms, `Esc` cancels; a navigation
  key is inert and does NOT disarm), matching the notes/profiles confirm dialog — I pinned the true
  observable behaviour (no request from a non-confirm key), not a disarm. Gates: `fmt --check`,
  `lint` (--all-targets, GREEN) and `test` all green (tui tasks suite 30 passed; server tasks suite
  16 passed via the throwaway test Postgres).

## Summary

The Tasks-pane render was overhauled across all four slices, governed by [ADR-0014][adr-0014-sum]
(the one wire-shaping change: a pagination-ready `limit`/`offset` on the task-list request).
What shipped, keyed to acceptance:

1. **Completed-last.** A TUI-side **stable** sort of the snapshot re-orders `open` before `done`
   at both the task and the sub-task-within-parent level, re-derived every render so a
   complete/reopen/toggle re-orders on the next frame with **no re-fetch** (#1). The server keeps
   `ORDER BY created_at DESC` unchanged.
2. **Today date header.** The current date renders top-center **inside the Tasks pane only**
   (not Notes/Profiles), human-readable weekday/month/**ordinal**-day/year (ordinal st/nd/rd/th
   computed TUI-side, incl. 11–13 → th).
3. **Today / older split.** Tasks group by `created_at` into created-today (above) and older
   (below) with an **"Older tasks"** separator; the older group renders **collapsed regardless of
   status**, a render-time forcing kept **separate** from `collapse_overrides` (per-task `x` toggle
   semantics unchanged).
4. **`h` hides the older group** (+ separator); default shown; ephemeral `hide_older: bool` on
   `TaskListState` (#1, never persisted); added to the `?` help overlay's second Tasks line.
5. **200-task cap.** `contract::task::TaskListQuery { limit, offset }` (both `Option<u32>`,
   `skip_serializing_if`) + `MAX_TASK_LIST_LIMIT = 500`; the server clamps/validates
   (over-ceiling → `400 validation_failed`, absent `limit` → ceiling, absent `offset` → 0) via
   `i64::from` (no `as`); the TUI hard-codes a `tui`-local `TASK_LIST_LIMIT = 200` / offset 0
   across all six `ListTasks` sites.
6. **Pagination-ready (design only).** Response stays the **bare `[Task]` array**; offset
   pagination is a future caller change + at most an additive header — no wire break (ADR-0014
   §3). No new `ErrorCode`, no migration, no `TaskStatus`/`Task`/`Subtask` field (#3 confirmed).

**ADR-0014 decisions:** offset over cursor (A1); ceiling in `contract`, value in TUI; completed-last
as a TUI-side snapshot sort (A4, forced by #1); bare-array response preserved; no domain-structure
change (§5).

**Recorded assumption (A5/A8):** "today" is computed as the **UTC civil day** (epoch-seconds
`div_euclid 86400`), **not** the operator's local date, because the `tui` crate carries no timezone
dependency and adding one would be a #6/ADR event — the smallest chrono-free, deterministic-under-test
choice. The reviewer judged this within the AFK smallest-change + recorded-assumption policy. The
local-date refinement (and reconciling ADR-0014 §5 / plan "local date" wording, which still says
"local") is parked for human triage as idea `0009` (on `main`); a second minor edge (empty-string
`?limit=` returns a plain-text 400 bypassing the JSON error contract, unreachable by the shipped
client) is idea `0010`.

**Verdicts** — reviewer `REVIEW-STATUS: approved` + verifier `VERIFY-STATUS: verified`, both pinned
to code-tree hash `25ed4351d5beedb2d4f0cc517e3bdd867389cedc`. Verifier booted the stack live: default
list newest-first, limit caps, offset skips, `limit=500` ok, `limit=501` → `400 validation_failed`,
cross-profile → `404` (#4); drove the shipped reqwest `HttpClient` end-to-end; OTel `list_tasks`
spans present; TUI `TestBackend` suite green (ADR-0003).

coverage: 72.26%

[adr-0014-sum]: ../../docs/adr/0014-task-list-pagination-ready-limit.md
