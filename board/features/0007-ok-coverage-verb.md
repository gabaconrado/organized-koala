---
id: 0007
title: Add a reported-only `./ok.sh coverage` verb (cargo-llvm-cov, no threshold)
type: chore         # feature | chore
status: merged         # inbox → planned → ready → working → review → awaiting-merge → merged | blocked
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

- 2026-06-23 [reviewer] cold review — **REVIEW-STATUS: approved** @ code-hash
  `3fa0adefce8cd6d67ae716dae7a24ce6dbf9defd` (commit c4387b7, for reference). Gates green
  (`fmt --check` / `lint` / `test`). Diff is `ok.sh` (+31) + this Board file only — zero
  `crates/` source. **Chore-invariant attested:** no behaviour change, no `contract`/wire
  change (#2), no domain-structure change (#3). Verb is report-only (no threshold/gate, not a
  DoD clause). code-hash byte-identical to last-merged head, corroborating tooling-only scope.

## Summary

**End state:** `./ok.sh` gained a report-only `coverage` verb. The cycle reached the
AI-terminal `awaiting-merge` on `feature/0007-ok-coverage-verb` via the **lighter chore DoD**;
the human performs the final merge.

**What was added (`ok.sh` only):** a `cmd_coverage` function, a `coverage)` case branch, and a
no-arg usage/help line. The verb runs `cargo llvm-cov --workspace --summary-only "$@"` (extra
ARGS pass through) and **mirrors `cmd_test`'s live-DB wiring**: honour a caller-supplied
`DATABASE_URL`, else boot the throwaway test Postgres via the test compose file and tear it down
on a `RETURN` trap. `cargo-llvm-cov` 0.8.7 was already present and operator-sanctioned
(CLAUDE.md #6) — nothing was acquired.

**Report-only decision:** the verb prints a per-file table + a `TOTAL` line and **exits 0
regardless of the number** — **no threshold**, not wired into any Definition-of-done clause.
This is the operator-sanctioned shape (captured in the 0003 handoff follow-up): coverage made
*visible* without becoming a brittle pass/fail bar. Baseline at implementation: **~66% line /
~66% function / ~61% region** (`TOTAL` reported 61.48% region / 66.36% line) — a reference
point, not a target.

**Chore invariant:** the diff is `ok.sh` (+31) plus this Board file — **no crate source, no
behaviour, no `contract`/wire (#2), no domain-structure (#3) change**. The code-hash is
byte-identical to the last-merged head, corroborating tooling-only scope.

**Gate results (chore track):**

- Clauses 1–3 — `./ok.sh test` green, `./ok.sh lint` clean, `./ok.sh fmt --check` clean.
- Clause 4 (live `verifier`) — **correctly SKIPPED.** A chore changes no behaviour/wire/API, so
  there is nothing for a live boot to exercise; the cold reviewer is the safety net.
- Clause 5 (ADR) — N/A (a chore makes no contract/domain decision).
- Clause 6 (`reviewer` approved) — **REVIEW-STATUS: approved** @ code-hash
  `3fa0adefce8cd6d67ae716dae7a24ce6dbf9defd` (commit `c4387b7`, for reference), with the chore
  invariant explicitly attested (no behaviour / no `contract`-wire / no domain-structure change).
- Clause 7 — branch rebased current on `main` (verdict pins to the code-tree hash, unchanged
  across the docs-/board-only `main` advance).

- 2026-06-23 [orchestrator] step-7 freshen — rebased onto `main`@9bc12f9 (eng-manager
  learnings + dashboard). Only conflict was this feature-local Board file (dropped `main`'s
  frozen-pointer note in favour of the branch copy). **code-hash unchanged @ `3fa0adef`** →
  reviewer `approved` verdict carries forward untouched (no relabel). Gates re-run green on the
  rebased tree (`fmt --check` / `lint` / `test`). Verify correctly skipped (chore).

- 2026-06-24 [orchestrator] operator authorized close ("happy with the changes"). Re-freshened
  onto `main`@11593bf (the 0009 plan commit) — clean rebase, **code-hash unchanged @ `3fa0adef`**
  so the reviewer `approved` verdict carries forward untouched; gates re-confirmed (fmt/lint
  clean). Fast-forward merged into `main`. status → merged.
