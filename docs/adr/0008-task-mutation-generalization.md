# ADR-0008: Task mutation — generalize `close` into `PATCH` update, add `DELETE`

**Status:** Accepted · 2026-06-24

## Context

Board item [0011 (task update/delete/reopen)][feat-0011] generalizes task mutation. Today the only
task mutation is the one-way `POST /api/profiles/{id}/tasks/{task_id}/close` ([ADR-0005][adr-0005]
§5): no edit, no reopen, no delete. The operator has decided to replace `close` with a single
`PATCH` update and add a `DELETE`. This reshapes the task wire surface and **removes** an existing
route, so — per hard-constraint #2 and ADR-0005 §8 (no URI versioning; `contract` is the
compatibility authority) — it is an ADR event, settled here before implementation.

### Forces

- A single editable surface (title, description, status) is simpler than a proliferation of
  single-purpose endpoints (`/close`, `/reopen`, `/rename`, …) — deep-module design.
- Reopen must **clear** `closed_at`; close must **set** it — the status transition and the
  `closed_at` timestamp are coupled and must move together atomically.
- Flat domain (#3): no new fields, **no `updated_at`** (operator-locked) — the only timestamps stay
  `created_at` and `closed_at`.
- The sole consumer of `/close` is the in-repo TUI, migrated in the same item — so a clean removal
  (not a versioned deprecation) is correct under ADR-0005 §8.
- Profile-scoping (#4) and 404-for-unowned (ADR-0005 §4) must hold for the new routes exactly as
  for the old one.

## Decision

### 1. `UpdateTaskRequest` — an all-optional partial update

`contract::task` gains `UpdateTaskRequest { title: Option<String>, description: Option<String>,
status: Option<TaskStatus> }`, each field `skip_serializing_if = "Option::is_none"`. A patch
carries only the fields it changes. The operator locked the editable scope to **title +
description** (plus `status` for toggle/reopen). `Task`, `TaskStatus`, and `CreateTaskRequest` are
**unchanged**.

### 2. `PATCH /api/profiles/{pid}/tasks/{task_id}` → `200 Task`

Applies the supplied fields in place:

- `title` present ⇒ must be non-empty after trimming (else `400 validation_failed`), stored trimmed.
- `description` present ⇒ may be empty.
- `status: done` ⇒ sets `closed_at = now()` (preserving an existing `closed_at` if already done,
  via `COALESCE`, matching the old idempotent close).
- `status: open` (reopen) ⇒ **clears** `closed_at` (sets it `NULL`).
- `status` **absent** ⇒ `closed_at` untouched.
- An **empty patch** is a no-op returning the task unchanged (`200`).

The handler issues a single parameterized `UPDATE … RETURNING` (per-field `COALESCE($n, column)`
plus a `CASE` for the status→`closed_at` coupling) — no string interpolation, one sqlx-checkable
static query.

### 3. `DELETE /api/profiles/{pid}/tasks/{task_id}` → `204 No Content`

Ownership-scoped delete. A second delete or an unowned/missing id → `404 not_found`.

### 4. `POST .../close` is removed

The `close` route and its handler are deleted; the TUI's close action is rewired onto
`PATCH { status: done }` in the same item. No URI versioning (ADR-0005 §8); `contract` is the
compatibility authority and the single consumer migrates atomically. No new `ErrorCode` — the
surface reuses `validation_failed` and `not_found`.

## Consequences

- `contract` adds `UpdateTaskRequest`; the task wire surface changes shape (a route removed, two
  added). The error code set is unchanged.
- The status↔`closed_at` coupling now lives in one place (the PATCH handler), making reopen a
  first-class, tested transition rather than an impossible state.
- No schema migration is required: the existing `tasks` table (nullable `closed_at`, mutable
  title/description/status) already supports every transition. **No `updated_at`** is added (#3).
- Removing `/close` is a deliberate breaking change, acceptable because the only consumer is the
  in-repo TUI migrated in lockstep; an external consumer would have required versioning (a separate
  ADR).

[feat-0011]: ../../board/features/0011-task-update-delete-reopen.md
[adr-0005]: ./0005-foundational-wire-contract.md
