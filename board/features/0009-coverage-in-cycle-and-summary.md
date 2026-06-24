---
id: 0009
title: Run `./ok.sh coverage` in the drive cycle and record the % in each item's Summary
type: chore         # feature | chore
status: review          # inbox → planned → ready → working → review → awaiting-merge → merged | blocked
priority: low       # high | medium | low
parent: null
depends-on: [0007]  # SATISFIED 2026-06-24: 0007 merged, `./ok.sh coverage` verb now on `main`
branch: null        # main-only governance change — NO worktree is cut (home #1 / shared state)
worktree: null
created: 2026-06-24
updated: 2026-06-24
---

## Feature request

**Goal (operator request, verbatim intent):** *"Add the coverage run in the process, and report
the code coverage percentage in the summary of the tasks when they are awaiting merge."*

Concretely, two governance edits to the **cycle definition** (home #1 / shared state — see
CLAUDE.md "State has three homes"):

1. **Run `./ok.sh coverage` as part of the drive cycle**, at **step 6 (Learn + summarise,
   `eng-manager`)** — the natural capture point, because step 6 already owns filling the item's
   `## Summary` and runs on **every** cycle (feature and chore).
2. **Record the resulting coverage percentage in each Board item's `## Summary`** so that, by the
   time the item reaches `awaiting-merge`, its Summary carries the headline coverage number.

**Settled operator decisions (design to these — do NOT re-litigate):**

- **Sequencing — gated on 0007.** This item DEPENDS ON `0007` (the `./ok.sh coverage` verb)
  being **merged to `main` first**. Verified 2026-06-24: the verb is **not** on `main`
  (`grep -c cmd_coverage ok.sh` on `main` == 0; 0007 is `awaiting-merge`). The planned item lands
  on `main` now; **implementation must not start until 0007 merges.**
- **Scope — ALL items report coverage.** Both `feature` and `chore` items record a coverage % in
  their Summary, since `eng-manager` runs step 6 on every cycle. Uniform; gives a continuous
  baseline even for chores.
- **Report-only — NEVER a gate.** Coverage stays **report-only**: no threshold, no pass/fail bar,
  and it is **not** added to any Definition-of-done clause. This is consistent with 0007's
  explicit decision and CLAUDE.md's "How to run" `coverage` row (dev-facing, report-only, not a
  DoD gate). The Summary records a *number*; it never blocks reaching `awaiting-merge`.
