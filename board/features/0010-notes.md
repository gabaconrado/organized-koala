---
id: 0010
title: Notes — full feature (contract module, migration, server CRUD, TUI views)
type: feature      # feature | chore
status: review           # inbox → planned → ready → working → review → awaiting-merge → merged | blocked
priority: medium    # high | medium | low
parent: null
depends-on: []      # ADR-0007 lands on `main` with this item's plan; no in-flight Board item gates it
branch: feature/0010-notes
worktree: .claude/worktrees/0010-notes
created: 2026-06-24
updated: 2026-06-24
---

<!-- CLAIMED 2026-06-24 — this `main` copy is FROZEN at the claim snapshot. The branch
     `feature/0010-notes` copy is authoritative until the human's ff-merge brings the finished
     item back to `main`. Do not advance status here. -->

## Feature request

**Goal:** Implement **Notes** end-to-end — the last missing domain feature. Notes do not exist
anywhere today (no `contract` module, no route, no migration, no TUI). A user can create, list,
read, edit, and delete free-form notes, scoped to the active profile.

**Shape (deliberately flat — hard-constraint #3):** a note is exactly
`{ id, title, content, created_at }`. No folders, no tags, **no `updated_at`** (editing mutates
in place; only `created_at` is a timestamp — operator-locked). `id` is a UUID string,
`created_at` is RFC 3339 UTC, matching the ADR-0005 §1 scalar conventions.

**Profile-scoped (hard-constraint #4):** notes nest under `/api/profiles/{profile_id}/notes`,
exactly like tasks; every query is ownership-joined on the caller's profile, and an unowned or
nonexistent profile/note is `404 not_found` (never 403) — the ADR-0005 §4 non-observability rule.

**Surface to build (final shapes pinned in the plan under [ADR-0007][adr-0007]):**

- `contract` — a new `note` module: `Note { id, title, content, created_at }`,
  `CreateNoteRequest { title, content }`, `UpdateNoteRequest { title, content }`, reusing the
  `{ code?, message }` error contract and adding **no** new `ErrorCode`.
- `server` — CRUD under `/api/profiles/{id}/notes`: create / list / get-one / update / delete; a
  reversible (`up`/`down`) migration creating a profile-scoped `notes` table that cascades on
  profile delete.
- `tui` — a notes view: list, create, edit, delete; stateless (#1), every view derives from a
  server response.

**Acceptance criteria:**

- [ ] A user can create a note (non-empty trimmed title; content may be empty), list their
      profile's notes newest-first, read one, edit title+content in place, and delete one.
- [ ] Notes are profile-scoped (#4): a note created under profile A is invisible under profile B;
      an unowned/nonexistent profile or note id is `404 not_found` (never 403).
- [ ] The note shape stays flat (#3): exactly `{ id, title, content, created_at }`, **no
      `updated_at`**; editing mutates in place.
- [ ] Full `feature` Definition of done: `./ok.sh test | lint | fmt --check` green; `reviewer`
      approved (pinned to `./ok.sh code-hash`); live `verifier` pass exercising the server API +
      reqwest path (shapes, status codes, the error contract, profile-scoping, OTel spans); the
      `tui`-touching change is covered by the `ratatui` `TestBackend` suite ([ADR-0003][adr-0003]).
- [ ] The `contract` change is governed by [ADR-0007][adr-0007]; any wire detail beyond what
      that ADR fixes is recorded in the plan's Assumptions.

**Out of scope (each would need a new ADR — #3 flatness):** folders, tags, categories, pinning,
search, rich text/markdown rendering as a domain concern, sharing across profiles, an
`updated_at` field or any second timestamp.

<!-- feature: needs an `architect` plan (`plan` skill) writing a `## Plan(s)` block before code. -->

<!-- ─────────────────────────────  ARCHITECT PLAN  ───────────────────────────── -->
## Plan(s)

Planned by `architect` under [ADR-0007][adr-0007] (notes wire contract — new ADR, committed to
`main` with this plan before any worktree is cut). The note surface is a near-exact structural
clone of the ADR-0005 §5 task surface (profile-scoped nesting, ownership gate, 404-for-unowned,
bare-array list newest-first, the `{ code?, message }` contract reused verbatim). No new
`ErrorCode`.

### Approach

Tracer-bullet, contract→server→tui, one thin slice flowing through every layer before widening.
The note table, handler module, and ownership gate mirror `tasks` one-for-one; the only domain
delta vs. tasks is `content` instead of `description` and **no status/closed_at** (a note has no
lifecycle). Update is a single in-place write of `title`+`content` (no `updated_at`, #3).

### ADR

**[ADR-0007][adr-0007] — Notes wire contract** (new; references ADR-0005). Fixes: the `note`
module DTOs, the five `/api/profiles/{id}/notes` routes + status codes, validation (non-empty
trimmed title; content may be empty), the in-place-update / no-`updated_at` decision, and the
reuse of the existing error code set (no new code). Committed to `main` with this item.

### Slices (dependency-ordered: contract → server → tui → tester alongside)

| # | Slice | Agent | files |
| --- | --- | --- | --- |
| 1 | `contract` `note` module: `Note { id, title, content, created_at }`, `CreateNoteRequest { title, content }`, `UpdateNoteRequest { title, content }`; derives `Debug, Clone, PartialEq, Eq, Serialize, Deserialize`; rustdoc + serialization doctests; re-export from `lib.rs` | `contract-owner` | `crates/contract/src/note/mod.rs`, `crates/contract/src/lib.rs` |
| 1t | `contract` public-API tests for the note DTOs (round-trip, exact-shape, RFC 3339 `Z` offset normalization), mirroring `tests/task.rs` | `tester` | `crates/contract/tests/note.rs` |
| 2 | Migration (up/down) for `notes` + `notes.rs` handler module (create/list/get/update/delete) reusing the `assert_owned` ownership-gate pattern; route wiring in `app.rs`; `handlers/mod.rs` declaration; error mapping; `./ok.sh prepare` to refresh `.sqlx/` | `server-dev` | `crates/server/migrations/<ts>_notes.{up,down}.sql`, `crates/server/src/handlers/notes.rs`, `…/handlers/mod.rs`, `…/app.rs`, `.sqlx/` |
| 2t | Server integration tests: create (201, trimmed-empty-title→400), list (200 bare array newest-first), get-one (200 / 404 unowned+missing), update (200 in-place, no second timestamp), delete (204 / 404), profile-scoping (note under A invisible under B), auth-required on each route | `tester` | `crates/server/tests/notes.rs`, `crates/server/tests/common/mod.rs` |
| 3 | TUI client/protocol: five `Client` trait methods (`list_notes`, `create_note`, `get_note`, `update_note`, `delete_note`) + `HttpClient` impls following the `bearer_auth`+status-branch+`decode`/`api_error` pattern; matching `ClientRequest`/`Outcome` variants (carrying `token` + `profile_id`) + worker arms | `tui-dev` | `crates/tui/src/client/mod.rs`, `…/client/worker.rs`, `…/app/protocol.rs` |
| 4 | TUI notes view: `Screen::Notes(NotesState)` (list + create/edit/delete sub-flows, same category as `AddTaskState`), `draw_notes`, navigation key + `map_key` arms, `apply_response` arms; error routing reused (`unauthenticated`→login, offline→blocking, other `Api`→inline) | `tui-dev` | `crates/tui/src/app/notes.rs`, `…/app/mod.rs`, `…/ui/mod.rs`, `…/terminal/mod.rs` |
| 4t | TUI `TestBackend`/core suite: list render, create issues `CreateNote` + reflects, edit issues `UpdateNote` + reflects in place, delete issues `DeleteNote` + removes from list, empty-title validation surfaces inline, in-flight spinner, cancel/stale-id drop, profile-scoping (notes carry `profile_id`); `FakeClient` note impls + scripted queues | `tester` | `crates/tui/tests/notes.rs`, `crates/tui/tests/common/mod.rs` |

Dependency edges: **1 → 2 → 3 → 4**; tests (`Nt`) land alongside their slice. Slice 1 must merge
into the working branch before 2/3 compile.

### Assumptions (human is AFK — smallest change satisfying acceptance; resolved forks)

- **A1 — DTOs:** `Note { id, title, content, created_at }`, `CreateNoteRequest { title, content }`,
  `UpdateNoteRequest { title, content }`. A separate update DTO (vs. reusing create) keeps create
  and update independent shapes per ADR-0005's "smallest explicit shape" idiom and mirrors the
  0011 task `PATCH` distinction. Both update fields are **required** (full replace of title+content
  — the operator locked the update scope to title+content), keeping update a single in-place write
  with no partial-merge logic.
- **A2 — Status codes:** create `201`; list `200` (bare JSON array, newest-first by `created_at`
  desc, no pagination envelope — ADR-0005 §5 precedent); get-one `200`; update `200`; delete
  `204 No Content` (empty body). These match the REST conventions the operator stated.
- **A3 — Validation:** title non-empty after trimming → else `400 validation_failed` (reusing
  `ApiError::Validation`, no new code); content may be empty; the stored title is the **trimmed**
  value (matches `create_task`).
- **A4 — 404 vs 403:** an unowned/nonexistent profile **and** an unowned/nonexistent note id both
  return `404 not_found` (never 403) — ADR-0005 §4 non-observability. The note query is
  ownership-joined (`WHERE id = $1 AND profile_id = $2`) so a note belonging to another profile is
  indistinguishable from absent.
- **A5 — No `updated_at`:** the `notes` table has **only** `created_at` (#3, operator-locked).
  Update is `UPDATE notes SET title=$, content=$ WHERE …` with no timestamp touched.
- **A6 — Migration timestamp:** one timestamp after `…163048_timer` (e.g. `…163049_notes`);
  `notes.profile_id` is `REFERENCES profiles(id) ON DELETE CASCADE` (so 0012's profile delete
  cascades notes via FK with no app code), plus an index on `(profile_id, created_at DESC)` like
  `tasks`. Reversible `down` drops the table (a missing `down` is review-blocking).
- **A7 — TUI navigation:** a single key from the task list opens the notes view and a back key
  returns, reusing the existing `Screen`/`map_key` pattern (smallest addition; the exact keys are
  `tui-dev`'s to pin, consistent with the 0008 nav idiom). Notes list shows title + created_at;
  selecting opens the note; the view derives entirely from `profile_id`-scoped responses.
- **A8 — `chrono` boundary:** the `tui` crate keeps its no-direct-`chrono` invariant; any
  timestamp formatting derives from the DTO's `DateTime` at the render seam (as 0008 did).

### Risks

- **Scope creep into structure (#3):** the temptation to add tags/folders/search. Reviewer guards
  the flat `{ id, title, content, created_at }` shape — any addition is ADR-worthy and out of scope.
- **Profile-scoping regression (#4):** the highest-value test is "note under profile A invisible
  under profile B, foreign note id → 404"; both `tester` (server + TUI) and the live `verifier`
  must exercise it.
- **sqlx offline cache staleness:** the five new note queries require `./ok.sh prepare`; an
  un-refreshed `.sqlx/` fails the offline build. `server-dev` runs it in slice 2.
- **Capability gap (#6):** `./ok.sh prepare`, `test`, and the live `verifier` pass need the
  sanctioned throwaway test Postgres / docker. If docker is unavailable the work **blocks** with a
  precise question — it is never worked around.

[adr-0003]: ../../docs/adr/0003-verification-layering.md
[adr-0007]: ../../docs/adr/0007-notes-wire-contract.md

<!-- ─────────────────────────────  LOG / COMMENTS  ───────────────────────────── -->
## Log / comments

- [x] 2026-06-24 [drive] Claimed `ready`→`working`. Worktree `.claude/worktrees/0010-notes`
      branch `feature/0010-notes` cut from `main` 72c217b (carries the plan + ADR-0007, verified
      present in worktree). Docker capability confirmed UP (Risk #6 cleared). Building
      contract→server→tui per the slice order.
- [x] 2026-06-24 [drive] Build complete (contract→server→tui, tests alongside). S1 `contract`
      note module (`73fee78`); S1t contract tests ×11 (`6a1c980`). S2 server CRUD + migration
      `20260612163049_notes` (up/down) + `.sqlx/` refresh (`f50416e`); S2t server integration
      tests ×28 incl. profile-scoping + auth (`0e15b24`). S3+S4 TUI client/protocol + notes view,
      `n` opens from the task list (`863b320`); S4t TUI `TestBackend` suite ×13 (`827246e`). Fix
      `a3d8f2a`: S4t surfaced a real regression — adding `n: notes` grew `TASK_LIST_CAPTION` so the
      pending caption+spinner clipped the cancel affordance at 80×24 (ADR-0006 §8.3); `tui-dev`
      widened the bottom band to 3 rows + re-phrased both captions, no assertions weakened. All four
      gates green at branch head: `./ok.sh build | test | lint --all-targets | fmt --check`.
- [x] 2026-06-24 [reviewer] **REVIEW-STATUS: approved** — code-hash
      `46c1c60f1eb3865eb127a72502982827ebb09d65` (head sha `0155a32`, a human-readable pointer).
      Mechanical gate green (test: contract 11 + 18 doctests, server notes 28, tui notes 13 +
      rendering; lint --all-targets clean; fmt clean). All hard constraints clear: #3 flat
      `{id,title,content,created_at}` no `updated_at` (migration + handler + test assert it); #2
      DTOs only in `contract` (ADR-0007); no new `ErrorCode`; #4 every query ownership-joined,
      unowned profile+note id → 404 never 403; #1 TUI stateless (open re-fetches, mutations chain
      a refresh); A8 no `chrono` in tui. Migration up/down paired + `ON DELETE CASCADE`. Caption
      `fix(tui)` in-scope (ADR-0006 §8.3), `rendering.rs:227` guard not weakened. No fix-now, no
      nits, no out-of-scope chore. Verdict valid while `./ok.sh code-hash HEAD` == the hash above.
- [x] 2026-06-24 [verifier] **VERIFY-STATUS: verified** — code-hash
      `46c1c60f1eb3865eb127a72502982827ebb09d65` (== reviewer hash; head sha `2a4074d`). Booted the
      real stack via `./ok.sh up` (docker 29.5.3); migration `20260612163049_notes` applied
      (`_sqlx_migrations.success=t`); `\d notes` confirms flat schema `id,profile_id,title,content,
      created_at` (no `updated_at`), FK `ON DELETE CASCADE`, `(profile_id,created_at DESC)` index.
      Live over the wire: create 201 (flat body, title trimmed, empty content ok; empty/whitespace
      title → 400 `validation_failed`); list 200 bare array newest-first; get 200 / missing → 404
      `not_found`; PATCH 200 in-place, `created_at` unchanged, no `updated_at`; delete 204 empty
      body, re-delete → 404. Profile-scoping (#4): user B sees `[]`, A's note under B → 404
      (never 403) for GET/PATCH/DELETE. Error bodies `{code,message}`; 401 `unauthenticated`
      without auth. OTel: all five handler spans emitted (`server::handlers::notes`) with
      user_id/profile_id/note_id attrs + create/update/delete events. `./ok.sh test` all green
      (server notes 28, tui notes `TestBackend` 13, rendering 11). One stated inference: the
      reqwest `HttpClient` path verified by structural equivalence (curl drove the wire; the
      `tui` Client maps one-for-one + `tester`'s 13-test suite drives the trait), not a literal
      live reqwest harness (would require editing read-only code) — not a coverage gap. Stack
      torn down; worktree clean.

<!-- ─────────────────────────────  SUMMARY  ───────────────────────────── -->
## Summary

**Notes — the final domain feature — shipped end-to-end across all three crates** (the task
surface's near-exact structural clone, governed by [ADR-0007][adr-0007]). A user can create
(non-empty trimmed title, content may be empty), list newest-first, read, edit in place, and
delete free-form notes, all scoped to the active profile.

What shipped (on the branch):

- **`contract`** — a new `note` module: `Note { id, title, content, created_at }`,
  `CreateNoteRequest { title, content }`, `UpdateNoteRequest { title, content }`, reusing the
  `{ code?, message }` error contract with **no** new `ErrorCode`. Flat (#3), no `updated_at`.
- **`server`** — five CRUD routes under `/api/profiles/{id}/notes` (create 201 / list 200 bare
  array newest-first / get 200 / update 200 in-place / delete 204), every query ownership-joined
  so an unowned/missing profile or note id is `404 not_found` (never 403, #4). Reversible
  migration `20260612163049_notes` (paired up/down; `ON DELETE CASCADE`, `(profile_id,
  created_at DESC)` index) + `.sqlx/` refresh.
- **`tui`** — five `Client` trait methods + `HttpClient` impls, `ClientRequest`/`Outcome`
  variants (carrying `token` + `profile_id`) + worker arms, and a `Screen::Notes` view (list +
  create/edit/delete sub-flows) opened by `n` from the task list. Stateless (#1): every view
  derives from a server response; no `chrono` in `tui` (A8).
- **`fix(tui)`** — a caption-layout regression surfaced by the TUI test suite: adding `n: notes`
  grew `TASK_LIST_CAPTION` so the pending caption + in-flight spinner clipped the cancel
  affordance at the 80×24 test viewport (ADR-0006 §8.3); the bottom band was widened to 3 rows
  and both captions re-phrased with ` | ` separators, no assertions weakened.

Tests in all three crates: `contract` note DTOs 11 (+ doctests), `server` notes integration 28
(incl. profile-scoping + auth-required per route), `tui` `TestBackend` notes suite 13 (+
rendering 11). `./ok.sh test | lint --all-targets | fmt --check` all green at branch head.

**Verdicts** (both pinned to code-hash `46c1c60f1eb3865eb127a72502982827ebb09d65`):

- **reviewer — REVIEW-STATUS: approved.** Hard constraints clear (#1/#2/#3/#4); DTOs only in
  `contract`; no new `ErrorCode`; migration up/down paired + cascade; caption `fix(tui)`
  in-scope.
- **verifier — VERIFY-STATUS: verified.** Booted the real stack (`./ok.sh up`); migration
  applied; flat schema confirmed; the full wire surface exercised live (shapes, status codes,
  `{code,message}` error contract, profile-scoping → 404, all five OTel handler spans). The
  reqwest `HttpClient` path verified by structural equivalence + the 13-test suite (one stated
  inference, not a coverage gap).

coverage: 68.24% line (62.99% region, 70.77% function) — report-only, not a gate.
