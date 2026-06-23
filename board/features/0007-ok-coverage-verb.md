---
id: 0007
title: Add a reported-only `./ok.sh coverage` verb (cargo-llvm-cov, no threshold)
type: chore         # feature | chore
status: working        # inbox → planned → ready → working → review → awaiting-merge → merged | blocked
priority: low       # high | medium | low
parent: null
depends-on: []      # touches ok.sh + workspace tooling only; no crate source, no contract
branch: feature/0007-ok-coverage-verb
worktree: .claude/worktrees/0007-ok-coverage-verb
created: 2026-06-23
updated: 2026-06-23
---

## Feature request

**Goal:** Add a new `./ok.sh coverage` verb that wraps [`cargo-llvm-cov`][llvm-cov] and
**reports** a code-coverage metric for the workspace. It is a developer-facing report only — it
has **no hard threshold** and is **not** a Definition-of-done gate.

**Why:** Captured as an operator-sanctioned follow-up during the 0003 feedback cycle (see
`docs/handoff.md`, the 2026-06-12 entry, item #2) and carried in the Board dashboard's
"sanctioned follow-up" note. The question "where is our coverage?" came up during review; the
answer the operator sanctioned is a *reported* number with no gate, so coverage is visible
without becoming a brittle pass/fail bar. `platform-dev` owns the verb (`ok.sh` is infra);
`eng-manager` documents it.

**Acceptance criteria:**

- [ ] `./ok.sh coverage` runs `cargo-llvm-cov` over the workspace and prints a coverage summary.
- [ ] **No threshold / no gate.** The verb never fails the build on a coverage number, and it is
      **not** added to any Definition-of-done clause. It is purely reported.
- [ ] The verb is added in `ok.sh` (not improvised at call sites), consistent with the existing
      verbs, and shows up in the no-argument usage/help output.
- [ ] **Tooling-only — the chore invariant holds.** No crate source, no behaviour, no
      `contract`/wire shape (#2), and no domain structure (#3) changes. `cargo-llvm-cov` is the
      one operator-sanctioned tool for this (CLAUDE.md hard-constraint #6); if it is not already
      present, that is a capability gap → `blocked` for operator install, never self-acquired.
- [ ] `./ok.sh test | lint | fmt --check` green (unchanged by this verb).

**Out of scope:** any coverage **threshold** or DoD gate; CI wiring / coverage upload; changing
crate source to chase a number; HTML report hosting. Any of these would exceed the chore
invariant and re-scope the item to a `feature` via the scope guard.

<!-- minted directly by the orchestrator as a `chore` — no `architect` plan / no `## Plan(s)`. -->

<!-- append-only; dated, AUTHORED entries. Human feedback = an UNCHECKED box: [ ] unhandled, [x] addressed + re-reviewed (the ONLY re-entry signal). -->
## Log / comments

- 2026-06-23 [orchestrator] minted as a `chore` (no plan) from the operator-sanctioned 0003
  handoff follow-up: a reported-only `./ok.sh coverage` verb over `cargo-llvm-cov`, no hard
  threshold, not a DoD gate. Pure dev-tooling — no product behaviour/contract/domain delta — so
  it fits the chore track (lighter DoD: gates green + a cold `reviewer` approval attesting the
  chore invariant; live verifier pass skipped). Owner on claim: `platform-dev`. Scope guard: if
  the change is found to need a threshold, CI wiring, or source edits, it re-scopes to `feature`
  via `architect`.

[llvm-cov]: https://github.com/taiki-e/cargo-llvm-cov

- 2026-06-23 [orchestrator] claimed for build; cut worktree on branch
  `feature/0007-ok-coverage-verb` from `main`@09445c6. status → working. Owner: `platform-dev`.
  session: drive-0007.

- 2026-06-23 [platform-dev] added a `coverage` verb to `ok.sh`: a `cmd_coverage` function, a
  `coverage)` case branch, and a usage/help line. It runs `cargo llvm-cov --workspace
  --summary-only` (passing through any extra ARGS) and mirrors `cmd_test`'s live-DB wiring —
  honour a caller-provided `DATABASE_URL`, else boot the throwaway test Postgres via the test
  compose file and tear it down on `RETURN`. REPORT-ONLY: no threshold, no gate, not wired into
  any DoD clause. Tooling-only; the chore invariant holds (no crate source, behaviour, contract,
  or domain change). Verified: `./ok.sh fmt --check` clean, `./ok.sh lint` clean, `./ok.sh test`
  green, `./ok.sh coverage` exits 0 printing a per-file table + a TOTAL line (61.48% region /
  66.36% line), and `coverage` appears in the no-arg help. Tool: cargo-llvm-cov 0.8.7
  (operator-sanctioned, already installed — nothing acquired).
