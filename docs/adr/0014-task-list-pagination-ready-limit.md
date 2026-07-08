# ADR-0014: Task-list pagination-ready limit — additive `limit`+`offset`, bare-array response preserved

**Status:** Accepted · 2026-07-02

## Context

Board item [0020][feat-0020] overhauls how the TUI **Tasks pane** renders its list: completed
tasks sink below active ones, tasks split into a *created-today* / *older* grouping, an `h` key
hides the older group, and — the one **wire-shaping** change — the fetch is **bounded to 200
tasks**. Per hard-constraint #2 (a change to a wire shape is an ADR event) and [ADR-0005][adr-0005]
§8 (`contract` is the compatibility authority, no URI versioning), adding a **limit** to the
task-list request is settled here **before any code**.

[ADR-0005][adr-0005] §5 froze the task-list response as a **bare JSON array**, newest-first
(`created_at DESC`), and explicitly deferred pagination: *"No pagination envelope at personal
scale; adding one later is a breaking change and therefore an ADR anyway."* This is that ADR. The
operator's requirement is narrow and forward-looking: bound the fetch **now** with a limit the
wire carries and the server enforces, but shape it so **pagination can be added later without a
wire break** — the TUI hard-codes 200 for this feature and does not paginate.

### Forces

- **#2 — `contract` is the single source of truth.** The limit capability lives on the wire; both
  server (enforce) and TUI (choose 200) consume one shape. Neither redefines it.
- **#3 — the domain stays flat.** This change adds **no** `TaskStatus` variant and **no**
  per-task/per-sub-task field. The limit and future pagination params are **request/transport**
  concerns, not domain structure. The today/older grouping, the "Older tasks" separator,
  collapse-older, and the `h`-hide are **pure TUI-render concepts derived from `created_at`** — no
  domain change (confirmed in §5).
- **#1 — the TUI is stateless.** The chosen ordering and hide behaviour must derive from the
  server snapshot each render; the TUI holds no persistence. This steers the completed-last
  decision toward a TUI-side sort of the current snapshot (§4).
- **No wire break.** Existing consumers (the current TUI, any future client) must keep working
  after this change **and** after real pagination is added later. Both must be additive.
- **Smallest shape wins.** ADR-0005 §5 deferred pagination deliberately; this ADR adds the minimum
  that satisfies the 200-cap acceptance and leaves a clean, non-breaking path to pagination — no
  speculative envelope, no cursor machinery built before there is a real need.

## Decision

### 1. Request shape — additive `limit` + `offset` **query parameters** (offset pagination)

`GET /api/profiles/{pid}/tasks` gains two **optional** query parameters:

| Param | Type | Meaning | Absent → |
| --- | --- | --- | --- |
| `limit` | `u32`, `1..=MAX` | max tasks to return | server default (see §2) |
| `offset` | `u32`, `>= 0` | number of leading tasks to skip | `0` |

**Offset pagination is chosen over cursor pagination.** Justification:

