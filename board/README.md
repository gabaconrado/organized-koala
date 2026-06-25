# Board

Coordination state for organized-koala (this replaces a ticket tracker). One file per work
item in `features/`; the `status:` frontmatter **is** the state machine. This dashboard is
**derived** — `eng-manager` regenerates it from item frontmatter; do not hand-edit as truth.

> The Board is committed and potentially public. **Never write secrets or sensitive payloads
> into any item.** Describe behaviour and shape.

## State machine

```text
inbox → planned → ready → working → review → awaiting-merge → merged | blocked
```

The AI cycle is terminal at `awaiting-merge`; only the human merges (→ `merged`). An item is
born on `main` during planning, then becomes **branch-owned on claim**: its live status
advances on the feature branch while `main`'s snapshot stays frozen at the claim until the
human's merge (see CLAUDE.md "The Board"). The `Status` column below shows `main`'s snapshot;
for an in-flight item the authoritative live status is on its branch.

**Item `type`.** Each item is `feature` (default) or `chore` (see CLAUDE.md "The Board"). A
`feature` carries an `architect` plan + any ADR and runs the full Definition of done; a `chore`
is a strictly scope-limited change (no behaviour / no `contract`-wire / no domain-structure
delta) on the lighter chore DoD — the live verifier pass is skipped, the cold reviewer attesting
the no-change invariant is the safety net. A missing `type:` in an item's frontmatter renders as
`feature` here (the field is new; existing items predate it).

## Items

| ID | Title | Type | Status (main snapshot) | Priority | Depends on | Branch |
| --- | --- | --- | --- | --- | --- | --- |
| [0001](./features/0001-foundational-slice.md) | Foundational vertical slice (auth + profile + minimal TODO) | feature | merged | high | umbrella → 0002, 0003, 0004 | — (merged) |
| [0002](./features/0002-contract-crate.md) | Contract crate + workspace restructure (slice 1 of 0001) | feature | merged | high | — | — (merged) |
| [0003](./features/0003-server-auth-profile-tasks.md) | Server — auth, default profile, tasks, migrations, docker stack (slice 2 of 0001) | feature | merged | high | 0002 | — (merged) |
| [0004](./features/0004-tui-foundational.md) | TUI — register/login, default profile, task add/list/close (slice 3 of 0001) | feature | merged | high | 0003 | — (merged) |
| [0005](./features/0005-tui-responsive-event-loop.md) | TUI — responsive (non-blocking) event loop + `tui::app` submodule reorg | feature | merged | high | 0004 | — (merged) |
| [0006](./features/0006-tui-mainrs-stale-doccomment.md) | Fix stale doc comment in `tui/src/main.rs` | chore | merged | low | — | — (merged) |
| [0007](./features/0007-ok-coverage-verb.md) | Add a reported-only `./ok.sh coverage` verb (cargo-llvm-cov, no threshold) | chore | merged | low | — | — (merged) |
| [0008](./features/0008-pomodoro-timer.md) | Pomodoro focus timer — global duration config + start/stop session | feature | merged | medium | — | — (merged) |
| [0009](./features/0009-coverage-in-cycle-and-summary.md) | Run `./ok.sh coverage` in the drive cycle and record the % in each item's Summary | chore | merged | low | 0007 (merged ✓) | — (main-only governance; no worktree) |
| [0010](./features/0010-notes.md) | Notes — full feature (contract module, migration, server CRUD, TUI views) | feature | merged | medium | — | — (merged) |
| [0011](./features/0011-task-update-delete-reopen.md) | Task update + delete + reopen — generalize close into PATCH (breaking) | feature | ready (branch: awaiting-merge) | medium | — | feature/0011-task-update-delete-reopen |
| [0012](./features/0012-profiles-crud-and-switcher.md) | Profiles create/update/delete + TUI switcher (delete cascades; last-profile guard) | feature | ready | medium | — | — (unclaimed) |

