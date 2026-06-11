---
id: 0001
title: Foundational vertical slice (auth + profile + minimal TODO)
status: inbox        # inbox → planned → ready → working → review → awaiting-merge → merged | blocked
priority: high       # high | medium | low
branch: null         # feature/0001-foundational-slice once a worktree is cut
worktree: null       # .claude/worktrees/0001-foundational-slice
created: 2026-06-10
updated: 2026-06-10
---

## Feature request

**Goal:** I can register/log in, have a default profile that I chose the name, and add + list + close
TODO items in that profile — end-to-end through the TUI talking to a live server.

**Why:** The smallest useful slice that proves the whole loop: TUI ↔ `contract` ↔ server ↔
Postgres, plus local auth and profile-namespacing. It de-risks every later feature.

**Acceptance criteria:**

- [ ] `./ok.sh up` brings up the server + Postgres;
- [ ] It is not necessary to run any command in the host (like `./ok.sh migrate`) for the system to
      run correctly; Migrations should be handled internally by the application
- [ ] A user can register and log in with username/email + password (argon2 + JWT).
- [ ] On first login the user has a default profile with a name chosen by the user; TODOs are scoped
      to it.
- [ ] In the TUI: add a task (Title + Description), list tasks with a done/undone marker,
      and mark a task done (sets Status + Closed-at).
- [ ] All wire shapes (auth, profile, task DTOs, error payload) live in the `contract` crate.
- [ ] Basic traces for audit/debugging, all endpoints instrumented with spans and events for mutations
      in INFO level + errors
- [ ] Errors return HTTP status + `{ code?, message }`.

**Out of scope:** Pomodoro, Notes, multiple-profile UX, deletion/editing of tasks, on-disk TUI
state, SSO. (Pomodoro is also blocked on ADR-0002.)

**Constraints / non-functional:** TUI stateless (server online required); domain stays flat;
queries profile-scoped; sqlx offline mode; tests cover the public API in their own files.

**Priority:** high

<!-- written by `architect` via the `plan` skill -->
## Plan(s)

<!-- append-only; dated, AUTHORED entries. Human feedback = an UNCHECKED box: [ ] unhandled, [x] addressed + re-reviewed (the ONLY re-entry signal). -->
## Log / comments

- 2026-06-10 [eng-manager] item seeded during bootstrap; status `inbox`. Likely splits into
  sub-items (contract → server(auth+profile+tasks) → tui) when the architect plans it.
- 2026-06-11 [human] feature request enriched by the human

<!-- written at end of cycle; what the human reviews -->
## Summary
