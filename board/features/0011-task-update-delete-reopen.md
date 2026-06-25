---
id: 0011
title: Task update + delete + reopen — generalize close into PATCH (breaking)
type: feature      # feature | chore
status: review          # inbox → planned → ready → working → review → awaiting-merge → merged | blocked
priority: medium    # high | medium | low
parent: null
depends-on: []      # ADR-0008 lands on `main` with this plan; independent of 0010/0012 (different files)
branch: feature/0011-task-update-delete-reopen
worktree: .claude/worktrees/0011-task-update-delete-reopen
created: 2026-06-24
updated: 2026-06-25
---

<!-- CLAIMED 2026-06-25 — this `main` copy is FROZEN at the claim snapshot. The branch
     `feature/0011-task-update-delete-reopen` copy is authoritative until the human's ff-merge
     brings the finished item back to `main`. Do not advance status here. -->

## Feature request

**Goal:** Generalize the one-way task `close` into full task **edit / toggle-done / reopen /
delete**. Today the only mutation is `POST /api/profiles/{id}/tasks/{task_id}/close` (one-way,
ADR-0005 §5). Replace it with a single `PATCH` update plus a `DELETE`.

**Breaking change (operator-locked):** the existing `POST .../close` endpoint is **removed** and
**replaced** by `PATCH /api/profiles/{id}/tasks/{task_id}`; the TUI's close path migrates to the
new endpoint in the same item. Per ADR-0005 §8 (no URI versioning, `contract` is the compatibility
authority) and #2, this is an ADR event — settled in [ADR-0008][adr-0008].

**Surface to build (final shapes pinned in the plan under [ADR-0008][adr-0008]):**

