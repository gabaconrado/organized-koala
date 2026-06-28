---
id: 0017
title: Desktop notification when the focus timer ends (cross-OS, Ubuntu-first)
type: feature      # feature | chore
status: review          # inbox → planned → ready → working → review → awaiting-merge → merged | blocked
priority: medium    # high | medium | low
parent: null
depends-on: []      # builds on 0008 (timer, merged); ADR-0002 already on `main`. No in-flight item gates this.
branch: feature/0017-timer-completion-desktop-notification
worktree: .claude/worktrees/0017-timer-completion-desktop-notification
created: 2026-06-28
updated: 2026-06-28
---

## Feature request

**Goal:** When a focus session reaches its end, the TUI fires a **single desktop
notification** telling the user the timer ran out. This is the operator's last adoption
blocker before using organized-koala regularly.

**Motivation (operator):** The countdown is rendered inside the TUI, but a user running a
focus session has usually switched away to do the focus work — they are not staring at the
terminal. A native desktop notification is what actually tells them the session is over.

**Shape (deliberately minimal):**

- **Fires only on the timer *ending*.** The one and only trigger is a focus session reaching
  its end. No notifications for start, stop, or any other event.
- **Simple text, no sound.** A plain notification — title + a short body saying the timer ran
  out (e.g. *"Focus timer"* / *"Your focus session has ended."*). **No sound**, no action
  buttons, no progress, no rich content.
- **One notification per completion.** Exactly one notification per session that ends — it
  must **not** re-fire on every render tick while the session sits in the completed state.
