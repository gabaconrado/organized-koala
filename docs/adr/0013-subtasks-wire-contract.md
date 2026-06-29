# ADR-0013: Sub-tasks wire contract — DTO, profile-scoped endpoints, FK-cascade persistence

**Status:** Accepted · 2026-06-29

## Context

[ADR-0012][adr-0012] amended hard-constraint #3 to admit **sub-tasks** — a title+status-only,
one-level child of a task. Adding sub-tasks introduces a **new wire shape**, which per
hard-constraint #2 (and ADR-0005 §8: no URI versioning, `contract` is the compatibility
authority) is itself an ADR event. This ADR settles the concrete `contract` DTO, the
profile-scoped HTTP surface, and the persistence shape, **before any code**, for Board item
[0019][feat-0019].

### Forces

- **`contract` is the single source of truth (#2).** Server and TUI consume one sub-task DTO;
  neither redefines it. The shape must be minimal (ADR-0012: title + status, no timestamps).
- **`Task` must not be reshaped.** ADR-0005 §5 and ADR-0008 froze the flat `Task` DTO; the TUI
  detail view (ADR-0010 §4) renders exactly its existing fields. Sub-tasks must travel on their
  **own** wire shape, not as a new field embedded in `Task`, so existing `Task` consumers are
  untouched and the "no field beyond the flat shape" discipline holds.
- **Profile-scoping is structural and inherited (#4 / ADR-0012 §3).** Every sub-task route must
  be reachable only through the caller's owned profile and the parent task within it; an unowned
  or cross-profile reach is `404 not_found` (ADR-0005 §4), never 403.
- **Avoid N+1 on list.** The TUI Tasks tab renders every task *and its sub-tasks* together. A
  per-task sub-task fetch would be N round-trips; the list surface must let the TUI assemble the
  tree in a bounded number of calls.
- **Reuse the existing patterns.** The route nesting, the `assert_owned` ownership gate, the
  `validation_failed` / `not_found` codes, the `RETURNING`-based handlers, and the reversible
  paired-migration discipline are all established (ADR-0005/0008/0009); the sub-task surface
  mirrors them rather than inventing new mechanics. **No new `ErrorCode`** is required.

## Decision

### 1. `contract::task::Subtask` — the title+status-only DTO

`contract` gains, in the existing `task` module:

```text
Subtask {
    id: String,             // server-generated UUID string
    task_id: String,        // parent task id (UUID string)
    title: String,          // non-empty after trimming (server-enforced)
    status: TaskStatus,     // reuses the existing open/done enum
}
```

- It **reuses the existing `TaskStatus`** enum (no second status type).
- It carries **no** `description`, `created_at`, `closed_at`, or `updated_at` (ADR-0012 §1).
- It carries `task_id` so the TUI can group sub-tasks under their parent when assembling the
  list tree (see §3); `task_id` is the parent linkage, never a profile id (scoping is via the
  parent, §4).
- snake_case JSON, UUID-string ids, lowercase-string status — the ADR-0005 §1 scalar
  conventions, unchanged.

### 2. Request DTOs — create and edit-title

`contract::task` gains two request bodies, mirroring the task equivalents:

```text
CreateSubtaskRequest { title: String }                 // POST body; status starts `open`
UpdateSubtaskRequest {                                 // PATCH body, all-optional partial
    title:  Option<String>,   // skip_serializing_if = Option::is_none
    status: Option<TaskStatus> // skip_serializing_if = Option::is_none
}
```

- **Create** carries only `title` (non-empty after trimming, else `400 validation_failed`); a new
  sub-task always starts `status: open` (server default), so create takes no status.
- **Edit-title and toggle share one all-optional `PATCH`** (the ADR-0008 partial-update pattern):
  the TUI's `e` (edit title) sends `{ title }`; its `Space` (toggle) sends `{ status }`. A present
  `title` must be non-empty after trimming; `status: done`/`open` flips the lifecycle. An empty
  patch is a no-op returning the sub-task unchanged. This keeps the surface to **one** mutation
  route rather than separate `/rename` + `/toggle`. (A sub-task has no `closed_at`, so the
  status→timestamp coupling of ADR-0008 §2 does **not** apply — `status` is a plain column.)

### 3. HTTP surface — profile + parent-task scoped, plus a flat per-profile list

Routes nest under the **existing** profile+task path so ownership is structural:

| Method & path | Body → result | Notes |
| --- | --- | --- |
| `GET /api/profiles/{pid}/tasks/{tid}/subtasks` | → `200 [Subtask]` | one parent's sub-tasks, **creation order** (`created_at ASC` internally; `created_at` is not exposed) |
| `POST /api/profiles/{pid}/tasks/{tid}/subtasks` | `CreateSubtaskRequest` → `201 Subtask` | starts `open` |
| `PATCH /api/profiles/{pid}/tasks/{tid}/subtasks/{sid}` | `UpdateSubtaskRequest` → `200 Subtask` | edit title and/or toggle status |
| `DELETE /api/profiles/{pid}/tasks/{tid}/subtasks/{sid}` | → `204 No Content` | a second delete or unowned/missing → `404` |
| `GET /api/profiles/{pid}/subtasks` | → `200 [Subtask]` | **all** sub-tasks in the profile, for the Tasks-tab tree load (avoids N+1) |

- **The flat per-profile list** (`GET /api/profiles/{pid}/subtasks`) is the load the TUI Tasks tab
  uses: it fetches the profile's tasks (existing route) **and** all its sub-tasks in **two** calls,
  then groups sub-tasks under parents by `task_id` client-side. This bounds the list load at two
  round-trips regardless of task count. The per-task list (`…/tasks/{tid}/subtasks`) backs the Task
  Detail page's "Sub-tasks" section.
- **`Task` and its routes are unchanged.** No field added to `Task`; the existing
  `GET/POST .../tasks` and `PATCH/DELETE .../tasks/{tid}` keep their ADR-0005/0008 shapes.

### 4. Profile-scoping and parent-scoping are enforced in the query (#4, ADR-0012 §3)

Every sub-task handler passes the **existing `assert_owned(profile_id)`** gate first (ADR-0005 §4),
then every sub-task query is **joined to its parent task and the parent's `profile_id`**, so:

- the parent task must exist **and** belong to `{pid}` — else `404 not_found`;
- a sub-task is matched only when `subtasks.task_id = {tid}` **and** the task's `profile_id = {pid}`.

There is no query path that reaches a sub-task without its parent task's profile matching the
caller's owned profile. The per-profile list (`GET …/subtasks`) selects `subtasks` joined to
`tasks WHERE tasks.profile_id = $pid`. Unowned/cross-profile is indistinguishable from absent
(`404`), exactly as for tasks.

### 5. Persistence — a `subtasks` table with an FK cascade to its parent task

A **reversible** migration (paired `*.up.sql` / `*.down.sql`, the standard) adds:

```text
subtasks (
    id         UUID  PK DEFAULT gen_random_uuid(),
    task_id    UUID  NOT NULL REFERENCES tasks (id) ON DELETE CASCADE,
    title      TEXT  NOT NULL,
    status     TEXT  NOT NULL DEFAULT 'open' CHECK (status IN ('open','done')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()   -- ordering only; NOT exposed on the wire
)
INDEX subtasks_task_id_created_at_idx ON subtasks (task_id, created_at)
```

- **`ON DELETE CASCADE` to `tasks`** satisfies the cascade-delete acceptance (ADR-0012 §4): deleting
  a task removes its sub-tasks; deleting a profile cascades `tasks` (existing FK) which transitively
  cascades `subtasks` — no orphans, enforced by the schema, not handler discipline.
- **`created_at` is internal**: it exists only to give a stable creation-order sort; it is **not**
  in the `Subtask` DTO (ADR-0012 §1 — a sub-task has no exposed timestamps). The index keys list
  reads on `(task_id, created_at)`.
- **No nesting column.** There is no `parent_subtask_id`; the schema structurally enforces one
  level of nesting (ADR-0012 §2) — a sub-task references a `task`, never another `subtask`.
- The committed `.sqlx/` query cache is refreshed (`./ok.sh prepare`) for the new static queries.

### 6. No new error code; no URI versioning

The surface reuses `validation_failed` (blank title) and `not_found` (unowned/missing profile,
task, or sub-task). No new `ErrorCode` variant. No `/v1` prefix (ADR-0005 §8); `contract` is the
compatibility authority and the sole consumer (the in-repo TUI) migrates in the same item.

## Consequences

- **`contract` adds** `Subtask`, `CreateSubtaskRequest`, `UpdateSubtaskRequest` in the `task`
  module; `Task`, `TaskStatus`, `CreateTaskRequest`, `UpdateTaskRequest` are **unchanged**. The
  error-code set is unchanged.
- **`server` adds** a `subtasks` table (reversible migration with the FK cascade), five handlers
  on the nested routes, and refreshes `.sqlx/`. The `tasks` schema and handlers are untouched.
- **`tui` adds** five `Client` methods + `ClientRequest`/`Outcome` variants, the Tasks-tab
  two-call tree load (tasks + per-profile sub-tasks), the indented list rendering with the `+`/`>`
  indicator, the `A`/`e`/`Space`/`x` handling, and the Task Detail "Sub-tasks" section. It defines
  **no** DTO of its own (#2).
- **Collapse state stays client-side** (ADR-0012 §5): no DTO field, no route, no server storage —
  the initial state derives from parent status each render, `x` overrides in-session.
- **Reversibility.** Dropping the `subtasks` table (its `down.sql`) and removing the DTO + routes +
  client methods returns the system to its pre-0019 shape; `tasks`/`notes`/`profiles` are untouched.
- **Risk.** The blast radius is real but contained: a new table + five endpoints (server), a new
  list-assembly + render path + four keybindings (tui). No existing wire shape changes, so existing
  task/note/profile flows carry no regression risk beyond the shared keymap (`A` and `x` are newly
  bound — the plan confirms no live collision).

[feat-0019]: ../../board/features/0019-task-subtasks.md
[adr-0012]: ./0012-subtasks-domain-exception.md
