# ADR-0009: Profile mutations — create / rename / delete-cascade / last-profile guard / name uniqueness

**Status:** Accepted · 2026-06-24

## Context

Board item [0012 (profiles CRUD + switcher)][feat-0012] completes profile management. Today the
only profile surface is `GET /api/profiles` plus the register-time default-profile bootstrap
([ADR-0005][adr-0005] §2/§4). The operator has decided to add create, rename, and delete, with the
namespace semantics of #4: delete cascades the profile's tasks **and** notes, the account must
always retain ≥1 profile, and names are unique per account. This adds new routes, new DTOs, and —
crucially — **two new error codes**, extending the ADR-0005 §6 stable set. Because the code set is
append-only and any wire change is an ADR event (#2), this is settled here before implementation.

### Forces

- #4 (namespaces): a profile owns its tasks and notes; deleting it must remove them — and nothing
  belonging to another profile.
- ADR-0005 §2 invariant: *a user without a profile cannot exist* — so the last profile cannot be
  deleted.
- Names must be unique **per account** (not globally) so a switcher can present unambiguous labels;
  cross-account collisions are fine.
- ADR-0005 §6 fixes the error code set as append-only — new conflict cases need new codes, and
  adding them is an ADR event.
- #1 (stateless TUI): "switching" profiles is the client choosing which `profile_id` to scope to;
  it must **not** become server state or client persistence.
- 404-for-unowned (ADR-0005 §4) must hold for rename/delete of profiles the caller does not own.

## Decision

### 1. DTOs

`contract::profile` gains `CreateProfileRequest { name }` and `UpdateProfileRequest { name }`
(name is the only editable field). `Profile` is unchanged.

### 2. Routes and status codes

| Method + path | Success | Notes |
| --- | --- | --- |
| `POST  /api/profiles` | `201 Profile` | name non-empty after trim (else `400`); duplicate per-account name → `409 profile_name_taken` |
| `PATCH /api/profiles/{id}` | `200 Profile` | rename; duplicate name → `409 profile_name_taken`; unowned/missing → `404 not_found` |
| `DELETE /api/profiles/{id}` | `204 No Content` | cascades tasks+notes; deleting the last profile → `409 last_profile`; unowned/missing → `404 not_found` |

There is **no** server "switch" endpoint — switching is client-side only (§5).

### 3. Two new error codes (append-only extension of ADR-0005 §6)

| code | HTTP | meaning |
| --- | --- | --- |
| `profile_name_taken` | 409 | create/rename to a name the account already uses |
| `last_profile` | 409 | refused delete of the account's only remaining profile |

New `ErrorCode` variants `ProfileNameTaken` / `LastProfile` (with `as_str` / `From<&str>` /
serialize round-trip) and matching `ApiError` variants mapped at the server boundary. The existing
`ErrorCode::Unknown` forward-compatibility means older clients still parse these without breaking.

### 4. Delete-cascade and the last-profile guard

- **Cascade** is achieved by the FK `ON DELETE CASCADE` already on `tasks.profile_id` and added on
  `notes.profile_id` (ADR-0007 / item 0010). `DELETE FROM profiles WHERE id=$1 AND user_id=$2`
  cascades both children at the DB level — **no app-level fan-out**, no risk of forgetting a child
  table.
- **Last-profile guard:** the delete is atomic and refuses when it would empty the account —
  implemented as a single guarded statement (delete only when the account holds > 1 profile) or a
  count-then-delete in one transaction, so concurrent deletes cannot both pass. Returns
  `409 last_profile` without deleting.
- **Name uniqueness** is enforced by a DB `UNIQUE (user_id, name)` constraint (added by a reversible
  migration); the handler catches the unique-violation and maps it to `409 profile_name_taken`
  (race-safe — no TOCTOU pre-check).

### 5. Switching is client-side only

The TUI holds the active `profile_id` in memory for the process lifetime (already true for the
default profile today — ADR-0005 Consequences). "Switch" rebinds that in-memory id and re-issues
the profile-scoped reads. There is **no** server endpoint and **no** client persistence (#1). If
the currently-active profile is deleted, the TUI re-points to another profile from the list.

## Consequences

- `contract` adds two profile request DTOs and **two error codes**; the error set grows by two
  (append-only — older consumers unaffected via `ErrorCode::Unknown`).
- A reversible migration adds `UNIQUE (user_id, name)` to `profiles`. The cascade relies on FKs
  already present (tasks) and added in 0010 (notes) — see the cross-item dependency in the 0012
  plan and the sequencing note.
- The ADR-0005 §2 "a user always has ≥1 profile" invariant is now enforced on the delete path, not
  just the register path.
- Profile-switching adds **no** server state and **no** client persistence, holding #1; a
  server-side "active profile" concept would be a separate ADR.

[feat-0012]: ../../board/features/0012-profiles-crud-and-switcher.md
[adr-0005]: ./0005-foundational-wire-contract.md
