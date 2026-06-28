# tui — the `organized-koala` terminal client

The `organized-koala` TUI: a stateless terminal client over the organized-koala server,
built on `ratatui` + `crossterm` for the terminal surface and `reqwest` for the HTTP path.
It consumes the [`contract`](../contract) crate's DTOs as the single source of truth for
every wire shape; it defines no DTOs of its own.

## Layers

The crate is split into cleanly separable layers so the interactive surface can be driven
through a `ratatui` `TestBackend` with the server mocked (ADR-0003), and so the live reqwest
path can be exercised against a real server:

- [`client`](src/client) — the HTTP boundary. A [`Client`](crate::client::Client) trait over
  the server endpoints (register, login, list/create/rename/delete profiles,
  list/add/update/delete tasks and notes, the timer config/session, and a health probe),
  implemented by [`HttpClient`](crate::client::HttpClient) on `reqwest`. The standard
  error body `{ code, message }` is mapped to a typed
  [`ClientError`](crate::client::ClientError) that preserves the machine-matchable `code`.
- [`app`](src/app) — the app core. A screen state machine ([`App`](crate::app::App)) advanced
  by **two pure functions**: [`handle_event`](crate::app::App::handle_event) folds an
  [`Event`](crate::app::Event) into the next state and returns an optional
  [`ClientRequest`](crate::app::ClientRequest) to run, and
  [`apply_response`](crate::app::App::apply_response) folds a completed server
  [`ClientResponse`](crate::app::ClientResponse) back into state. The core holds **no** client
  and performs no I/O, so it is exhaustively unit-testable with no threads. Per-feature
  submodules (`auth`, `task_add`, `task_list`, `notes`, `profiles`) own their screen state;
  `app/mod.rs` keeps the `App`/`Screen` wiring and the request/response protocol. Switching the
  active profile is **client-side only** — picking a profile rebinds the in-memory active id and
  re-scopes the reads; there is no server "switch" call and no persistence (#1, ADR-0009).
- [`ui`](src/ui) — rendering. Pure draw functions from an [`App`](crate::app::App) onto a
  `ratatui` frame, including the in-flight spinner; no state lives here.
- [`terminal`](src/terminal) — the crossterm driver. Owns raw-mode setup/teardown and the
  **non-blocking poll loop** (ADR-0006): it polls input with a tick timeout, drains the worker's
  response channel, and redraws every tick, so the UI stays live and a spinner animates while a
  request is outstanding. The [`worker`](crate::client::worker) thread owns the real
  [`HttpClient`](crate::client::HttpClient) and executes requests off the UI thread; at most one
  request is in flight, and a cancelled request's late response is dropped by id.

## Statelessness

The TUI holds **no** on-disk or cross-run state (hard-constraint #1). The session JWT and the
active profile id live in process memory only, for the lifetime of the run; every view is
derived from a server response. When the server is unreachable the client surfaces a clear
blocking error and a manual retry — it never fabricates or caches domain state.

## Desktop notifications

When a focus session reaches its end, the TUI fires **one** plain desktop notification (title
*"Focus timer"*, body *"Your focus session has ended."* — no sound, no buttons), exactly once per
completed session. It re-arms when a new session starts; it never re-fires while the session sits
completed, and it does **not** replay a stale completion observed on the first session load after
login. Notifications use [`notify-rust`](https://docs.rs/notify-rust) via the injected
[`Notifier`](crate::client::Notifier) seam (production impl
[`DesktopNotifier`](crate::client::DesktopNotifier)).

### Build-time and runtime requirements

The dependency is declared with **default features only** — on Linux this keeps the **pure-Rust
`zbus` D-Bus backend**; the optional C `dbus`/`d` (`libdbus`) feature is deliberately left off.

| Platform | Backend | Build-time system package | Runtime requirement |
| --- | --- | --- | --- |
| **Linux / Ubuntu** | `zbus` (pure-Rust D-Bus) | **None.** Confirmed by a clean `./ok.sh build` in this worktree: no `libdbus-1-dev`, no `pkg-config`, no system `.so` — the pure-Rust stack compiles with only the Rust toolchain. | A notification daemon owning `org.freedesktop.Notifications` on the **session** D-Bus. Present on Ubuntu GNOME out of the box; **absent** on a bare TTY / headless / SSH-without-a-graphical-session. |
| **macOS** | native (bundled in the crate) | **None.** | The system Notification Center; delivery from a bare unsigned terminal binary may be limited. |
| **Windows** | WinRT toast | **None.** | The Windows notification subsystem (present on modern Windows). |

**No build-time apt package is required on Ubuntu.** Delivery is **best-effort and silent**: any
failure (no daemon, unsupported platform, closed D-Bus session) is mapped to a no-op inside
`DesktopNotifier` — it never crashes, blocks, or writes to the terminal (which would corrupt the
alt-screen display). A missing daemon simply means no notification appears.
