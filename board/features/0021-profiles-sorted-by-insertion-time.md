---
id: 0021
title: Profiles sorted by insertion time (not alphabetically) in the Profile list
type: feature      # feature | chore
status: review          # inbox → planned → ready → working → review → awaiting-merge → merged | blocked
priority: medium    # high | medium | low
parent: null
depends-on: [0012]  # profiles CRUD + switcher (merged)
branch: feature/0021-profiles-sorted-by-insertion-time
worktree: .claude/worktrees/0021-profiles-sorted-by-insertion-time
created: 2026-07-02
updated: 2026-07-02
---

## Feature request

**Goal:** In the **Profile list** (and switcher), sort profiles by their **insertion
(creation) time** instead of **alphabetically**.

**Context (current behaviour to change):** the profile list/switcher (from 0012) presents
profiles in alphabetical order.

### Behaviour (acceptance)

1. The Profile list and the profile switcher render profiles in **insertion-time order**
   (**oldest-first** — see assumption below), **not** alphabetically.
2. The ordering is consistent everywhere profiles are listed.

### Assumptions / notes for the architect

- **Direction.** The request says "sorted by their insertion time" without a direction;
  **assumption: oldest-first (ascending insertion order)** — the order profiles were created.
  Flag for the operator if descending is wanted instead.
