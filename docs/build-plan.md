# Build plan — roadmap / backlog

Coarse, longer-lived than the Board; mirrors it at a planning grain. Status values match the
Board state machine.

| # | Feature | Phase | Status | Notes |
| --- | --- | --- | --- | --- |
| 0001 | Foundational vertical slice (auth + profile + minimal TODO, end-to-end) | Foundation | planned | umbrella, fanned into 0002→0003→0004; restructures crates into contract/server/tui |
| 0002 | Contract crate + workspace restructure (slice 1 of 0001) | Foundation | in-flight (branch-owned) | pure-DTO seam per ADR-0005; reviewed + verified; live status on `feature/0002-contract-crate`, awaiting human merge |
| 0003 | Server — auth, default profile, tasks, migrations, docker stack (slice 2 of 0001) | Foundation | ready | depends-on 0002 (unblocked once 0002 merges) |
| 0004 | TUI — register/login, default profile, task add/list/close (slice 3 of 0001) | Foundation | ready | depends-on 0003 |
| — | Pomodoro timer | Focus | not-started | blocked on ADR-0002 (timer authority) |
| — | Notes | Capture | not-started | flat: Title/Content/Created |
| — | Multiple profiles UX | Foundation | not-started | profile switch in the TUI |
| — | Observability wiring | Platform | not-started | OTLP export, spans on key flows |
| — | Docker deployment | Platform | not-started | compose: server + Postgres + OTel collector |

## Phases

- **Foundation** — auth, profiles, the contract seam, the first TODO slice.
- **Focus** — Pomodoro timer (needs ADR-0002).
- **Capture** — notes.
- **Platform** — observability + deployment.
