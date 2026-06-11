# ADR-0005: Foundational wire contract — auth, profile bootstrap, tasks, error codes

**Status:** Accepted · 2026-06-11

## Context

Board item [0001 (foundational slice)][feat-0001] builds the first end-to-end loop:
register/login, a default profile, and add/list/close TODO items, TUI ↔ `contract` ↔
server ↔ Postgres. [ADR-0001][adr-0001] fixed the principles (local auth with argon2 + JWT,
the `{ code?, message }` error body, profiles as namespaces, `contract` as the single source
of truth); it did **not** fix concrete shapes. Since any wire shape is an ADR event, the
initial shapes are settled here, before implementation.

### Forces

- Acceptance criterion: *"on first login the user has a default profile with a name chosen
  by the user"* — the name must be captured somewhere, with no client-side onboarding state
  (the TUI is stateless, hard-constraint #1).
- Hard-constraint #4: profile scoping must be structural — impossible to forget per-handler.
- Login accepts username **or** email, so the identifier space must be collision-free.
- The contract must extend later (notes, pomodoro, multi-profile UX) without breaking this
  slice's consumers.
- Smallest shapes win; anything speculative (refresh tokens, pagination, URI versioning) is
  deferred until a real need forces a new ADR.

## Decision

### 1. Wire scalar conventions

snake_case JSON fields; ids are server-generated **UUID strings**; timestamps are
**RFC 3339 UTC strings**; enums are lowercase strings. These conventions bind all future
DTOs, not just this slice's.

### 2. Registration bootstraps the default profile atomically

`POST /api/auth/register` with `{ username, email, password, profile_name }`. The server
creates the user **and** their default profile (named `profile_name`) in one transaction —
a user without a profile cannot exist. **Usernames may not contain `@`** (and emails must),
keeping the login identifier space unambiguous. Success returns **201** with the same
session body as login (§3), so the TUI lands in the app without a second credential entry.

### 3. Login and sessions

`POST /api/auth/login` with `{ identifier, password }` — `identifier` matches username or
email. Success returns `200 { token }` (a JWT). Sessions: **JWT HS256**, signing secret
from server env (held as a `SecretString`, never logged), claims `sub` (user UUID), `iat`,
`exp`; TTL defaults to **24 h**, env-configurable. The token travels as
`Authorization: Bearer <token>`. No refresh tokens, no logout endpoint this slice — the TUI
drops the token from memory; expiry surfaces as `401 unauthenticated` and the TUI returns
to the login screen.

### 4. Profile discovery and structural scoping

`GET /api/profiles` → array of `Profile { id, name, created_at }`. All domain routes nest
under `/api/profiles/{profile_id}/…`; every query joins on the authenticated user's
ownership of `{profile_id}`. A profile the caller does not own is indistinguishable from a
nonexistent one: **404 `not_found`** (never 403), so cross-profile probing is unobservable.

### 5. Tasks

- `Task { id, title, description, status, created_at, closed_at }` with
  `status ∈ { "open", "done" }` and `closed_at` nullable — exactly the flat shape of
  hard-constraint #3.
- `POST /api/profiles/{pid}/tasks` `{ title, description }` → `201 Task`. Title must be
  non-empty after trimming (else `400 validation_failed`); description may be empty.
- `GET /api/profiles/{pid}/tasks` → `200` **bare JSON array**, newest-first
  (`created_at` desc). No pagination envelope at personal scale; adding one later is a
  breaking change and therefore an ADR anyway.
- `POST /api/profiles/{pid}/tasks/{task_id}/close` → `200 Task`, setting `status = "done"`
  and `closed_at = now`. **Idempotent**: closing an already-done task returns it unchanged
  (`closed_at` preserved) — simpler and retry-safe for the TUI.
- No task update or delete this slice (explicitly out of scope in 0001).

### 6. Error payload concretized; initial stable code set

`ErrorBody { code: <optional string>, message: <string> }` lives in `contract`. Initial
codes (append-only; renaming or removing a code is an ADR event):

| code | HTTP | meaning |
| --- | --- | --- |
| `validation_failed` | 400 | request body failed validation |
| `invalid_credentials` | 401 | login identifier/password mismatch |
| `unauthenticated` | 401 | missing, malformed, or expired token |
| `not_found` | 404 | resource absent **or not owned by the caller** |
| `username_taken` | 409 | registration username already exists |
| `email_taken` | 409 | registration email already exists |
| `internal` | 500 | unexpected server error; message is generic, never leaks internals |

### 7. Health endpoint

`GET /healthz` → `200`, unauthenticated, empty/trivial body. Used by compose healthchecks
and by the TUI's "is the server online" probe.

### 8. No URI versioning

No `/v1` prefix. The `contract` crate is the compatibility authority; URI versioning is
deferred until a real compatibility break forces it (which would be an ADR regardless).

## Consequences

- `contract` ships: `RegisterRequest`, `LoginRequest`, `SessionResponse`, `Profile`,
  `Task`, `TaskStatus`, `CreateTaskRequest`, `ErrorBody` plus the stable code identifiers.
  Field shapes are fixed here; Rust naming/representation details belong to
  `contract-owner` within these shapes.
- Register-creates-profile guarantees every user has ≥ 1 profile with zero onboarding
  state. A later `POST /api/profiles` (multi-profile UX) slots in without conflict.
- 404-for-unowned trades a friendlier 403 for non-observability of other accounts'
  namespaces — the right side of hard-constraint #4.
- The bare-array list response and missing refresh tokens are accepted simplifications at
  personal scale; revisiting either is an ADR.
- The TUI holds the JWT and active profile id **in memory only** for the process lifetime —
  consistent with hard-constraint #1 (no persistence).

[feat-0001]: ../../board/features/0001-foundational-slice.md
[adr-0001]: ./0001-foundational-architecture.md
