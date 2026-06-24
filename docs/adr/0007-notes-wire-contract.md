# ADR-0007: Notes wire contract — module, CRUD routes, flat shape

**Status:** Accepted · 2026-06-24

## Context

Board item [0010 (notes)][feat-0010] adds the **Notes** feature, the last missing domain area.
Notes do not exist anywhere today. [ADR-0005][adr-0005] fixed the foundational wire contract
(scalar conventions, profile-scoped nesting, the `{ code?, message }` error body, the stable error
code set, 404-for-unowned, bare-array lists). Notes are a new wire surface, so — per
hard-constraint #2 (a contract change is an ADR event) — their shapes are settled here before
implementation. The operator has locked the note shape flat and forbidden any `updated_at`.

### Forces

- Hard-constraint #3 (flat domain): a note is exactly `{ id, title, content, created_at }` — no
  folders, tags, categories, or second timestamp.
- Hard-constraint #4 (namespaces): every note is profile-scoped; no cross-profile access.
- ADR-0005's task surface is the established precedent — reuse its conventions verbatim rather than
  invent new ones (scalar shapes, ownership gate, 404-for-unowned, bare-array list newest-first,
  the error contract).
- Smallest shapes win; nothing speculative (search, pagination, rich-text) is introduced.

## Decision

### 1. The `note` module and DTOs

A new `contract::note` module (re-exported from `lib.rs`), following ADR-0005 §1 scalar
conventions (snake_case fields; UUID-string `id`; RFC 3339 UTC `created_at` with `Z` offset):

- `Note { id, title, content, created_at }` — the flat shape of hard-constraint #3. **No
  `updated_at`**, no status, no lifecycle.
- `CreateNoteRequest { title, content }` — body for create.
- `UpdateNoteRequest { title, content }` — body for in-place edit; a **full replace** of the two
  editable fields (the operator locked the editable scope to title + content). Editing mutates in
  place; no timestamp is touched.

Every public type derives `Debug, Clone, PartialEq, Eq, Serialize, Deserialize` and carries
rustdoc plus a serialization doctest, matching the established `contract` layout.

### 2. Routes, status codes, and ordering

Notes nest under `/api/profiles/{profile_id}/notes`, exactly mirroring tasks:

| Method + path | Success | Notes |
| --- | --- | --- |
| `POST /api/profiles/{pid}/notes` | `201 Note` | title non-empty after trim (else `400 validation_failed`); content may be empty |
| `GET  /api/profiles/{pid}/notes` | `200 [Note]` | bare JSON array, newest-first (`created_at` desc), no pagination envelope (ADR-0005 §5 precedent) |
| `GET  /api/profiles/{pid}/notes/{note_id}` | `200 Note` | unowned/missing → `404 not_found` |
| `PATCH /api/profiles/{pid}/notes/{note_id}` | `200 Note` | in-place full replace of title+content; unowned/missing → `404` |
| `DELETE /api/profiles/{pid}/notes/{note_id}` | `204 No Content` | empty body; second delete or unowned/missing → `404` |

### 3. Validation, scoping, and error codes

- Title non-empty after trimming (stored trimmed) → else `400 validation_failed`; content may be
  empty. No new `ErrorCode` — validation reuses `validation_failed`, absence/non-ownership reuses
  `not_found`.
- Every query is ownership-joined on the caller's profile (`WHERE id = $note AND profile_id = $pid`
  with the profile pre-checked for ownership), so an unowned/nonexistent profile **or** note id is
  `404 not_found` (never 403) — ADR-0005 §4 non-observability holds for notes.

### 4. Persistence

A reversible (`up`/`down`) migration creates a `notes` table:
`{ id UUID PK, profile_id UUID NOT NULL REFERENCES profiles(id) ON DELETE CASCADE, title TEXT NOT
NULL, content TEXT NOT NULL DEFAULT '', created_at TIMESTAMPTZ NOT NULL DEFAULT now() }` with an
index on `(profile_id, created_at DESC)`. The `ON DELETE CASCADE` means a future profile delete
(ADR-0009 / item 0012) cascades notes at the DB level. **No `updated_at` column** (#3).

## Consequences

- `contract` ships `Note`, `CreateNoteRequest`, `UpdateNoteRequest`; the error code set is
  unchanged (append-only set untouched).
- Notes inherit the full ADR-0005 profile-scoping and error-contract posture for free, because the
  surface is a structural clone of tasks — minimizing new decisions and review surface.
- The flat `{ id, title, content, created_at }` shape and the absence of `updated_at` make any
  later structure (tags, folders, edit history) an explicit ADR event, as #3 requires.
- The `ON DELETE CASCADE` FK pre-wires the namespace-delete semantics that item 0012 relies on,
  with no app-level fan-out.

[feat-0010]: ../../board/features/0010-notes.md
[adr-0005]: ./0005-foundational-wire-contract.md
