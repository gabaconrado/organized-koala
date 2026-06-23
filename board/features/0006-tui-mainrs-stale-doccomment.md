---
id: 0006
title: Fix stale doc comment in tui/src/main.rs
type: chore         # feature | chore
status: awaiting-merge  # inbox → planned → ready → working → review → awaiting-merge → merged | blocked
priority: low       # high | medium | low
parent: null
depends-on: []      # touches main.rs only; not blocked on 0005, but if claimed after 0005
                    # merges the worktree should be cut from a main that contains 0005's main.rs
branch: feature/0006-tui-mainrs-stale-doccomment
worktree: .claude/worktrees/0006-tui-mainrs-stale-doccomment
created: 2026-06-22
updated: 2026-06-23
---

## Feature request

**Goal:** Correct the stale doc comment at `crates/tui/src/main.rs:4`. It describes a prior
health-probe behaviour that no longer reflects what the binary does; bring the comment in line
with the current `organized-koala` TUI entrypoint (the crossterm/worker-thread shell over the
pure `tui::app` core).

**Why:** Flagged as an out-of-scope pre-existing nit by the `reviewer` during the 0005 cycle and
recorded in `docs/handoff.md` (the 0005 entry) as a "free pickup for the next `tui-dev` touch."
It was stranded as handoff prose because, before the `chore` type existed, the only lane was a
full `feature` item. This is the inaugural `chore`: a comment-only fix.

**Acceptance criteria:**

- [ ] The doc comment at `crates/tui/src/main.rs:4` accurately describes the current TUI
      entrypoint; no stale health-probe wording remains.
- [ ] **Comment-only change.** No code path, signature, behaviour, `contract`/wire shape (#2),
      or domain structure (#3) changes — the chore invariant holds.
- [ ] `./ok.sh test | lint | fmt --check` green.

**Out of scope:** any code/behaviour change; touching any file other than
`crates/tui/src/main.rs`; the `main.rs`-as-thin-lib-shell layout (already correct); anything in
the deferred TUI backlog (profile-switch UX, task edit/delete, Notes, Pomodoro, TUI-side OTel).

<!-- minted directly by the orchestrator as a `chore` — no `architect` plan / no `## Plan(s)`. -->

<!-- append-only; dated, AUTHORED entries. Human feedback = an UNCHECKED box: [ ] unhandled, [x] addressed + re-reviewed (the ONLY re-entry signal). -->
## Log / comments

- 2026-06-22 [orchestrator] minted as the inaugural `chore` (no plan) from the 0005 handoff
  "free pickup": stale `tui/src/main.rs:4` doc comment. Runs the lighter chore DoD —
  tests/lint/fmt + a cold `reviewer` approval attesting the no-behaviour/no-contract/no-domain
  invariant; live verifier pass skipped (CLAUDE.md "Definition of done", chore track).
- 2026-06-23 [orchestrator] claimed → `working`. Cut worktree
  `.claude/worktrees/0006-tui-mainrs-stale-doccomment` + branch
  `feature/0006-tui-mainrs-stale-doccomment` from `main` @ 0585bbf (contains 0005's main.rs).
  session: drive-0006-20260623. Dispatching `tui-dev` for the comment-only fix.
- 2026-06-23 [tui-dev] corrected the stale module doc comment in `crates/tui/src/main.rs`
  (removed the removed-health-probe clause; now describes the worker-thread/pure-`App`
  entrypoint per ADR-0006). Comment-only — no code/behaviour/contract/domain change. Gates
  green: `fmt --check` clean, `lint` clean, `test` all pass (tui TestBackend 11/0). Commit
  e218f73. → `review`.
- 2026-06-23 [reviewer] cold review. Diff vs `main` = the `//!` doc comment in
  `crates/tui/src/main.rs` only (+ feature-local Board state); new comment verified accurate
  against `main()` line-by-line (no health probe; worker-spawn/pure-`App`/`terminal::run`/anyhow).
  Gates green (`fmt --check`, `lint`, `test` all pass). **Chore invariant attested.**
  `REVIEW-STATUS: approved 401ad3de59c4cc7e33c3ebf8308c171d80659e4e (sha 5b5c788) — chore
  invariant attested: no behaviour, no contract/wire (#2), no domain-structure (#3) change;
  comment-only.`
- 2026-06-23 [orchestrator] chore track: live `verifier` pass SKIPPED (clause 4 N/A — no
  behaviour/wire to exercise; the cold reviewer above is the safety net). Dispatching
  `eng-manager` for handoff + summary, then step-7 freshen.
- 2026-06-23 [orchestrator] step-7 freshen: rebased onto `main` @ b1f6848 (eng-manager's
  docs/dashboard advance, no `crates/` change). `./ok.sh code-hash HEAD` unchanged at
  `401ad3de…` = the attested verdict hash, so the reviewer approval carries forward untouched
  (no relabelling). Gates re-run green on the rebased tree (`fmt --check`, `lint`, `test`).
  Board-only commit — no re-review. → `awaiting-merge`.

<!-- written at end of cycle; what the human reviews -->
## Summary

The module doc comment at the top of `crates/tui/src/main.rs` described a prior behaviour —
an *"initial health probe so an unreachable server is reported up front"* — that no longer
matches the binary. That entrypoint was reshaped by 0005 (ADR-0006 Model A): `main` resolves
the server base URL, builds the `reqwest`-backed client, **spawns the worker thread that owns
it**, and hands control to the interactive loop, where the UI thread drives the pure
`tui::app::App` core and never blocks on I/O. The comment was rewritten to describe exactly
that, with the stale health-probe clause removed; the `anyhow` error-propagation note was
kept. Verified line-by-line against `main()`.

**Comment-only — the chore invariant held.** No code path, signature, behaviour, `contract`/
wire shape (#2), or domain structure (#3) changed; the diff vs `main` is the `//!` doc comment
in `crates/tui/src/main.rs` (plus feature-local Board state). Gates green: `./ok.sh fmt
--check`, `./ok.sh lint`, `./ok.sh test` (tui `TestBackend` suite 11 rendering / all flows
pass).

**Inaugural `chore`.** This is the first item to run the lighter chore Definition of done:
clauses 1–3 (test/lint/fmt) plus a cold `reviewer` approval that **explicitly attests the
chore invariant** (no behaviour, no contract/wire, no domain-structure change), pinned to the
code-tree hash `401ad3de59c4cc7e33c3ebf8308c171d80659e4e` (sha `5b5c788`). The live `verifier`
pass (clause 4) was correctly **skipped** — a comment-only change has nothing new for a live
boot to exercise; the cold reviewer is the safety net in its place. Origin: an out-of-scope
pre-existing nit flagged by the `reviewer` during the 0005 cycle and recorded in
`docs/handoff.md` as a "free pickup," minted directly as a `chore` (no `architect` plan).
