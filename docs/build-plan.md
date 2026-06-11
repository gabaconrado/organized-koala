# Build plan — roadmap / backlog

Coarse, longer-lived than the Board; mirrors it at a planning grain. Status values match the
Board state machine.

| # | Feature | Phase | Status | Notes |
| --- | --- | --- | --- | --- |
| 0001 | Foundational vertical slice (auth + profile + minimal TODO, end-to-end) | Foundation | inbox | the walking skeleton; restructures crates into contract/server/tui |
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
