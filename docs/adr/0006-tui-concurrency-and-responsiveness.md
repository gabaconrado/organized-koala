# ADR-0006: TUI concurrency and responsiveness model

**Status:** Accepted · 2026-06-22 (amended 2026-06-23 §8 Board 0008; 2026-06-26 §8.3 Board 0015)

## Context

The 0004 TUI (`organized-koala`) runs a single-threaded blocking loop in `terminal::run`
(`crates/tui/src/terminal/mod.rs`): draw the frame, `event::read()` **blocks** until a
keypress, then `App::handle_event` (`crates/tui/src/app/mod.rs`) calls the injected blocking
`reqwest` client **inline** before the next draw. While a request is outstanding the UI thread
is parked inside `reqwest`: no redraw, no spinner, no cancel, no quit. 0004 chose blocking
`reqwest` deliberately ("acceptable at personal scale", recorded in that item's Risks), and no
ADR mandated an async TUI.

Human feedback (re-homed to Board item [0005][feat-0005]) makes **UI responsiveness a
first-class property**: the UI must keep rendering, show that work is in flight, and accept
cancel/quit while a server request is outstanding. This reshapes the TUI's runtime model — a
cross-cutting design decision — so it is recorded as an ADR before implementation.

This ADR changes **no wire shape**: it touches neither the `contract` crate nor any endpoint.
It is a `tui`-crate concurrency decision plus a small internal refactor of the `App` update
seam.

### Forces

- **Flatness / simplicity** (CLAUDE.md, [coding-standards][coding-standards] priority order:
  correctness, security, simplicity, performance). The smallest model that is correct wins;
  new runtime machinery must earn its place.
- **The ADR-0003 layer-2 testability seam** ([ADR-0003][adr-0003]). The whole interactive
  surface is driven through `ratatui`'s `TestBackend` with **no live server and no real
  terminal**, because the pure `App` state machine reaches its one external service through an
  **injected synchronous `Client` trait** (the sanctioned external-service mock), and tests
  drive it by calling `App::handle_event(Event::…)` then synchronously rendering. Any new model
  **must keep this seam**: a fake/synchronous client must still drive the core deterministically,
  with the threaded/async layer confined to the edge — exactly where the `terminal` driver and
  the real `reqwest` client already sit ([rust-standards][rust-standards] "separate the pure
  core from the effectful shell", learned 0004).
- **Hard-constraint #1 — the TUI is stateless.** No on-disk or cross-run persistence; every
  view derives from a server response. A *pending in-flight request* (a spinner, a
  "request outstanding" marker, an optional pending-request identity) is **transient UI state
  for the process lifetime**, the same category as the in-memory JWT and the current
  `AuthState` field buffers — it is **not** cached server data and never persists. We argue
  below that this is compatible with #1.
- **The error-code branching contract.** `unauthenticated` → login, `validation_failed` →
  inline, offline/unreachable → blocking retry screen ([ADR-0005][adr-0005] codes; 0004
  `App::handle_*_error`). A response that arrives asynchronously must route through the **same**
  branching, unchanged.
- **No new heavyweight dependency without cause** (hard-constraint #6 governs binaries; the
  spirit extends to runtime weight). `reqwest::blocking` and `std` threads/channels are already
  present; a `tokio` runtime is not. (An async TUI is entirely *feasible* — `ratatui` is
  render-agnostic and `crossterm` ships an `EventStream` for async input — so this is a
  cost/benefit decision, not a technical impossibility.)

## Decision

### 1. Model (A): synchronous `Client` on a worker thread, polled render loop

Keep the **synchronous `Client` trait exactly as is**. Move request *execution* off the UI
thread onto a single long-lived **worker thread**; the UI thread and the worker communicate
over two `std::sync::mpsc` channels. The render loop becomes a **poll loop**: it polls the
terminal for input with a short timeout (`crossterm::event::poll(tick)`) **and** drains the
worker's response channel each tick, redrawing every tick so a spinner animates and the UI
stays live while a request is outstanding.

Rejected alternative **(B): a `tokio` runtime + async `reqwest` + async event stream.** It is
technically viable (`ratatui` is rendering-agnostic; `crossterm`'s `event-stream` feature
exposes a `futures::Stream` of events for `tokio::select!`), but it would force the `Client`
trait to become `async` (or be duplicated), which ripples into the pure `App` core (handler
methods would become `async` or the core would have to be split anyway), the `FakeClient`, and
every `tests/*.rs` flow — a large churn of the contract crate's only client and the entire
ADR-0003 test harness, to buy responsiveness we already get from (A). Async would be the right
call if the TUI multiplexed many concurrent streams or needed cancellable structured
concurrency; at personal scale, with **one request outstanding at a time**, it is unjustified
complexity. (A) reuses dependencies already in the tree and leaves the synchronous testability
seam intact. **We choose (A).**

### 2. The `App` update seam splits into two pure steps

Today `App::handle_event` performs IO inline (`self.client.login(&req)` etc.). That inlining is
exactly what blocks the UI thread, and it is incompatible with executing the request elsewhere.
We split the seam so the **core stays pure and synchronous** and the IO moves to the edge:

- `App::handle_event(Event) -> Option<ClientRequest>` — pure. A request-triggering event
  (submit auth, add task, close task, refresh, retry) transitions the screen into an **in-flight
  state** and **returns** a `ClientRequest` describing what to execute, instead of calling the
  client. A non-request event (typing, focus movement, mode toggle, begin-add, cancel of a local
  sub-flow, quit) returns `None` and mutates state as today.
- `App::apply_response(ClientResponse)` — pure. Applies a completed result to the in-flight
  state, running the **same** success/error-code branching the inline code runs today
  (`enter_app`, `handle_*_error`, list re-render, row update). Clears the in-flight marker.

The client is **no longer a field the core calls**; `App` holds no `C: Client` at all. The
generic `App<C>` collapses to a plain `App`, and the `Client` is owned by the **edge** (the
worker thread). This is *more* testable than today: a test calls `handle_event`, gets the
`ClientRequest` (or `None`), and calls `apply_response(scripted_result)` — fully synchronous,
no threads, no client injected into the core at all.

### 3. The message protocol (UI ↔ worker)

Two `mpsc` channels and one in-memory worker:

- **UI → worker: `ClientRequest`** — an enum mirroring the `Client` trait's methods, carrying
  owned payloads and the bearer token where needed, plus a monotonically increasing
  **`RequestId`** stamped by the UI thread:
  `Health`, `Register(RegisterRequest)`, `Login(LoginRequest)`, `ListProfiles`,
  `ListTasks { profile_id }`, `CreateTask { profile_id, req }`, `CloseTask { profile_id, task_id }`.
  Each carries the `token` for authenticated calls (as today, the token is passed explicitly).
- **worker → UI: `ClientResponse { id: RequestId, outcome }`** — the `RequestId` it was asked
  to run plus the `ClientResult<…>` outcome, wrapped in a `WorkerEvent` the render loop applies
  as the equivalent of today's `Ok(_)`/`Err(_)` arms.

The worker loop is trivial and lives entirely at the edge: `recv()` a `ClientRequest`, match it
to the corresponding synchronous `Client` method, `send()` back a `ClientResponse`. The worker
owns the real `reqwest::blocking` `HttpClient`; nothing about the worker enters the `App` core
or the test harness.

### 4. One request in flight; keystrokes during flight; cancel semantics

- **At most one request is outstanding.** While a screen is in its in-flight state,
  `handle_event` returns `None` for any new request-triggering event (the spinner already
  communicates "busy"); local, non-IO events (cursor/focus moves are disabled in-flight to keep
  the model simple; **`Cancel` and `Quit` remain live** — see below). This matches the existing
  request-per-action design (0004 Risks: "no optimistic state").
- **Keystrokes that arrive in flight** are delivered to `handle_event` as always; in an
  in-flight state they are simply no-ops (return `None`) except `Cancel`/`Quit`. No keystroke
  is silently lost to a parked thread, because the thread is no longer parked.
- **Cancel semantics.** `Esc` (and `Ctrl+C`/`q` per context) is honoured immediately: the UI
  marks the in-flight request **abandoned** by recording the in-flight `RequestId` and
  transitioning out of the in-flight state (back to the prior screen for `Cancel`, or to quit).
  The worker thread is **not** force-killed (a blocking `reqwest` call cannot be interrupted
  mid-flight without a heavier mechanism); instead, when its `ClientResponse` eventually
  arrives, the UI **drops any response whose `RequestId` does not match the currently-awaited
  one** (stale-response rejection). This gives correct *user-perceived* cancel — the UI is
  immediately responsive and the abandoned result never mutates state — without async
  cancellation machinery. A `reqwest` client-side timeout (set on the `HttpClient`) bounds how
  long an abandoned request occupies the worker.
- **Quit while in flight** sets the quit flag and exits the loop immediately; the worker thread
  is detached and the process exits (the worker holds no state needing flush — #1).

### 5. The in-flight UI state and the spinner

Each screen that can issue a request gains a transient **in-flight marker** (e.g. an
`Option<RequestId>` / a small `Pending` sub-state on `AuthState`/`TaskListState`, and the
`Offline` retry). When set, the draw functions render a **spinner/loading indicator** and a
"working… (Esc to cancel)" hint; the poll loop redraws each tick so the spinner animates. This
marker is transient process-lifetime UI state — **not** persisted, **not** cached server data —
so it is fully compatible with hard-constraint #1 (the same category as the in-memory session
and form buffers). When `apply_response` runs, the marker clears and the view derives from the
server response exactly as today.

### 6. Error routing is unchanged

`apply_response` runs the **same** branching the inline arms run today: `is_offline()` → the
blocking `Offline` screen; `code() == Some(Unauthenticated)` → drop session, return to login;
`validation_failed` and other `Api` codes → inline message on the active screen. An
`unauthenticated`/offline result that arrives asynchronously routes through this identical path —
the only change is *when* it runs (on a polled response, not inline), never *how* it branches.

### 7. Testability seam preserved (the load-bearing constraint)

The threaded layer sits **entirely at the edge**, beside the existing `terminal` driver:

- The **pure core** is now `handle_event` (event → `Option<ClientRequest>`) + `apply_response`
  (`ClientResponse`-outcome → next state) + the unchanged pure draw functions and `map_key`.
  Tests drive it with **no client and no threads**: feed an `Event`, assert the returned
  `ClientRequest` (proving what would cross the wire), then feed a scripted outcome to
  `apply_response` and render. This is strictly more deterministic than today's inline-IO core.
- The **`Client` trait stays synchronous and unchanged**, so the scripted `FakeClient` in
  `tests/common/mod.rs` and the `HttpClient` keep their current shape. The worker thread is the
  *only* place that calls `Client`, and it is edge code (like `terminal::run`), not covered by
  the `TestBackend` core suite — the same boundary the real `reqwest` client already sits behind.
- Existing flow tests adapt mechanically: where a test today calls `handle_event(Submit)` and
  expects the next render to reflect the response, it now calls `handle_event(Submit)` (gets a
  `ClientRequest`) then `apply_response(scripted)` before rendering. The assertions on recorded
  request payloads move from `FakeClient::calls()` to the returned `ClientRequest` (or are kept
  via a thin synchronous test executor that maps a `ClientRequest` through the `FakeClient` and
  returns a `ClientResponse` — the harness's call, owned by `tester`).
- New `tester` coverage: the in-flight state renders a spinner/loading hint; a request-triggering
  event while in flight is a no-op; `Cancel` exits the in-flight state and a late stale
  `RequestId` response is dropped; `apply_response` error-code branching matches the pre-split
  behaviour for every code.

If splitting the seam this way makes the suite *harder* to write rather than easier, that is the
ADR-0003 architecture smell and `tui-dev`/`tester` bubble up rather than bend a test.

## Amendment 2026-06-23 — §8: global timer widget, append-spinner indicator, coarse cadence (Board 0008)

Human feedback on Board item [0008][feat-0008] (the Pomodoro timer, built on this model and
ADR-0002) reshapes three TUI-presentation choices. None touches a wire shape, the `contract`
crate, an endpoint, the server, or ADR-0002's authority/render model (the server still owns the
countdown; the TUI still renders it from the absolute `ends_at` + `server_now`, §3 of ADR-0002
unchanged). All three are `tui`-crate presentation/structure decisions, recorded here where the
TUI runtime structure and the in-flight indicator (§5) already live.

### 8.1 The timer is an always-visible global widget, not a dedicated screen

The Pomodoro timer is a **global** concept (account-global, one per account — ADR-0002 §5), so
it is **not** a navigable `Screen`. The dedicated `Screen::Timer` (reached with `t`, exited with
`Esc`) is **removed**. Instead the timer is a **persistent widget rendered in the bottom-right
corner under every post-auth screen**, beside the existing bottom-left hotkey caption. It shows
the current session at a glance — the live `MM:SS` countdown when running (recomputed each render
tick from `ends_at − (server_now + monotonic delta)`, exactly as ADR-0002 §3 and the prior 0008
build already do — render, not state, so #1 holds), `idle`, the configured duration, or
`completed` — on every screen, with no navigation.

- **Where the timer state lives now.** Because the widget is global, its transient render state
  (the last `TimerConfig` + `TimerSession`, the monotonic `applied_at` instant, and its own
  in-flight `RequestId` marker for the toggle) moves **out of a per-screen `TimerState` and onto
  `App`** as a single app-level field, rendered by every draw path. This is the same
  process-lifetime, never-persisted, derived-from-a-server-response category as the in-memory
  session and the in-flight marker (§5) — **#1 holds unchanged**. There is no stored authoritative
  remaining-seconds counter; the countdown is recomputed each draw.
- **Auth screen carve-out.** Before login there is no session and no token, so the widget is not
  rendered on `Screen::Auth` (and no timer request is issued until authenticated). It appears once
  a session exists, on every post-auth screen.

### 8.2 Start/stop is a global `p` toggle, surfaced in the hotkey menu

A single global key — **`p`** — toggles the focus session start/stop from **any** post-auth
screen (it maps to a new transport-agnostic `Event::ToggleTimer`, resolved in the pure core to
`StartTimerSession` when idle/completed and `StopTimerSession` when running — reusing the existing
start/stop client methods and protocol variants from the 0008 build; **no new `contract`/protocol
shape**). The duration is still set via the existing update-config path; how that edit is reached
without a dedicated screen is left to the plan (Assumption B2) and is a TUI-presentation choice,
not an ADR decision. `p` is added to the **bottom-left hotkey caption** on every screen that shows
it, so the binding is discoverable (the feedback's "help menu").

### 8.3 The in-flight indicator APPENDS a spinner; it never replaces the caption

ADR-0006 §5's in-flight indicator is refined: a request in flight must **append a trailing spinner
glyph to the end of the existing hotkey caption**, rather than **replacing** the caption text with
a "working…" string. Replacing the whole caption each refresh causes visible flicker (the caption
text vanishes and returns every coarse poll). The caption stays stable; only a trailing spinner
glyph is added/animated while a request is outstanding (and the "Esc to cancel" affordance is
preserved as part of the stable caption or alongside the spinner). This is a pure `tui::ui`
rendering change — the spinner is still the §5 transient process-lifetime marker, just rendered
additively. §5's substance (a spinner communicates "busy"; it animates on the poll tick; it is
never persisted) is unchanged.

> **Amendment 2026-06-26 (Board [0015][feat-0015]) — the "(Esc to cancel)" affordance moves
> out of the footer caption.** The original §8.3 above kept the textual *"(Esc to cancel)"*
> hint "as part of the stable caption or alongside the spinner," and the footer reserved a
> multi-row bottom band so that hint could not be clipped when it wrapped against the
> right-column timer widget (§8.1). Operator decision for the 0015 dialog cycle relocates it:
>
> - **The spinner-append behaviour is UNCHANGED — this amendment does not touch §8.3's
>   anti-flicker core.** A request in flight still **appends** a trailing spinner glyph to the
>   end of the stable hotkey caption and never **replaces** the caption text; the spinner is
>   still the §5 transient process-lifetime marker, animated on the poll tick, never persisted.
> - **The textual "(Esc to cancel)" affordance is REMOVED from the footer caption** and is
>   instead **documented in the `?` help modal** ([ADR-0010][adr-0010] §3): the help modal
>   carries a line stating that `Esc` cancels an in-flight / loading request.
> - **The footer becomes a single row, flush to the bottom** (caption-left / timer-right),
>   enabling ADR-0010 §2's *tight footer* goal (the bottom band is pulled flush to the last
>   row and does **not** grow). Because the longest caption no longer carries the affordance,
>   the band can now be a **single row** — the multi-row reservation that existed only to keep
>   the wrapping affordance from being clipped is no longer needed.
> - **Functionally nothing changes:** `Esc` still cancels an in-flight request — the keymap /
>   cancel semantics of §4 are untouched. Only the *location of the hint* moves from the footer
>   caption to the `?` help modal. This is pure `tui::ui` presentation — no `contract`/wire
>   (#2), no server, no domain (#3) change.

### 8.4 The coarse timer-session refresh cadence is ~1 minute

The coarse `GetTimerSession` cadence is loosened from ~5 s to **~1 minute** (ADR-0002 §3's "coarse
interval, not every second" is unchanged in spirit — only the constant moves). At ~80 ms per
poll-loop tick this is `TIMER_REFRESH_TICKS ≈ 750`; `tui-dev` picks the exact constant. The local
countdown still animates every render tick from the absolute `ends_at` (no per-second network
traffic — the property ADR-0002 §3 and this ADR §1 require), and start/stop still refresh
immediately on the `p` action. A coarser cadence means the server's running→`completed` verdict
can lag up to ~1 min behind the locally-displayed `00:00`; this is cosmetic (the local countdown
already shows `00:00`/"completed (awaiting server confirmation)") and acceptable — completion is
still the server's authority, just confirmed less eagerly.

## Consequences

- **Responsiveness becomes a structural property**, not a hope: the UI thread never blocks on
  IO, so it always redraws, animates a spinner, and honours cancel/quit while a request is
  outstanding.
- **No new runtime dependency.** `reqwest::blocking`, `std::thread`, and `std::sync::mpsc` are
  already available; no `tokio`, no async `Client`. (If `tui-dev` finds `crossbeam-channel`
  ergonomically preferable to `std::mpsc`, that is a small dependency choice within this model,
  not a re-decision of A-vs-B.)
- **The `App<C>` generic disappears**; the core no longer owns a client. `main.rs`/`terminal`
  own the worker + `HttpClient` and pump the protocol. This is a `tui`-internal refactor with
  **no `contract` change and no ADR ripple to the server side.**
- **Cancel is user-perceived, not a hard abort.** An abandoned blocking request runs to
  completion on the worker and its result is dropped by `RequestId` mismatch; a `reqwest`
  timeout bounds the worst case. We accept this over async cancellation machinery at personal
  scale. If true mid-flight abort is ever required, that is a new ADR (revisiting A-vs-B).
- **Hard-constraint #1 holds.** The only new state is the transient in-flight marker and spinner
  tick — process-lifetime UI state, never persisted, never cached server data. Reviewer guards
  this as it guards the in-memory session today.
- **The ADR-0003 routing is unaffected.** Live API/client verification still belongs to the
  verifier; interactive in-flight/spinner/cancel behaviour is `tester`'s `TestBackend` suite —
  this ADR strengthens, not changes, that boundary.
- **Folded scope:** Board item 0005 carries this loop change **plus** the requested
  `tui::app` submodule reorganization (split into `auth`, `task_add`, `task_list`; `mod.rs`
  keeps wiring/infrastructure), because both restructure `tui::app` and are cleanest done
  together.

[feat-0005]: ../../board/features/0005-tui-responsive-event-loop.md
[feat-0008]: ../../board/features/0008-pomodoro-timer.md
[feat-0015]: ../../board/features/0015-tui-dialog-system.md
[adr-0010]: ./0010-tui-navigation-and-interaction-model.md
[adr-0003]: ./0003-verification-layering.md
[adr-0005]: ./0005-foundational-wire-contract.md
[coding-standards]: ../../.claude/skills/coding-standards/SKILL.md
[rust-standards]: ../../.claude/skills/rust-standards/SKILL.md