- **Ordering source + possible ADR (#2).** Prefer a deterministic **server `ORDER BY
  created_at`** over a TUI-side sort (keeps ordering authoritative server-side; #1 stateless
  TUI). Confirm the profile row/DTO already carries a creation timestamp to order by:
  - If the profile row has `created_at` and it need not appear on the wire, this is a
    **server-query-only** change with **no `contract` change** → it may be a **`chore`** (no
    behaviour-visible wire delta; ordering is observable but no shape changes — architect to
    judge whether the visible reorder counts as behaviour under the chore scope guard).
  - If ordering must be exposed on the DTO (e.g. a `created_at` field added to the profile wire
    type), that is a **`contract` change → ADR event (#2)** and it stays a `feature`.
- **Scope guard.** Left as `feature` pending the architect's sizing; downgradable to `chore` if
  it is purely a server `ORDER BY` with no wire/behaviour-contract change.

## Plan(s)

### Plan: profiles list ordered oldest-first (server `ORDER BY created_at ASC`)

**Sizing verdict — stays `feature`.** The deliverable is a single server-query direction flip
(`ORDER BY created_at DESC` → `ASC`) with **no** `contract`/wire change (#2) and **no**
domain-structure change (#3). But a visible reorder of the profile list/switcher **is
observable behaviour** — precisely the case CLAUDE.md flags ("a visible reorder IS observable
behaviour, so lean toward keeping it a `feature`"). So it fails the chore no-behaviour
invariant and remains a `feature`, carrying the full 7-clause DoD including the live
`verifier` pass. The scope guard does **not** downgrade it to `chore`.

**Findings (current state, corrects the request premise):**

- The `contract::Profile` DTO **already carries `created_at`** (RFC 3339 UTC), so ordering can
  be driven server-side with **zero wire-shape change** — no `contract-owner` slice, no ADR
  event (#2).
- The server `GET /api/profiles` query is **already** `ORDER BY created_at DESC`
  (newest-first), **not alphabetical** — the request's stated "alphabetical" premise is
  inaccurate against the codebase. Either way the well-defined deliverable is unchanged:
  **oldest-first, server-authoritative.**
- The TUI does **no** client-side sort anywhere: `ProfilesState` and the switcher render
  profiles in the exact order the server returns (the existing suite even asserts "the switcher
  mirrors exactly the server's profile list"). So flipping the server order flips both the
  Profile list and the switcher together — acceptance #2 (consistency everywhere) is satisfied
  by the single server change, with **no `tui-dev` slice**.

**Approach:** One-line server-query direction flip in `list_profiles`
(`crates/server/src/handlers/profiles.rs`): `ORDER BY created_at DESC` → `ORDER BY created_at
ASC`, plus the two stale "newest-first" doc comments (the handler doc line and the
`ProfilesState.profiles` field comment note, which is `tui`-side) corrected to "oldest-first".
Because the `Profile` DTO is unchanged, `.sqlx/` needs no refresh for a shape change; the
column set (`id, name, created_at`) is unchanged, so the committed query cache still matches
(the dev confirms with `./ok.sh prepare` only if sqlx flags a cache mismatch on the changed
SQL text). The tracer bullet is the query itself flowing to a live `GET /api/profiles`; the
`tester` slice pins the order so it cannot silently regress.

**ADR:** none. No `contract`/wire change (#2), no domain structure (#3), no auth (#5), no
timer authority, nothing in Hard constraints is shaped. The ordering *direction* diverges from
the tasks/notes precedent (both `DESC`/newest-first) — that divergence is a small,
low-risk product decision recorded here in Assumptions, not a contract-shaping decision, so it
does not clear the ADR bar (`plan` skill §2). ADR-0009 (profile mutations) stated no ordering
and needs no amendment.

**Slices:**

1. [server-dev] Flip `list_profiles` to `ORDER BY created_at ASC` (oldest-first) and correct the
   handler's `newest-first` doc line to `oldest-first`. — files:
   `crates/server/src/handlers/profiles.rs` (and `.sqlx/` only if `./ok.sh prepare` flags the
   changed query text; column set is unchanged). Confirm `./ok.sh build`/`lint`/`fmt` green.
2. [tui-dev] **Doc-comment-only:** correct the stale `profiles: Vec<Profile>` field comment from
   "newest-first" to "oldest-first" so the TUI's rendered-order note matches the server. No
   logic change (the TUI already renders server order verbatim). — files:
   `crates/tui/src/app/profiles.rs`. *(If preferred, `server-dev` cannot touch `tui/`; this is
   its own trivial owned slice. It is a comment, not behaviour — but it lives in a `tui`-owned
   file, so it must be the `tui-dev`'s edit.)*
3. [tester] Add a server integration assertion that `GET /api/profiles` returns the account's
   profiles **oldest-first** by `created_at`: create ≥3 profiles with distinct creation order and
   assert the response order is ascending (and stable — not alphabetical, e.g. name the profiles
   so alpha-order differs from insertion-order to catch a regression to either alpha or `DESC`).
   — files: `crates/server/tests/profiles.rs`. The existing `tui` profile suite (which asserts
   the switcher mirrors the server's returned order) needs **no change** — it already validates
   acceptance #2 by construction; the `tester` confirms it stays green.

**Assumptions:**

- **Direction: oldest-first (ascending `created_at`),** taken verbatim from the request's stated
  assumption. This is the request's own explicit assumption, not a genuine blocking fork, so per
  the AFK ambiguity policy it is recorded here and implemented rather than blocked. If the
  operator wants newest-first instead, that is the current `DESC` behaviour — flag on the item
  and this reduces to "no change".
- **Ordering is server-authoritative** (server `ORDER BY`), honouring #1 (stateless TUI): the
  TUI adds no client-side sort; it renders exactly what the server returns.
- **Direction diverges from tasks/notes precedent by design.** Tasks and notes order
  `DESC`/newest-first; profiles will order `ASC`/oldest-first per this request. This is a
  deliberate, isolated product choice (a short, stable account-level list where creation order
  reads naturally), not a contract decision — recorded here rather than in an ADR.
- **No `.sqlx/` shape churn.** The changed query returns the identical column set; only the
  `ORDER BY` clause text changes. `./ok.sh prepare` is run only if sqlx flags the changed SQL.

**Risks:**

- **Low blast radius.** A single `ORDER BY` direction on one read query; no wire shape, no
  mutation path, no auth, no profile-scoping change. Profile isolation (#4) is untouched (the
  `WHERE user_id = $1` predicate is unchanged).
- **Premise mismatch (already handled):** the request says "alphabetical today" but the code is
  `DESC`/newest-first today; the deliverable (oldest-first) is unambiguous either way, so this
  does not block — noted for the operator.
- **Stale doc comments** ("newest-first" in two places) would otherwise drift from behaviour;
  slices 1 and 2 correct both, split by file ownership.
- **Verifier scope:** the live pass exercises `GET /api/profiles` against a live server and
  confirms the returned array is oldest-first (and the reqwest client path/OTel span), per DoD
  clause 4. Interactive switcher render stays with the `tester` `TestBackend` suite (ADR-0003).

## Log / comments

- [ ] 2026-07-02 [human] Filed from an operator interface-improvements request; see acceptance above.
- [x] 2026-07-02 [architect] Planned. Investigation: `contract::Profile` already carries
  `created_at` (no wire change / no ADR); server `list_profiles` is currently `ORDER BY
  created_at DESC` (newest-first, **not** alphabetical as the request states); the TUI does no
  client-side sort (renders server order verbatim). Sized as **`feature`** (a visible reorder is
  observable behaviour — fails the chore no-behaviour invariant). Direction: **oldest-first
  (ASC)** per the request's stated assumption. Plan is server-only ordering flip + doc-comment
  fixes + a server ordering test; no `contract-owner` slice, no ADR.
- [x] 2026-07-02 [orchestrator] Claimed `ready`→`working`. Worktree cut from `main` @1914d0c
  (carries the plan). Branch `feature/0021-profiles-sorted-by-insertion-time`. Session: drive
  cycle 0021. Build dispatch: server-dev (query flip + handler doc) → tui-dev (state doc-comment)
  → tester (server ordering assertion); no contract-owner slice.
- [x] 2026-07-02 [server-dev] Slice 1 done. Flipped `list_profiles` in
  `crates/server/src/handlers/profiles.rs` to `ORDER BY created_at ASC` (oldest-first,
  line 65) and corrected the handler doc line (lines 56–57) from "newest-first" to
  "oldest-first (ascending insertion order)". Column set unchanged, but the SQL text changed,
  so the offline `.sqlx/` cache needed regeneration: ran `./ok.sh prepare` (booted the repo's
  sanctioned throwaway test Postgres via docker compose, torn down after) — the old
  `DESC` cache entry was deleted and a new `ASC` entry committed. Gates green in-worktree:
  `./ok.sh build`, `./ok.sh lint`, `./ok.sh fmt --check`. No `contract`/wire, no
  domain-structure, no profile-scoping change (`WHERE user_id = $1` untouched).
- [x] 2026-07-02 [tui-dev] Slice 2 done (doc-comment only, no logic). Corrected the stale
  `ProfilesState.profiles` field comment in `crates/tui/src/app/profiles.rs` (line 79) from
  "Profiles as returned by the server, newest-first." to note the profiles render in the exact
  order the server returns (oldest-first, ascending insertion order) and that the TUI does no
  client-side sort (hard-constraint #1: server is authoritative). No logic change — the TUI
  already renders server order verbatim, so `ProfilesState`/switcher behaviour is unchanged.
  Gates green in-worktree: `./ok.sh build`, `./ok.sh lint --lib --bins`, `./ok.sh fmt --check`.
  No `Client`/`ClientRequest`/`Outcome`/state-field surface touched, so the tester-owned
  `crates/tui/tests/` harness is not stranded by this slice.
- [x] 2026-07-02 [tester] Slice 3 done. Added `list_profiles_ordered_oldest_first` to
  `crates/server/tests/profiles.rs` (new `// ---- ordering` section, ~50 lines before the
  per-account-uniqueness section). It reuses the existing `create_profile`/`list_profiles`
  helpers: after the default "work" it inserts "zulu" → "alpha" → "mike" — an insertion order
  distinct from BOTH alphabetical (`alpha, mike, work, zulu`) and newest-first/DESC (`mike,
  alpha, zulu, work`) — and asserts the returned names are `["work","zulu","alpha","mike"]`,
  the created ids follow insertion order, and `created_at` is non-decreasing (`is_sorted`), so
  a regression to either alphabetical or `DESC` fails. Gates green in-worktree: `./ok.sh test`
  (profiles suite 20→21, whole workspace 0 failed), `./ok.sh lint` clean (rewrote the
  monotonic check off `windows()[..]` to `timestamps.is_sorted()` to satisfy
  `clippy::indexing_slicing`), `./ok.sh fmt --check` clean. Confirmed the existing `tui`
  profile suite (`login_flow_fetches_profiles_and_enters_task_list`,
  `profiles_tab_list_command_keys`) stays green untouched — acceptance #2 needs no `tui`
  change. Test-only change: no source touched.
