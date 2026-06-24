---
id: 0012
title: Profiles create/update/delete + TUI switcher (delete cascades; last-profile guard)
type: feature      # feature | chore
status: planned         # inbox → planned → ready → working → review → awaiting-merge → merged | blocked
priority: medium    # high | medium | low
parent: null
depends-on: []      # ADR-0009 lands on `main` with this plan. See sequencing note re: 0010 (notes cascade).
branch: null
worktree: null
created: 2026-06-24
updated: 2026-06-24
---

## Feature request

**Goal:** Complete profile management. Today the only profile surface is `GET /api/profiles`
(list) plus the register-time default-profile bootstrap (ADR-0005 §2/§4). Add create, rename,
and delete, plus a TUI profile-picker/switcher.

**Surface to build (final shapes pinned in the plan under [ADR-0009][adr-0009]):**

- `server` — `POST /api/profiles` (create, returns `201 Profile`); `PATCH /api/profiles/{id}`
  (rename, `200 Profile`); `DELETE /api/profiles/{id}` (`204`). Delete **cascades** the profile's
  tasks **and** notes (a profile is a namespace, #4). **Cannot delete the last remaining profile**
  (the account must keep ≥1 namespace) → `409`. Profile **names are unique per account** → `409`
  on a duplicate create/rename. A reversible migration adds the unique constraint.
- `contract` — request DTOs for create/rename (`CreateProfileRequest { name }`,
  `UpdateProfileRequest { name }`), and **two new error codes** for the new conflict cases.
- `tui` — a profile-picker/switcher view. **"Switch" is purely TUI client state** (which
  `profile_id` the client scopes to) — there is **NO** server switch endpoint (operator-locked).

**Acceptance criteria:**

- [ ] A user can create a profile (non-empty trimmed, unique name → `201`; duplicate name →
      `409`), rename one (`200`; duplicate name → `409`), and delete one (`204`).
- [ ] Deleting a profile cascades **both** its tasks and its notes (#4 namespace). After delete,
      neither the profile nor its tasks/notes are reachable.
- [ ] Deleting the **last** remaining profile is refused (`409`); the account always retains ≥1
      profile.
- [ ] Profile **names are unique per account** (enforced by a DB unique constraint + mapped at the
      handler boundary); cross-account name collisions are allowed.
- [ ] The TUI offers a switcher that lists the account's profiles and lets the user pick the active
      one; switching is **client-side only** (changes which `profile_id` the TUI scopes subsequent
      task/note calls to) — **no** server "switch" call, no client persistence (#1).
- [ ] Full `feature` Definition of done: `./ok.sh test | lint | fmt --check` green; `reviewer`
      approved (pinned to `./ok.sh code-hash`); live `verifier` pass exercising the server API +
      reqwest path (create/rename/delete, cascade, last-profile guard, name-uniqueness conflict,
      error contract, OTel spans); the `tui` change covered by the `TestBackend` suite
      ([ADR-0003][adr-0003]).
- [ ] The contract change (new DTOs + two new error codes) carries [ADR-0009][adr-0009].

**Out of scope (would need an ADR — #3/#4):** cross-profile reads/writes, profile sharing, a
server-side "active profile" concept or switch endpoint, per-profile settings beyond name,
soft-delete/trash.

<!-- feature: needs an `architect` plan (`plan` skill) writing a `## Plan(s)` block before code. -->

<!-- ─────────────────────────────  ARCHITECT PLAN  ───────────────────────────── -->
## Plan(s)

Planned by `architect` under [ADR-0009][adr-0009] (profile mutations: create/rename/delete,
delete-cascade, last-profile guard, per-account name uniqueness; new ADR referencing ADR-0005,
committed to `main` with this plan before any worktree is cut). This ADR adds **two** error codes
to the ADR-0005 §6 set (append-only is itself an ADR event).

### Approach

Tracer-bullet contract→server→tui. The new routes sit alongside the existing `GET /api/profiles`
in `app.rs`, all ownership-scoped on the `AuthUser`. Delete-cascade is achieved by the **FK
`ON DELETE CASCADE`** already on `tasks.profile_id` (and added on `notes.profile_id` in 0010) —
deleting the profile row cascades both children at the DB level, no app fan-out. The last-profile
guard and name-uniqueness are enforced at the boundary (count-check before delete; a unique index
mapped to a conflict code). **"Switch" never reaches the server** — it is the TUI choosing which
`profile_id` to send (#1: held in memory for the process lifetime, exactly as the default profile
id is today, ADR-0005 Consequences).

### ADR

**[ADR-0009][adr-0009] — Profile mutations: create / rename / delete-cascade / last-profile guard
/ per-account name uniqueness** (new; references ADR-0005 §2/§4/§6). Fixes: the create/rename DTOs,
the three routes + status codes, the delete-cascade-via-FK decision (tasks **and** notes), the
last-profile invariant (`409`), per-account name uniqueness (DB unique index + boundary mapping),
the **two new error codes**, and the decision that profile-switching is **client-side only** (no
server endpoint). Committed to `main` with this item.

### Slices (dependency-ordered: contract → server → tui → tester alongside)

| # | Slice | Agent | files |
| --- | --- | --- | --- |
| 1 | `contract`: add `CreateProfileRequest { name }`, `UpdateProfileRequest { name }` (derives + rustdoc + doctests, re-export from `lib.rs`); add **two `ErrorCode` variants** `ProfileNameTaken` and `LastProfile` to `error/mod.rs` (extend `as_str`/`From<&str>`/doctest, append-only) | `contract-owner` | `crates/contract/src/profile/mod.rs`, `crates/contract/src/error/mod.rs`, `crates/contract/src/lib.rs` |
| 1t | `contract` tests: profile request DTO round-trip/exact-shape; new error codes round-trip + `as_str` + unknown-preservation still holds | `tester` | `crates/contract/tests/profile.rs`, `crates/contract/tests/error.rs` |
| 2 | `server`: migration (up/down) adding `UNIQUE (user_id, name)` on `profiles`; `create_profile`/`rename_profile`/`delete_profile` handlers in `handlers/profiles.rs`; map the new `ApiError` variants (`ProfileNameTaken`→409, `LastProfile`→409) in `error.rs`; route wiring in `app.rs` (`.post(create_profile)` on `/api/profiles`, `.patch(rename_profile).delete(delete_profile)` on `/api/profiles/{id}`); last-profile count-guard; `./ok.sh prepare` | `server-dev` | `crates/server/migrations/<ts>_profile_name_unique.{up,down}.sql`, `crates/server/src/handlers/profiles.rs`, `…/app.rs`, `…/error.rs`, `.sqlx/` |
| 2t | Server integration tests: create (201 / duplicate→409 ProfileNameTaken / empty-title→400); rename (200 / duplicate→409 / unowned→404); delete (204 / last-profile→409 LastProfile / unowned→404); **cascade — create profile, add a task AND a note, delete profile, assert both gone**; cross-account same-name allowed; auth-required | `tester` | `crates/server/tests/profiles.rs`, `crates/server/tests/common/mod.rs` |
| 3 | TUI client/protocol: `create_profile`/`rename_profile`/`delete_profile` `Client` methods + `HttpClient` impls; `ClientRequest`/`Outcome` variants + worker arms; (list_profiles already exists) | `tui-dev` | `crates/tui/src/client/mod.rs`, `…/client/worker.rs`, `…/app/protocol.rs` |
| 4 | TUI switcher: `Screen::Profiles(ProfilesState)` listing the account's profiles with pick-active (sets the in-memory `active_profile_id`, re-scoping subsequent task/note calls — **client-side switch, no server call**), create/rename/delete sub-flows, navigation + `map_key`, `apply_response` arms; surface `ProfileNameTaken`/`LastProfile` inline | `tui-dev` | `crates/tui/src/app/profiles.rs`, `…/app/mod.rs`, `…/ui/mod.rs`, `…/terminal/mod.rs` |
| 4t | TUI `TestBackend`/core suite: switcher lists profiles, picking one re-scopes the active profile_id (next `ListTasks` carries the new id) **without any server switch call**, create issues `CreateProfile` + reflects, rename issues `UpdateProfile`, delete issues `DeleteProfile` + removes, duplicate-name→inline `ProfileNameTaken`, last-profile delete→inline `LastProfile`, in-flight spinner, cancel/stale-id drop; `FakeClient` profile impls | `tester` | `crates/tui/tests/profiles.rs`, `crates/tui/tests/common/mod.rs` |

Dependency edges: **1 → 2 → 3 → 4**; tests alongside. Slice 1 must merge before 2/3 compile.

### Assumptions (human is AFK — smallest change satisfying acceptance; resolved forks)

- **A1 — DTOs:** `CreateProfileRequest { name }` and a separate `UpdateProfileRequest { name }`
  (mirrors the create/update split used for tasks/notes; both carry the single editable field —
  name). `Profile` itself is unchanged.
- **A2 — Two new error codes** (append-only extension of ADR-0005 §6, hence an ADR event):
  `profile_name_taken` (409) — create/rename to a name the account already uses; and
  `last_profile` (409) — refused delete of the only remaining profile. New `ErrorCode` variants
  `ProfileNameTaken`/`LastProfile`, new `ApiError` variants mapped at the boundary. The append-only
  forward-compat (`ErrorCode::Unknown`) guarantees older TUIs still parse these.
- **A3 — Status codes:** create `201` (returns the `Profile`); rename `200` (returns the updated
  `Profile`); delete `204 No Content`.
- **A4 — Validation:** name non-empty after trimming → else `400 validation_failed`; stored
  trimmed. Uniqueness is **per account** (the `UNIQUE (user_id, name)` index); the conflict is
  mapped to `409 profile_name_taken` by catching the unique-violation at the handler (sqlx unique
  constraint error → the typed `ApiError`), not by a pre-check race.
- **A5 — Delete-cascade via FK:** `tasks.profile_id` already has `ON DELETE CASCADE`;
  `notes.profile_id` is created with `ON DELETE CASCADE` in 0010. So `DELETE FROM profiles WHERE
  id=$1 AND user_id=$2` cascades both children at the DB level — **no app-level fan-out**. (See the
  sequencing note: if 0012 lands before 0010, the notes cascade is automatically satisfied the
  moment the `notes` table exists; nothing in 0012 references the `notes` table directly.)
- **A6 — Last-profile guard:** delete first checks the account's profile count; if deleting would
  drop it to zero, return `409 last_profile` without deleting. Implemented as a single guarded
  statement (`DELETE … WHERE … AND (SELECT count(*) FROM profiles WHERE user_id=$2) > 1`) or a
  count-then-delete in one transaction — `server-dev` picks the race-safe form. The account always
  retains ≥1 namespace (ADR-0005 §2 invariant: a user without a profile cannot exist).
- **A7 — Switch is client-side only (operator-locked, #1):** the TUI holds `active_profile_id` in
  memory (already true today for the default profile, ADR-0005 Consequences). "Switch" rebinds that
  field and re-issues the scoped reads; **no** server endpoint, **no** persistence. Deleting the
  currently-active profile re-points the TUI to another profile from the list (smallest behaviour;
  exact choice — e.g. first remaining — is `tui-dev`'s to pin).
- **A8 — 404 vs 403:** rename/delete of an unowned/missing profile → `404 not_found` (ADR-0005 §4),
  ownership-joined on `user_id`. The last-profile and name-taken conflicts are `409` (the resource
  is owned; the operation conflicts with an invariant), distinct from the 404 not-owned case.
- **A9 — Migration:** one timestamp after 0010's notes migration; adds `UNIQUE (user_id, name)` to
  `profiles`. Reversible `down` drops the constraint. (If a pre-existing account somehow held
  duplicate names the constraint creation would fail — none can today: the only profile-creating
  path is register, one name per account, so the constraint applies cleanly.)

### Risks

- **Cascade-completeness (#4):** the headline test is "create profile + a task + a note, delete
  profile, both children gone." If 0010 has not merged, `notes` may not exist yet in this branch —
  see the sequencing note; the test must run against a tree where `notes` exists. Both `tester` and
  the live `verifier` exercise the full cascade.
- **Name-uniqueness race:** mapping the DB unique-violation (rather than a TOCTOU pre-check) is the
  race-safe path; reviewer guards that the handler catches the constraint error and maps it to
  `ProfileNameTaken`, not a generic 500.
- **Last-profile invariant under concurrency:** the guard must be atomic (single statement or
  in-transaction count) so two concurrent deletes can't both pass the check and empty the account.
- **#1 leak risk:** the active-profile choice is in-memory render/session state only — reviewer
  guards that nothing about the switcher persists to disk or adds a server "active profile" concept.
- **Capability gap (#6):** `./ok.sh prepare`/`test`/live `verifier` need the sanctioned test
  Postgres / docker; unavailable ⇒ **block** with a precise question, never worked around.

[adr-0009]: ../../docs/adr/0009-profile-mutations.md
[adr-0003]: ../../docs/adr/0003-verification-layering.md
