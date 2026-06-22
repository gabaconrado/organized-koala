---
id: 0001
title: Foundational vertical slice (auth + profile + minimal TODO)
status: merged       # inbox → planned → ready → working → review → awaiting-merge → merged | blocked
priority: high       # high | medium | low
children: [0002, 0003, 0004]
branch: null         # umbrella — work happens on the children's branches
worktree: null
created: 2026-06-10
updated: 2026-06-22
---

## Feature request

**Goal:** I can register/log in, have a default profile that I chose the name, and add + list + close
TODO items in that profile — end-to-end through the TUI talking to a live server.

**Why:** The smallest useful slice that proves the whole loop: TUI ↔ `contract` ↔ server ↔
Postgres, plus local auth and profile-namespacing. It de-risks every later feature.

**Acceptance criteria:**

- [x] `./ok.sh up` brings up the server + Postgres;
- [x] It is not necessary to run any command in the host (like `./ok.sh migrate`) for the system to
      run correctly; Migrations should be handled internally by the application
- [x] A user can register and log in with username/email + password (argon2 + JWT).
- [x] On first login the user has a default profile with a name chosen by the user; TODOs are scoped
      to it.
- [x] In the TUI: add a task (Title + Description), list tasks with a done/undone marker,
      and mark a task done (sets Status + Closed-at).
- [x] All wire shapes (auth, profile, task DTOs, error payload) live in the `contract` crate.
- [x] Basic traces for audit/debugging, all endpoints instrumented with spans and events for mutations
      in INFO level + errors
- [x] Errors return HTTP status + `{ code?, message }`.

**Out of scope:** Pomodoro, Notes, multiple-profile UX, deletion/editing of tasks, on-disk TUI
state, SSO. (Pomodoro is also blocked on ADR-0002.)

**Constraints / non-functional:** TUI stateless (server online required); domain stays flat;
queries profile-scoped; sqlx offline mode; tests cover the public API in their own files.

**Priority:** high

<!-- written by `architect` via the `plan` skill -->
## Plan(s)

### Plan: fan-out into three dependent slices (2026-06-11, architect)

**Approach:** Tracer-bullet, built in dependency order across three child items so each
lands reviewable and (where live) verifiable on its own: **(1)** freeze the wire seam,
**(2)** stand the server + stack up and verify it over live HTTP, **(3)** close the loop
with the TUI. This item is the **umbrella**: it carries the end-to-end acceptance criteria,
which are asserted collectively at 0004's verification; 0001 advances to `awaiting-merge`
in lockstep with 0004, and the human's merges of the children complete it.

**ADR:** [ADR-0005][adr-0005] (foundational wire contract: register-bootstraps-profile,
login/JWT shape, profile-scoped routing with 404-for-unowned, task DTOs/endpoints, the
initial stable error-code set, `healthz`) — **written and accepted with this plan**.
Migrations/admin-CLI behaviour is already settled by [ADR-0004][adr-0004]; verification
routing by [ADR-0003][adr-0003]. ADR-0002 (timer authority) is **not** needed — Pomodoro is
out of scope here.

**Slices (one child item each; dependency order is strict):**

1. [contract-owner] **[0002 — contract crate + workspace restructure][feat-0002]** — files:
   `Cargo.toml`, `crates/contract/**` (placeholder crate removed). No dependency; workable
   immediately.
2. [server-dev + platform-dev + tester] **[0003 — server: auth, default profile, tasks,
   migrations, docker stack][feat-0003]** — files: `crates/server/**`, `deploy/**`,
   `ok.sh`, `.sqlx/`. Depends on 0002.
3. [tui-dev + tester] **[0004 — TUI: register/login, task add/list/close][feat-0004]** —
   files: `crates/tui/**`. Depends on 0003 (live stack needed for verification).

**Assumptions (master list; details live in the children):**

- The default profile's name is captured **at registration** (`profile_name` in the
  register request) and created atomically with the user — the smallest design that
  guarantees "on first login the user has a default profile with a name chosen by the
  user" with zero client-side onboarding state. (ADR-0005 §2.)
- Usernames may not contain `@`, keeping the username-or-email login identifier
  unambiguous. (ADR-0005 §2, grill finding.)
- Profile scoping is structural: routes nest under `/api/profiles/{id}/…` and unowned
  profiles return 404 `not_found` — never 403 — so cross-account existence never leaks.
  (ADR-0005 §4.)
- "Basic traces" is satisfied by `tracing` spans on all endpoints + INFO mutation/error
  events **with minimal OTLP export to a collector in the compose stack** (the verifier
  DoD names OTel spans); dashboards/sampling stay in the backlog "Observability wiring"
  item.
