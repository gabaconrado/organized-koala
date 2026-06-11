# ADR-0001: Foundational architecture

**Status:** Accepted · 2026-06-10

## Context

organized-koala is a minimal personal-productivity suite (flat TODO, Pomodoro timer,
free-form notes, profile namespaces). It needs a shape that keeps the domain deliberately
small, isolates a thin client from server-owned state, and is testable and observable. These
decisions were settled in the bootstrap interview and shape the contract, so they are recorded
before implementation.

## Decision

1. **Two components over a shared contract.** A Rust HTTP server (`organized-koalad`) and a
   Rust TUI (`organized-koala`) communicate over HTTP/JSON. All wire types live in a single
   `contract` crate that is the source of truth; neither side redefines a DTO. Stack:
   `axum`+`tokio`+`sqlx`(Postgres) on the server, `ratatui`+`crossterm`+`reqwest` on the TUI,
   `serde` for wire types.
2. **The TUI is stateless.** It requires the server online and holds no local persistence;
   every view derives from a server response.
3. **The domain is deliberately flat.** TODO = {Title, Description, Status, Created-at,
   Closed-at}; Notes = {Title, Content, Created-at}; Pomodoro = global config with duration as
   the only knob (default 30 min, no pause, stop resets). No subtasks/tags/categories.
4. **Profiles are namespaces.** An account holds many profiles; each owns its TODOs and Notes;
   no cross-profile access. All domain queries are profile-scoped.
5. **Auth is local-only.** Username/email + password hashed with `argon2`; sessions via JWT.
   No SSO/external IdP.
6. **Error contract.** Every error is the standard HTTP status code plus a JSON body
   `{ "code": <optional app-error-code>, "message": <string> }`.
7. **Operations go through `./ok.sh`.** A single root script wraps build/test/lint/fmt/migrate/
   stack; sqlx runs in offline mode (committed `.sqlx/`). Deployment is Docker (server +
   Postgres + OTel collector); the TUI runs on the host.
8. **Observability is OpenTelemetry** via `tracing` + `tracing-opentelemetry` +
   `opentelemetry-otlp` (OTLP export to the collector).

## Consequences

- The `contract` crate is a hard seam: any wire change is ADR-worthy and ripples to both sides.
- A stateless TUI means responsiveness depends on the server; offline UX is explicitly out of
  scope, and the **timer authority** question (server owns the countdown vs. client) must be
  settled in ADR-0002 before Pomodoro work.
- Flatness trades future flexibility for simplicity; adding structure later requires an ADR.
- Local auth keeps scope small but rules out federation without a future decision.