- [x] 2026-07-02 [reviewer] **REVIEW-STATUS: approved** — code-tree hash
  `b8591d70250155b79c209d4b14b59f6b2abb00fd` (code commit `831634b`). All gates green
  (build/test/lint/fmt); query flip to `ORDER BY created_at ASC` correct; `.sqlx/` cache
  regenerated (old DESC removed, new ASC added, hash-consistent); ordering test catches
  regression to both alphabetical and newest-first (three mutually-distinct orderings). No
  `contract`/wire (#2), no domain-structure (#3), stateless-TUI (#1) and profile-scoping (#4)
  intact; no ADR required; no migration needed. No findings, no out-of-scope nits.
- [x] 2026-07-02 [verifier] **VERDICT: verified** — code-tree hash
  `b8591d70250155b79c209d4b14b59f6b2abb00fd` (code commit `831634b`). Booted the stack
  (`./ok.sh up`) live; `GET /api/profiles` returned `[work, zulu, alpha, mike]` = oldest-first
  by `created_at` (NOT alphabetical, NOT newest-first), HTTP 200, shape `{id,name,created_at}`
  unchanged. Account-scoping (#4) confirmed (account B sees only its own). Error contract
  intact (401 `{code:"unauthenticated",…}` on missing/garbage token). OTel `list_profiles`
  span exported. Full `./ok.sh test` green (37 ok lines, 0 fail), incl.
  `list_profiles_ordered_oldest_first` and the `tui` `TestBackend` switcher suite (ADR-0003).
  No gaps. Torn down with `./ok.sh down` (no `-v`; shared volume preserved).