- **Simplicity (coding-standards priority 3).** Offset+limit is a two-scalar addition over the
  existing `ORDER BY created_at DESC` query — `… ORDER BY created_at DESC LIMIT $2 OFFSET $3`. A
  cursor scheme requires an opaque, encoded cursor token, a stable tiebreak key, and cursor
  decode/validation on every request — machinery with no payoff at personal scale (a single
  user's task list), where the total is bounded and the 200-cap already covers the realistic
  ceiling.
- **Pagination-ready without a wire break (the core requirement).** Both params are **optional
  query params with server defaults**; an existing caller that sends neither (today's TUI before
  this feature, or any future client) behaves exactly as before. Adding real pagination later is
  purely a **caller** change — start sending `offset` — with **no** change to the wire *shape*.
  The cursor alternative cannot make this claim as cleanly: retrofitting a cursor onto callers
  that only know offset is itself a break.
- **Query params, not a request body.** `GET` carries no body; the params ride the URL
  (`?limit=200`). This matches ADR-0005's REST style and keeps the route path unchanged.

The `MAX` upper bound is defined in `contract` (§2) so the server rejects an over-large `limit`
uniformly.

### 2. Limit configured in `contract` + enforced by the server; the value hard-coded by the TUI

The "configurable in contract/server, hard-coded in TUI" split the operator asked for is realized
as:

- **`contract`** exposes the limit **capability and its bounds** — a public constant
  `contract::task::MAX_TASK_LIST_LIMIT: u32` (the ceiling the server enforces) and a **typed query
  struct** `contract::task::TaskListQuery { limit: Option<u32>, offset: Option<u32> }`
  (`skip_serializing_if = Option::is_none`, both optional, `serde`-(de)serializable as query
  params). This is the single source of truth for the shape and the ceiling; server and TUI both
  depend on it.
- **The server** enforces the capability: it reads `TaskListQuery` from the query string, clamps /
  validates `limit` against `MAX_TASK_LIST_LIMIT` (a `limit` above the ceiling is a
  `400 validation_failed`; an absent `limit` falls back to a **server default**, which is
  `MAX_TASK_LIST_LIMIT` — so an old no-param caller keeps getting the whole list up to the ceiling,
  preserving ADR-0005 §5 behaviour at personal scale), and applies `LIMIT`/`OFFSET` in the query.
- **The TUI caller chooses the number.** The TUI hard-codes `limit = 200` (a `tui`-local constant,
  e.g. `TASK_LIST_LIMIT: u32 = 200`) and `offset = 0`, and does not paginate in this feature. 200
  is the **caller's choice of the capability**, not a wire or server constant. (200 ≤
  `MAX_TASK_LIST_LIMIT`; the ADR sets `MAX_TASK_LIST_LIMIT = 500` as a comfortable headroom over
  the TUI's 200 — a single value the server clamps to and the TUI stays under.)

This keeps the wire honest: the capability (and its safety ceiling) is on the wire and enforced
centrally; the *policy* (fetch 200, no pagination) is the caller's.

### 3. Response shape — the bare JSON array is **preserved**; no envelope now

`GET …/tasks` **continues to return the bare `200 [Task]` array** (ADR-0005 §5), newest-first. It
is **not** wrapped in an envelope in this feature. Rationale and the non-breaking path to a
next-page marker:

- Wrapping the array in `{ items: [...], next: ... }` **now** would break every existing consumer
  for zero present benefit (the TUI does not paginate). The smallest correct shape is the
  unchanged bare array.
- **A next-page marker can be added later without breaking existing consumers**, via either of two
  additive escape hatches (the ADR records both; the choice is deferred to the future pagination
  ADR that has a real need):
  1. **A response header** — e.g. `X-Total-Count` and/or a `Link` next-page header. Purely
     additive: a client that ignores headers is unaffected; the body stays a bare array. This is
     the preferred path because it leaves the body byte-identical.
  2. **A distinct opt-in paginated route/param** — e.g. a future `?page=` mode or a sibling
     endpoint that returns an envelope, leaving the bare-array route untouched. Heavier; only if
     header-carried metadata proves insufficient.

  Because offset pagination needs no server-issued cursor token to function (the client computes
  the next `offset` itself as `offset + limit`), the TUI can paginate **without any response-shape
  change at all** — it just needs to know when to stop, which the header (path 1) supplies when
  the need arises. So the bare array is genuinely pagination-ready: real pagination is a caller
  change plus, at most, an additive header.

Adding real pagination (an envelope or header) later remains an **ADR event** (ADR-0005 §5) — this
ADR does not pre-authorize it; it only guarantees the current shape does not foreclose it.

### 4. Completed-last ordering — **TUI-side sort of the snapshot**; server keeps a stable default

**Completed-last is a TUI-side sort of the current snapshot, not a server `ORDER BY` change.**

- The **server** keeps its existing, stable **`ORDER BY created_at DESC`** (now with
  `LIMIT`/`OFFSET`). It does **not** sort by status. The server order is the stable *default*
  ordering and the basis for the 200-window (newest-first, so the cap keeps the most-recent 200).
- The **TUI** applies the completed-last ordering **locally, per render**, over the snapshot it
  holds: within the task list, non-completed (`open`) tasks render before completed (`done`) ones;
  within each parent, non-completed sub-tasks render before completed ones (acceptance #1). The
  sort is **stable** on the server's `created_at DESC` order (a stable sort keyed only on
  `status`, so relative created-at order is preserved within each status group).

Justification, keyed to the hard constraints:

- **#1 (stateless TUI) — the decisive force.** Acceptance #1 requires the ordering to **re-sort
  immediately** when a task/sub-task changes state (complete / reopen / toggle) with **no manual
  refresh**. The TUI already re-derives its render from the in-memory snapshot each frame and folds
  a mutation's returned DTO back in (ADR-0006 apply/refresh path); a **TUI-side sort of that
  snapshot** re-orders instantly on the next render with **zero** extra round-trip. A server
  `ORDER BY status, created_at` would only re-order after a **re-fetch**, which contradicts
  "re-sort immediately … no manual refresh" and would add a network round-trip the stateless model
  does not need. So #1 makes the TUI-side sort the natural and correct fit.
- **No domain change.** Sorting a snapshot for display is a pure render concern; it adds no field,
  no enum variant, no wire shape (#3). The server's ordering stays a stable default; it is not
  asked to encode a presentation policy.

### 5. Confirmed: no domain-structure change (#3)

This feature — and this ADR — add **no** new `TaskStatus` variant and **no** new per-task or
per-sub-task **field** beyond the **request-transport** params (`limit`, `offset`) and the
`contract` bounds constant. Specifically:

- `Task`, `Subtask`, `TaskStatus`, `CreateTaskRequest`, `UpdateTaskRequest`,
  `CreateSubtaskRequest`, `UpdateSubtaskRequest` are **unchanged**.
- The **today / older grouping**, the **"Older tasks" separator**, the **collapse-older** state,
  and the **`h`-hide** flag are **TUI-render concepts derived from `created_at`** — ephemeral,
  per-render, process-lifetime view state (#1), never persisted, never on the wire. "Today" is
  computed TUI-side from the current **UTC civil day** vs each task's `created_at`.

  > **Correction (2026-07-08):** this bullet originally read "local date." The shipped 0020
  > behaviour is the **UTC civil day** (`epoch.div_euclid(86400)`, chrono-free); the keep-UTC
  > decision is settled in [ADR-0015](./0015-task-list-date-window-query.md), which closed the
  > local-vs-UTC fork raised as idea 0009. Wording reconciled to match shipped behaviour.
- The only additions are `contract::task::TaskListQuery` (a request query DTO) and
  `contract::task::MAX_TASK_LIST_LIMIT` (a bounds constant) — request/transport, not domain
  structure.

### 6. No new error code; no URI versioning

An over-large or malformed `limit`/`offset` reuses **`validation_failed`** (ADR-0005 §6). No new
`ErrorCode` variant. No `/v1` prefix (ADR-0005 §8); `contract` remains the compatibility authority
and the in-repo TUI migrates in the same item.

## Consequences

- **`contract` adds** `TaskListQuery { limit: Option<u32>, offset: Option<u32> }` and the constant
  `MAX_TASK_LIST_LIMIT` in the `task` module. `Task`, `Subtask`, the request/update DTOs, and the
  error-code set are **unchanged**. The task-list **response** DTO (the bare `[Task]` array) is
  **unchanged**.
- **The server** reads `TaskListQuery` on `GET …/tasks`, clamps/validates `limit` against
  `MAX_TASK_LIST_LIMIT` (over-ceiling → `400 validation_failed`), defaults an absent `limit` to the
  ceiling and an absent `offset` to `0`, and applies `LIMIT`/`OFFSET` to the existing
  `ORDER BY created_at DESC` query. The committed `.sqlx/` cache is refreshed (`./ok.sh prepare`)
  for the changed static query. No migration (no schema change). The response body is unchanged.
- **The TUI** threads `limit = 200` / `offset = 0` through its `ListTasks` request path (a `tui`
  constant, not a wire constant), applies the completed-last **stable sort** at render, computes
  the today/older split from `created_at`, renders the "Older tasks" separator, forces the older
  group **collapsed regardless of status**, and adds the **`h`** hide toggle (ephemeral view state)
  plus its help-overlay reference line. It defines **no** DTO of its own (#2).
- **Backwards compatible in both directions.** An existing no-param caller behaves exactly as
  before (whole list up to the ceiling, newest-first). A future paginating caller sends
  `offset`/`limit` with **no wire-shape change**, and a next-page marker (if ever needed) arrives
  as an **additive header** — neither breaks this feature's consumers. Real pagination remains a
  future ADR event.
- **Reversibility.** Removing `TaskListQuery`/`MAX_TASK_LIST_LIMIT` from `contract`, reverting the
  server query to the un-parameterized `SELECT … ORDER BY created_at DESC`, and dropping the TUI's
  limit plumbing + render changes returns the system to its pre-0020 shape. No schema, no data
  migration, so the revert is clean.
- **Risk.** The blast radius is the shared task-list path (one server handler + query, one
  `contract` addition, and the TUI's `ListTasks` request/render surface). The render reordering
  interacts with 0019's task-tree collapse rendering; 0020 deliberately owns all of that reordering
  in one change (per the card) rather than splitting it across two rebases. The `tui`
  protocol/state extension (limit plumbing, `h`-hide flag) strands the tester harness
  (`crates/tui/tests/common/mod.rs`) under `--all-targets` (learned 0019) — the tester slice lands
  the harness update in the same cycle. Adding the `h` reference line to the `?` help overlay can
  overflow the fixed-width dialog and wrap flush-left (learned 0015, recurred 0019) — a regression
  test in `crates/tui/tests/dialogs.rs` pins it.

[feat-0020]: ../../board/features/0020-tui-tasks-pane-rendering-overhaul.md
[adr-0005]: ./0005-foundational-wire-contract.md