- **No-docker fallback.** `./ok.sh coverage` boots docker + a throwaway test Postgres. If docker
  is unavailable at the coverage step, the Summary records `coverage: unavailable (docker)` and
  the cycle **still reaches `awaiting-merge`**. A missing docker here is NOT a capability-gap
  block (hard-constraint #6 / the AFK policy): coverage is report-only and never load-bearing on
  the DoD, so its absence cannot fail an item — it is recorded as unavailable and the cycle
  proceeds. (Contrast: docker missing for the **verifier** live pass on a `feature` IS a blocking
  capability gap, because that pass IS a DoD clause. This fallback applies only to the
  report-only coverage capture, never to the verifier gate.)

## Implementation notes (this item is a `chore`; no worktree)

**Why `chore`, not `feature`.** This touches ONLY home-#1 / shared governance state —
`.claude/skills/drive/SKILL.md`, `CLAUDE.md` (docs), and `.claude/agents/eng-manager.md`. It
changes **no** crate source, **no** `contract`/wire shape (#2), **no** domain structure (#3), and
**no** product behaviour — it refines *process/DoD wording* and adds a report-only capture step.
That is exactly the chore no-change invariant. (`./ok.sh coverage` itself already exists once 0007
merges; this item does not author or change that verb.)

**`main`-only — NO worktree is cut.** Per the three-home model, ADRs / `CLAUDE.md` / standards
skills / `.claude/` agent-skill defs are shared/cross-cutting state that lives on `main` ONLY and
must **never** ride a feature branch (the 0002 out-of-sync bug class). Every file this item edits
is in that set, so there is no crate code to isolate and **no worktree/branch is created**.
`eng-manager` applies these edits **directly on `main`** as a governance change (like every prior
learning/process edit), with `branch: null` / `worktree: null` kept. The orchestrator advances
status in place on `main`; there is no claim-and-branch step 2.

> Note for the cycle: because this is a `main`-only `.claude/`+docs change with no worktree, it
> does not flow through the build-in-worktree path. The "implementing agent" is `eng-manager`
> (owner of `docs/**` + `.claude/**`); the cold `reviewer` reads the `main`-side diff and attests
> the chore invariant (clause-6 chore DoD). There is nothing for the verifier to boot (clause-4
> N/A) — consistent with the chore track.

**ADR decision: NO ADR.** This refines the *process/DoD wording* only. It records *where* an
already-decided, already-sanctioned metric (0007's report-only coverage) is captured; it makes no
contract/wire (#2) or domain (#3) decision and explicitly keeps coverage report-only. The
governing decisions already live in 0007 + CLAUDE.md's "How to run" note, so no new recorded
decision is warranted (CLAUDE.md clause-5 / chore-track: a chore makes no contract/domain
decision by definition).

### Precise edit points (exact targets for the implementing `eng-manager`)

1. **`.claude/skills/drive/SKILL.md` — step 6 ("Learn + summarise"), lines ~121–128.** Extend the
   `eng-manager` dispatch description so step 6 explicitly:
   - runs **`./ok.sh coverage`** and parses the **headline workspace coverage percentage** from
     its summary output;
   - writes that percentage into the item's `## Summary` (a one-line `coverage: NN.N%` entry, or
     `coverage: unavailable (docker)` when docker/the test Postgres cannot boot);
   - notes this runs on **every** cycle (feature and chore) and is **report-only — never a gate**;
     it must NOT block the item from reaching `awaiting-merge`. The coverage line is part of the
     `## Summary`, which is committed **on the branch** for branched items (home #2) and on `main`
     for `main`-only governance items.

2. **`CLAUDE.md` — Definition of done, lines ~204–262.** Add a short, gate-neutral sentence
   (either in the preamble at ~206–210 or as a note after the feature clauses ~219–232 and the
   chore track ~245–262) stating: *the item's `## Summary` records the workspace coverage
   percentage from `./ok.sh coverage` (captured at step 6) for both `feature` and `chore` items;
   coverage is **report-only**, recorded for visibility — it is **not** a DoD clause, has **no**
   threshold, and never blocks reaching `awaiting-merge`; if docker is unavailable the Summary
   records `coverage: unavailable (docker)` and the cycle proceeds.* Do **not** number it as a new
   clause 8 and do **not** insert it into clauses 1–7 — it is explicitly NOT a gate.
   - Optionally cross-reference the existing "How to run" `coverage` row (line 58), which already
     calls the verb dev-facing / report-only / not-a-DoD-gate — keep that wording consistent.

3. **`.claude/agents/eng-manager.md` — Primary responsibilities, line ~25** (the bullet: *"Append
   the `docs/handoff.md` entry ... fill the Board item's `## Summary`."*). Extend that bullet so
   `eng-manager`'s charter explicitly includes: run `./ok.sh coverage`, parse the headline
   percentage, and record it in the `## Summary` (with the `unavailable (docker)` fallback);
   report-only, never a gate.

## Acceptance criteria

- [ ] **Gated on 0007.** Implementation does not begin until `0007` (the `./ok.sh coverage` verb)
      is **merged to `main`**; `depends-on: [0007]` is honored.
- [ ] `drive` SKILL **step 6** updated so `eng-manager` runs `./ok.sh coverage`, parses the
      headline coverage %, and writes it into the item's `## Summary` — on **every** cycle (feature
      and chore).
- [ ] The coverage percentage **appears in the `## Summary`** of items by the time they reach
      `awaiting-merge`.
- [ ] **Report-only / no-gate preserved.** No threshold, no pass/fail, NOT added as a DoD clause;
      it never blocks reaching `awaiting-merge`. Consistent with 0007 + CLAUDE.md "How to run".
- [ ] **No-docker fallback wording present:** Summary records `coverage: unavailable (docker)`
      and the cycle still reaches `awaiting-merge` when docker/the test Postgres is unavailable.
- [ ] `CLAUDE.md` Definition-of-done wording updated to describe the (non-gate) coverage Summary
      capture; the "How to run" `coverage` row stays consistent.
- [ ] `.claude/agents/eng-manager.md` charter updated to include the coverage capture in `##
      Summary`.
- [ ] **Chore invariant holds:** docs/`.claude`-only; no crate source, no `contract`/wire (#2),
      no domain (#3), no product behaviour change. Applied **on `main`** with **no worktree**.
- [ ] `./ok.sh test | lint | fmt --check` green (unchanged by this docs/`.claude`-only edit).

**Out of scope (would re-scope to `feature` via the scope guard):** any coverage **threshold** or
DoD gate; per-crate coverage breakdowns or coverage diffs; CI wiring / coverage upload / report
hosting; editing `ok.sh` or the `coverage` verb itself (that is 0007); changing crate source to
move a coverage number.

<!-- planned by architect; main-only governance chore — no worktree, applied on main by eng-manager. -->

<!-- append-only; dated, AUTHORED entries. Human feedback = an UNCHECKED box: [ ] unhandled, [x] addressed + re-reviewed (the ONLY re-entry signal). -->
## Log / comments

- 2026-06-24 [architect] Planned the operator's process/governance request: (1) run
  `./ok.sh coverage` in the drive cycle and (2) record the coverage % in each item's `## Summary`
  by `awaiting-merge`. Typed `chore` (docs + `.claude`-only; no behaviour/contract/domain delta).
  **No ADR** — refines DoD *wording* around an already-decided, already-sanctioned report-only
  metric (0007 + CLAUDE.md "How to run"); makes no contract/domain decision. `depends-on: [0007]`:
  verified the `coverage` verb is NOT yet on `main` (`grep -c cmd_coverage ok.sh` == 0); build
  waits for 0007 to merge. **`main`-only — NO worktree**: every edited file (drive SKILL,
  CLAUDE.md, eng-manager agent) is home-#1 shared state and must never ride a branch; `eng-manager`
  applies it directly on `main`. status: planned → ready (self-accepted; smallest plan, settled
  decisions encoded, no genuine fork).
- 2026-06-24 [orchestrator] Claimed for the cycle. Dependency `0007` is now **merged to `main`**
  (`grep -c cmd_coverage ok.sh` == 2 on `main`); 0009 unblocked. Per plan this is a `main`-only
  governance chore — **NO worktree** is cut; `eng-manager` applies the edits directly on `main`,
  the cold `reviewer` reads the `main`-side diff + attests the chore invariant, verifier SKIPPED
  (chore). status: ready → working (in place on `main`).
- 2026-06-24 [eng-manager] Built the three governance edits on `main` (commit `6b6e373`):
  drive SKILL step 6 (run `./ok.sh coverage`, parse headline %, write `coverage: NN.N%` /
  `coverage: unavailable (docker)` into `## Summary` every cycle, report-only); CLAUDE.md DoD
  gate-neutral note; eng-manager charter bullet. Gates green: `fmt --check`, `lint`, `test` all
  pass (docs/`.claude`-only). status: working → review. (Co-author trailers on `6c83e73`/`6b6e373`
  corrected to `*@organized-koala.local` per git-standards — the dispatch's `noreply@anthropic.com`
  is never correct in this repo; learned 0003/0004.)
- 2026-06-24 [reviewer] Cold review of the `main`-side governance diff (`6b6e373`). All acceptance
  criteria met: drive SKILL step 6 captures the headline coverage % into `## Summary` every cycle
  (report-only); CLAUDE.md DoD note is gate-neutral (not a clause, no threshold); eng-manager
  charter updated; no-docker fallback present in all three files; wording consistent with the "How
  to run" `coverage` row and the three-home model. Gates green (`fmt --check`/`lint`/`test`).
  **Chore invariant ATTESTED** — `./ok.sh code-hash HEAD` == `3fa0adef…` == pre-0009 `cef68fe`
  (zero code-path delta ⇒ no behaviour / no `contract`-wire (#2) / no domain-structure (#3)
  change); no scope-guard re-type warranted.

  ```text
  REVIEW-STATUS: approved
  code-hash: 3fa0adefce8cd6d67ae716dae7a24ce6dbf9defd
  commit: 97b9794  (human-readable pointer; binding key is the code-hash)
  chore-invariant: ATTESTED (no behaviour / no contract-wire #2 / no domain-structure #3 change)
  ```

## Summary

Wired **report-only coverage capture into the development cycle** and made each Board item's
`## Summary` carry the headline coverage number by the time it reaches `awaiting-merge`. Three
governance edits, all home-#1 shared state:

- **`drive` SKILL step 6 ("Learn + summarise")** — `eng-manager` now runs `./ok.sh coverage`,
  parses the headline workspace coverage %, and writes a one-line `coverage: NN.N%` entry into the
  item's `## Summary` (or `coverage: unavailable (docker)` when docker / the throwaway test
  Postgres cannot boot). Runs on **every** cycle (feature and chore); **report-only — never a
  gate**, so it never blocks reaching `awaiting-merge`.
- **`CLAUDE.md` Definition of done** — a short, gate-neutral note: the Summary records the
  `./ok.sh coverage` percentage for both `feature` and `chore` items, for visibility only — not a
  DoD clause, no threshold, never blocking; docker-unavailable records `unavailable (docker)` and
  the cycle proceeds. Not numbered as a clause; consistent with the "How to run" `coverage` row.
- **`.claude/agents/eng-manager.md` charter** — the Summary-filling responsibility now explicitly
  includes the coverage capture + the `unavailable (docker)` fallback (report-only).

**Classification:** `chore` — docs/`.claude`-only, no crate source, no `contract`/wire (#2), no
domain-structure (#3), no product behaviour. **`main`-only governance change — NO worktree was
cut** (every edited file is home-#1 shared state that must never ride a feature branch). Applied
directly on `main` by `eng-manager`; the orchestrator advanced status in place. Cold `reviewer`
**approved** at code-hash `3fa0adefce8cd6d67ae716dae7a24ce6dbf9defd` with the **chore invariant
attested** (code-path digest byte-identical to pre-0009 `cef68fe` ⇒ zero code delta); the live
`verifier` pass was **skipped** (chore track, clause 4 N/A).

**Depended on 0007** (the `./ok.sh coverage` verb), which **merged to `main` first** — this item
consumes that verb and could not start until it landed.

coverage: 66.36% line (61.48% region, 66.67% function) — headline `TOTAL` from `./ok.sh coverage`.
This is the **first item to carry a coverage line**, dogfooding the very rule it introduces.
