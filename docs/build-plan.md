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
| 0006 | Fix stale doc comment in `tui/src/main.rs` | Foundation | merged | inaugural `chore` (comment-only); corrected the stale health-probe doc to the ADR-0006 worker/pure-`App` entrypoint; lighter chore DoD (gates + invariant-attesting cold review; live verifier skipped) |
| 0007 | `./ok.sh coverage` verb (cargo-llvm-cov, report-only) | Platform | in-flight (branch-owned) | `chore`; reported-only, no threshold, not a DoD gate; reuses `cmd_test`'s live-DB wiring; baseline ~66% line / ~66% function / ~61% region. Cold `reviewer` **approved** (chore invariant attested) @ code-hash `3fa0adef`; live verifier skipped (chore). Live status `awaiting-merge` on `feature/0007-ok-coverage-verb` |
| 0008 | Pomodoro timer — global duration config + start/stop session | Focus | merged | implements ADR-0002 (timer authority); account-global config + session keyed on `user_id`; five `/api/timer/...` endpoints + reversible migration (`ends_at` derived); TUI render-only countdown from absolute `ends_at` + `server_now` (#1-safe, inside ADR-0006). **0008-R1 feedback re-entry (TUI-only, ADR-0006 §8):** always-visible bottom-right global widget, global `p` toggle, append-spinner (no flicker), ~1-min cadence. Reviewed + live-verified @ code-hash `3fa0adef`; fast-forwarded to `main` |
| — | Notes | Capture | not-started | flat: Title/Content/Created |
| — | Multiple profiles UX | Foundation | not-started | profile switch in the TUI |
| — | Observability wiring | Platform | not-started | OTLP export, spans on key flows |
| — | Docker deployment | Platform | not-started | compose: server + Postgres + OTel collector |

## Phases

- **Foundation** — auth, profiles, the contract seam, the first TODO slice.
- **Focus** — Pomodoro timer (ADR-0002 accepted; see Board item 0008).
- **Capture** — notes.
- **Platform** — observability + deployment.
