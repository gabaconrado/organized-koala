---
id: 0004
title: TUI — register/login, default profile, task add/list/close (slice 3 of 0001)
status: ready        # inbox → planned → ready → working → review → awaiting-merge → merged | blocked
priority: high       # high | medium | low
parent: 0001
depends-on: [0003]
branch: null         # feature/0004-tui-foundational once a worktree is cut
worktree: null       # .claude/worktrees/0004-tui-foundational
created: 2026-06-11
updated: 2026-06-11
---

## Feature request

**Goal:** The `organized-koala` TUI completes the loop: a user registers (choosing the
default profile name) or logs in, lands in their profile's task list, adds a task
(Title + Description), sees done/undone markers, and marks a task done — all against the
live server, holding no local persistence.

**Why:** Slice 3 of [0001][feat-0001]: closes the tracer bullet
TUI ↔ `contract` ↔ server ↔ Postgres; completing it satisfies the parent item's
acceptance criteria end-to-end.

**Acceptance criteria:**

- [ ] `crates/tui` (binary `organized-koala`) exists; `ratatui` + `crossterm` + `reqwest`;
      all wire types imported from `contract` (no local DTOs — hard-constraint #2).
- [ ] Auth screen: register (username, email, password, profile name) and login
      (identifier + password); on success the TUI fetches `GET /api/profiles` and enters
      the (single) profile's task list.
- [ ] Task list view: tasks newest-first with done/undone markers; add-task flow
      (Title + Description); mark-done sends `…/close` and re-renders from the server
      response (Status + Closed-at set).
- [ ] Error handling branches on the ADR-0005 `code`: `unauthenticated` → back to login;
      `validation_failed` → inline message; server offline/unreachable → clear blocking
      message with retry (no cached data — hard-constraint #1).
- [ ] No on-disk or cross-run state; JWT + active profile id live in process memory only.
- [ ] `tester`'s `ratatui` `TestBackend` suite covers view/update, keybindings, and
      error-code branching per [ADR-0003][adr-0003];
      `./ok.sh test|lint|fmt --check` green.

**Out of scope:** profile switching/creation UX, task edit/delete, Notes, Pomodoro,
any persistence, theming polish.

<!-- written by `architect` via the `plan` skill -->
## Plan(s)

### Plan: TUI client (2026-06-11, architect)

**Approach:** A thin stateless client over the frozen contract: an HTTP client module
(reqwest, typed on `contract` DTOs, mapping `ErrorBody` to a typed client error) and an
app core (screen state machine: Auth → TaskList; pure update functions over events) kept
strictly separable so `tester` can drive the whole interactive surface through
`TestBackend` with the server mocked (ADR-0003 layer 2), while the verifier exercises the
reqwest path against the live 0003 stack (layer 1).

**ADR:** [ADR-0005][adr-0005] (accepted) +
[ADR-0003][adr-0003] verification routing; no new ADR.

**Slices:**

1. [tui-dev] Scaffold `crates/tui` (`new-crate` skill); reqwest client module over
   `contract` types (register/login/profiles/tasks/close, `healthz` probe, `ErrorBody` →
   typed error) — files: `crates/tui/**`, root `Cargo.toml` member.
2. [tui-dev] App core + views: auth screen (register/login forms), task-list screen
   (markers, add-task input flow, mark-done), event/keybinding mapping, error-code
   branching, server-offline handling — files: `crates/tui/src/**`.
3. [tester] `TestBackend` suite with the server mocked: keybinding → action mapping,
   buffer-snapshot rendering of auth + task list, error-code branches
   (`unauthenticated` → login, `validation_failed` inline, offline message), statelessness
   (every view derives from a (mock) server response) — files: `crates/tui/tests/**` and
   module-sibling `tests.rs` files.
4. [verifier] Per ADR-0003: live pass over the reqwest client path against `./ok.sh up`,
   plus the delegation handshake — confirm the `TestBackend` suite exists and is green
   under `./ok.sh test`, quoting results. On completion, parent 0001's acceptance criteria
   are checked off and 0001 advances with this item.

**Assumptions:**

- This slice's accounts have exactly one profile (created at registration), so the TUI
  auto-selects the first profile from `GET /api/profiles`; a profile picker is the
  backlog "multiple profiles UX" item.
- Concrete keybindings (e.g. `a` add, `d`/`space` toggle-done, `q` quit) are `tui-dev`'s
  call; the tester suite pins whatever is chosen.
- After a mutation the TUI re-renders from the mutation response and/or a fresh list fetch
  — either satisfies statelessness; `tui-dev` picks the simpler.
- TUI-side tracing/OTel is **not** required by 0001's criteria (server-side audit traces
  satisfy them); deferred to the backlog observability item.
- The server-offline probe uses `healthz` and/or request-error mapping; no retry daemon —
  a manual retry key is the smallest UX.
- Mock mechanism for the client in tests (e.g. `wiremock`/local stub server vs trait-level
  fake) is `tester`'s call — server mocking is the sanctioned external-service mock.

**Risks:**

- If view code and HTTP code intertwine, the `TestBackend` suite becomes hard to write —
  that is the ADR-0003 architecture smell; bubble up instead of bending tests.
- Terminal raw-mode/teardown bugs are invisible to `TestBackend` — accepted residual risk;
  covered by the `docs/manual-smoke.md` checklist (ADR-0003 §3), not a gate here.
- Latency on every keystroke-triggered request: keep mutations request-per-action (no
  optimistic state — that would be client-side state); acceptable at personal scale.

<!-- append-only; dated, AUTHORED entries. Human feedback = an UNCHECKED box: [ ] unhandled, [x] addressed + re-reviewed (the ONLY re-entry signal). -->
## Log / comments

- 2026-06-11 [architect] item created as slice 3/3 of 0001 via the `plan` skill; plan
  authored; governed by ADR-0005 with ADR-0003 verification routing; status `planned` →
  `ready`. **Dependency gate:** do not start until
  [0003][feat-0003] is `awaiting-merge`/`merged` (the verifier
  needs the live stack); compile-time work only needs 0002, but the gate stays on 0003 to
  keep the cycle linear.

<!-- written at end of cycle; what the human reviews -->
## Summary

[adr-0003]: ../../docs/adr/0003-verification-layering.md
[adr-0005]: ../../docs/adr/0005-foundational-wire-contract.md
[feat-0001]: ./0001-foundational-slice.md
[feat-0003]: ./0003-server-auth-profile-tasks.md