- `contract` — an `UpdateTaskRequest { title?, description?, status? }` (all optional — a partial
  update; the operator locked the editable scope to title + description, plus status for
  toggle/reopen). **No `updated_at`** (#3, operator-locked).
- `server` — `PATCH /api/profiles/{id}/tasks/{task_id}` applying the supplied fields:
  setting `status: done` sets `closed_at`; setting `status: open` (reopen) **clears** `closed_at`;
  title/description edited in place. `DELETE /api/profiles/{id}/tasks/{task_id}`. The `POST
  .../close` route + handler are **removed**.
- `tui` — the task list gains edit, toggle-done, and delete; the existing close action is rewired
  onto `PATCH { status: done }`.

**Acceptance criteria:**

- [ ] `PATCH .../tasks/{id}` with any subset of `{ title, description, status }` updates exactly
      those fields in place. `status: done` sets `closed_at = now`; `status: open` (reopen)
      **clears** `closed_at` (sets it null). An empty patch is a no-op returning the task unchanged.
- [ ] `DELETE .../tasks/{id}` removes the task (`204`); a second delete or an unowned/missing id →
      `404 not_found`.
- [ ] The `POST .../close` endpoint **no longer exists**; the TUI close action now issues
      `PATCH { status: done }`. No code path references the old route.
- [ ] Profile-scoping (#4) and 404-for-unowned hold for both PATCH and DELETE; flat shape (#3)
      preserved — no new fields, **no `updated_at`**.
- [ ] Full `feature` Definition of done: `./ok.sh test | lint | fmt --check` green; `reviewer`
      approved (pinned to `./ok.sh code-hash`); live `verifier` pass exercising the new server API +
      reqwest path (PATCH partial updates, reopen-clears-closed_at, DELETE, 404s, error contract,
      profile-scoping, OTel spans); the `tui` change covered by the `TestBackend` suite
      ([ADR-0003][adr-0003]).
- [ ] The contract change carries [ADR-0008][adr-0008]; the `close` removal is recorded as the
      breaking change it is.

**Out of scope (would need an ADR — #3):** bulk operations, undo/history, per-field audit, any
new task field, or any second timestamp beyond `closed_at`.

<!-- feature: needs an `architect` plan (`plan` skill) writing a `## Plan(s)` block before code. -->

<!-- ─────────────────────────────  ARCHITECT PLAN  ───────────────────────────── -->
## Plan(s)

Planned by `architect` under [ADR-0008][adr-0008] (task mutation generalization + delete; new ADR
referencing ADR-0005, committed to `main` with this plan before any worktree is cut). This is a
**breaking** contract change: it removes the `close` route and the `CreateTaskRequest`-adjacent
close semantics in favour of a single PATCH. No new `ErrorCode`.

### Approach

Tracer-bullet contract→server→tui. The new `UpdateTaskRequest` is an all-optional partial-update
DTO. The handler builds a single `UPDATE … RETURNING` from the supplied fields; the `status`
transition drives `closed_at` (done → `now`, open → `NULL`) in the same statement. The old
`close_task` handler and its route are deleted; the TUI's close call site is rewired. DELETE is a
plain ownership-scoped `DELETE … RETURNING`/rowcount → `204` or `404`.

### ADR

**[ADR-0008][adr-0008] — Task mutation: generalize `close` into `PATCH` update + add `DELETE`**
(new; references ADR-0005 §5/§8). Fixes: `UpdateTaskRequest` shape (all-optional partial), the
PATCH semantics (status↔closed_at coupling, reopen clears `closed_at`, empty-patch no-op), the
DELETE route + status codes, the **removal** of `POST .../close` (the breaking change), and the
reuse of the existing error code set. Committed to `main` with this item.

### Slices (dependency-ordered: contract → server → tui → tester alongside)

| # | Slice | Agent | files |
| --- | --- | --- | --- |
| 1 | `contract` `task` module: add `UpdateTaskRequest { title: Option<String>, description: Option<String>, status: Option<TaskStatus> }` with derives + rustdoc + a partial-update doctest (`skip_serializing_if = "Option::is_none"` so absent fields are omitted); re-export from `lib.rs`. **No** `Task`/`TaskStatus`/`CreateTaskRequest` field changes | `contract-owner` | `crates/contract/src/task/mod.rs`, `crates/contract/src/lib.rs` |
| 1t | `contract` tests for `UpdateTaskRequest` (full patch, single-field patch, empty patch serializes `{}`, round-trip) appended to the existing task suite | `tester` | `crates/contract/tests/task.rs` |
| 2 | `server`: add `patch_task` + `delete_task` handlers in `handlers/tasks.rs`; **remove** `close_task` and its route; rewire `app.rs` (`.patch(patch_task).delete(delete_task)` on `/api/profiles/{pid}/tasks/{task_id}`, drop the `…/close` route); error mapping reused; `./ok.sh prepare` | `server-dev` | `crates/server/src/handlers/tasks.rs`, `…/app.rs`, `.sqlx/` |
| 2t | Server integration tests: PATCH title-only / description-only / status done-sets-closed_at / status open-clears-closed_at (reopen) / multi-field / empty-patch no-op; DELETE→204 then 404; unowned profile+missing task→404 on PATCH & DELETE; auth-required; **and the `close` route is gone** (old path → 404/405). Update the existing `tasks.rs` close-tests to the new PATCH path | `tester` | `crates/server/tests/tasks.rs`, `crates/server/tests/common/mod.rs` |
| 3 | TUI client/protocol: replace `close_task` with `update_task(token, profile_id, task_id, &UpdateTaskRequest)` + add `delete_task(...)`; `HttpClient` impls (PATCH carries body, DELETE bodyless); update `ClientRequest`/`Outcome` variants (`CloseTask` → `UpdateTask`, add `DeleteTask`) + worker arms | `tui-dev` | `crates/tui/src/client/mod.rs`, `…/client/worker.rs`, `…/app/protocol.rs` |
| 4 | TUI task list: rewire the close key onto `UpdateTask { status: done }` (toggle-done also issues `{ status: open }` to reopen), add an edit sub-flow (title+description, same category as `AddTaskState`) issuing `UpdateTask`, add a delete key issuing `DeleteTask` (with a confirm affordance), `apply_response` arms, `map_key` arms | `tui-dev` | `crates/tui/src/app/mod.rs`, `…/app/task*.rs`, `…/ui/mod.rs`, `…/terminal/mod.rs` |
| 4t | TUI `TestBackend`/core suite: toggle-done issues `UpdateTask{done}` and reflects done, reopen issues `UpdateTask{open}` and clears closed render, edit issues `UpdateTask{title,desc}` and reflects, delete issues `DeleteTask` and removes the row, empty-title edit validation inline, in-flight spinner, cancel/stale-id drop; update existing close-tests to the new flow; `FakeClient` `update_task`/`delete_task` impls | `tester` | `crates/tui/tests/tasks.rs`, `crates/tui/tests/common/mod.rs` |

Dependency edges: **1 → 2 → 3 → 4**; tests alongside. Because this is breaking, slice 2's route
removal and slice 3's client rewire must land together on the branch (the `tui` lib won't compile
against a removed `close_task` until rewired) — but they merge as one branch, so intra-branch
ordering (2 before 3) suffices.

### Assumptions (human is AFK — smallest change satisfying acceptance; resolved forks)

- **A1 — `UpdateTaskRequest` is all-optional partial** (`Option<_>` per field,
  `skip_serializing_if = "Option::is_none"`), not a full-replace. Smallest shape supporting
  edit-title-only, reopen (`status` only), and toggle without forcing the client to resend
  unchanged fields. An **empty patch `{}` is a no-op** returning the task unchanged (retry-safe,
  mirrors the idempotent spirit of the old `close`).
- **A2 — status↔closed_at coupling:** `status: done` ⇒ `closed_at = now()` (operator-locked);
  `status: open` ⇒ `closed_at = NULL` (reopen clears it, operator-locked). When `status` is
  **absent** from the patch, `closed_at` is left untouched. Done-while-already-done preserves the
  existing `closed_at` (`COALESCE`, matching today's idempotent close) only when status is set to
  done.
- **A3 — Status codes:** PATCH `200` (returns the updated `Task`); DELETE `204 No Content`. Empty
  patch still `200` with the unchanged task.
- **A4 — Validation:** if `title` is present it must be non-empty after trimming (→
  `400 validation_failed`, reusing `ApiError::Validation`, no new code) and is stored trimmed;
  `description`, if present, may be empty; `status`, if present, is the `TaskStatus` enum (open/done).
- **A5 — 404 vs 403:** unowned/missing profile or task id → `404 not_found` on both PATCH and
  DELETE (ADR-0005 §4); the statements are ownership-joined (`WHERE id=$1 AND profile_id=$2`).
- **A6 — `close` is removed, not deprecated:** ADR-0005 §8 makes `contract` the compatibility
  authority and forbids URI versioning; with a single consumer (the in-repo TUI, migrated in the
  same item) a clean removal is correct. The old route is deleted from `app.rs` and the
  `close_task` handler removed.
- **A7 — No `updated_at`** (#3, operator-locked): edits mutate in place; the only timestamps are
  `created_at` and `closed_at`. No schema migration is needed — this item touches **no** migration
  (the existing `tasks` table already supports nullable `closed_at` and an in-place title/desc/status
  update).
- **A8 — `chrono`/contract boundary** unchanged; the TUI keeps no direct `chrono` dep.

### Risks

- **Reopen correctness:** the highest-value server test is "done then reopen → `status: open` and
  `closed_at` is null again"; the highest-value TUI test is the toggle round-trip. Both `tester`
  and the live `verifier` exercise it.
- **Breaking-change fallout:** any lingering reference to the old `close` route/`CloseTask`
  protocol variant fails compilation/tests — caught by `./ok.sh test` + reviewer. The acceptance
  criterion "no code path references the old route" is explicitly verified.
- **Partial-update SQL:** building a dynamic `UPDATE` from optional fields risks an injection or a
  malformed query if hand-concatenated. Use a single parameterized statement with
  `COALESCE($n, column)` per optional field (and a `CASE` for the `status`→`closed_at` coupling),
  never string interpolation — keeps it one static query, sqlx-checkable, lint-clean.
- **Capability gap (#6):** `./ok.sh prepare`/`test`/live `verifier` need the sanctioned test
  Postgres / docker; unavailable ⇒ **block** with a precise question, never worked around.

[adr-0008]: ../../docs/adr/0008-task-mutation-generalization.md
[adr-0003]: ../../docs/adr/0003-verification-layering.md

## Log / comments

- [x] 2026-06-25 [drive] Claimed `ready`→`working`. Worktree
      `.claude/worktrees/0011-task-update-delete-reopen` branch
      `feature/0011-task-update-delete-reopen` cut from `main` 61101e0 (carries the plan +
      ADR-0008, verified present in the base commit and inside the worktree). Docker capability
      confirmed UP (29.5.3; Risk #6 / hard-constraint #6 cleared). Building contract→server→tui per
      the slice order (1→2→3→4, tests alongside). Breaking change: `POST .../close` removed and
      replaced by `PATCH …/tasks/{id}` + `DELETE …/tasks/{id}`; TUI close path rewired in the same
      branch.
- [x] 2026-06-25 [drive] Build complete (contract→server→tui, tests alongside). S1 `contract`
      `UpdateTaskRequest { title?, description?, status? }` all-optional partial DTO + doctests
      (`fdf25cb`); S1t contract tests ×5 → suite 21 (`094865b`). S2 server `patch_task`/`delete_task`,
      single static `UPDATE … RETURNING` with `COALESCE`/`CASE` status↔closed_at coupling, ownership-
      scoped `DELETE`, `close_task` + `POST .../close` route removed, `.sqlx/` refreshed (`b46a6a6`);
      S2t server integration tests incl. reopen-clears-closed_at, empty-patch no-op, blank-title 400,
      DELETE→204→404, profile-scoped 404, auth, old-close-route-gone, migrated close→PATCH
      (`1fa1461`; tasks.rs 20, profile_isolation.rs 6). S3+S4 TUI client `update_task`/`delete_task`,
      protocol `UpdateTask`/`DeleteTask`, keys `e` edit / `c` toggle-done(+reopen) / `x` delete with
      two-step confirm, mutations chain a `ListTasks` refresh (stateless #1), caption re-budgeted for
      80×24 (ADR-0006 §8.3) (`52904a4`); S4t TUI TestBackend suite migrated + new (`6c3b987`; tui 80
      tests + 2 doctests). No `close_task`/`CloseTask`/`CloseSelected` residue in any `crates/*/src/`.
      All gates green at branch head `6c3b987`: `./ok.sh build | test | lint --all-targets |
      fmt --check`. Code-hash `e66426f0a6fcb9c0ba3f7e6baf1f3b606708a6cf`.
- [x] 2026-06-25 [reviewer] **REVIEW-STATUS: approved** — code-hash
      `e66426f0a6fcb9c0ba3f7e6baf1f3b606708a6cf` (head sha `6c3b987`, a human-readable pointer).
      Mechanical gate green (test: contract task 21 + 16 doctests, server tasks 20 + profile_isolation
      6 + auth/timer, tui tasks 8 + full suite + 2 doctests; lint --all-targets clean, no `#[allow]`;
      fmt clean). All hard constraints clear: #2 `UpdateTaskRequest` only in `contract` (ADR-0008),
      consumed by both sides; #3 flat — no new fields, **no `updated_at`** anywhere; #4 every PATCH/
      DELETE ownership-joined `WHERE id=$1 AND profile_id=$2`, unowned/missing → 404 never 403, no
      cross-profile leakage (profile_isolation asserts the write/delete didn't land); A6 breaking-
      change complete — `close_task`/`POST .../close`/`CloseTask`/`CloseSelected` fully gone (grep
      clean), `old_close_route_is_gone` asserts 404/405; A2 single static parameterized `UPDATE …
      RETURNING` with `COALESCE`/`CASE` (no injection surface), done→COALESCE(closed_at,now()),
      open→NULL, absent→untouched, empty-patch 200 no-op; A4 blank title → 400 `ValidationFailed`
      (no new code), stored trimmed; A3 PATCH 200 / DELETE 204; #1 TUI stateless (mutations chain a
      `ListTasks` refresh), no `chrono` dep (A8); no migration added, `.sqlx/` refreshed to match;
      ADR-0003 TUI behaviour covered by green TestBackend suite. No fix-now blockers. **Nit (non-
      blocking, chore candidate):** `crates/tui/README.md:15` still says "close tasks" — stale after
      the migration (server README route table was correctly updated). Verdict valid while
      `./ok.sh code-hash HEAD` == the hash above.
- [x] 2026-06-25 [verifier] **VERIFY-STATUS: not-verified** — code-hash
      `e66426f0a6fcb9c0ba3f7e6baf1f3b606708a6cf` (== reviewer hash; worktree head `1be3704`, a
      Board-only commit; code-hash identical before/after). **Capability/environment blocker, NOT a
      0011 defect.** `./ok.sh up` failed at the one-shot `organized-koalad migrate`: *"migration
      20260612163049 was previously applied but is missing in the resolved migrations."* Root cause:
      the persistent named volume `deploy_postgres-data` carries migration `20260612163049 (notes)`
      from the concurrent **0010** worktree (same compose project name `deploy`, shared volume); 0011's
      migration tree correctly ends at `20260612163048_timer` (A7 — task update/delete needs no
      schema change). sqlx's strict migration-history consistency check then refuses to proceed and
      the `run` service gates on `migrate`. The clean fix (`docker compose down -v` to reset the dev
      volume) destroys another branch's local data, so the verifier's safety classifier denied it and
      per #6 it was **not** worked around; stack torn down non-destructively, no scratch left.
      **All 8 live flows NOT RUN** (PATCH partial/multi-field; reopen-clears-closed_at; empty-patch
      no-op;
      blank-title 400; DELETE 204→404; cross-profile/missing-id 404; old `…/close` route gone; error
      contract + OTel spans) — not inferred. Confirmed at the tester layer only (`./ok.sh test` on the
      throwaway test Postgres): full suite green incl. server tasks 20 + profile_isolation 6, contract
      task 21, TUI TestBackend tasks 8 (+full suite) — ADR-0003 clause-4 TestBackend confirmation
      holds, but this is **not** a substitute for the live pass.
- [x] 2026-06-25 [drive] **BLOCKED pending operator decision (DoD clause 4 / #6) — RESOLVED.** The
      live verifier pass could not run while the shared `deploy_postgres-data` volume carried 0010's
      `notes` migration. Operator authorized **option (a)**: reset the `deploy_postgres-data` Docker
      volume. Orchestrator removed the volume (`docker volume rm deploy_postgres-data`); the next
      `./ok.sh up` recreated it clean and re-applied 0011's migration tree from scratch (ending at
      `20260612163048_timer`, no `notes`). Verifier re-ran — see the verified verdict below. Item
      unblocked back to `review` and proceeding to summarise + freshen.
- [x] 2026-06-25 [verifier] **VERIFY-STATUS: verified** (clean-volume re-run) — code-hash
      `e66426f0a6fcb9c0ba3f7e6baf1f3b606708a6cf` (== reviewer hash; last code sha `6c3b987`; confirmed
      identical before/after). `./ok.sh up` booted clean: migrate one-shot exited cleanly, server
      healthy on `:8080`, `_sqlx_migrations` ends at `20260612163048_timer` (no `notes`, A7).
      **All 8 live flows RAN (nothing inferred), quoting real request/response:** (1) PATCH
      title-only / desc-only / multi-field → 200, only supplied fields change; (2) reopen
      round-trip — `{status:done}` → `closed_at` non-null, then `{status:open}` → `closed_at:null`;
      (3) empty patch
      `{}` → 200
      unchanged; (4) `{title:"   "}` → `400 {"code":"validation_failed","message":"title must not be
      empty"}`; (5) DELETE → 204, second DELETE → `404 not_found`, PATCH on deleted id → 404; (6) cross-
      profile PATCH+DELETE under another user's profile → `404 not_found` (never 403), victim task
      unchanged, missing id → 404, unauthenticated → 401; (7) old `POST …/close` → 404; (8) error
      contract `{code,message}` on all failures + OTel spans `patch_task`/`delete_task` exported
      (`service.name organized-koalad`, ids only — no titles/bodies/tokens). TUI `TestBackend` suite
      confirmed green (ADR-0003 clause 4): tui tasks 8 + full suite, server tasks 20 + profile_isolation
      6, contract task 21, all doctests. No gaps. Stack torn down (`./ok.sh down`), clean volume
      preserved. Verdict valid while `./ok.sh code-hash HEAD` == the hash above.

## Summary

_(filled by `eng-manager` at drive step 6)_
