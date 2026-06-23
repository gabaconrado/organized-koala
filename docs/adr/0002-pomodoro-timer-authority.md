# ADR-0002: Pomodoro timer authority

**Status:** Accepted · 2026-06-23

## Context

The Pomodoro timer is the next feature after the foundational slice. Before any code is
written it needs one decision settled: **who owns the running countdown** — the server, or the
TUI. [ADR-0001][adr-0001] left this open and flagged it as the gate for all Pomodoro work, and
hard-constraint #1 (the TUI is stateless; all state lives server-side) already points at the
answer. This ADR records it and the model that follows, so the contract surface and the feature
plan have a fixed footing.

The domain is fixed and deliberately flat (CLAUDE.md, [ADR-0001][adr-0001] decision 3): the
Pomodoro config is **global to the app**, its **only** knob is the session **duration**
(default 30 minutes), there is **no pause**, and **stop resets**. Profiles namespace TODOs and
Notes only (#4) — the timer is explicitly **not** profile-scoped. This ADR does not revisit any
of that; it settles authority and the rendering model within those constraints.

A naive design would have the server push per-second ticks or the TUI poll every second for a
remaining-seconds count. Both fight [ADR-0006][adr-0006], whose responsiveness model is a
synchronous client on a worker thread with a polled render loop and **no** per-second network
traffic. The model below renders a live countdown with **no** server tick stream and **no**
per-second polling.

## Decision

1. **The server is the sole authority for timer state.** This is hard-constraint #1 applied to
   the timer: the TUI holds no countdown of its own. Both the duration config and the active
   focus session live server-side.

2. **A running session is an absolute end-instant, not a decrementing counter.** Starting a
   session records `started_at` and the configured `duration`; the server derives an absolute
   `ends_at`. The wire never carries "seconds remaining" as authoritative state — it carries
   `ends_at` plus the **server's current instant** at the time of the response.

3. **The TUI renders the countdown locally from that instant; it does not poll per second.**
   On receiving a session, the TUI computes `remaining = ends_at − server_now` once, then ticks
   it down on its own render loop using a monotonic delta. Including `server_now` in the
   response neutralizes client clock skew — the TUI never trusts its own wall clock against the
   server's. Computing a display value from a server response is **rendering**, not state, so
   #1 holds. A session is refreshed from the server on user action and on a coarse interval (not
   every second); the server remains the authority for whether the session is still running or
   has **completed** (`now ≥ ends_at`).

4. **Both the config and the active session persist in Postgres.** All timer state is durable
   server-side, so a reconnecting or second TUI instance sees the same live countdown and a
   session survives a server restart — consistent with "all state lives server-side" and the
   persistence-across-restart property the verifier already checks. *(Considered and rejected:
   an in-memory-only session. Lighter, but it loses the session on restart and cannot be shared
   across server replicas, breaking the stateless-multi-client story.)*

5. **Scope: one duration and one active session per account, shared across that account's
   profiles.** "Global to the app" is read as account-global, not profile-scoped (#4 namespaces
   TODOs and Notes, not the timer). At most one focus session is active at a time. **No pause:**
   there is no paused state; **stop** clears the active session (resets), it does not freeze it.

6. **The contract gains a timer surface (a #2 / contract event, authorized here).** Server and
   TUI share new DTOs in the `contract` crate for the global config and the session state; the
   active-session DTO carries `ends_at` **and** a server-instant field per decision 3. The
   shape supports, conceptually: read/update the global duration config; read the current
   session (its `ends_at` + server-now, or an idle/completed marker); start a session; stop a
   session. The standard error contract (`{ code, message }`) is reused. The **exact** DTO
   field names and endpoint paths are pinned in the Pomodoro feature's plan under this ADR.

## Consequences

- **The contract crate grows a timer module.** Per #2 this rippled decision is ADR-gated, which
  this ADR satisfies; the concrete wire shape is finalized in the feature plan and, like every
  contract change, is consumed identically by server and TUI.
- **A new migration adds the timer config + session tables** (reversible up/down per CLAUDE.md).
- **The "absolute instant + server-now" model keeps the TUI responsive** with no tick stream and
  no per-second polling, staying inside [ADR-0006][adr-0006]'s loop. The cost is that the TUI's
  displayed seconds can drift sub-second between refreshes; this is cosmetic and corrected on the
  next refresh, and completion is always decided by the server.
- **No pause / stop-resets / single global duration** keep the surface tiny; adding per-profile
  timers, pause/resume, or multiple concurrent sessions would each be a new ADR (#3 flatness).
- **The Pomodoro feature is unblocked**: the build-plan row and the feature card may proceed,
  citing this ADR.

[adr-0001]: ./0001-foundational-architecture.md
[adr-0006]: ./0006-tui-concurrency-and-responsiveness.md
