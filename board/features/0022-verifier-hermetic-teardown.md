---
id: 0022
title: Make the verifier stack boot hermetic — always tear down its own volume (down -v on any exit)
type: chore         # feature | chore
status: review          # inbox → planned → ready → working → review → awaiting-merge → merged | blocked
priority: low       # high | medium | low
parent: null
depends-on: []      # touches ok.sh + verifier discipline only; no crate source, no contract
branch: null        # main-only chore — NO worktree is cut (home #1 / shared infra: ok.sh + .claude/)
worktree: null
created: 2026-07-02
updated: 2026-07-09
---

## Feature request

**Goal:** Make each verifier stack boot **hermetic** — the stack is brought up, exercised, and
then **always** torn down destroying its own volume (`down -v`), on **any** exit (success,
failure, or signal). A verifier never leaves behind a Postgres volume / migration history for a
later boot to inherit.

**Why:** Captured as idea [`board/ideas/0001-per-worktree-compose-isolation.md`][idea-0001]
(surfaced 0011, operator-triaged 2026-07-02). Concurrent worktrees today share one compose
project (`deploy`) and one named volume (`deploy_postgres-data`), so a verifier booting on
worktree X can inherit the migration history left by worktree Y — sqlx's strict migration-history
check then fails and the one-shot `migrate` errors (the documented CLAUDE.md gotcha, learned
0011). The idea proposed two fixes; the **operator accepted only approach (1)** (this chore) and
**declined approach (2)** (per-worktree `COMPOSE_PROJECT_NAME` isolation), because development is
intentionally **serialized** for the foreseeable future — dev/verify sessions never run in
parallel, so the cross-worktree conflict cannot arise in practice and the isolation wiring is
unwarranted complexity. In serial execution, hermetic teardown eliminates the failure mode: with
no state surviving a run, there is never a leftover migration history to inherit. Bonus — a
verifier tearing down state **it just created itself** is cleaning up its own mess, so `down -v`
here needs **no** operator authorization (that sign-off was only ever about destroying *another*
branch's data); this **removes** a human-in-the-loop block rather than adding one.

**Acceptance criteria:**

- [ ] The verifier's live-boot flow tears down its own volume (`down -v`) on **any** exit —
      success, failure, and signal — via a `trap`/`finally`-style guarantee, not a happy-path-only
      final command. The teardown targets the same compose project/volume the boot created.
- [ ] Teardown discipline is expressed in `ok.sh` (extend the script, per CLAUDE.md — verbs/flow
      are not improvised at call sites) and/or the `verifier` agent instructions, so a verifier
      run is hermetic by construction rather than by remembering to clean up.
- [ ] **No operator authorization required** for this self-cleanup: the item's teardown destroys
      only state the same run created, distinct from the operator-gated reset that destroys another
      branch's data.
- [ ] **Tooling/process-only — the chore invariant holds.** No crate source, no product behaviour,
      no `contract`/wire shape (#2), and no domain structure (#3) changes. Docker is the one
      sanctioned tool (CLAUDE.md hard-constraint #6); if it is unavailable that is a capability gap
      → `blocked` for the operator, never self-acquired or worked around.
- [ ] `./ok.sh test | lint | fmt --check` green (unchanged by this change).

**Out of scope (explicitly, per the operator's decision):**

- **Per-worktree `COMPOSE_PROJECT_NAME` isolation (idea 0001 approach (2))** — declined; the
  serialized workflow makes it unnecessary. Not part of this chore.
- The **hard-crash residual** (reboot / OOM-kill can strand a volume before a trap fires) — a trap
  cannot cover it, and only approach (2) would make the failure structurally impossible. It remains
  the rare case handled by the existing operator-authorized reset; not addressed here.
- Any CI wiring or changes to product behaviour, contract, or domain structure. Any of these would
  exceed the chore invariant and re-scope the item to a `feature` via the scope guard.

<!-- minted directly by the orchestrator as a `chore` — no `architect` plan / no `## Plan(s)`. -->

[idea-0001]: ../ideas/0001-per-worktree-compose-isolation.md

<!-- append-only; dated, AUTHORED entries. Human feedback = an UNCHECKED box: [ ] unhandled, [x] addressed + re-reviewed (the ONLY re-entry signal). -->
## Log / comments

- 2026-07-02 [orchestrator] minted as a `chore` (no plan) from operator-accepted idea 0001,
  approach (1) only. Make the verifier stack boot hermetic — always `down -v` on any exit — so no
  Postgres volume / migration history survives for a later boot to inherit; in the serialized
  workflow this eliminates the learned-0011 migration-history conflict. Approach (2) (per-worktree
  `COMPOSE_PROJECT_NAME`) is declined and explicitly out of scope. Pure infra/process — no product
  behaviour/contract/domain delta — so it fits the chore track (lighter DoD: gates green + a cold
  `reviewer` approval attesting the chore invariant; live verifier pass skipped). Owner on claim:
  `platform-dev`. Scope guard: if the change is found to need a contract/behaviour/domain delta, it
  re-scopes to `feature` via `architect`.
- 2026-07-09 [orchestrator] (session drive-20260709-173945) Claimed → `working`. **Main-only
  chore, NO worktree cut** — every file this item edits (`ok.sh`, `.claude/agents/verifier.md`) is
  home #1 shared/cross-cutting infra that must never ride a feature branch (0002 out-of-sync bug
  class); it changes no crate source, so there is nothing to isolate. Advanced in place on `main`,
  same pattern as 0009. Owner `platform-dev` implements the `ok.sh` teardown mechanism; `eng-manager`
  (owns `.claude/**`) applies the `verifier.md` instruction edit referencing it. Tooling present and
  sanctioned: docker daemon UP, shellcheck + cargo on PATH — no capability gap.
- 2026-07-09 [platform-dev] Landed `ok.sh` mechanism (commit f764dbe): new `./ok.sh verify-boot
  <command> [args...]` verb — one process brings the deploy stack up (`--wait`), runs the caller's
  exercise `<command>` against the live stack, then GUARANTEES `down --volumes` teardown on any exit
  (EXIT + INT/TERM/HUP traps), targeting the same project/volume it created (`deploy` /
  `deploy_postgres-data`) and preserving the exercise's exit status. Plain dev `./ok.sh down`
  unchanged (keeps the volume). Gates green (fmt/lint/shellcheck/test); code-hash unchanged
  (ok.sh-only).
- 2026-07-09 [eng-manager] Landed `verifier.md` wiring (commit 5195745): verifier now boots+exercises
  via `./ok.sh verify-boot <command>` (whole exercise passed as the command) instead of manual
  `up`+`down`; teardown is guaranteed-by-construction, no lingering `deploy_postgres-data` volume.
  Read-only/report-only/chore-mis-dispatch/capability-gap notes preserved.
- 2026-07-09 [reviewer] **REVIEW-STATUS: approved** — cold review, gates all green
  (`test`/`lint`/`fmt --check`/`shellcheck ok.sh`). Trap logic verified: teardown fires on success,
  failure, AND signal; exercise exit status preserved (`local status="$?"` first); targets the same
  `deploy`/`deploy_postgres-data` created by the boot; no operator-authorization dependency. Out-of-
  scope items confirmed NOT done (no per-worktree `COMPOSE_PROJECT_NAME`, no CI, no crash-residual).
  **CHORE-INVARIANT confirmed:** no behaviour change (shipped `cmd_up`/`cmd_down`/serve path
  unchanged; `verify-boot` is verifier-only tooling), no `contract`/wire change (#2) (`crates/`
  untouched), no domain-structure change (#3). Pinned to CODE-HASH `700e3b535c587fd309e4de0a5f973867a577fc02`,
  REVIEWED-SHA `51957454909e762f423cd5ad6662716357c2b746`.
- 2026-07-09 [orchestrator] Chore track: **step-5 live verifier pass SKIPPED** (chore clause 4 N/A —
  no behaviour/wire to exercise; the cold reviewer's invariant attestation is the safety net).
  Proceeding to eng-manager tail (Summary + coverage + handoff), then awaiting-merge.

## Summary

A **main-only `chore`** landing **approach (1)** of idea
[`0001`][idea-0001] — hermetic verifier stack boot — so a verifier never strands a Postgres
volume / migration history for a later boot to inherit.

- **`ok.sh` — new `verify-boot <command>` verb** (commit `f764dbe`): one process brings the
  `deploy` stack up (`--wait`), runs the caller's exercise `<command>` against the live stack,
  then **guarantees `down --volumes` teardown on any exit** — success, failure, or signal — via
  EXIT + INT/TERM/HUP traps, targeting the same project/volume it created
  (`deploy` / `deploy_postgres-data`) and **preserving the exercise's exit status**. Plain dev
  `./ok.sh down` is unchanged (keeps the volume).
- **`verifier` agent wiring** (commit `5195745`): the verifier now boots + exercises via
  `./ok.sh verify-boot <command>` instead of a manual `up` + happy-path `down`, so teardown is
  guaranteed by construction rather than by remembering to clean up.
- **Effect:** in the intentionally **serialized** dev/verify workflow this **eliminates** the
  learned-0011 cross-worktree migration-history conflict — with no state surviving a run there is
  never a leftover migration history to inherit — and the self-cleanup needs **no** operator
  authorization (it destroys only state the same run created, distinct from the operator-gated
  reset that would destroy another branch's data).
- **Honest residual (out of scope, per the operator's decision):** the **hard-crash residual**
  (reboot / OOM-kill before the trap fires) and true **concurrent** worktrees are **not** covered
  by a trap — only the declined approach (2) (per-worktree `COMPOSE_PROJECT_NAME` isolation) would
  make the failure structurally impossible; that rare case remains handled by the existing
  operator-authorized `docker compose down -v` reset.
- **Chore invariant held** — cold `reviewer` **approved**, attesting **no** behaviour change
  (shipped `cmd_up`/`cmd_down`/serve path untouched; `verify-boot` is verifier-only tooling),
  **no** `contract`/wire change (#2), **no** domain-structure change (#3). `crates/` untouched;
  pinned to CODE-HASH `700e3b535c587fd309e4de0a5f973867a577fc02`
  (REVIEWED-SHA `51957454909e762f423cd5ad6662716357c2b746`). Chore track → the **live verifier
  pass is skipped** (clause 4 N/A). Gates green (`test` / `lint` / `fmt --check` / `shellcheck`).

coverage: 73.20% (report-only, not a gate — **unchanged** by this chore; no crate code touched,
so identical to 0023's headline figure. `TOTAL … 73.20% … 543 97 82.14% … 75.12%` from
`./ok.sh coverage`).
