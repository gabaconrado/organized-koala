# Decisions (ADR index)

One row per ADR. Newest at the bottom.

| ADR | Title | Status | Date |
| --- | --- | --- | --- |
| [0001][adr-0001] | Foundational architecture | Accepted | 2026-06-10 |
| [0002][adr-0002] | Pomodoro timer authority | Accepted | 2026-06-23 |
| [0003][adr-0003] | Verification layering — who validates the TUI | Accepted | 2026-06-11 |
| [0004][adr-0004] | Migration authority and the server-binary admin CLI | Accepted | 2026-06-11 |
| [0005][adr-0005] | Foundational wire contract — auth, profile bootstrap, tasks, error codes | Accepted | 2026-06-11 |
| [0006][adr-0006] | TUI concurrency and responsiveness model | Accepted | 2026-06-22 (amended 2026-06-23, 2026-06-26) |
| [0007][adr-0007] | Notes wire contract — module, CRUD routes, flat shape | Accepted | 2026-06-24 |
| [0008][adr-0008] | Task mutation — generalize `close` into PATCH, add DELETE | Accepted | 2026-06-24 |
| [0009][adr-0009] | Profile mutations — create/rename/delete-cascade, last-profile guard, name uniqueness | Accepted | 2026-06-24 |
| [0010][adr-0010] | TUI navigation and interaction model (tabs, dialogs, detail views) | Accepted | 2026-06-26 |
| [0011][adr-0011] | Multiline Content editing in the note detail view — context-dependent commit keymap | Accepted | 2026-06-28 |
| [0012][adr-0012] | Sub-tasks — bounded exception to the flat-domain constraint (#3) | Accepted | 2026-06-29 |
| [0013][adr-0013] | Sub-tasks wire contract — DTO, profile-scoped endpoints, FK-cascade persistence | Accepted | 2026-06-29 |
| [0014][adr-0014] | Task-list pagination-ready limit — additive `limit`+`offset`, bare-array response preserved | Accepted | 2026-07-02 |
| [0015][adr-0015] | Task-list date-window filtering — additive UTC epoch-second bounds; TUI owns civil-day math | Accepted | 2026-07-08 |

[adr-0001]: ./adr/0001-foundational-architecture.md
[adr-0002]: ./adr/0002-pomodoro-timer-authority.md
[adr-0003]: ./adr/0003-verification-layering.md
[adr-0004]: ./adr/0004-migration-authority-and-binary-cli.md
[adr-0005]: ./adr/0005-foundational-wire-contract.md
[adr-0006]: ./adr/0006-tui-concurrency-and-responsiveness.md
[adr-0007]: ./adr/0007-notes-wire-contract.md
[adr-0008]: ./adr/0008-task-mutation-generalization.md
[adr-0009]: ./adr/0009-profile-mutations.md
[adr-0010]: ./adr/0010-tui-navigation-and-interaction-model.md
[adr-0011]: ./adr/0011-multiline-content-editing-keymap.md
[adr-0012]: ./adr/0012-subtasks-domain-exception.md
[adr-0013]: ./adr/0013-subtasks-wire-contract.md
[adr-0014]: ./adr/0014-task-list-pagination-ready-limit.md
[adr-0015]: ./adr/0015-task-list-date-window-query.md
