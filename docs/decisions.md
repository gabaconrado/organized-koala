# Decisions (ADR index)

One row per ADR. Newest at the bottom.

| ADR | Title | Status | Date |
| --- | --- | --- | --- |
| [0001][adr-0001] | Foundational architecture | Accepted | 2026-06-10 |
| [0003][adr-0003] | Verification layering — who validates the TUI | Accepted | 2026-06-11 |
| [0004][adr-0004] | Migration authority and the server-binary admin CLI | Accepted | 2026-06-11 |
| [0005][adr-0005] | Foundational wire contract — auth, profile bootstrap, tasks, error codes | Accepted | 2026-06-11 |
| [0006][adr-0006] | TUI concurrency and responsiveness model | Accepted | 2026-06-22 |

> Pending: **ADR-0002 — Pomodoro timer authority** (server-owned countdown vs. client-side).
> The `architect` must write this before any Pomodoro implementation. (Number reserved; do not
> reuse — ADR-0003 deliberately skips it.)

[adr-0001]: ./adr/0001-foundational-architecture.md
[adr-0003]: ./adr/0003-verification-layering.md
[adr-0004]: ./adr/0004-migration-authority-and-binary-cli.md
[adr-0005]: ./adr/0005-foundational-wire-contract.md
[adr-0006]: ./adr/0006-tui-concurrency-and-responsiveness.md
