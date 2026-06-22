---
id: 0006
title: Fix stale doc comment in tui/src/main.rs
type: chore         # feature | chore
status: inbox       # inbox → planned → ready → working → review → awaiting-merge → merged | blocked
priority: low       # high | medium | low
parent: null
depends-on: []      # touches main.rs only; not blocked on 0005, but if claimed after 0005
                    # merges the worktree should be cut from a main that contains 0005's main.rs
branch: null        # feature/0006-tui-mainrs-stale-doccomment once a worktree is cut
worktree: null      # .claude/worktrees/0006-tui-mainrs-stale-doccomment
created: 2026-06-22
updated: 2026-06-22
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

<!-- written at end of cycle; what the human reviews -->
## Summary
