---
id: 0021
title: Profiles sorted by insertion time (not alphabetically) in the Profile list
type: feature      # feature | chore
status: ready           # inbox → planned → ready → working → review → awaiting-merge → merged | blocked
priority: medium    # high | medium | low
parent: null
depends-on: [0012]  # profiles CRUD + switcher (merged)
branch: null
worktree: null
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
- [x] 2026-07-02 [orchestrator] Claimed onto `feature/0021-profiles-sorted-by-insertion-time`
  (worktree cut from main@1914d0c). **This `main` copy is frozen at the claim snapshot; the
  branch copy is authoritative until the human's merge brings it back.**
