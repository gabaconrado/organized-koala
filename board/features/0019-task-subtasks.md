---
id: 0019
title: Sub-tasks — flat title/status children of a task, with TUI list nesting + collapse
type: feature      # feature | chore
status: working          # inbox → planned → ready → working → review → awaiting-merge → merged | blocked
priority: medium    # high | medium | low
parent: null
depends-on: [0016]  # builds on the task detail view + final hotkey scheme (merged)
branch: feature/0019-task-subtasks
worktree: .claude/worktrees/0019-task-subtasks
created: 2026-06-29
updated: 2026-06-29
---

## Feature request

**Goal:** Add **sub-tasks** to the TODO feature. A sub-task is a deliberately-minimal child
of a task: it has **only a Title and a Status** — *no* Description, *no* Created-at, and *no*
detailed view of its own. Sub-tasks are created, edited, toggled, and collapsed from the
**Tasks tab** and the **Task Detail page**.

> **⚠️ Hard-constraint #3 (ADR-worthy).** `CLAUDE.md` explicitly lists **subtasks** as
> structure the flat domain does *not* have — *"TODO = {Title, Description, Status,
> Created-at, Closed-at} … **Do not** add structure (subtasks, …) without an ADR."* The
> operator has decided to add it anyway (this card is that decision). The `architect` must
> therefore **author an ADR** that amends the flat-domain constraint before any code, and the
> `contract` change it implies is itself an ADR event (#2). Planning starts there.

### Behaviour (acceptance)

1. **Create — `A` (capital A).** With a task selected (Tasks-tab list **or** its Task Detail
   page), pressing **`A`** creates a new sub-task **for that task**. (`a` remains "add task";
   `A` is the new "add sub-task" key.) The new sub-task starts at the open/not-done status.
2. **Shape — title + status only.** A sub-task carries **only** `title` and `status`. It has
   **no** description, **no** `created_at`, and **no** detail view — selecting one never opens
   a per-field pane.
3. **Edit — `e`.** With a sub-task selected, **`e`** edits its **title** (same edit lifecycle
   as a task title: commit / cancel per the 0016 scheme).
4. **Toggle — `Space`.** With a sub-task selected, **`Space`** toggles its status open ↔ closed
   (done/undone), mirroring the task toggle.
5. **Sub-tasks section in the Task Detail page.** The Task Detail page (0016) gains a **new
   "Sub-tasks" section** listing the task's sub-tasks, each showing its **title and status**.
6. **Indentation in the list.** Sub-tasks are rendered **indented one level** under their
   parent task in the Tasks-tab list.
7. **Collapse / expand — `x`.** Sub-tasks can be **collapsed** (hidden in the list) or
   **expanded** under their parent. **`x`** toggles collapse/expand for the selected task.
   **Defaults:** an **open** parent task shows its sub-tasks **expanded**; a **closed** parent
   task shows them **collapsed**.
   - *(Note: `x` was the old "delete" key pre-0016; under the 0016 final scheme delete is `d`.
     This card binds `x` = toggle-collapse. Planning confirms there is no live `x` collision.)*
8. **List indicator.** A task's list indicator is **`+`** when it has sub-tasks **and** they
   are **collapsed**; otherwise (expanded, or no sub-tasks) it stays **`>`**.
9. **Cascade delete.** Deleting a task **automatically deletes all of its sub-tasks**. No
   orphaned sub-tasks remain.

### Surface this is expected to touch (for planning, not binding)

- **`contract`** — a new sub-task wire type (`{ id, title, status, parent task id }`) plus the
  request/response shapes for create / edit-title / toggle / list. Single source of truth (#2).
- **`server`** — persistence (a `subtasks` table scoped under its parent task, itself
  profile-scoped per #4), endpoints for the four operations, and **`ON DELETE CASCADE`** (or
  equivalent in the delete handler) so removing a task removes its sub-tasks. Reversible
  migration (`*.up.sql` / `*.down.sql`).
- **`tui`** — `A` / `e` / `Space` / `x` handling scoped to the Tasks context, indented list
  rendering, the `+` vs `>` indicator, collapse state, and the Task Detail "Sub-tasks" section.

### Open questions for the architect / `grill`

- **Are sub-tasks themselves profile-scoped, or scoped purely via their parent task?** (#4 says
  every TODO is profile-scoped; a sub-task inherits its parent's profile — confirm the query
  shape enforces no cross-profile reach.)
- **Where does collapse state live?** It is presentation state — does it belong server-side, or
  is it TUI-local? (#1 says the TUI is stateless and holds *no* local persistence; reconcile
  the per-task collapse default with that constraint — likely derived from parent status each
  render rather than persisted.)
- **Can a sub-task have sub-tasks?** Assumed **no** — exactly one level of nesting. Confirm.
- **Does closing a parent auto-collapse already-expanded children, or only set the initial
  default?** ("By default" suggests the latter — an initial state, with `x` then free to
  override — but this needs to be pinned down.)
- **Ordering** of sub-tasks within a parent (creation order assumed).

## Log / comments

- [ ] 2026-06-29 [human] Filed: add sub-tasks. See acceptance points 1–9 above. I know I
  said no subtasks at the start — I want them now; treat the ADR amending hard-constraint #3
  as part of this work.

## Plan(s)

### Plan: Sub-tasks — flat title/status children of a task, list nesting + collapse

**Approach (tracer-bullet, then widen).** Build the thinnest end-to-end slice first: the
`Subtask` DTO in `contract`, the `subtasks` table + the **create** and **per-profile list**
endpoints in `server`, the matching `Client` methods + the Tasks-tab two-call tree load and
indented rendering in `tui` — so a real `A`-create flows TUI → contract → server → Postgres and
the new sub-task renders indented under its parent on the next list load. That proves every seam
(new table, FK cascade, profile-scoped join, list assembly, indented render). Then widen with the
remaining mutations (edit-title, toggle, delete + cascade), the `+`/`>` indicator and `x`
collapse, and the Task Detail "Sub-tasks" section. Single Board item, sliced by crate +
dependency order; the contract→server→tui dependency chain is strict (each consumes the prior).

**ADR:** **ADR-0012** (amends hard-constraint #3 to admit sub-tasks as a bounded, one-level,
title+status-only exception) **and ADR-0013** (the wire contract: `Subtask` DTO, the
profile+parent-scoped endpoints, the reversible `subtasks` migration with the `ON DELETE CASCADE`
to `tasks`). **Both authored and committed to `main` before the worktree is cut.**

**Slices (dependency order; each bounded by crate ownership):**

1. **[contract-owner]** Add the sub-task wire types to the `task` module (ADR-0013 §1–2) — files:
   `crates/contract/src/task/mod.rs` (+ `crates/contract/src/task/tests.rs` only if private logic
   appears; this is a pure-DTO crate so the crate-root `tests/` public-API suite + doctests are the
   correct home — `tester` owns those). Adds `Subtask { id, task_id, title, status }` (reusing
   `TaskStatus`), `CreateSubtaskRequest { title }`, and `UpdateSubtaskRequest { title?, status? }`
   (each `Option` field `skip_serializing_if = "Option::is_none"`; derive `Default`). Re-export
   from `crates/contract/src/lib.rs` alongside the existing task types. snake_case / UUID-string /
   lowercase-status conventions (ADR-0005 §1). Each public type derives `Debug` + carries a
   doctest (rust-standards). **No change to `Task`/`TaskStatus`/`CreateTaskRequest`/
   `UpdateTaskRequest`.**

2. **[server-dev]** Persistence + endpoints (ADR-0013 §3–6) — files:
   `crates/server/migrations/<ts>_subtasks.up.sql` + `<ts>_subtasks.down.sql` (new, paired
   reversible: `subtasks` table with `task_id UUID NOT NULL REFERENCES tasks(id) ON DELETE CASCADE`,
   `title TEXT`, `status TEXT CHECK (status IN ('open','done')) DEFAULT 'open'`, internal
   `created_at TIMESTAMPTZ DEFAULT now()`, index on `(task_id, created_at)`; `down.sql` =
   `DROP TABLE subtasks`); `crates/server/src/handlers/subtasks.rs` (new); register routes in
   `crates/server/src/app.rs`; declare the module in `crates/server/src/handlers/mod.rs`; refresh
   `.sqlx/` via `./ok.sh prepare`. Five handlers, each passing the existing `assert_owned(pid)` gate
   then a query **joined to `tasks` on `task_id` AND `tasks.profile_id = $pid`** (ADR-0013 §4):
   - `GET  /api/profiles/{pid}/tasks/{tid}/subtasks` → `200 [Subtask]`, creation order.
   - `POST /api/profiles/{pid}/tasks/{tid}/subtasks` → `201 Subtask`, starts `open`; blank title →
     `400 validation_failed`; missing/unowned parent task → `404`.
   - `PATCH /api/profiles/{pid}/tasks/{tid}/subtasks/{sid}` → `200 Subtask`, all-optional partial
     (title and/or status); empty patch is a no-op; blank title → `400`; missing → `404`.
   - `DELETE /api/profiles/{pid}/tasks/{tid}/subtasks/{sid}` → `204`; second/missing → `404`.
   - `GET  /api/profiles/{pid}/subtasks` → `200 [Subtask]` (all the profile's sub-tasks, for the
     Tasks-tab tree load; selects `subtasks` joined to `tasks WHERE tasks.profile_id = $pid`).

   No new `ErrorCode`; reuse `validation_failed` / `not_found`. `tasks`/`notes`/`profiles` schema
   and handlers untouched. Cascade-on-task-delete needs **no** handler code — it is the FK.

3. **[tui-dev]** Client boundary + interaction + rendering (ADR-0013 §3, ADR-0012 §5) — files:
   `crates/tui/src/client/mod.rs` (five `Client` trait methods + `HttpClient` impls, mirroring the
   task methods); `crates/tui/src/app/protocol.rs` (`ClientRequest` + `Outcome` variants for the
   five calls); `crates/tui/src/client/worker.rs` (map each new `ClientRequest` to its client call);
   `crates/tui/src/app/mod.rs` (new `Event` variants: `BeginAddSubtask`, `ToggleCollapse`; thread
   apply_response folding for the new outcomes); `crates/tui/src/app/task_list.rs` (hold the
   profile's `Vec<Subtask>` alongside `tasks`, the two-call tree load on list refresh, per-parent
   in-session collapse override map, `A`/`e`/`Space`/`x` handling **scoped to the Tasks context**,
   selection model spanning task + visible sub-task rows); `crates/tui/src/app/task_detail.rs`
   (the "Sub-tasks" section listing title+status; sub-task rows are **not** focusable detail panes —
   a sub-task has no detail view, ADR-0012 §1); `crates/tui/src/terminal/mod.rs` (`map_key`: bind
   `A` (Shift+a) → `BeginAddSubtask`, `x` → `ToggleCollapse`, both Tasks-tab/idle only; confirm no
   live collision — see Risks); `crates/tui/src/ui/mod.rs` (indent sub-task rows one level under
   their parent; parent indicator `+` when it has collapsed sub-tasks, else `>`).
   - **Collapse default derives from parent status each render** (ADR-0012 §5): open parent →
     expanded, closed parent → collapsed, **unless** the in-session override map has an entry for
     that parent id (set by `x`). The override map is transient process-lifetime UI state (#1) —
     never persisted, keyed by task id, reset on a fresh list load for a task no longer present.
   - **`e` and `Space` act on a sub-task when a sub-task row is selected**, on the task when a task
     row is selected (the existing task `e`/`Space` paths stay for task rows). `A` always adds a
     sub-task to the *parent* task of the current selection (the selected task, or the selected
     sub-task's parent). Defines **no** DTO of its own (#2).

4. **[tester]** Tests across all three crates — files: `crates/contract/tests/…` (sub-task DTO
   ser/de round-trips, `skip_serializing_if`, empty-patch `{}`); `crates/server/tests/…`
   (integration over the public API: create/list/edit/toggle/delete; **profile-scoping** — a
   sub-task under another profile's task is `404`; **parent-scoping** — wrong `{tid}` is `404`;
   **cascade** — deleting a parent task removes its sub-tasks, deleting a profile removes them
   transitively; creation-order list; blank-title `400`); `crates/tui` `TestBackend` suite via the
   `common` harness with the fake `Client` (ADR-0003 layer-2): `A` create, `e` edit-title, `Space`
   toggle, `x` collapse/expand override, the `+`/`>` indicator, indented render, the Detail
   "Sub-tasks" section, collapse-default-from-parent-status, selection traversal over task +
   sub-task rows. Every cited coverage names a real test (coding-standards).

**Assumptions (ambiguity policy — every fork resolved here):**

- **A1 — Sub-tasks are profile-scoped *via their parent task*, not independently** (#4 / ADR-0012
  §3). Every query joins `subtasks → tasks` and filters `tasks.profile_id = $pid`; no `profile_id`
  column on `subtasks`. Cross-profile/parent reach is `404` (ADR-0005 §4).
- **A2 — Collapse state is TUI-local, derived, transient** (#1 / ADR-0012 §5). No server storage,
  no DTO field. Initial state derived from parent status **each render**; `x` sets an in-session,
  process-lifetime override. Card point 7's "Defaults" is read as the *initial* render state, not
  persisted state — reconciled with #1 by deriving it rather than storing it.
- **A3 — Exactly one level of nesting** (ADR-0012 §2): a sub-task cannot have sub-tasks; the
  schema has no `parent_subtask_id`, structurally enforcing it.
- **A4 — Closing a parent sets the *initial* collapse default only; it does not forcibly collapse
  an already-overridden expansion.** Resolving card open-question 4: "by default" = the derived
  initial state; once the user presses `x` on a parent, that in-session override wins until the
  override is cleared (a fresh load drops overrides for absent task ids). A toggle-done that flips a
  parent's status changes the *derived* default, but an explicit prior `x` override for that parent
  still takes precedence (last explicit user intent wins).
- **A5 — Sub-tasks ordered by creation order** (ADR-0013 §3): `created_at ASC` internally;
  `created_at` is **not** exposed on the wire.
- **A6 — Tasks-tab list loads sub-tasks in one extra call** via `GET …/subtasks` (ADR-0013 §3),
  grouped under parents by `task_id` client-side — two round-trips total, no N+1. The Task Detail
  "Sub-tasks" section uses the per-task `GET …/tasks/{tid}/subtasks` (it is already focused on one
  task) **or** filters the already-loaded profile set — `tui-dev`'s choice, bounded to no new wire.
- **A7 — `A` = Shift+a is the add-sub-task key; `x` = toggle-collapse.** Per the card, `a` stays
  add-task, `A` adds a sub-task; `x` (freed when 0016 remapped delete to `d`) is collapse. The plan
  *requires* `tui-dev` to confirm no live binding owns `A`/`x` on the Tasks tab before wiring (see
  Risks R1).
- **A8 — A sub-task has no detail view** (ADR-0012 §1): selecting one and pressing `Enter` does
  **not** open a per-field pane; `Enter` on a sub-task row is inert (or, `tui-dev`'s call, opens the
  *parent* task's detail — default: inert, smallest behaviour).
- **A9 — Single Board item.** The work is cohesive and strictly ordered (contract → server → tui →
  tests); splitting would create cross-branch contract churn. Kept as one well-sliced item.

**Risks:**

- **R1 — Keymap collision (`A`, `x`).** `x` was pre-0016 delete; 0016 remapped delete → `d`, so `x`
  should be free, and `A`/Shift+a is new. **Mitigation:** `tui-dev` greps `map_key` and the keymap
  tests for any live `A`/`x` binding on the Tasks tab before wiring; the `map_key` keybinding tests
  pin the new bindings so nothing silently regresses. If a live collision exists, it is a genuine
  fork → block and ask. (Card point 7 explicitly asks planning to confirm this.)
- **R2 — Selection model complexity.** The Tasks list now interleaves task and sub-task rows with
  collapse hiding some; selection must traverse only *visible* rows and know whether the cursor is
  on a task or a sub-task (routing `e`/`Space`/`A`). Blast radius is contained to `task_list.rs` +
  `ui/mod.rs`; the `TestBackend` suite must cover traversal across a collapsed parent.
- **R3 — Two-call list consistency.** Tasks and sub-tasks load in two requests; a sub-task whose
  parent is absent from the task list (a race) must render safely (dropped/ignored), not panic.
  Mitigation: group defensively by `task_id`, ignore orphans in the view.
- **R4 — Cascade correctness.** The no-orphans guarantee rests entirely on the FK
  `ON DELETE CASCADE`; the reviewer must confirm the down-migration drops the table and the
  `tester` integration test exercises both task-delete and profile-delete cascade paths.
- **R5 — Grill candidate (noted, resolved via Assumptions under AFK).** The collapse-override vs.
  derived-default interaction (A4) and the selection traversal (R2) are the two spots a `grill`
  would harden. Under the AFK posture they are resolved by the Assumptions above; if the operator
  prefers, a `grill` on A4/R2 before `tui-dev` starts would de-risk the interaction model. Not
  blocking.

**Self-acceptance:** plan + both ADRs reviewed against CLAUDE.md hard constraints (#1 stateless —
collapse is derived/transient; #2 contract is sole source — TUI defines no DTO; #3 amended by
ADR-0012 to a bounded exception; #4 profile-scoping structural via parent join; #5/#6 untouched)
and the feature-track DoD. No genuine fork remains open. → `status: ready`.

- 2026-06-29 [orchestrator] Claimed → `working`. Worktree `.claude/worktrees/0019-task-subtasks`
  / branch `feature/0019-task-subtasks` cut from `main@e80909b` (carries ADR-0012/0013 + plan).
  Build order per plan: contract-owner → server-dev → tui-dev → tester. Session: drive/0019.
- 2026-06-29 [contract-owner] Slice 1 done: added `Subtask`/`CreateSubtaskRequest`/
  `UpdateSubtaskRequest` to `crates/contract/src/task/mod.rs` (ADR-0013 §1–2), re-exported from
  `lib.rs`; `Task`/`TaskStatus`/`CreateTaskRequest`/`UpdateTaskRequest` untouched. build/lint/
  fmt --check clean.
- 2026-06-29 [server-dev] Slice 2 done: paired reversible migration
  `20260612163051_subtasks.{up,down}.sql` (`subtasks` table, `task_id` FK to `tasks`
  ON DELETE CASCADE — the no-orphans guarantee R4, `status` CHECK open/done, internal
  `created_at`, index on `(task_id, created_at)`; down = DROP TABLE). New
  `crates/server/src/handlers/subtasks.rs` with five handlers (list per-task, create, patch,
  delete, list per-profile), declared in `handlers/mod.rs`, routed in `app.rs`. Each passes
  `assert_owned(pid)` then a query joined `subtasks → tasks` on `task_id` AND
  `tasks.profile_id = $pid` (A1): cross-profile/wrong-parent reach is `404`, indistinguishable
  from absent. Reuses `validation_failed`/`not_found`; no new `ErrorCode`. `created_at` never
  on the wire (A5). `tasks`/`notes`/`profiles` untouched. `./ok.sh prepare` ran (docker
  available; throwaway test Postgres) — `.sqlx/` refreshed with 5 new query files.
  build/lint/fmt --check clean; `./ok.sh test` green (subtask integration tests are tester's
  slice).
- 2026-06-29 [tui-dev] Slice 3 done: client boundary + interaction + rendering (ADR-0013 §3,
  ADR-0012 §5). R1 collision-check (grepped `map_key` + the keybinding tests/handlers): no live
  `A`/`x` on the Tasks-tab idle context — `keybindings.rs` asserts `x` maps to `None` ("x no
  longer deletes") and `A`/Shift+a is bound nowhere; the only other `x` hits are in text-entry /
  in-flight contexts (typed literally). No collision; proceeded. Added five `Client` methods
  (`list_subtasks`, `list_task_subtasks`, `create_subtask`, `update_subtask`, `delete_subtask`)
  plus their `HttpClient` impls; `ClientRequest`/`Outcome` variants for each; worker arms;
  `Event::BeginAddSubtask`/`ToggleCollapse`; `apply_response` folds (two-call tree load chains
  `ListTasks`→`ListSubtasks`; create/edit/toggle refresh the tree). `task_list` holds
  `subtasks: Vec<Subtask>` + a transient per-parent `collapse_overrides` map (#1, dropped on
  fresh load for absent task ids); a `VisibleRow` selection model traverses only visible rows;
  `A`/`e`/`Space`/`x` scoped to the Tasks context (`e`/`Space` route to a selected sub-task row,
  else the task; `A` adds to the selection's parent). Collapse derives from parent status each
  render (open→expanded, done→collapsed) unless an `x` override exists (A4). Defensive grouping
  by `task_id` (orphans dropped, R3). Task Detail gains a read-only "Sub-tasks" section
  (title+status, not focusable, A8); sub-task rows indented one level; parent indicator `+` only
  when has-subtasks AND collapsed, else `>`. `FOOTER_CAPTION` unchanged (bottom-band coupling
  invariant held). Defines no DTO of its own (#2). `./ok.sh build` clean; clippy clean on
  `tui --lib --bins`; `./ok.sh fmt --check` clean. `./ok.sh lint`/`test` (`--all-targets`) await
  tester's slice-4 harness update (the fake `Client` + worker-analogue executor track the trait;
  not edited here, per crate ownership).
- 2026-06-29 [tester] Slice 4 done: tests across all three crates + un-stranded the tui harness.
  (a) Harness: `crates/tui/tests/common/mod.rs` gains the five sub-task `Client` methods on the
  fake, their `Call` variants + `push_*` queues, the five worker-analogue executor arms, the new
  `TaskListState` fields (`subtasks`/`collapse_overrides`/`adding_subtask`/`editing_subtask`) in the
  screen builders, and `open_subtask`/`done_subtask` DTO builders; the two auto-chained tree-load
  list calls (`list_subtasks`/`list_task_subtasks`) default to an empty list when unscripted (the
  natural "no sub-tasks" state for the many flows that aren't about sub-tasks), while the mutating
  sub-task calls keep the strict panic-on-empty net. Threaded the new tree-load chain through the
  existing detail/flows/tasks/profiles suites (an `open_task_detail` helper; `ListTasks`→
  `ListSubtasks` tail assertions). `keybindings.rs` now pins `A`→`BeginAddSubtask` and `x`→
  `ToggleCollapse` on the Tasks tab (was asserting `x`→None pre-0019). (b) Coverage: new
  `crates/contract/tests/subtask.rs` (14 tests: DTO ser/de round-trips, `skip_serializing_if`
  title-only/status-only, empty-patch `{}`→all-`None`); new `crates/server/tests/subtasks.rs` (21
  `#[sqlx::test]`: create/list-per-task/list-per-profile creation-order/edit-title/toggle/delete,
  blank-title 400, empty-patch no-op, parent-scoping wrong-`{tid}`→404, profile-scoping
  cross-profile→404, cascade R4 — task-delete and profile-delete both remove sub-tasks with no
  orphan addressable); new `crates/tui/tests/subtasks.rs` (16 TestBackend tests: `A` create, `e`
  edit-title, `Space` toggle, `x` collapse/expand override, `+`/`>` indicator, indented render,
  Detail "Sub-tasks" section, collapse-default-from-parent-status, selection traversal across a
  collapsed parent R2). Gates green: `./ok.sh test` (live test Postgres via docker), `./ok.sh lint`
  (`--all-targets`), `./ok.sh fmt --check` all clean.