> **0010 — Notes — MERGED.** The final missing
> domain feature shipped end-to-end across all three crates — a near-exact structural clone of the
> task surface governed by [ADR-0007](../docs/adr/0007-notes-wire-contract.md): a `contract` `note`
> module (`Note { id, title, content, created_at }`, no new `ErrorCode`, no `updated_at` — flat
> #3), five profile-scoped server CRUD routes under `/api/profiles/{id}/notes` (ownership-joined →
> `404` never 403, #4; reversible migration `20260612163049_notes` with `ON DELETE CASCADE`), and a
> TUI `Screen::Notes` view opened by `n` (stateless, #1). Tests in all three crates (contract 11,
> server 28, tui `TestBackend` 13). Reviewer **approved** + verifier **verified**, both pinned to
> code-hash `46c1c60f1eb3865eb127a72502982827ebb09d65`; coverage 68.24% line (report-only).
> Operator reviewed and authorized the close; re-freshened onto current `main` (code-hash
> unchanged → verdicts carried forward), fast-forwarded to `main` at `754e876`; worktree + branch
> removed.
>
> **0011 — task update/delete/reopen — AWAITING-MERGE on `feature/0011-task-update-delete-reopen`,
> re-rebased onto post-0010 `main` and re-approved + re-verified** (live status on the branch; the
> `main` snapshot stays frozen at the claim `ready` until the human's ff-merge). The one-way task
> `close` generalized into full edit / toggle-done / reopen / delete — a
> **breaking** change ([ADR-0008](../docs/adr/0008-task-mutation-generalization.md), ref ADR-0005
> §5/§8) that **removes** the `POST .../tasks/{id}/close` route (clean removal, single in-repo
> consumer). A new `contract` `UpdateTaskRequest { title?, description?, status? }` (all-optional
> partial, no `updated_at` — flat #3); `PATCH …/tasks/{id}` via one static `UPDATE … RETURNING`
> (`COALESCE`/`CASE`: done sets `closed_at`, open clears it, empty patch a 200 no-op, blank title →
> 400) + `DELETE …/tasks/{id}` (204 / 404), both ownership-joined → `404` never 403 (#4), **no
> migration**; the TUI gains edit/toggle/delete keys (`e`/`c`/`x`, two-step confirm), stateless (#1).
> **After 0010 merged, 0011 was re-rebased onto post-0010 `main`** — that rebase pulled the merged
> Notes feature into 0011's `crates/` tree, **changing its code-hash**
> `e66426f0…` → `ee5047c9abf1e4196ed1933655a61fcf41148bcb` and voiding the prior verdicts (per
> verdict-pinning, `code-hash` is a whole-`crates/`-tree digest). Both **re-passed** at `ee5047c9…`:
> reviewer **re-approved** (cold re-review confirming the union merge preserves both the Notes and
> task-mutation surfaces), verifier **re-verified** live (the earlier cross-worktree shared-volume
> migration-history conflict is gone — 0011's tree now legitimately carries the `notes` migration).
> Coverage 68.24% line (report-only; now reflects the merged tree). This parallel-feature re-verify
> is recorded as a new CLAUDE.md gotcha (alongside the cross-worktree volume gotcha + a `platform-dev`
> per-worktree-isolation follow-up).
>
> **0012 — READY (planned, unclaimed).** The last domain feature completing organized-koala — Profiles
> create/update/delete + TUI switcher — born on `main` with its governing ADR-0009 (profile mutations).
> 0012's full delete-cascade test wants the `notes` table to exist — now satisfied on `main`
> (0010 merged); `depends-on` was left `[]` because 0012's *code* never depended on 0010, only its
> cascade *test* did. With 0010 merged and 0011 awaiting merge, 0012 is the remaining `ready` item.
>
> **Foundational slice 0001 — CLOSED.** All three children are **merged** on `main`:
> `0002` (contract) → `0003` (server) → `0004` (TUI). The umbrella `0001` is therefore **merged**
> too — its end-to-end acceptance was satisfied collectively at 0004's live verification (full
> reqwest path, ADR-0005 error contract with exact wire strings, profile-scoping, persistence
> across restart, OTel spans; the ADR-0003 layer-2 `TestBackend` suite green). The tracer bullet
> TUI ↔ `contract` ↔ server ↔ Postgres is complete.
>
> **`0005` — MERGED.** The TUI is responsive while a request is in flight (animated spinner +
> Esc-cancel, no UI freeze) and `tui::app` is reorganized into `auth`/`task_add`/`task_list`
> submodules + `protocol.rs`. Governed by
> [ADR-0006](../docs/adr/0006-tui-concurrency-and-responsiveness.md) (**Model A**: synchronous
> `Client` on a worker thread + `std::sync::mpsc` + polled render loop; no async runtime).
> TUI-only — `contract`/`server` unchanged. Reviewer **approved** + verifier **verified** (both
> pinned to code-hash `bc89672d4be5cdecd0bb54b340a24a5b8741cf21`), fast-forwarded to `main` at
> `6f9a80a`; worktree + branch removed.
>
> **`0006` — MERGED.** The inaugural `chore` (new lightweight item type): the
> `tui/src/main.rs` stale-doc-comment fix, now describing the ADR-0006 worker/pure-`App`
> entrypoint. Scope-limited, comment-only — no behaviour/contract/domain change. Ran the
> lighter chore DoD (gates green + a cold `reviewer` **approved** attesting the chore invariant,
> pinned to code-hash `401ad3de59c4cc7e33c3ebf8308c171d80659e4e`; the live verifier pass was
> correctly **skipped**). Fast-forwarded to `main` at `2b400ab`; worktree + branch removed.
>
> **0007 — coverage verb — MERGED.** The 0003 "sanctioned follow-up" is now **consumed**:
> `./ok.sh coverage` runs `cargo llvm-cov --workspace --summary-only` (reusing `cmd_test`'s
> live-DB wiring — throwaway test Postgres booted + torn down on a `RETURN` trap) and appears in
> the no-arg help. **Report-only — no threshold, not a DoD gate**; baseline at implementation ~66%
> line / ~66% function / ~61% region. Tooling-only (no crate source/behaviour/`contract`/domain
> change), so it ran the lighter chore DoD: gates green + a cold `reviewer` **approved** attesting
> the chore invariant, pinned to code-hash `3fa0adefce8cd6d67ae716dae7a24ce6dbf9defd`. Operator
> authorized the close; re-freshened onto current `main` (code-hash unchanged → verdict carried
> forward), fast-forwarded to `main` at `6860b28`; worktree + branch removed.
>
> **0009 — coverage in the cycle + in each Summary — MERGED.** (a `chore`,
> operator-requested; a **`main`-only governance change, no worktree** — already on `main`, so the
> human's close is a status flip, not a branch ff-merge). `drive` step 6 now runs
> `./ok.sh coverage`, parses the headline %, and records it in
> each item's `## Summary` by `awaiting-merge` — for **all** items (feature + chore). **Report-only,
> never a gate** (consistent with 0007); if docker is unavailable the Summary records
> `coverage: unavailable (docker)` and the cycle still completes. **No ADR** (DoD-wording
> refinement only). Three governance edits to home-#1 shared state — `.claude/skills/drive` step 6,
> `CLAUDE.md` Definition of done, `.claude/agents/eng-manager` charter — applied directly on `main`
> by `eng-manager` (commit `6b6e373`). Ran the lighter chore DoD: gates green + cold `reviewer`
> **approved** with the chore invariant attested, pinned to code-hash
> `3fa0adefce8cd6d67ae716dae7a24ce6dbf9defd`; live verifier **skipped** (chore track). Dogfoods its
> own rule — 0009's `## Summary` is the **first to carry a coverage line** (66.36% line / 61.48%
> region / 66.67% function). Depended on **0007** (the `coverage` verb), which merged first.
>
> **0008 — Pomodoro timer — MERGED.** The first Focus-phase
> feature, implementing [ADR-0002](../docs/adr/0002-pomodoro-timer-authority.md) (timer authority)
> with no new/amended ADR on the contract/domain surface. A new `contract` `timer` module
> (`TimerConfig`, `UpdateTimerConfigRequest`, the tagged `TimerSession` carrying `ends_at` +
> `server_now`), five **account-global** `/api/timer/...` server endpoints keyed on `user_id`
> (config get/update, session get/start/stop) + a reversible migration creating `timer_configs` +
> `timer_sessions` (`ends_at` derived, not stored), and a TUI presentation whose live `MM:SS`
> countdown is **render-only** — recomputed each ~80 ms draw from the server's absolute `ends_at`,
> `server_now`, and a monotonic `Instant`, never a stored counter (#1-safe; inside ADR-0006, no
> per-second polling). Account-global (#4 / ADR-0002 §5), flat (#3, duration the only knob).
> **0008-R1 feedback re-entry (TUI-only, governed by the
> [ADR-0006](../docs/adr/0006-tui-concurrency-and-responsiveness.md) §8 amendment — authority/render
> model still ADR-0002):** the timer became an **always-visible bottom-right global widget** on
> every post-auth screen (no dedicated page), toggled by a global **`p`** (start/stop) listed in the
> bottom-left help caption; the in-flight indicator now **appends a trailing spinner** to the stable
> caption instead of replacing it (flicker fix), and the coarse session refresh loosened ~5 s →
> ~1 min — **no `contract`/server/migration change** (reviewer + verifier confirmed the wire surface
> byte-identical). Reviewer **approved** and verifier **verified** at the 0008-R1 end state, both
> pinned to code-hash `3fa0adefce8cd6d67ae716dae7a24ce6dbf9defd` (the original 0008 build was
> approved + verified at `708ee8d0085ce9b3af68eb7e1b76dbe56a6185da`, voided when the re-entry moved
> the tree). Fast-forwarded to `main` at `c32f0ad` (linear, no merge commit); worktree + branch
> removed.