- No refresh tokens / logout endpoint; JWT TTL default 24 h; expiry → 401 → TUI returns to
  login. Task close is idempotent; task list is a bare JSON array, newest-first. (ADR-0005
  §3, §5.)
- No new shared crates this slice (observability init stays server-local), so no new dev
  agents are required.

**Risks:**

- Contract churn discovered in 0003/0004 ripples to both consumers — mitigated by freezing
  shapes in ADR-0005 first; any deviation re-enters via `architect` and amends the ADR.
- 0003 is the heavyweight item (auth + schema + stack); its internal ordering (CLI/migrate
  seam → auth → tasks) localizes failure. If it bogs down, `architect` can split it further
  on re-entry.
- Auth correctness (argon2 params, JWT `exp` enforcement, 404-not-403 scoping) is the
  highest-severity failure mode — flagged for reviewer attention in 0003.

<!-- append-only; dated, AUTHORED entries. Human feedback = an UNCHECKED box: [ ] unhandled, [x] addressed + re-reviewed (the ONLY re-entry signal). -->
## Log / comments

- 2026-06-10 [eng-manager] item seeded during bootstrap; status `inbox`. Likely splits into
  sub-items (contract → server(auth+profile+tasks) → tui) when the architect plans it.
- 2026-06-11 [human] feature request enriched by the human
- 2026-06-11 [architect] planned via the `plan` skill: fanned out into children
  [0002][feat-0002] → [0003][feat-0003] → [0004][feat-0004] (each `ready`, dependency
  gates in their logs); authored and accepted [ADR-0005][adr-0005] (foundational wire
  contract) and indexed it in `docs/decisions.md`; grilled the riskiest forks
  (profile-bootstrap-at-registration, 404-for-unowned scoping, username `@` ban,
  idempotent close) and folded outcomes into Assumptions. This item stays `planned` as the
  **umbrella** — it carries no directly-workable code slice; it advances when 0004 does
  and its criteria are checked at 0004's end-to-end verification. `board/README.md`
  regeneration is left to `eng-manager`.
- 2026-06-22 [orchestrator] all three children are now **merged** (0002 contract, 0003 server,
  0004 TUI). The umbrella's end-to-end acceptance is satisfied collectively, as planned: the
  stack comes up via `./ok.sh up` with migrations run internally (no host command — ADR-0004),
  register/login is argon2 + JWT, the named default profile is created at registration with
  profile-scoped TODOs, the TUI does add/list/close with done markers, every wire shape lives
  in `contract`, endpoints are span-instrumented with OTel export, and errors carry
  `{ code?, message }` — all exercised live during 0004's verification (and 0003's before it).
  All acceptance boxes checked; `planned` → `merged`. The foundational tracer bullet is closed.
  (Responsive-UI work is the separate follow-up 0005 — not in this slice's scope.)

[adr-0003]: ../../docs/adr/0003-verification-layering.md
[adr-0004]: ../../docs/adr/0004-migration-authority-and-binary-cli.md
[adr-0005]: ../../docs/adr/0005-foundational-wire-contract.md
[feat-0002]: ./0002-contract-crate.md
[feat-0003]: ./0003-server-auth-profile-tasks.md
[feat-0004]: ./0004-tui-foundational.md

<!-- written at end of cycle; what the human reviews -->
## Summary

**The foundational vertical slice is complete and on `main`.** Built as a tracer bullet across
three dependency-ordered children, each reviewed and (where live) verified on its own:

- **[0002][feat-0002]** — the `contract` crate froze every wire shape (auth, profile, task DTOs,
  the `{ code?, message }` error payload), the single source of truth both other crates consume.
- **[0003][feat-0003]** — the `organized-koalad` server: argon2 + JWT auth, register-bootstraps-a-
  named-default-profile, profile-scoped tasks (404-for-unowned, no existence leak), migrations
  owned by the binary and run as a compose one-shot (no host command), OTel span instrumentation,
  and the `./ok.sh up` docker stack.
- **[0004][feat-0004]** — the `organized-koala` TUI: register/login → auto-selected profile →
  task add/list/close with done markers, stateless (server-online required), error-code branching
  per ADR-0005, with an ADR-0003 layer-2 `TestBackend` suite.

End-to-end acceptance was asserted collectively at 0004's live verification (full reqwest path,
exact error wire strings, profile-scoping, persistence across restart, OTel spans received).
Governed by [ADR-0005][adr-0005] (wire contract), with [ADR-0003][adr-0003] (verification
layering) and [ADR-0004][adr-0004] (migration authority). Follow-on work — responsive TUI
([0005](./0005-tui-responsive-event-loop.md)), Notes, Pomodoro (gated on ADR-0002), multi-profile
UX — is tracked as separate items.