- **Cross-OS, Ubuntu-first.** Must work on **Ubuntu** (the operator's primary OS). Should
  ideally work on macOS and Windows too via a cross-platform Rust notification crate
  (e.g. `notify-rust` or similar — final choice is the architect's). Where a platform isn't
  supported, failure must be **silent and non-fatal** (see below).

**Where it lives (TUI-only — no `contract`/server change expected):**

- The server stays the sole authority for the running-vs-completed verdict
  ([ADR-0002][adr-0002]); this feature changes **no** wire shape and adds **no** server
  endpoint. It is a TUI side-effect fired when the TUI **observes** the session transition to
  `Completed`.
- The TUI already detects completion: it re-pulls the session and renders the
  `idle / running / completed` states (`crates/tui/src/app/timer.rs`,
  `crates/tui/src/ui/mod.rs`, `crates/tui/src/terminal/mod.rs`). The notification hooks the
  **running → completed edge** at that seam.
- **Hard-constraint #1 (stateless TUI) holds:** the only new state is a transient,
  in-memory "already notified for this session" guard at the event/render seam (so the
  notification fires once, not every tick) — **not** persisted, no on-disk store, every view
  still derives from a server response. The architect should confirm this stays inside #1
  and inside the [ADR-0006][adr-0006] render-loop model (no new polling, no per-second
  server traffic); it is most likely **no-ADR** TUI work, but that call is the architect's.

**OS-package / runtime documentation (explicit operator request):**

- If the chosen crate needs **build-time** system packages on Linux (e.g. a `libdbus`/D-Bus
  dev package), they **must be documented explicitly** — in `README.md` and wherever build
  prerequisites live — so the requirement is never implicit. (Many modern crates use a
  pure-Rust D-Bus stack and need nothing at build time; the architect/dev confirms and
  documents whichever is true.)
- **Runtime:** on Linux the notification is delivered to a running **notification daemon**
  over D-Bus. On Ubuntu's default desktop (GNOME) one is present out of the box; in a bare
  TTY / headless / SSH-without-a-session context there may be **no** daemon. The feature must
  treat a delivery failure as **non-fatal**: log/trace it and continue — a missing daemon
  **never** crashes or blocks the TUI. Document this runtime expectation alongside the
  build-time packages.
- Per **hard-constraint #6**, no system package is installed as part of satisfying this item
  without the operator's explicit approval — the requirement is *documented*, and if a
  required build-time capability is genuinely missing the item **blocks** and asks, it is
  not worked around.

**Acceptance criteria:**

- [ ] When a focus session reaches its end, exactly **one** desktop notification appears,
      stating the timer ran out. No sound.
- [ ] The notification fires **once per completed session** — it does not repeat on
      subsequent render ticks while the session remains completed, and a new session arms it
      again.
- [ ] No notification fires for session **start** or **stop**, or for the idle/running
      states.
- [ ] Works on **Ubuntu** (verified by the operator / verifier on a real desktop session);
      builds and runs cross-platform with notification delivery degrading **silently and
      non-fatally** where unsupported or where no daemon is present.
- [ ] Any required **OS packages** (build-time) and the **runtime daemon** expectation are
      documented explicitly in `README.md` / build docs.
- [ ] Hard-constraint #1 preserved: no persisted/on-disk TUI state; the only addition is a
      transient in-memory "notified" guard. No `contract`/server change (#2); no new domain
      structure (#3); stays inside the [ADR-0006][adr-0006] render-loop model (no new
      polling).
- [ ] Full `feature` Definition of done: `./ok.sh test | lint | fmt --check` green; the
      TUI behaviour (edge-detection / fire-once guard) covered by the `ratatui` `TestBackend`
      suite ([ADR-0003][adr-0003], with the notification dispatch behind a seam that tests
      can observe without a live daemon); `reviewer` approved (pinned to `./ok.sh code-hash`);
      live `verifier` pass confirming the affected paths and the green TUI suite.
- [ ] If the design turns out to need a wire/contract change or new domain structure, it is
      **not** a quiet in-place expansion — it escalates to `architect` for an ADR first
      (#2, #3).

## Open questions for planning

- **Crate choice & platform matrix.** `notify-rust` is the obvious candidate (Linux D-Bus,
  macOS, Windows). The architect pins the crate and states, per platform, build-time deps and
  runtime requirements.
- **Test seam.** How to assert "fired exactly once on the completed edge" without a live
  notification daemon — most likely a trait/function seam the `TestBackend` suite can observe
  (a spy), with the real D-Bus call behind it.

## Plan(s)

This is **TUI-only** work. No `contract` change, no server endpoint, no migration. The server
remains the sole authority for the running-vs-completed verdict ([ADR-0002][adr-0002] §3); the
TUI fires a single desktop notification as a **side-effect** when it observes the session
transition into `Completed`.

### Decision 1 — Crate: `notify-rust` (default `zbus` backend), no build-time system package

We pin **`notify-rust`** as the cross-platform notification crate. The key constraint baked
into the dependency declaration: use its **default `zbus` backend** (pure-Rust D-Bus) and do
**not** enable the optional `dbus`/`d` (C-binding `libdbus`) feature. With the default features
this gives, per platform:

| Platform | Backend | Build-time system package | Runtime requirement |
| --- | --- | --- | --- |
| **Linux / Ubuntu** | `zbus` (pure-Rust D-Bus) | **None** — no `libdbus-1-dev`, no `pkg-config`, no system `.so`. The pure-Rust stack compiles with only the Rust toolchain. | A notification daemon owning `org.freedesktop.Notifications` on the **session** D-Bus. Present on Ubuntu GNOME out of the box; **absent** on a bare TTY / headless / SSH-without-a-graphical-session. |
| **macOS** | native (`NSUserNotification` family, bundled in the crate) | **None.** | The system Notification Center; delivery from a bare unsigned terminal binary may be limited — degrade non-fatally where it is. |
| **Windows** | WinRT toast | **None.** | The Windows notification subsystem (present on modern Windows). |

`tui-dev` declares the dependency with **`default-features = true` and the C `dbus` feature
left off** (a comment on the dep line stating the pure-Rust-backend / no-`libdbus` rationale, so
a future bump cannot silently flip to the C backend). If, on the real build, `notify-rust` is
found to pull a C-library/`pkg-config` build dependency on Linux under its defaults (contrary to
the above), that is a **capability question, not a workaround**: `tui-dev` sets the item
`blocked` and asks the operator before installing any system package (#6) — see Assumption A1
and Risk R1.

> Build-time fact to confirm-on-build, not block-on-plan: the plan's expectation is that the
> default `notify-rust` build needs **no** apt package on Ubuntu. `tui-dev` confirms this by a
> clean `./ok.sh build` in the worktree and records the observed truth in the README matrix
> (Slice 4). The DoD live build/test in CI is the proof.

### Decision 2 — No ADR (inside hard-constraint #1 and ADR-0006)

**Confirmed no-ADR, TUI-only.** Reasoning, against each gate:

- **#1 (stateless TUI):** the only new state is a transient, in-memory, process-lifetime
  "notified for this session" guard that lives on the `Timer` struct (alongside `pending`,
  `loaded`, `applied_at` — all already-sanctioned transient UI markers). It is never persisted,
  never on disk, and is **not** cached server data — every view still derives from a server
  response, and the completion verdict itself remains the server's (ADR-0002). This is the exact
  same category as the in-memory session/spinner markers ADR-0006 §5 already blessed under #1.
- **#2 (contract single source of truth):** **no** new DTO, no field, no endpoint. The feature
  reads the existing `TimerSession::Completed` variant that the timer surface already carries.
- **#3 (flat domain):** no new domain structure — the notification is a presentation/output
  side-effect, not a domain concept.
- **ADR-0006 render-loop model:** **no new polling and no new server traffic.** The edge already
  pulls the session on the coarse `TIMER_REFRESH_TICKS` cadence and on the `p` toggle; the
  notification is fired purely as a reaction to a response **already** being folded in
  `apply_response`. No new request is issued, no per-second traffic is added. The notifier call
  happens on the **edge thread that drains responses** (the poll loop), exactly where the real
  `Client` already does its I/O — the pure core stays pure.
- **ADR-0003 verification layering:** unchanged. The edge-detection + fire-once logic is pure
  `App` state the `tester` covers via `TestBackend`; the actual D-Bus call sits behind an
  injected trait at the edge (the same boundary as the `Client` trait), which the live
  `verifier` does **not** drive (it is not server-observable), and the operator confirms on a
  real Ubuntu desktop per the acceptance criteria.

Because there is **no** contract/domain/process decision, ADR-0006's existing "the only new
state is a transient marker" envelope already covers this. No new ADR and no ADR amendment is
written. (If, contrary to this analysis, the build forces a contract/wire or domain change, the
scope guard applies: `tui-dev` blocks and routes back to `architect` for an ADR first — #2/#3.)

### Decision 3 — The test seam: an injected `Notifier` trait, spied in `TestBackend` tests

The notification I/O is an **external effect**, so it is modelled exactly like the `Client`
trait (the sanctioned external-service seam, ADR-0003 / rust-standards "separate the pure core
from the effectful shell"). The pure core never calls it; it **emits a signal**, and the edge
performs the effect through an injected trait. Concretely:

1. **The pure core emits a one-shot signal, it does not notify.** `App::apply_timer_session`
   (`crates/tui/src/app/mod.rs`) is where the server-returned `TimerSession` is folded into
   `self.timer.session`. The previous session value is still present at function entry, so the
   **running → completed edge** is detected by comparing previous-vs-new:
   - previous `TimerSession::Running { .. }` (or, defensively, any non-completed prior whose
     guard is un-fired) **and** new `TimerSession::Completed { .. }` ⇒ this is the completion
     edge.
   - On that edge, **and only if the fire-once guard for this session has not already fired**,
     set a transient flag `Timer::notify_pending: bool` (the "please fire a notification" signal)
     and arm the guard so it cannot fire again for this session.
   - All other transitions (`Idle→Running`, `Running→Running` re-pull, `Completed→Completed`
     re-pull, `Completed→Idle` on stop, `*→Idle`) set **no** signal.

   The signal is a pure boolean on `Timer`; `apply_timer_session` is still pure (no I/O) and
   stays fully `TestBackend`/unit-testable. A small accessor — e.g.
   `App::take_pending_notification(&mut self) -> Option<TimerNotification>` — returns and clears
   the signal (consume-once), returning the title+body text to fire.

2. **The edge fires the effect through an injected `Notifier`.** A narrow trait in the `client`
   (edge) layer — proposed `crates/tui/src/client/notify.rs`:

       pub trait Notifier {
           // Fire a single transient desktop notification. Best-effort: any delivery
           // failure (no daemon, unsupported platform) is swallowed by the impl — never
           // an error the caller must handle, never fatal to the TUI.
           fn notify_timer_complete(&self, title: &str, body: &str);
       }

   - Production impl `DesktopNotifier` wraps `notify-rust`: builds a sound-less, button-less,
     plain title+body notification and `.show()`s it; **maps every error to a no-op** (logs
     nothing to stdout/stderr — writing to the terminal would corrupt the alt-screen TUI; see
     Assumption A2). This impl is **edge code** (like `HttpClient`), not covered by the
     `TestBackend` core suite, exactly as the real `reqwest` client is not.
   - The poll loop (`terminal::run`) owns a `&dyn Notifier` (or generic `N: Notifier`). After it
     drains a worker response and calls `app.apply_response(...)`, it calls
     `app.take_pending_notification()` and, if `Some`, invokes `notifier.notify_timer_complete(..)`.
     `main.rs` wires the production `DesktopNotifier`.

3. **The spy the `tester` observes.** A test-side `SpyNotifier` records each
   `notify_timer_complete` call (count + last title/body) behind a `RefCell`/`Mutex`. Because the
   *decision* to notify is the pure core's `notify_pending` signal, **most tests do not even need
   the notifier**: they drive `apply_timer_session` with scripted `TimerSession` responses and
   assert `take_pending_notification()` returns `Some(..)` exactly once on the Running→Completed
   response and `None` on every other transition and on a repeated `Completed` re-pull. A thin
   edge-integration test may additionally pump the signal through a `SpyNotifier` to assert the
   text and the one-call count, mirroring the synchronous worker-analogue executor pattern
   (rust-standards, learned 0005). The only mock remains a **sanctioned external-service trait**;
   no internal collaborator is mocked.

This keeps the edge-detection + fire-once logic in the deterministic, daemon-free `TestBackend`
layer (ADR-0003 layer 2), with the real D-Bus call isolated at the edge.

### Decision 4 — Fire-once guard: a flag on `Timer`, re-armed when a new session starts

The guard lives on the `Timer` struct (`crates/tui/src/app/timer.rs`), the same struct that
already holds the session and its other transient markers — keeping all timer-render state in
one place (#1 transient category). Proposed shape:

- Add `Timer::notified_for_session: bool` (the fire-once guard) and `Timer::notify_pending: bool`
  (the one-shot signal for the edge). Both default `false` in `Timer::new()`; both cleared by
  `Timer::reset()` (logout), so a fresh login re-arms cleanly.
- **Arm/fire/re-arm rule**, applied inside `apply_timer_session` when folding the new session:
  - Detect the edge **before** overwriting `self.timer.session`. If new is `Completed` and the
    guard `notified_for_session` is `false` ⇒ set `notify_pending = true` and
    `notified_for_session = true` (fire exactly once; subsequent `Completed` re-pulls see the
    guard already set and do nothing).
  - **Re-arm on a new session:** whenever the new session is `Running` (a fresh start), clear
    `notified_for_session = false`. This is the precise re-arm point — a new `StartTimerSession`
    response always carries `Running`, so the next completion of that new session fires again.
    `Idle` (stop/reset) also clears the guard (defensive; an idle state cannot complete anyway).
- **Why the guard keys on the transition, not on session identity:** the timer surface has no
  per-session id in the DTO (ADR-0002 models a single active session; `TimerSession::Running`
  carries `started_at`). Keying the guard on the **Running→Completed edge + a boolean re-armed by
  the next Running** is sufficient and simplest given "at most one active session per account"
  (ADR-0002 §5) and "stop resets" — there is no concurrent-session ambiguity. (If a future change
  introduced overlapping sessions this would need revisiting, but that would itself be an
  ADR-gated #3 change.) See Assumption A3.

> **Edge case — first response after login is already `Completed`.** If a user starts a session,
> backgrounds the TUI, and the session completes while the TUI is closed, the **initial**
> `GetTimerSession` after re-login returns `Completed`. Per Assumption A4 we **do not** fire on
> that initial load: the notification's purpose is to alert the user at the moment of completion,
> not to replay a stale completion on every launch. Concretely: the initial config→session chain
> arms the guard as already-fired for an initial `Completed` (i.e. an initial `Completed` sets
> `notified_for_session = true` **without** setting `notify_pending`). A subsequent real
> Running→Completed edge still fires normally. `tui-dev` implements this as "the first session
> fold after a load/reset does not emit, only arms"; `tester` covers it.

### Task breakdown, agent assignments, file ownership

**Slice 1 — `Notifier` seam + production `DesktopNotifier` (edge). Owner: `tui-dev`.**

- New `crates/tui/src/client/notify.rs`: the `Notifier` trait + `DesktopNotifier` (`notify-rust`),
  best-effort (errors → no-op, never written to the terminal). Export from `client/mod.rs`.
- `crates/tui/Cargo.toml`: add `notify-rust` (default features; **C `dbus` feature off**; comment
  the pure-Rust-backend rationale).
- Files: `crates/tui/src/client/notify.rs` (new), `crates/tui/src/client/mod.rs`,
  `crates/tui/Cargo.toml`.

**Slice 2 — Pure edge-detection + fire-once guard + one-shot signal (core). Owner: `tui-dev`.**

- `crates/tui/src/app/timer.rs`: add `notified_for_session` + `notify_pending` to `Timer`
  (defaults + `reset()` clears); a small method to compute the edge given prev/new session.
- `crates/tui/src/app/mod.rs`: in `apply_timer_session`, detect the Running→Completed edge before
  overwriting `session`, apply the arm/fire/re-arm + initial-load rules; add
  `App::take_pending_notification(&mut self) -> Option<TimerNotification>` (consume-once,
  returning the fixed title/body — e.g. title `"Focus timer"`, body `"Your focus session has
  ended."`; exact copy is `tui-dev`'s call within the feature-request shape, no sound, no
  actions).
- Files: `crates/tui/src/app/timer.rs`, `crates/tui/src/app/mod.rs`.

**Slice 3 — Wire the edge: poll loop fires the notifier. Owner: `tui-dev`.**

- `crates/tui/src/terminal/mod.rs`: `run` takes a `Notifier` (param or generic); after draining a
  response + `apply_response`, call `take_pending_notification()` and fire. No new request, no new
  poll — purely reactive (ADR-0006 unchanged).
- `crates/tui/src/main.rs`: construct `DesktopNotifier` and pass it into `terminal::run`.
- Files: `crates/tui/src/terminal/mod.rs`, `crates/tui/src/main.rs`.

**Slice 4 — Documentation (build-time package matrix + runtime daemon expectation). Owner:
`tui-dev`** (crate-local README) **with `eng-manager`** confirming the root `README.md` dev-env
section.

- `crates/tui/README.md`: a short "Desktop notifications" subsection — the platform matrix
  (Decision 1), the **no build-time apt package** statement (confirmed by the worktree build),
  and the runtime daemon expectation + silent-non-fatal degradation.
- Root `README.md` "Setting up a development environment": add the runtime note (a notification
  daemon is needed on Linux for notifications to *appear*; none is needed to build/run) under the
  per-verb / required sections as appropriate.
- Files: `crates/tui/README.md`, `README.md`.

**Slice 5 — Tests. Owner: `tester`.**

- `TestBackend`/unit suite for the pure core (the bulk): drive `apply_timer_session` /
  `apply_response` with scripted `TimerSession` outcomes and assert `take_pending_notification()`:
  - fires exactly once on Running→Completed;
  - does **not** fire on Idle→Running, Running→Running re-pull, Completed→Completed re-pull,
    Running→Idle (stop), or the initial-load `Completed` (Assumption A4);
  - re-arms: a new Running after a fired Completed, then Completed again ⇒ fires a second time;
  - `Timer::reset()` (logout) re-arms.
- A thin edge-level test pumping the signal through a `SpyNotifier` to assert the fired text and
  the one-call count.
- Files: under `crates/tui/tests/` and/or `crates/tui/src/app/timer/tests.rs` /
  `app/tests.rs` per rust-standards layout (test files are `tester`-owned).

**No `contract-owner` or `server-dev` work** is expected (Decision 2). If a slice surfaces a
need for either, it is the scope-guard escalation back to `architect`, not a quiet expansion.

### Assumptions (AFK ambiguity policy — recorded, not blocking)

- **A1 — `notify-rust` default build needs no apt package on Ubuntu.** The plan assumes the
  default `zbus` (pure-Rust) backend compiles with only the Rust toolchain. `tui-dev` confirms by
  a clean worktree `./ok.sh build` and documents the observed truth. If the default build in fact
  requires a system C library / `pkg-config` on Linux, that is a **capability gap (#6)** — block
  and ask the operator before installing anything; do not flip to a workaround.
- **A2 — Delivery failures are swallowed silently, with no new logging dependency.** The TUI has
  no `tracing`/`log` dependency today, and the alt-screen terminal makes stderr/stdout writes
  unsafe (they corrupt the display). So a failed notification is mapped to a **no-op** inside
  `DesktopNotifier` — non-fatal, never crashes or blocks the loop (satisfies the
  "silent and non-fatal" criterion). The feature request says "log/trace it and continue"; we
  read "silent and non-fatal" as the binding requirement and drop the log to avoid both a new
  dependency and terminal corruption. If the operator wants the failure surfaced, that is a small
  follow-up (an idea), not a blocker.
- **A3 — The fire-once guard keys on the Running→Completed transition + a boolean re-armed by the
  next Running, not on a session id.** ADR-0002 models a single active session with no per-session
  id on the wire; "at most one active session, stop resets" makes the transition unambiguous. No
  contract field is added (#2).
- **A4 — No notification on the initial post-login `GetTimerSession` that is already `Completed`.**
  The notification alerts at the moment of completion, not on replaying a stale completion at every
  launch. The first session fold after a load/reset only **arms** the guard; it never emits.
- **A5 — Notification copy is fixed text, no sound, no actions.** Title `"Focus timer"`, body
  `"Your focus session has ended."` (exact wording `tui-dev`'s call within the feature-request
  shape). No sound, no action buttons, no rich content, no timeout customization beyond the
  platform default.
- **A6 — Fired on the edge thread (poll loop), synchronously after `apply_response`.** `notify-rust`
  `.show()` returns quickly (a D-Bus message); we accept a brief synchronous call on the poll loop
  rather than adding it to the worker protocol, keeping ADR-0006's request protocol untouched. If
  `.show()` is found to block materially on some platform, moving it onto the worker is a small
  follow-up (still no contract change).

### Risks

- **R1 — Build-time system dependency surprise on Linux (mitigated by A1 + #6).** If the default
  backend pulls `libdbus`/`pkg-config`, the build fails in the worktree → block-and-ask, document
  the real requirement. Likelihood low (zbus is the default), impact contained by the
  capability-gap rule.
- **R2 — No daemon in the verifier / CI environment.** The live `verifier` cannot assert a
  notification *appears* (no desktop session) — by design this is the operator's real-Ubuntu
  manual confirmation (acceptance criterion 4) plus the `tester` spy suite for the fire-once logic.
  The verifier confirms the build/run is unaffected and the TUI `TestBackend` suite is green
  (ADR-0003 handshake). Mitigated: the seam makes the *logic* fully testable without a daemon.
- **R3 — Spurious or missed fire on edge cases** (re-pull jitter, stop-then-start, initial
  Completed). Mitigated by Slice 5's explicit per-transition assertions and A3/A4.
- **R4 — Terminal corruption if a notification path writes to stdout/stderr.** Mitigated by A2
  (the notifier writes nothing to the terminal; errors → no-op).
- **R5 — macOS delivery from an unsigned terminal binary may be limited.** Accepted: cross-OS is
  "ideal", Ubuntu is the binding target; degradation is silent and non-fatal (criterion 4).

### Self-acceptance

- Smallest change that satisfies the criteria: one injected trait + a boolean guard + a one-shot
  signal; no contract/server/migration touch. ✓ (#1/#2/#3 held, ADR-0006 render-loop untouched.)
- Testability: the decision-to-notify is pure core state (`TestBackend`-observable); the effect is
  an injected sanctioned external-service trait (spy). No internal collaborator mocked (ADR-0003 /
  rust-standards). ✓
- Crate choice documented with a per-platform build/runtime matrix per the operator's explicit
  request; the no-apt-package claim is build-confirmed, not asserted blind. ✓
- Capability discipline: a build-time package surprise blocks-and-asks (#6); no binary/package is
  installed to satisfy the item. ✓

Design is low-risk and bounded; no `grill` pass needed.

## Log / comments

- [x] 2026-06-28 [tui-dev] Slice 1 done: added the `Notifier` seam + production `DesktopNotifier`
  (`crates/tui/src/client/notify.rs`, exported from `client/mod.rs`); `notify-rust = "4"` declared
  with **default features only** (C `dbus`/`d` feature left OFF, rationale commented on the dep
  line). `./ok.sh build` clean. **A1 confirmed:** the default `zbus` (pure-Rust D-Bus) backend
  compiled with **no apt package** — `zbus`/`zbus_macros`/`zbus_names` built, **no `dbus` C-binding
  crate** in `Cargo.lock`, and the only `pkg-config` lockfile entry pre-dates this change (2 entries
  before and after). No `libdbus-1-dev`/system `.so` needed on Ubuntu.
- [x] 2026-06-28 [tui-dev] Slice 2 done: pure edge-detection + fire-once guard + one-shot signal.
  Added `Timer::notified_for_session` + `Timer::notify_pending` (both default `false` in
  `Timer::new()`; `reset()` clears via `*self = Self::new()`). `apply_timer_session` now calls
  `detect_completion_edge(&new)` **before** overwriting `self.timer.session`, applying the
  arm/fire/re-arm rules (Decision 4) and the initial-load rule (A4): a `Completed` while the guard
  is un-fired arms the guard and sets `notify_pending`, **except** when this is the first fold
  (`applied_at.is_none()`) where it only arms (no emit); a new `Running`/`Idle` re-arms. Added the
  public `TimerNotification` type (fixed copy: title `"Focus timer"`, body
  `"Your focus session has ended."`) and `App::take_pending_notification(&mut self) ->
  Option<TimerNotification>` (consume-once). `./ok.sh build` clean.
- [x] 2026-06-28 [tui-dev] Slice 3 done: wired the edge. `terminal::run` is now
  `run<N: Notifier>(app, requests, responses, notifier)`; after draining worker responses (and
  their chained follow-ups) it calls `app.take_pending_notification()` and, if `Some`, fires
  `notifier.notify_timer_complete(..)`. Purely reactive — no new request, no new poll (ADR-0006
  unchanged). `main.rs` constructs `DesktopNotifier::new()` and passes it in. `./ok.sh build` clean.
- [x] 2026-06-28 [tui-dev] Slice 4 done (crate-local): added a "Desktop notifications" subsection
  to `crates/tui/README.md` — the per-platform build/runtime matrix (Decision 1), the **no
  build-time apt package** statement recorded as the **observed** truth from the clean worktree
  build (A1/R1 confirmed), and the runtime daemon expectation + silent-non-fatal degradation.
  `rumdl check` clean. **Root `README.md` left for `eng-manager`:** the root README is a repo-wide
  shared doc (home #1 — must land on `main`, never ride a feature branch), and the plan assigns its
  confirmation to `eng-manager` at the learn step. The note to add under "Setting up a development
  environment": *a notification daemon is needed on Linux for timer notifications to appear; none
  is needed to build or run.* All four slices complete — `./ok.sh build | lint | fmt --check` green.
- [x] 2026-06-28 [tester] Slice 5 done: added `crates/tui/tests/notifications.rs` (13 tests) for
  the timer-completion notification. Bulk drives the pure fire-once core through the public two-step
  `App` API + the synchronous worker-analogue executor in `tests/common/mod.rs` (the only mock is
  the sanctioned `Client` external-service trait), asserting `take_pending_notification()`: fires
  exactly once on the Running→Completed edge with the fixed copy (`"Focus timer"` /
  `"Your focus session has ended."`) then `None` on the immediate re-call (consume-once); does **not**
  fire on Idle→Running, Running→Running re-pull, Completed→Completed re-pull, Running→Idle (stop), or
  Idle→Idle; the initial-load `Completed` only arms (A4) and a follow-up re-pull stays silent; re-arms
  on a new Running and fires a **second** time on the next completion; logout (`Timer::reset` via the
  unauthenticated path) clears `notified_for_session`/`notify_pending` (re-arm) and drops any
  undrained signal. A thin edge-level pair pumps the signal through a test `SpyNotifier`
  (implementing `tui::client::Notifier`, recording count + last title/body behind a `RefCell`) to
  assert the one-call count + fixed text and a two-call count across two sessions — mirroring the
  edge's drain-and-fire step in `terminal::run`; no internal collaborator mocked. **`common.rs`
  diagnostic was stale:** `crates/tui/tests/common/mod.rs` exists and is the standard `mod common;`
  shared fixture every integration test already uses; nothing was missing. `./ok.sh test | lint |
  fmt --check` all green (notifications: 13 passed).
- [x] 2026-06-28 [orchestrator] Claimed `ready`→`working`; cut worktree
  `.claude/worktrees/0017-timer-completion-desktop-notification` + branch
  `feature/0017-timer-completion-desktop-notification` from `main@a016e6d` (the commit carrying
  the plan; no ADR — Decision 2 is no-ADR). Branch copy of this item is now authoritative.
- [x] 2026-06-28 [reviewer] Cold review (full feature gate). `./ok.sh build|test|lint|fmt --check`
  all green; `notifications.rs` 13/13 passed. Verified: #1 (only transient `Timer` booleans, no
  persisted state), #2 (no `contract`/server diff — TUI-only), #3 (no domain structure), ADR-0006
  (notifier fires purely reactively after the response drain — no new poll/traffic), fire-once
  semantics + A4 initial-load-only-arms + logout re-arm all correct in code, Decision-3 seam clean
  (only the sanctioned `Notifier` mock; no internal collaborator mocked), A2 silent-non-fatal
  (`let _ = show()`, no stdout/stderr), `notify-rust` default zbus backend / C feature off (A1
  confirmed — no `dbus` C crate in lockfile), docs matrix present. No unjustified `#[allow]`.
  No fix-now findings.
  **`REVIEW-STATUS: approved` — code-hash:d3fa1fc5b3ed5ac0770085809aac150e25012849 sha:d082668.**
  Out-of-scope (to be filed as ideas, non-blocking): A2 drops failure logging; A6 fires `.show()`
  synchronously on the poll loop.

[adr-0002]: ../../docs/adr/0002-pomodoro-timer-authority.md
[adr-0003]: ../../docs/adr/0003-verification-layering.md
[adr-0006]: ../../docs/adr/0006-tui-concurrency-and-responsiveness.md
