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
  the server endpoints (register, login, list profiles, list/add/close tasks, and a health
  probe), implemented by [`HttpClient`](crate::client::HttpClient) on `reqwest`. The standard
  error body `{ code, message }` is mapped to a typed
  [`ClientError`](crate::client::ClientError) that preserves the machine-matchable `code`.
- [`app`](src/app) — the app core. A screen state machine
  ([`App`](crate::app::App)) advanced by **pure update functions** over
  [`Event`](crate::app::Event)s, with the [`Client`](crate::client::Client) injected. It
  holds no terminal or transport types and performs no I/O of its own, so it is exhaustively
  unit-testable.
- [`ui`](src/ui) — rendering. Pure draw functions from an [`App`](crate::app::App) onto a
  `ratatui` frame; no state lives here.
- [`terminal`](src/terminal) — the crossterm driver. Owns raw-mode setup/teardown and the
  blocking input loop that pumps key events into the app core and renders each frame.

## Statelessness

The TUI holds **no** on-disk or cross-run state (hard-constraint #1). The session JWT and the
active profile id live in process memory only, for the lifetime of the run; every view is
derived from a server response. When the server is unreachable the client surfaces a clear
blocking error and a manual retry — it never fabricates or caches domain state.
