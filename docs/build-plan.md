# Build plan — roadmap / backlog

Coarse, longer-lived than the Board; mirrors it at a planning grain. Status values match the
Board state machine.

| # | Feature | Phase | Status | Notes |
| --- | --- | --- | --- | --- |
| 0001 | Foundational vertical slice (auth + profile + minimal TODO, end-to-end) | Foundation | planned | umbrella, fanned into 0002→0003→0004; restructures crates into contract/server/tui; end-to-end acceptance closeable once 0004 merges |
| 0002 | Contract crate + workspace restructure (slice 1 of 0001) | Foundation | merged | pure-DTO seam per ADR-0005 |
| 0003 | Server — auth, default profile, tasks, migrations, docker stack (slice 2 of 0001) | Foundation | merged | full ADR-0005 HTTP API on Postgres; docker stack; reviewed + live-verified |
| 0004 | TUI — register/login, default profile, task add/list/close (slice 3 of 0001) | Foundation | merged | completes the foundational tracer bullet; reviewed + live-verified |
| 0005 | TUI — responsive (non-blocking) event loop + `tui::app` submodule reorg | Foundation | merged | ADR-0006 Model A (worker thread + mpsc + polled loop, no async); TUI-only; reviewed + live-verified |
| 0006 | Fix stale doc comment in `tui/src/main.rs` | Foundation | in-flight (branch-owned) | inaugural `chore` (comment-only); corrects the stale health-probe doc to the ADR-0006 worker/pure-`App` entrypoint; lighter chore DoD (gates + invariant-attesting cold review; live verifier skipped); live status on `feature/0006-tui-mainrs-stale-doccomment` |
| 0007 | `./ok.sh coverage` verb (cargo-llvm-cov, report-only) | Platform | inbox | `chore`; reported-only, no threshold, not a DoD gate |
| 0008 | Pomodoro timer | Focus | inbox | unblocked — ADR-0002 (timer authority) accepted; feature card minted, awaiting `architect` plan |
| — | Notes | Capture | not-started | flat: Title/Content/Created |
| — | Multiple profiles UX | Foundation | not-started | profile switch in the TUI |
| — | Observability wiring | Platform | not-started | OTLP export, spans on key flows |
| — | Docker deployment | Platform | not-started | compose: server + Postgres + OTel collector |

## Phases

- **Foundation** — auth, profiles, the contract seam, the first TODO slice.
- **Focus** — Pomodoro timer (ADR-0002 accepted; see Board item 0008).
- **Capture** — notes.
- **Platform** — observability + deployment.
