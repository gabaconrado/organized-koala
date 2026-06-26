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
| 0007 | `./ok.sh coverage` verb (cargo-llvm-cov, report-only) | Platform | merged | `chore`; reported-only, no threshold, not a DoD gate; reuses `cmd_test`'s live-DB wiring; baseline ~66% line / ~66% function / ~61% region. Cold `reviewer` **approved** (chore invariant attested) @ code-hash `3fa0adef`; live verifier skipped (chore). Fast-forwarded to `main` at `6860b28` |
| 0008 | Pomodoro timer — global duration config + start/stop session | Focus | merged | implements ADR-0002 (timer authority); account-global config + session keyed on `user_id`; five `/api/timer/...` endpoints + reversible migration (`ends_at` derived); TUI render-only countdown from absolute `ends_at` + `server_now` (#1-safe, inside ADR-0006). **0008-R1 feedback re-entry (TUI-only, ADR-0006 §8):** always-visible bottom-right global widget, global `p` toggle, append-spinner (no flicker), ~1-min cadence. Reviewed + live-verified @ code-hash `3fa0adef`; fast-forwarded to `main` |
| 0009 | Run `./ok.sh coverage` in the drive cycle + record the % in each Summary | Platform | merged | `chore`, governance (`main`-only, no worktree); `drive` step 6 captures the headline coverage % into every item's `## Summary`. Report-only, never a gate. Depended on 0007. Cold `reviewer` **approved** (invariant attested); live verifier skipped |
| 0010 | Notes — full feature (contract module, migration, server CRUD, TUI views) | Capture | merged | flat #3 (Title/Content/Created, no `updated_at`); ADR-0007; five profile-scoped routes under `/api/profiles/{id}/notes` (ownership-joined → 404, #4); reversible migration w/ `ON DELETE CASCADE`; TUI `Screen::Notes` (`n`), stateless #1. Reviewed + live-verified @ code-hash `46c1c60f`; fast-forwarded to `main` |
| 0011 | Task update + delete + reopen — generalize close into PATCH (breaking) | Capture | merged | **breaking** (ADR-0008); removes `POST .../tasks/{id}/close`; new `UpdateTaskRequest` partial; `PATCH`/`DELETE …/tasks/{id}` ownership-joined → 404, no migration; TUI `e`/`c`/`x` keys, stateless #1. Re-rebased onto post-0010 `main` (code-hash → `ee5047c9`, re-reviewed + re-verified); fast-forwarded to `main` |
| 0012 | Profiles create/update/delete + TUI switcher (cascade; last-profile guard) | Foundation | merged | **final domain feature** (ADR-0009; two append-only error codes); server `POST`/`PATCH`/`DELETE /api/profiles` owner-scoped (race-safe `409 profile_name_taken`, atomic `409 last_profile`, delete cascades tasks+notes via FK #4); TUI client-side switcher (`s`), no persistence #1. Reviewed + live-verified @ code-hash `71fb7ecf`; fast-forwarded to `main` at `685b4de` |
| 0013 | Redact the session JWT in the `tui` `Session` Debug leak | Platform | merged | high `chore`; bearer JWT held in a `SessionToken(String)` redacting newtype (hand-written `Debug` → `[REDACTED]`, `expose()` at point of use) replacing the bare `String` across `Session` + 17 `ClientRequest::*` + `Outcome::ListProfiles`. `tui`-only, no wire #2 / no domain #3 / no behaviour change. Cold `reviewer` **approved** (invariant attested) @ code-hash `e5925c51`; live verifier skipped (chore). Sharpened `rust-standards` (Debug-lint-vs-secret tension); fast-forwarded to `main` |
| 0014 | TUI layout shell — top-level tabs, centred title, centred auth form, tight footer | TUI overhaul | awaiting-merge (branch-owned) | Phase 1 of 3 (0014→0015→0016); `tui`-only, no `contract`/server/domain change (ADR-0010 §5 binding). `Screen::Main(Box<MainState>)` w/ `Tab{Tasks,Notes,Profiles}` + three live panes; `Tab`/`Shift+Tab` cycle, arrows move selection, switch re-derives pane from a fresh server load (#1/#4); removed `n`/`s`/idle-`Esc`-back, `t` left unbound for 0016; client-captured `Session.account` (no new wire); centred auth box + verbatim title + flush footer. New `navigation.rs` (14 tests). Reviewed + live-verified @ code-hash `bf65aa96`; coverage 72.96% line |
| 0015 | TUI dialog system — help/add/delete/timer modals, trimmed footer caption, purple focus | TUI overhaul | inbox | Phase 2 of 3; depends-on 0014; inherits + cites ADR-0010 (no new shell ADR expected) |
| 0016 | TUI detail views + final hotkey scheme — per-field task/note panes, full keymap | TUI overhaul | inbox | Phase 3 of 3; depends-on 0015; claims `t` for the timer in the full keymap remap; inherits + cites ADR-0010 |
| — | Observability wiring | Platform | not-started | OTLP export, spans on key flows |
| — | Docker deployment | Platform | not-started | compose: server + Postgres + OTel collector |

## Phases

- **Foundation** — auth, profiles, the contract seam, the first TODO slice.
- **Focus** — Pomodoro timer (ADR-0002 accepted; see Board item 0008).
- **Capture** — notes.
- **TUI overhaul** — three-phase TUI shell + interaction reshape (ADR-0010): 0014 layout shell,
  0015 dialog system, 0016 detail views + final hotkeys. Presentation-only (no `contract`/server/
  domain change).
- **Platform** — observability + deployment.
