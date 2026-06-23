---
id: 0008
title: Pomodoro focus timer — global duration config + start/stop session
type: feature      # feature | chore
status: inbox          # inbox → planned → ready → working → review → awaiting-merge → merged | blocked
priority: medium    # high | medium | low
parent: null
depends-on: []      # ADR-0002 (timer authority) is on `main`; no in-flight Board item gates this
created: 2026-06-23
updated: 2026-06-23
---

## Feature request

**Goal:** Implement the Pomodoro focus timer end-to-end, the first feature of the **Focus**
phase. A user can set the session duration, start a focus session, see a live countdown in the
TUI, and stop it. Authority and the rendering model are settled in
[ADR-0002][adr-0002] — this card implements that decision; it does not reopen it.

**Shape (deliberately flat — [ADR-0001][adr-0001] decision 3, hard-constraint #3):**

- **Config is global to the app, duration is the only knob** (default 30 minutes). Per
  [ADR-0002][adr-0002] this is account-global, **not** profile-scoped (#4 namespaces TODOs and
  Notes only).
- **No pause; stop resets.** There is no paused state — stopping clears the active session.
- **Server owns the timer; the TUI renders it** ([ADR-0002][adr-0002]). The server is the sole
  authority for both the duration config and the active session, and both persist in Postgres.
- **A session is an absolute end-instant, not a pushed counter.** The session response carries
  `ends_at` plus the server's current instant; the TUI computes `remaining` once and ticks it
  down locally on its render loop — **no** per-second polling, **no** server tick stream (stays
  inside [ADR-0006][adr-0006]). Completion (`now ≥ ends_at`) is decided by the server.

**Surface to build (final shapes pinned in the `architect` plan under [ADR-0002][adr-0002]):**

- `contract` — a new timer module: DTOs for the global config and the session state (the
  session DTO carries `ends_at` + a server-instant field), reusing the `{ code, message }` error
  contract.
- `server` — endpoints to read/update the global duration config, read the current session
  (idle / running-with-`ends_at` / completed), start a session, and stop a session; a reversible
  migration (up/down) for the config + session tables.
- `tui` — a focus/timer view that reads the config and session, renders the live countdown per
  the model above, and offers start / stop and duration adjustment. Stateless (#1): every view
  derives from a server response.

**Acceptance criteria:**

- [ ] A user can set the global session duration and have it persist across server restarts.
- [ ] Start → the TUI shows a live countdown derived from the server's `ends_at` + server-now;
      no per-second polling and no UI freeze (consistent with [ADR-0006][adr-0006]).
- [ ] Stop resets the session (no paused state); a session that reaches `ends_at` is reported
      completed by the server.
- [ ] The timer is account-global, **not** profile-scoped — switching profiles does not change
      the active session or duration ([ADR-0002][adr-0002], #4).
- [ ] Full `feature` Definition of done: `./ok.sh test | lint | fmt --check` green; `reviewer`
      approved (pinned to `./ok.sh code-hash`); live `verifier` pass exercising the server API +
      reqwest path (shapes, status codes, error contract, OTel spans); the `tui`-touching change
      is covered by the `ratatui` `TestBackend` suite ([ADR-0003][adr-0003]).
- [ ] The `contract` change is governed by [ADR-0002][adr-0002] (already accepted); any wire
      detail beyond what that ADR fixes is recorded in the plan.

**Out of scope (each would need a new ADR — #3 flatness):** pause/resume; per-profile or
multiple concurrent timers; break/long-break cycles or session history; notifications/sound;
any timer config knob other than duration.

<!-- feature: needs an `architect` plan (`plan` skill) writing a `## Plan(s)` block before code. -->

<!-- append-only; dated, AUTHORED entries. Human feedback = an UNCHECKED box: [ ] unhandled, [x] addressed + re-reviewed (the ONLY re-entry signal). -->
## Log / comments

- 2026-06-23 [orchestrator] minted the Pomodoro feature card now that [ADR-0002][adr-0002]
  (timer authority) is accepted on `main`, unblocking the Focus phase. This is the
  `## Feature request` only — as a `feature` it next goes to `architect` (`plan` skill) to write
  the `## Plan(s)` block (task breakdown, agent assignments, file ownership, the concrete
  `contract` wire shape under ADR-0002) before any code. No new ADR is needed — ADR-0002 already
  governs the contract surface; the plan pins the exact DTO/endpoint shapes under it.

[adr-0001]: ../../docs/adr/0001-foundational-architecture.md
[adr-0002]: ../../docs/adr/0002-pomodoro-timer-authority.md
[adr-0003]: ../../docs/adr/0003-verification-layering.md
[adr-0006]: ../../docs/adr/0006-tui-concurrency-and-responsiveness.md
