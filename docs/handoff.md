# Handoff — engineering journal

Reverse-chronological. `eng-manager` appends one entry per completed cycle at the **top** and
keeps the "What works right now" snapshot at the bottom current.

---

## Handoff — 2026-06-24 (0010 — Notes, the final domain feature, end-to-end across all three crates)

The **last missing domain feature** shipped: Notes, a near-exact structural clone of the task
surface, governed by [ADR-0007][adr-0007] (notes wire contract — already on `main` with the plan).
Branch: `feature/0010-notes` (code sha `2a4074d` at verification; current branch HEAD after this
eng-manager step). Reviewer **approved** and verifier **verified**, both pinned to code-hash
`46c1c60f1eb3865eb127a72502982827ebb09d65` (re-confirmed equal at this step — verdicts carry
forward, no relabelling). The cycle stopped at the AI-terminal `awaiting-merge` on the branch.

What shipped (on the branch, contract → server → tui per the slice order):

- **`contract`** — a new `note` module: `Note { id, title, content, created_at }`,
  `CreateNoteRequest { title, content }`, `UpdateNoteRequest { title, content }`, reusing the
  `{ code?, message }` error contract with **no** new `ErrorCode`. Flat (#3), no `updated_at`
  (editing mutates in place; only `created_at` is a timestamp, operator-locked).
- **`server`** — five CRUD routes under `/api/profiles/{id}/notes` (create 201 / list 200 bare
  array newest-first / get 200 / update 200 in-place / delete 204), every query ownership-joined
  so an unowned or missing profile/note id is `404 not_found` (never 403, #4 / ADR-0005 §4
  non-observability). Reversible migration `20260612163049_notes` (paired up/down; `ON DELETE
  CASCADE`, `(profile_id, created_at DESC)` index) + a `.sqlx/` refresh.
- **`tui`** — five `Client` trait methods + `HttpClient` impls, `ClientRequest`/`Outcome` variants
  (carrying `token` + `profile_id`) + worker arms, and a `Screen::Notes` view (list +
  create/edit/delete sub-flows) opened by `n` from the task list. Stateless (#1); no `chrono` in
  `tui` (A8 — timestamp formatting at the render seam).
- **`fix(tui)`** — a caption-layout regression the TUI suite surfaced (see learning below): adding
  `n: notes` grew `TASK_LIST_CAPTION` so the pending caption + spinner clipped the cancel
  affordance at 80×24 (ADR-0006 §8.3); the bottom band was widened to 3 rows and both captions
  re-phrased with ` | ` separators, no assertions weakened.

Tests in all three crates: `contract` note DTOs 11 (+ doctests), `server` notes integration 28
(incl. profile-scoping + auth-required per route), `tui` `TestBackend` notes suite 13 (+
rendering 11). All four gates green at branch head (`build | test | lint --all-targets |
fmt --check`).

Verdicts (both @ code-hash `46c1c60f1eb3865eb127a72502982827ebb09d65`):

- **reviewer — REVIEW-STATUS: approved.** Hard constraints clear (#1 stateless, #2 DTOs only in
  `contract`, #3 flat no-`updated_at`, #4 every query ownership-joined → 404 never 403); no new
  `ErrorCode`; migration up/down paired + cascade; the caption `fix(tui)` in-scope (ADR-0006 §8.3).
- **verifier — VERIFY-STATUS: verified.** Booted the real stack (`./ok.sh up`, docker 29.5.3);
  migration applied; flat schema confirmed (`id,profile_id,title,content,created_at`, no
  `updated_at`); the full wire surface exercised live (shapes, status codes, `{code,message}`
  contract, profile-scoping → 404, all five OTel handler spans). One stated inference: the reqwest
  `HttpClient` path verified by structural equivalence (curl drove the wire; the `tui` Client maps
  one-for-one + the 13-test suite drives the trait), not a literal live reqwest harness — not a
  coverage gap.

coverage: **68.24% line** (62.99% region, 70.77% function), the headline `TOTAL` from a fresh
`./ok.sh coverage` run in the worktree (docker + throwaway test Postgres booted cleanly — nothing
acquired, #6 intact). Up from the 0009 snapshot (66.36% line) — the notes server handlers and TUI
view land well-tested (`handlers/notes.rs` 100% line, `app/notes.rs` 90%). **Report-only — no
threshold, never a gate.**

Durable learning recorded (one, in the `tui-dev` agent): **caption width and bottom-band height
are coupled at the 80×24 test viewport.** This bit on 0008-R1 (the append-spinner work) and **again
on 0010** (adding the `n: notes` hotkey), so it earned a durable agent note rather than staying a
per-cycle surprise: growing a fixed-width caption can wrap the stable caption + appended spinner +
cancel affordance an extra line and clip it — a render regression the `TestBackend` suite catches,
not the compiler. The fix is always to budget the band row count (and pick ` | ` wrap points) in
the *same* change that grows the caption; the invariant is owned by ADR-0006 §8.3, and the
rendering code already carries inline comments naming the 80×24 boundary. No CLAUDE.md hard
constraint earned (this is a TUI-layout discipline, not a cross-cutting domain rule), no
standards-skill edit, **no new ADR** (ADR-0007 governs the contract and was already on `main`),
**no new crate → no new dev agent**.

**Homes.** Feature-local on the branch (home #2): the item's `## Summary` (with the coverage line)
and this `[eng-manager]` Log context, committed on `feature/0010-notes`. Cross-cutting/derived on
`main` (homes #1/#3): this `docs/handoff.md` entry (+ the "What works right now" snapshot refreshed
for the 0010 state), the `tui-dev` caption/band learning, and the regenerated `board/README.md`.
**`main`'s frozen copy of `board/features/0010-notes.md` stays untouched** at the claim snapshot
(`ready`) until the human's merge. The orchestrator flips the branch status to `awaiting-merge`
after this step.

**Free pickup noted (mintable `chore`):** none this cycle.

[adr-0007]: ./adr/0007-notes-wire-contract.md

---

## Handoff — 2026-06-24 (0009 — coverage capture wired into the cycle + each Summary; chore, `main`-only)

The operator's process request — *"add the coverage run in the process, and report the code
coverage percentage in the summary of the tasks when they are awaiting merge"* — shipped as a
`main`-only governance `chore` (commit `6b6e373`, code-hash
`3fa0adefce8cd6d67ae716dae7a24ce6dbf9defd`). **No worktree was cut**: every edited file is home-#1
shared state (drive SKILL, CLAUDE.md, the eng-manager agent def) that must never ride a feature
branch, so `eng-manager` applied the edits directly on `main` and the orchestrator advanced status
in place. It ran the **lighter chore DoD** — gates green + a cold reviewer approval **attesting the
chore invariant** (code-path digest byte-identical to pre-0009 `cef68fe` ⇒ zero code delta) — and
the **live verifier pass was correctly skipped** (chore clause 4 N/A).

What shipped (three governance edits, all on `main`):

- **`drive` SKILL step 6** — step 6 now runs `./ok.sh coverage`, parses the headline workspace
  coverage %, and writes a `coverage: NN.N%` line into the item's `## Summary` (or
  `coverage: unavailable (docker)` when docker / the throwaway test Postgres cannot boot). Runs on
  **every** cycle (feature and chore); **report-only — never a gate**.
- **`CLAUDE.md` Definition of done** — a short gate-neutral note: the Summary records the coverage
  % for both `feature` and `chore`, for visibility only — not a clause, no threshold, never
  blocking; docker-unavailable becomes `unavailable (docker)` and the cycle proceeds. Consistent
  with the "How to run" `coverage` row.
- **`.claude/agents/eng-manager.md` charter** — the Summary-filling bullet now explicitly includes
  the coverage capture + `unavailable (docker)` fallback (report-only).

**This cycle dogfoods the very feature 0009 introduces.** 0009's own `## Summary` is the **first
item Summary to carry a coverage line**: `coverage: 66.36% line (61.48% region, 66.67% function)`,
the headline `TOTAL` from a fresh `./ok.sh coverage` run (docker + throwaway test Postgres booted
cleanly, same as `./ok.sh test` this cycle — nothing acquired, hard constraint #6 intact). Matches
the ~66% line / ~61% region 0007 baseline; report-only, no target to hit.

**0009 depended on 0007** (the `./ok.sh coverage` verb), which **merged first** — 0009 consumes
that verb and could not start until it landed on `main` (`grep -c cmd_coverage ok.sh` == 2).

Durable learnings:

- **`drive` SKILL + `git-standards` (reinforcing learned 0003/0004).** The
  `noreply@anthropic.com`-in-a-dispatch-prompt failure surfaced **again** on 0009: the dispatch
  prompt hardcoded `Co-authored-by: … <noreply@anthropic.com>`, corrected to the
  `*@organized-koala.local` form per git-standards (the footer identity is owned by that skill,
  never copied from a prompt; `<noreply@anthropic.com>` is never correct in this repo). Because
  this is now a **third recurrence**, the durable fix moved to the *dispatcher* side: a new
  **"Dispatch discipline"** note in `drive`'s Procedure preamble — never write a `Co-authored-by:`
  line into a dispatch prompt; state the committing agent's role and let `git-standards` supply the
  trailer — plus a cross-referencing one-liner appended to `git-standards`. The agent-side rule was
  already correct; the gap was that prompts kept injecting the wrong trailer, so the prevention
  belongs where the prompt is authored.
- **No new ADR, no new crate, no new dev agent, no new CLAUDE.md hard-constraint.** A chore makes no
  contract/domain decision; the coverage metric is already operator-sanctioned (0007) and stays
  report-only. The three-home model, chore DoD, scope guard, and verdict-pinning all behaved as
  written.

Process note worth keeping (not an edit): a `main`-only governance chore has **no worktree and no
branch**, so step 6's "coverage line is committed on the branch (home #2)" guidance resolves to
"on `main`" for this item — the Summary lives on `main` alongside the rest of the change. The
SKILL/CLAUDE.md wording already states this explicitly, so it needed no correction; recorded here
as the worked example of the `main`-only path through the new step-6 rule.

**Homes.** Everything is on `main` (this is a `main`-only item): the three governance edits
(`6b6e373`), this `docs/handoff.md` entry (+ the "What works right now" snapshot refreshed for the
0009 state), the `drive`/`git-standards` dispatch-discipline edits, the item's `## Summary`
(home 1 for a `main`-only item — there is no branch), and the regenerated `board/README.md` (home 3).
`branch: null` / `worktree: null` stay. The orchestrator flips 0009 to `awaiting-merge` in place on
`main` after this step.

**Free pickup noted (mintable `chore`):** none this cycle.

---

## Handoff — 2026-06-23 (0007 — report-only `./ok.sh coverage` verb; chore, lighter DoD)

The operator-sanctioned coverage follow-up (captured in the 2026-06-12 0003 handoff, item #2,
and carried on the dashboard as the "sanctioned follow-up" note) shipped as a `chore`. Branch:
`feature/0007-ok-coverage-verb` (code sha `e65a097`, code-hash
`3fa0adefce8cd6d67ae716dae7a24ce6dbf9defd`). It ran the **lighter chore DoD** — gates green + a
cold reviewer approval attesting the chore invariant — and the **live `verifier` pass was
correctly skipped** (a chore changes no behaviour/wire/API, so there is nothing for a live boot
to exercise). The cycle stopped at the AI-terminal `awaiting-merge` on the branch.

What shipped (`ok.sh` only, on the branch):

- **A `coverage` verb** — `cmd_coverage` + a `coverage)` case branch + a no-arg usage/help line.
  It runs `cargo llvm-cov --workspace --summary-only "$@"` (extra ARGS pass through) and
  **mirrors `cmd_test`'s live-DB wiring verbatim**: honour a caller-supplied `DATABASE_URL`, else
  boot the throwaway test Postgres via the test compose file and tear it down on a `RETURN` trap.
- **Report-only, no gate.** Prints a per-file table + a `TOTAL` line and exits 0 regardless of
  the number; **no threshold**, not wired into any Definition-of-done clause. This was the
  operator-sanctioned shape: coverage made *visible* without becoming a brittle pass/fail bar.
- **Coverage baseline at implementation time:** ~66% line / ~66% function / ~61% region
  (`TOTAL` line reported 61.48% region / 66.36% line). Captured here as a reference point, not a
  bar — there is no target to hit.
- **Chore invariant held.** No crate source, no behaviour, no `contract`/wire (#2), no
  domain-structure (#3) change — the diff is `ok.sh` (+31) plus the Board file. `cargo-llvm-cov`
  0.8.7 was already present and operator-sanctioned (hard constraint #6) — nothing acquired.

Verdict (chore track): **reviewer REVIEW-STATUS approved** @ code-hash
`3fa0adefce8cd6d67ae716dae7a24ce6dbf9defd` (commit `c4387b7`, for reference). Gates green
(`fmt --check` / `lint` / `test`); the **chore invariant is explicitly attested** (no behaviour,
no `contract`/wire, no domain-structure change); the verb is report-only. The code-hash is
byte-identical to the last-merged head, corroborating the tooling-only scope. No live verifier
pass (chore clause 4 skipped).

Durable learning: one small `bash-standards` addition (learned 0007) — *a report-only tooling
verb reuses the shared live-DB wiring (the `cmd_test` `DATABASE_URL`/compose/`RETURN`-trap
pattern) rather than re-deriving it, and stays honest by exiting 0 regardless of the metric; a
verb that can fail the build on a value is a gate, not a report.* No new ADR (a chore makes no
contract/domain decision), no new crate → no new dev agent, no `CLAUDE.md` hard-constraint
addition beyond documenting the verb in the "How to run" table.

**Homes.** Cross-cutting/derived on `main` (homes #1/#3): the `CLAUDE.md` "How to run" `coverage`
row, this `docs/handoff.md` entry (+ the "What works right now" snapshot refreshed for the 0007
state), the `bash-standards` learning, and the regenerated `board/README.md`. **Feature-local on
the branch (home #2):** only the item's `## Summary` (and its Log entries/verdict, already
committed on the branch). `main`'s frozen copy of `board/features/0007-ok-coverage-verb.md` stays
untouched at the claim snapshot (`ready`) until the human's merge.

**Free pickup noted (mintable `chore`):** none this cycle.

---

## Handoff — 2026-06-23 (0008-R1 — feedback re-entry: Pomodoro becomes a global widget; TUI-only)

**Feedback re-entry, not a fresh feature — the first re-entry on an item that had already reached
`awaiting-merge`.** 0008 (the account-global Pomodoro timer) was at `awaiting-merge` (verified
code-hash `708ee8d0…`) when the operator authored two `[human]` UI-feedback lines in its Log. The
cycle re-entered (drive step 0 feedback sweep), `architect` triaged, the work ran forward TUI-only,
and the item is back at the AI-terminal state on its branch. Branch: `feature/0008-pomodoro-timer`
(source `97b2b32`, tests `67e40af`; current HEAD `7ea1292` after this eng-manager step). Both
`[human]` boxes are now `[x]`.

The two feedback items:

- **suggestion(ui) — no dedicated timer page; make it an always-visible global widget.** The timer
  is a global concept, so it should be visible across pages (bottom-right), `p` to start/stop, and
  listed in the bottom-left help caption.
- **issue(ui) — flicker + over-frequent refresh.** The "(working…)" text replacing the hotkey
  caption every coarse poll causes flicker; append a spinner to the end instead, and check the
  session ~1/min rather than ~5 s.

What changed (TUI-only, on the branch):

- **ADR amendment first, on `main`.** Because this is scope/approach feedback, `architect` amended
  **[ADR-0006][adr-0006] §8** (commit `af582e6` on `main`) before re-implementation — §8.1 global
  widget (not a `Screen`), §8.2 global `p` toggle + help-caption entry, §8.3 append-spinner (not
  caption-replacement), §8.4 ~1-min coarse cadence. **ADR-0002 (timer authority/render model) is
  unchanged** — the server still owns the countdown; the TUI still renders from `ends_at` +
  `server_now` + a monotonic `Instant`. The branch was rebased onto `af582e6` before `tui-dev`
  cited §8.
- **Source (`tui-dev`, `97b2b32`).** Removed `Screen::Timer` and its `t`/`Esc` navigation; promoted
  the timer's transient render state to an app-level `app::timer::Timer` field rendered bottom-right
  on every post-auth screen (auth/offline excluded). Added `Event::ToggleTimer` mapped to `p`
  (resolves to start when idle/completed, stop when running, stamping the timer's own in-flight
  marker independent of the screen marker); `p` + `d: set duration` added to the bottom-left
  caption; `p`/`d` suppressed while a text-entry sub-flow owns keystrokes. Replaced `working_hint`
  (caption substitution) with `caption_with_spinner` that **appends** a trailing spinner + "Esc to
  cancel" to the stable caption on every screen. Raised `TIMER_REFRESH_TICKS` 63 → **750** (~1 min);
  the refresh + initial load now fire on any post-auth screen. **No `contract`/protocol/client/
  worker shape changed** — the existing timer wire/protocol is reused verbatim; account-global
  preserved (no `profile_id`).
- **Tests (`tester`, `67e40af`).** Adapted the `TestBackend`/core suite to the global-widget model
  (`map_key` now takes `editing_duration: bool`; the timer loads off edge hooks, not an `Event`) and
  extended coverage to the re-entry acceptance criteria by name: global widget render, `p`
  start/stop/when-completed, second-`p`-while-pending no-op, append-spinner-no-flicker regression
  guard, `p` suppressed-while-editing, `t`-opens-nothing regression guard, coarse-refresh picks up
  the server verdict, account-global call-shape sweep. Counts: tui keybindings 19 / rendering 11 /
  timer 17 / flows 9 / in_flight 5 / error_branches 10; full workspace green.

Verdicts (both pinned to code-hash `3fa0adefce8cd6d67ae716dae7a24ce6dbf9defd`; the original 0008
`708ee8d0…` verdicts were **voided** when the re-entry moved the code-tree):

- **Reviewer: REVIEW-STATUS approved** (`67e40af`). Gates green; **#1** holds (app-level `Timer` is
  transient render state, no stored remaining-seconds); **#2** holds and is **byte-identical** —
  `git diff` over `crates/contract` + `crates/server` + the `tui` protocol/client is empty (this
  bounds the verifier to the TUI surface); **#4** holds (no `profile_id`); ADR-0006 §8 fidelity
  confirmed. No blocking findings, no out-of-scope nits worth a chore.
- **Verifier: VERIFY-STATUS verified** (`09470e9`). Independently confirmed the #2 byte-identity
  (full delta confined to `crates/tui/src/{app,terminal,ui}` + `crates/tui/tests/**` + the Board
  file). Docker present + sanctioned (installed nothing, as in the original pass), so the live wire
  pass was re-performed, not deferred: `./ok.sh up`, live `GET/PUT /api/timer/config`, session
  start/stop, error contract `{code,message}`, OTel spans on all five handlers; `./ok.sh down`
  clean. The `TestBackend` suite asserts the re-entry behaviour by name.

Durable learnings: **none earned a durable `CLAUDE.md`/standards-skill edit.** The candidate was
this being **the first feedback re-entry on an already-`awaiting-merge` item** — but the mechanics
played out *exactly* as the existing CLAUDE.md "Feedback re-entry" + "Verdict pinning" + three-home
text already prescribes, with no ambiguity to resolve: the unchecked `[human]` box was the only
re-entry signal; the scope/approach feedback required an ADR amendment, which (as home #1
cross-cutting state) landed on `main` first; the branch was rebased onto it; the item dropped
`awaiting-merge` → `working`; the prior `approved`/`verified` verdicts were void **because the
code-tree hash moved** (`708ee8d0…` → `3fa0adef…`, not because shas changed); and the full feature
track re-ran (build → review → verify) on the new tree. That is the written rule exercised
faithfully, not a gap in it — so it is recorded here as the worked example rather than manufactured
into a new gotcha. One observation worth keeping (not an edit): **an ADR *amendment* is home #1
just like a new ADR** — it must land on `main` before the branch can cite it, and the code-hash
movement it implies is what voids the prior verdicts; the re-entry confirmed both halves hold for
an amendment, not only a fresh ADR. No new `docs-/bash-/coding-/git-standards` edit, no new ADR
beyond the §8 amendment, no new crate → no new dev agent.

**Homes.** Cross-cutting/derived on `main` (homes #1/#3): this `docs/handoff.md` entry (+ the "What
works right now" snapshot refreshed for the 0008 end state), and the regenerated `board/README.md`.
The ADR-0006 §8 amendment + decisions-index row already landed on `main` (`af582e6`, by
`architect`). **Feature-local on the branch (home #2):** the item's updated `## Summary`, the
`[eng-manager]` Log entry, and the two `[x]`-checked `[human]` feedback boxes. The orchestrator
flips the branch status back to `awaiting-merge` after this; **`main`'s frozen copy of
`board/features/0008-pomodoro-timer.md` stays untouched** at the claim snapshot until the human's
merge.

---

## Handoff — 2026-06-23 (0008 — account-global Pomodoro focus timer, end-to-end across all three crates)

Branch: `feature/0008-pomodoro-timer` (last code sha `fc894ce`, code-hash
`708ee8d0085ce9b3af68eb7e1b76dbe56a6185da`). The **first feature of the Focus phase** — the
Pomodoro timer end-to-end, implementing [ADR-0002][adr-0002] (timer authority) without reopening
or amending it. The cycle ran build → cold review → live verify and stopped at the AI-terminal
`awaiting-merge` on the branch.

What shipped (on the branch):

- **`contract` — a new `timer` module.** `TimerConfig { duration_minutes }`,
  `UpdateTimerConfigRequest { duration_minutes }`, and a tagged `TimerSession` enum
  (`#[serde(tag = "state", rename_all = "lowercase")]`) with `Idle` / `Running` / `Completed`;
  the running/completed variants carry `started_at`, `ends_at`, `duration_minutes`, and
  `server_now`. Datetimes serialize RFC 3339 `Z` exactly as `Task::created_at`; the established
  derive/rustdoc/doctest layout is followed; the three items are re-exported from `lib.rs`. No new
  `ErrorCode`, no secrets, nothing beyond the ADR-0002 shapes (#3 flat).
- **`server` — five account-global endpoints + a reversible migration.** All keyed on
  `AuthUser.user_id` with **no `profile_id` in any path** (#4 / ADR-0002 §5): `GET`/`PUT
  /api/timer/config` (default 30 lazily, upsert, `[1, 1440]` bound → `400 ValidationFailed`
  outside, reusing the `{ code?, message }` contract, no new `ErrorCode`); `GET
  /api/timer/session` (idle / running / completed, completion decided read-time when `server_now
  >= ends_at`); `POST /api/timer/session/start` (snapshots the configured duration;
  start-while-active replaces — A5); `POST /api/timer/session/stop` (clears the active row,
  idempotent when idle). Migration `20260612163048_timer.{up,down}.sql` creates `timer_configs` +
  `timer_sessions`, both `user_id UUID PRIMARY KEY` (schema-enforced at-most-one config / one
  active session per account); `ends_at` is **derived** (`started_at + duration_minutes`), never
  stored; the `down` drops both tables. `#[tracing::instrument]` spans on every handler;
  `i32`↔`u32` at the DB boundary via `try_from`, never `as`. `.sqlx/` refreshed against the
  sanctioned project test Postgres.
- **`tui` — a focus/timer view with a render-only countdown.** `Screen::Timer`, reachable with
  `t` from the task list (`s` start, `x` stop, `d` set duration, `r` refresh, `Esc` back). The
  live `MM:SS` countdown is **render-only** (#1-safe): **no** authoritative remaining-seconds
  integer is stored — the label is recomputed every ~80 ms render tick as `ends_at` minus
  `(server_now + elapsed_since_response)`, where `elapsed_since_response` comes from a monotonic
  `Instant` captured when the response landed. Coarse session re-reads are ~5 s (A3) — never
  per-second, no tick stream (stays inside [ADR-0006][adr-0006]). On reaching `00:00` locally it
  shows "Completed (awaiting server confirmation)" until the server's authoritative `Completed`
  verdict arrives.
- **Tests (tester).** `contract` 19 (round-trip, tagged-enum wire shape, `Z` offsets), server 21
  `#[sqlx::test]` (config default/persist/bounds, start→running with consistent instants, stop,
  start-replaces-active, account-global with two accounts, auth-required), tui 14 `TestBackend` +
  5 keybinding (navigation, running countdown rendered via `countdown_label`, stop, set-duration +
  inline validation, completed render, in-flight spinner, cancel/stale-id drop, account-global /
  profile-switch-unchanged). The one positive completion-at-`ends_at` transition is deliberately
  left to the live verifier (forcing `now >= ends_at` would need a real ~60 s sleep the suite
  avoids — noted inline in `shortest_session_reads_running_not_completed`).

Verdicts (both pinned to code-hash `708ee8d0085ce9b3af68eb7e1b76dbe56a6185da`, sha `fc894ce`):

- **Reviewer: REVIEW-STATUS approved.** Gates green (contract 19 / server 21 / tui 14 + 5
  keybinding, lint `--all-targets` clean, fmt clean). Risk-surface all HOLD: **#1** stateless
  (countdown recomputed each draw, nothing persisted); **#4 / ADR-0002 §5** account-global (the
  routes and client methods key on `user_id`, tables `user_id PRIMARY KEY`); **#3** flat
  (duration the only knob, no pause); **#2 / ADR-0002** contract is single source of truth, no
  new/amended ADR; reversible migration with `ends_at` derived; `{ code?, message }` reused, no new
  `ErrorCode`; no `as` at the DB boundary; spans on all five handlers; the three `#[allow]` are the
  sanctioned test-only exception.
- **Verifier: VERIFY-STATUS verified.** Live against `./ok.sh up` (docker present, migrate
  one-shot exited 0, both tables created). **Completion DIRECTLY OBSERVED** (not inferred): a
  1-min session polled every 5 s flipped running→`completed` when `server_now >= ends_at`; the row
  was kept (`count=1`, re-read still `completed`) until `stop` (`count=0`, idle). **Persistence
  across `docker compose restart server`**: config + running session survived (only `server_now`
  advanced) → state lives in Postgres. Account-global (no `profile_id`; second account
  independent), auth (no-bearer → `401 unauthenticated`), and OTel spans for all five handlers
  with `code.namespace: server::handlers::timer` + the `user_id` attribute. ADR-0003 handshake:
  the `TestBackend` suite present + green. Stack torn down cleanly.

Durable learnings captured this cycle: **none earned a durable `CLAUDE.md`/standards-skill
edit.** The candidate considered was the **render-only countdown pattern** — a #1-safe
live-updating-but-server-authoritative value computed each draw from a server-provided absolute
instant (`ends_at` + `server_now`) plus a monotonic `Instant`, never stored as a counter. It is a
clean, reusable idiom, but it does **not** generalize beyond what is already written: it is a
direct specialization of [ADR-0006][adr-0006] §5 (transient process-lifetime render state, the
same category as the in-flight spinner marker), the #1 statelessness invariant, and the
pure-core/effectful-shell rule already in `rust-standards` (learned 0004/0005). Manufacturing a
new skill entry would duplicate those, so the pattern is recorded **here** as the worked example
rather than promoted into a standard. No new `CLAUDE.md` gotcha (no recurring miss surfaced — the
three-home model, contract-frozen boundary #2, statelessness #1, and account-global #4 all held
cleanly), no `docs-/bash-/coding-/git-standards` edit, no new ADR (inside ADR-0002/0003/0006).

**No new crate** → no new dev agent: the timer is a module *inside* the existing crates
(`crates/contract/src/timer/`, `crates/server/src/handlers/timer.rs`, `crates/tui/src/app/timer.rs`),
each already owned by `contract-owner` / `server-dev` / `tui-dev`. Confirmed, not skipped.

**Free pickup spotted (mintable `chore` for a future cycle):** the `tui` timer-edit sub-flow
mirrors the existing `AddTaskState` text-entry pattern closely enough that the two could share a
small `TextEntryState` helper — a pure refactor with no behaviour / contract / domain change. Not
filed here (recorded so the orchestrator can mint it directly if desired); low priority.

**Homes.** Cross-cutting/derived on `main` (homes #1/#3): this `docs/handoff.md` entry (+ the
"What works right now" snapshot refreshed), `docs/build-plan.md`, and the regenerated
`board/README.md`. **Feature-local on the branch (home #2):** the item's `## Summary` + the
`[eng-manager]` Log entry. The orchestrator advances the branch status to `awaiting-merge` after
this; **`main`'s frozen copy of `board/features/0008-pomodoro-timer.md` is left untouched** at the
claim snapshot until the human's merge.

[adr-0002]: ./adr/0002-pomodoro-timer-authority.md

---

## Handoff — 2026-06-23 (0006 — inaugural `chore`: stale `tui/src/main.rs` doc comment fixed)

Branch: `feature/0006-tui-mainrs-stale-doccomment` (last code sha `e218f73`, code-hash
`401ad3de59c4cc7e33c3ebf8308c171d80659e4e`). **The first `chore` through the pipeline** — the
new lightweight item type (introduced as a learned-0005 governance follow-up) made its first
real trip end-to-end. The cycle ran mint → claim → build → cold review → (verify skipped) and
stopped at the AI-terminal `awaiting-merge` on the branch.

What shipped (on the branch):

- **Comment-only fix.** The module doc comment at `crates/tui/src/main.rs:1` described an
  *"initial health probe so an unreachable server is reported up front"* — behaviour 0005
  removed when it reshaped the entrypoint to ADR-0006 Model A. The comment was rewritten to
  describe the actual entrypoint: resolve base URL → build the `reqwest` client → **spawn the
  worker thread that owns it** → hand control to the interactive loop, where the UI thread
  drives the pure `tui::app::App` core and never blocks on I/O. The `anyhow`
  error-propagation note was kept. The diff vs `main` is the `//!` block only.
- **Chore invariant held.** No code path, signature, behaviour, `contract`/wire (#2), or
  domain-structure (#3) change.

Verdict (chore track):

- **Reviewer: REVIEW-STATUS approved** pinned to code-hash
  `401ad3de59c4cc7e33c3ebf8308c171d80659e4e` (sha `5b5c788`). The cold pass verified the new
  comment line-by-line against `main()` (no health probe; worker-spawn / pure-`App` /
  `terminal::run` / `anyhow`), gates green, and — as the strengthened chore-DoD clause 6
  requires — **explicitly attested the chore invariant** (no behaviour, no contract/wire, no
  domain-structure change; comment-only).
- **Verifier: SKIPPED (clause 4 N/A).** Per the chore track, the live boot was not run — a
  comment-only change has nothing new to exercise, and the cold reviewer is the safety net in
  its place.

The chore lane worked exactly as designed: mint-without-`architect`-plan → claim →
single-agent build → invariant-attesting cold review → live verify skipped → `awaiting-merge`.
The 0005 handoff's **"free pickup" prose is now resolved** — it was tracked as `0006` and has
flowed to terminal.

Durable learnings: **none earned a durable edit.** The chore machinery (DoD, scope guard,
three-home model, verdict pinning) was freshly exercised and behaved as written — no clause
was ambiguous, the mint-without-plan path was unambiguous, and verdict pinning to the
code-tree hash held (the branch was already current on `main`, code-hash unchanged, so
step-7 was a no-op freshen). No `CLAUDE.md` gotcha, no standards-skill edit, no agent edit,
no new ADR, no new crate. Recording explicitly rather than inventing churn: **the first chore
needed zero process correction.** One observation worth keeping (not an edit): the chore lane's
value is precisely that a one-line doc fix no longer has to masquerade as a full `feature`
cycle — the thing it was created to fix.

**Homes.** Cross-cutting/derived on `main` (homes #1/#3): this `docs/handoff.md` entry (+ the
"What works right now" snapshot refreshed), `docs/build-plan.md`, and the regenerated
`board/README.md`. **Feature-local on the branch (home #2):** the item's `## Summary`. The
orchestrator advances the branch status to `awaiting-merge` after this; **`main`'s frozen copy
of `board/features/0006-tui-mainrs-stale-doccomment.md` is left untouched** at the claim
snapshot until the human's merge.

---

## Handoff — 2026-06-22 (0005 — TUI responsive (non-blocking) event loop + `tui::app` reorg)

Branch: `feature/0005-tui-responsive-event-loop` (last code sha `a4f99fd`, code-hash
`bc89672d4be5cdecd0bb54b340a24a5b8741cf21`). The first item past the foundational slice: it
resolves 0004's re-homed responsiveness feedback (*"the TUI freezes during every HTTP
request"*) and folds in the requested `tui::app` submodule reorg (both restructure the same
module). The cycle ran build → review → verify and stopped at the AI-terminal `awaiting-merge`
on the branch. **TUI-only — `crates/contract` and `crates/server` are byte-identical to base
`f0204fd`; no wire change, no new ADR beyond 0006.**

What shipped (on the branch):

- **Responsive UI per [ADR-0006][adr-0006] Model A** — synchronous `Client` on a worker thread,
  `std::sync::mpsc` request/response, a polled render loop, **no `tokio`/async runtime**. The UI
  thread never blocks on IO; a spinner animates and Esc(cancel)/Ctrl+C,`q`(quit) stay live in
  flight. `client/worker.rs` is a single thread owning the real `HttpClient`, mapping a
  `ClientRequest` → `Outcome` over two `mpsc` channels (no new dep). `terminal::run` is now a
  poll loop: `event::poll(80ms)` for input + `try_recv` response drain + per-tick redraw. A 30s
  `reqwest` timeout bounds an abandoned request (the `Client` trait is unchanged). `main.rs`
  spawns the worker and passes the channels in.
- **Client-free pure core.** The `App<C>` generic is gone. The core is two pure seams:
  `handle_event(Event) -> Option<Dispatch>` and `apply_response(ClientResponse) ->
  Option<Dispatch>` (chaining follow-ups — post-auth profile→task load, post-create refresh).
  Error-code branching is preserved unchanged and routes async-arriving responses through the
  same handlers.
- **One-in-flight + cancel.** Each screen carries a transient `pending: Option<RequestId>`;
  while set, request-triggering events are no-ops, `Cancel`/`Quit` stay live. Cancel is
  user-perceived — the screen leaves the in-flight state at once and a superseded response is
  dropped by `RequestId`-mismatch in `apply_response`; the worker is not force-killed.
- **`tui::app` reorg.** `app/mod.rs` keeps `App`/`Screen`/`Session`/`Event` + the
  `handle_event`/`apply_response` wiring; `app/protocol.rs` holds the pure
  `ClientRequest`/`ClientResponse`/`Outcome`/`RequestId`/`Dispatch` types; feature submodules
  `auth.rs`/`task_add.rs`/`task_list.rs` each own their screen state and handlers.
- **Tests (tester).** Added a synchronous request executor to `tests/common/mod.rs`
  (`execute`/`drive`/`submit`) — the test-side analogue of the worker thread: it maps a
  `Dispatch`'s `ClientRequest` through the `FakeClient` (the sole external-service mock) to a
  `ClientResponse` and feeds it back into `apply_response`, looping on chained follow-ups until
  the flow settles. No internal collaborator is mocked. The `TestBackend` suite (ADR-0003 layer
  2) is green and extended for in-flight render/no-op, cancel + stale-`RequestId` drop,
  at-most-one-chained-request, and Esc→Cancel/Ctrl+C→Quit while pending (tui: flows 9,
  error_branches 10, in_flight 5, keybindings 13, rendering 11).

Verdicts (both pinned to code-hash `bc89672d4be5cdecd0bb54b340a24a5b8741cf21`):

- **Reviewer: REVIEW-STATUS approved.** Gates green; `handle_event`/`apply_response` pure,
  `App<C>` gone; one-in-flight invariant holds; stale/superseded `RequestId`-mismatch drop
  correct; error-code branching preserved; `contract` diff empty; no tokio/async
  (`reqwest::blocking` + `std::thread` + `std::mpsc`); 30s timeout + clean worker teardown; no
  secret-leak path; tests are public-API with only the `Client` trait mocked. One non-blocking,
  **pre-existing** nit: a stale doc comment at `main.rs:4` (about an initial health probe —
  already stale at base `f0204fd`, out of scope here; **flagged for opportunistic cleanup in a
  future TUI-touching cycle**).
- **Verifier: VERIFY-STATUS verified.** Confirmed `crates/server`+`crates/contract` diff vs
  `f0204fd` empty. Live over `./ok.sh up` (Docker 29.5.3 + Compose; postgres → migrate one-shot
  → server → otel-collector): register/login, `GET /api/profiles`, task create/list/close, the
  `{code,message}` error contract with correct statuses, two-user profile-scoping isolation (no
  cross-profile read/write, 404 no existence leak), OTel server spans for every client call.
  ADR-0003 delegation handshake: `TestBackend` suites present + green. Inferred (code-read):
  that `HttpClient` issues exactly those requests — the standard ADR-0003 split (interactive TUI
  owned by the green `TestBackend` suite).

Durable learnings captured this cycle (each to the smallest right home, all on `main`):

- **rust-standards + tester agent — the worker-analogue synchronous test executor is the
  sanctioned way to test a pure `handle_event`/`apply_response` seam without async.** When the
  effectful shell is a worker thread + channels (ADR-0006 Model A), the test harness mirrors it
  with a small synchronous executor that maps each emitted `ClientRequest` through the injected
  fake `Client` and feeds the `ClientResponse` back into `apply_response`, looping on chained
  follow-ups. This drives the two-step seam end-to-end with the only mock being the sanctioned
  external-service trait — no internal collaborator, no async runtime. Recorded as the general
  pattern in `rust-standards` and as front-of-mind tester guidance.

Deliberately **skipped** (did not earn a durable edit): **no `CLAUDE.md` gotcha** — this cycle
hit no new recurring miss. The three-home model, the contract-frozen boundary (#2), and
statelessness (#1) all held cleanly, and the pure-core/effectful-shell rule (the executor's
foundation) already lives in `rust-standards`. No `docs-standards`/`bash-standards`/
`coding-standards`/`git-standards` change — nothing new surfaced there. **No new crate** → no
new dev agent; `tui-dev` already owns `crates/tui`. No new ADR — inside ADR-0006/ADR-0003.

Next cycle should know:

- **The poll-loop redraw path is a new candidate trigger for `docs/manual-smoke.md`.** Spinner
  repaint and terminal raw-mode teardown are invisible to `TestBackend` (accepted residual risk
  per ADR-0003 §3); when the manual-smoke checklist is authored, add a "request in flight →
  spinner animates, Esc cancels, terminal restores cleanly on quit" item.
  **✓ Resolved `4318d65`** — the checklist already existed; the in-flight item + a poll-loop-path
  trigger were added to `docs/manual-smoke.md` directly (docs-only, main-side, no cycle).
- The **`main.rs:4` stale doc comment** (pre-existing health-probe nit) is a free pickup for the
  next `tui-dev` touch.
  **✓ Resolved** — filed and run as Board chore `0006` (the inaugural `chore`); the comment now
  describes the ADR-0006 worker/pure-`App` entrypoint. Reviewed (chore invariant attested) at
  `awaiting-merge`; see the 0006 handoff entry above.
- Still pending from earlier cycles (not lost): the operator-sanctioned reported-only `./ok.sh
  coverage` verb over `cargo-llvm-cov` (no hard threshold) — `architect` to plan as a `main`-side
  item; and the deferred TUI backlog (profile-switch UX, task edit/delete, Notes, Pomodoro gated
  on ADR-0002, TUI-side OTel).

**Homes.** Cross-cutting edits on `main` (homes #1/#3): this `docs/handoff.md` entry (+ the
"What works right now" snapshot refreshed), the `rust-standards`/`tester` learning,
`docs/build-plan.md`, and the regenerated `board/README.md`. **Feature-local on the branch
(home #2):** the item's `## Summary`. The orchestrator advances the branch status to
`awaiting-merge` after this; **`main`'s frozen copy of
`board/features/0005-tui-responsive-event-loop.md` is left untouched** at the claim snapshot
until the human's merge.

[adr-0006]: ./adr/0006-tui-concurrency-and-responsiveness.md

---

## Handoff — 2026-06-18 (0004 — TUI: register/login + profile + task add/list/close; slice 0001 closes)

Branch: `feature/0004-tui-foundational` (last code sha `8fb0505`). Slice 3 of 3 of the 0001
umbrella — the TUI side of the tracer bullet, closing the loop TUI ↔ `contract` ↔ server ↔
Postgres. The cycle ran build → review → verify and stopped at the AI-terminal `awaiting-merge`
on the branch.

What shipped (on the branch):

- **`crates/tui`** (binary `organized-koala`, lib+bin split) — `ratatui` 0.29, `crossterm`
  0.28, blocking `reqwest` 0.12 (rustls). The crate was **auto-discovered** by the existing
  `members = ["crates/*"]` glob; **no root `Cargo.toml` edit** was needed.
- **`src/client/`** — a `Client` trait over health/register/login/list-profiles/list-tasks/
  create-task/close-task, every method typed on `contract` DTOs (no local wire types —
  hard-constraint #2). The `reqwest` impl is `HttpClient`; the standard `ErrorBody`
  (code + message) maps to a typed `ClientError` (`Api` preserving the `ErrorCode` for
  branching; `Offline` for any transport failure or unintelligible body).
- **`src/app/`** — a **pure** screen state machine (`Auth` → `TaskList`, plus a blocking
  `Offline` screen) advanced by `App::handle_event` over a transport-agnostic `Event` enum,
  with the `Client` injected. Auth: login (identifier + password) and register (username,
  email, password, profile-name); on success fetches `GET /api/profiles`, auto-selects the
  first profile (per the plan's single-profile Assumption), loads its task list. Task list:
  newest-first with done/undone markers, add-task sub-flow (Title + Description), mark-done
  sends `…/close` and replaces the row from the server response, refresh re-fetches.
  Error-code branching per ADR-0005: `unauthenticated` drops the in-memory session → login;
  `validation_failed`/other `Api` errors surface inline; transport failure → blocking offline
  screen with a manual retry. **JWT + active profile id live in process memory only**
  (hard-constraint #1; no on-disk/cross-run state).
- **`src/ui/`** pure draw fns; **`src/terminal/`** the crossterm driver with a pure `map_key`
  and a raw-mode guard restoring the terminal on drop.
- **Keybindings (now pinned by tests):** `Esc`/`Ctrl+C` quit (`Esc` = cancel in the add-task
  sub-flow); `Enter` submit; `Tab`/`Down` next, `Shift+Tab`/`Up` prev; `Backspace`; auth `F2`
  toggles login/register; task-list `a` add / `c` mark-done / `r` refresh / `q` quit; offline
  `r` retry; printable keys typed literally in text-entry contexts.
- **Tests (tester):** 35 `TestBackend` tests under `crates/tui/tests/` (the only mock a held,
  recording fake `Client` — ADR-0003 layer 2, no binary, no live DB): `keybindings.rs` (11)
  pinning the whole `map_key` contract incl. context-sensitivity, `rendering.rs` (7)
  buffer-snapshotting auth/task-list/add-task/offline (masked password — plaintext never
  rendered), `error_branches.rs` (9) driving the ADR-0005 `code` branches, `flows.rs` (8)
  the login/register→profile→list sequence, add-task, mark-done, and statelessness.

Verdicts:

- **Reviewer: REVIEW-STATUS approved `8fb0505`** — all four gates green at HEAD; hard-constraints
  #1/#2 held (no local DTOs; no persistence/file-IO; offline path fabricates no cached data),
  the ADR-0005 error contract wired+tested, the layer-2 `TestBackend` suite present and green,
  no contract/migration/shared-state drift, `#[allow]` audit clean. **No fix-now findings.** One
  non-blocking nit: the orchestrator's board-claim commit `846ba2a` used a
  `noreply@anthropic.com` co-author trailer instead of the project form (board-only, outside
  reviewed code) — now closed durably in `git-standards` (see learnings below).
- **Verifier: VERIFY-STATUS verified `8fb0505`** — capabilities present (Docker 29.5.3, Compose
  v5.1.4), **no gap**. Booted `./ok.sh up` in the worktree and exercised the live reqwest client
  path (ADR-0003 layer 1): every endpoint the `Client` consumes round-tripped with `contract`-
  matching shapes (`register` 201, `login` 200, `GET /api/profiles` 200, task list/create/close
  open→done with `closed_at` set); error contract verified live with exact wire strings
  (`unauthenticated`/`invalid_credentials` 401, `username_taken`/`email_taken` 409,
  `validation_failed` 400, `not_found` 404); profile-scoping (#4) with a second account → 404 no
  leak; persistence across a server restart; OTel spans received end-to-end by the collector. The
  layer-2 `TestBackend` suite confirmed green under `./ok.sh test`. Only un-driven items
  (neither a blocker): interactive crossterm on a real TTY (routed to the ungated
  `docs/manual-smoke.md` check per ADR-0003 §3) and the out-of-scope timer endpoint.

No contract change, no migration, no new ADR (TUI-only, inside the frozen ADR-0005 wire format
and ADR-0003 verification routing). **No new crate-dev agent** — `tui-dev` already owns
`crates/tui`.

Durable learnings captured this cycle (each to the smallest right home, all on `main`):

- **rust-standards + tui-dev agent — separate the pure core from the effectful shell to make an
  IO/interactive surface testable.** The whole TUI surface was `TestBackend`-driveable with no
  live server and no TTY because the crate is a pure update fn (`App::handle_event`), pure draw
  fns, and a pure `map_key`, with the one external service (the server) behind an injected
  `Client` trait. That is the ADR-0003 layer-2 enabler; recorded as the general rule in
  `rust-standards` and as a front-of-mind constraint on the `tui-dev` agent.
- **git-standards — the orchestrator's co-author trailer is `claude <claude@organized-koala.local>`,
  and that applies to Board-only commits too.** The 0004 board-claim commit used
  `<noreply@anthropic.com>` (the reviewer's nit). Tightened the existing footer rule to pin the
  orchestrator's domain form explicitly and state `<noreply@anthropic.com>` is never correct here.

Deliberately **skipped** (did not earn a durable edit): no `CLAUDE.md` gotcha — this cycle hit
no new recurring miss (the three-home model and #6 held cleanly; the auto-discovery of the crate
via `members = ["crates/*"]` and the lib+bin split are already captured). No `docs-standards`,
`bash-standards`, or `coding-standards` change — nothing new surfaced there. No new ADR — TUI-only.

Backlog deferred per the plan's Assumptions (next cycles, not lost): profile picker / multiple-
profiles switch UX; task edit/delete; Notes; Pomodoro (still gated on ADR-0002 timer authority);
TUI-side tracing/OTel; and the `docs/manual-smoke.md` TTY checklist for raw-mode/teardown
behaviour invisible to `TestBackend`. Also still pending from 0003: the operator-sanctioned
reported-only `./ok.sh coverage` verb over `cargo-llvm-cov` (no hard threshold) — `architect` to
plan as a new `main`-side Board item.

**Sequencing — the foundational slice closes with this merge.** Merging
`feature/0004-tui-foundational` puts all three children (0002/0003/0004) on `main`, so parent
0001's end-to-end acceptance (register/login → profile → task add/list/close, TUI ↔ contract ↔
server ↔ Postgres) becomes closeable. `0001` is the only foundational item left open after this.

**Homes.** Cross-cutting edits on `main` (homes #1/#3): this `docs/handoff.md` entry (+ the "What
works right now" snapshot refreshed), the `rust-standards`/`git-standards`/`tui-dev` learnings,
`docs/build-plan.md`, and the regenerated `board/README.md`. **Feature-local on the branch
(home #2):** the item's `## Summary`. The orchestrator advances the branch status to
`awaiting-merge` after this; **`main`'s frozen copy of `board/features/0004-tui-foundational.md`
is left untouched** at the claim snapshot until the human's merge.

---

## Handoff — 2026-06-12 (0003 feedback re-entry — four human items resolved, re-verified, `awaiting-merge`)

**Feedback re-entry, not a fresh feature.** 0003 was at `awaiting-merge` (verified `f67a883`)
when the operator authored four `[human]` items in its Log. `architect` triaged them; the cycle
ran forward (triage → fixes → review → verify) and the item is back at `awaiting-merge` on its
branch. The four resolutions:

- **#1 (suggestion) — compose server healthcheck.** `7833b15` (platform-dev): a `healthcheck:`
  on the compose `server` service hitting pure-liveness `GET /healthz` on the in-container port
  8080, plus `curl` added to the slim runtime image. The verifier observed the container reach
  Docker `healthy` for real (probe ExitCode 0 in-container).
- **#2 (question) — no unit tests / coverage DoD.** Answered + a real gap closed. Zero server
  unit tests is policy-consistent (the public API is HTTP; coding-standards favours public-API
  coverage — 28 such tests exist). But `4c679bd` (tester) closed a **genuine** gap:
  expired-token→401 was untested at *every* layer, while a prior slice-5 Log entry had falsely
  claimed "source-owned jwt unit tests" that never existed. Closed at the HTTP layer by
  hand-signing an hour-past-`exp` token (outside jsonwebtoken's 60 s leeway) → 401
  `unauthenticated`, with a fresh-token control. **The coverage-DoD-in-CI part is a separate
  `main`-side decision the operator SANCTIONED:** add `cargo-llvm-cov` behind a new `./ok.sh
  coverage` verb for a **REPORTED** coverage metric with **NO hard threshold** — to be planned as
  a new Board item (see follow-up below). Not created here (that is `architect` planning).
- **#3 (nitpick) — redundant custom `Debug`.** `353026f` (server-dev): dropped the hand-written
  `Debug` on `Jwt`/`JwtConfig` for `#[derive(Debug)]` (`SecretString` already redacts);
  load-bearing custom impls (`Password`/`AppState`/`TelemetryGuard`) left intact.
- **#4 (question, DoS) — DB hit on every authenticated request?** Clarified, no change. Auth is
  stateless JWT verification with **zero** DB queries (`session.rs:37` → `jwt.rs:63-68`, no
  session table; the user id is the token `sub` claim). The premise did not hold; the only DB
  work on an authed request is the business query itself.

**Verdicts (feedback delta `fca5f53..HEAD`).** reviewer **`REVIEW-STATUS: approved 4c679bd`**;
verifier **`VERIFY-STATUS: verified 4c679bd`** — re-verified live via the sanctioned `./ok.sh
up`/`down` (Docker 29.5.3 / Compose v5.1.4): the `server` container went `starting` → `healthy`
(curl present in the slim image), migrate one-shot exited 0 before server start, regression
spot-check of register/login/task CRUD + error contract green, OTLP export re-confirmed.

**Follow-up the next cycle picks up — operator-sanctioned coverage verb.** A new Board item is
to be planned on `main` (`architect`): an `./ok.sh coverage` verb wrapping `cargo-llvm-cov` that
**reports** a coverage metric with **no hard threshold** (not a DoD gate). `cargo-llvm-cov` is
operator-sanctioned for this; `platform-dev` owns the verb, `eng-manager` documents it. This is
deliberately **not** created here — recorded so it is not lost.

**Learnings captured (each to the smallest right home):**

- **git-standards** — the co-author footer identity is owned by `git-standards`, **never copied
  from a dispatch prompt**. `353026f` committed with `<noreply@anthropic.com>` because the
  orchestrator's dispatch prompt hardcoded that trailer; the `<agent>@organized-koala.local` form
  is the only authority.
- **docs-standards** — two notes: (a) never let a wrapped Board prose line begin with `#` or a
  list-like token — `rumdl fmt`'s auto-fix splits the paragraph with an inserted blank line
  (MD032); reword (e.g. "constraints 1–6"); never blindly accept `rumdl fmt` on prose. (b) A
  successful commit does **not** prove markdown is lint-clean — `.githooks/pre-commit` is a
  secret-scan only; markdown linting is the PostToolUse `.claude/lint.sh` hook and does not gate
  commits, so run `rumdl check --config .claude/rumdl.toml <file>` explicitly.
- **coding-standards** + **reviewer agent** — a "covered by …" claim must name a test that
  actually exists. The slice-5 phantom-test claim let an untested `exp` path reach
  `awaiting-merge`; the reviewer now spot-checks that cited coverage is real (a phantom claim is
  changes-requested).

**Homes.** Cross-cutting edits on `main` (homes #1/#3): this `docs/handoff.md` entry (+ the
"What works right now" snapshot refreshed), the four standards/agent edits above, and the
regenerated `board/README.md`. **Feature-local on the branch (home #2):** the item's `## Summary`
and the four `[x]`-checked feedback items live on `feature/0003-server-auth-profile-tasks` and
return to `main` atomically at the human's merge. `main`'s frozen copy of the item is left
untouched at the claim snapshot.

---

## Handoff — 2026-06-12 (0003 re-verified under the sanctioned mechanism — block cleared, `awaiting-merge`)

**The capability-gap block on 0003 is cleared.** Docker was provisioned by the operator (Engine
29.5.3, Compose v5.1.4), and 0003 was re-verified **under the sanctioned mechanism only** — the
real docker-compose stack via `./ok.sh`, **no external binary acquired, no improvised DB**. This
closes the loop opened by the policy-correction entry below: a `blocked` capability gap is
recoverable.

**Re-entry mechanics.** drive re-entered 0003 at the **verify** phase — there was **zero code
change** (last code sha is still `f67a883`; only Board-only commits follow it), so the reviewer's
**`REVIEW-STATUS: approved f67a883` was preserved** (a board-only commit does not invalidate the
approval; the orchestrator confirmed no code commit follows the approved sha). No re-review was
needed; only the previously-void verifier verdict had to be re-earned live.

**Verifier verdict: `VERIFY-STATUS: verified f67a883`** (on the branch), closing both prior
environmental gaps live under docker:

- `./ok.sh test` — **28/28 green** on the compose Postgres.
- `./ok.sh up` full stack — the ADR-0004 migrate→run `service_completed_successfully` gating
  **proven via `docker inspect`**: the one-shot `migrate` `exited(0)`, and the `run` service
  started ~0.49 s later — never before. (Prior gap-1, the un-booted compose gating, is closed.)
- Full ADR-0005 HTTP surface live with exact codes/bodies; two-user profile isolation → 404
  `not_found`; idempotent re-close (byte-identical `closed_at`).
- **OTLP export observed live** — 31 spans landed in the collector `debug` exporter. (Prior
  gap-2, log-only degraded mode, is closed.)
- Secrets clean in logs/Board; clean teardown (`./ok.sh down`); read-only throughout.

drive then flipped the branch item **`blocked` → `review` → `awaiting-merge`**. 0003 is now at
the AI-terminal state, awaiting the human merge.

**The validated process learning — the recovery loop works end-to-end.** The
**block → escalate → human provisions the capability → re-verify under the sanctioned mechanism**
loop ran to completion and is now demonstrated, not just asserted. The takeaways worth keeping:

- **A `blocked` item is recoverable with zero code churn.** Because the block was purely
  environmental (a missing capability, not a code defect), provisioning docker was sufficient;
  nothing in `crates/server` or `deploy/` changed. The hard-constraint-#6 discipline (block +
  escalate, never engineer around) cost **no** rework — it deferred the *runtime confirmation*,
  not the *code*.
- **Re-entry was at verify, and reviewer approval at the last code sha survived.** The three-home
  Board model held: the cycle's intervening status changes were board-only commits on the branch,
  so `approved f67a883` still names the last code sha. The orchestrator's "no code commit follows
  the approved sha" check is what makes a verify-only re-entry safe — re-review is unnecessary.
- The existing **verifier agent instructions needed no edit** — it already encodes the
  block-and-escalate-on-missing-capability rule and the sanctioned-mechanism-only constraint
  (added in the policy-correction cycle). This re-entry *exercised* those instructions exactly as
  written; no refinement was manufactured.

**Sequencing — merge of 0003 unblocks 0004.** 0004 (TUI, the third and final slice of the 0001
umbrella) depends-on 0003 and becomes claimable once the human merges
`feature/0003-server-auth-profile-tasks`. No new crate dev agent — `server-dev` already owns
`crates/server`.

Docs updated (all on `main` — derived/cross-cutting, homes #1/#3): `docs/handoff.md` (this entry,
plus the "What works right now" snapshot refreshed); `board/README.md` regenerated (home #3,
derived).
**`main`'s frozen copy of `board/features/0003-*.md` is left untouched** at the claim snapshot
(`ready` + pointer) — the branch copy carries the live `awaiting-merge` status and verdicts and
returns to `main` atomically with the code at the human's merge (home #2). No `CLAUDE.md` or
agent/skill edit — the #6 policy and verifier discipline are already in place and were validated,
not changed.

---

## Handoff — 2026-06-12 (policy correction — no unsanctioned binaries; 0003 reverted to `blocked`)

**Operator policy correction, encoded on `main`.** Supersedes the docker-fallback framing in the
0003 entry below. Two linked, load-bearing rules now binding on every agent in every phase
(CLAUDE.md hard constraint **#6** + tightened "Ambiguity policy"):

1. **No agent downloads, installs, or runs an external binary without the operator's explicit
   approval** — including anything written into a dispatch prompt.
2. **A missing capability the Definition of Done needs (docker, a live DB, any required tool)
   sets the item to `blocked` with a precise question and STOPS for human intervention — it is
   never engineered around.** `verified-with-gaps` is for genuinely-minor *inferred* sub-items,
   never for "couldn't run it because a required tool was missing."

**Origin.** In the 0003 cycle docker was absent in the sandbox. The orchestrator authorized the
tester/verifier to "bootstrap a throwaway local Postgres"; they downloaded/ran an embedded
Postgres 16.2 and the verifier reused a leftover `/tmp/pgextract` binary. The operator has
**disavowed** this. The "binary + live-Postgres fallback" verification of 0003 is therefore
**void for sign-off**.

**Status change.** 0003 was moved **`awaiting-merge` → `blocked`** (on its branch — the
orchestrator committed the block + Log entry there; `main`'s snapshot stays frozen at the claim).
It is **not** heading to merge.

**Re-entry plan.** Operator sets up docker → 0003 is re-verified under the **sanctioned mechanism
only** (`./ok.sh up` / the real compose stack, no improvised DB, no downloaded binary) → back to
`awaiting-merge`. The reviewer's **`REVIEW-STATUS: approved f67a883` stands** (cold code review is
unaffected by the runtime gap); only the **verifier verdict is void** until the sanctioned live
pass is done.

**Docs corrected on `main`:** CLAUDE.md (hard constraint #6 + tightened Ambiguity policy);
`.claude/agents/verifier.md` (the 3ac2a46 "sanctioned binary/live-Postgres fallback + merge-time
ask" language **removed** and replaced with report-not-verified + block-and-escalate);
`.claude/agents/tester.md`, `server-dev.md`, `platform-dev.md` (each now carries the
no-unsanctioned-binaries / block-on-missing-capability rule); `bash-standards` (scripts fail loud
and escalate, never fetch+run); `board/README.md` regenerated (0003 → `blocked`). **Kept intact**
(correct learnings from 3ac2a46): the lib+bin rule in `rust-standards`/`new-crate`, and the
net-new-infra carve-out in CLAUDE.md "The Board" home #1.

---

## Handoff — 2026-06-12 (0003 — server: auth + default profile + tasks + migrations + docker stack)

> **SUPERSEDED in part by the policy-correction entry above (2026-06-12).** The "docker
> unavailable → sanctioned binary + live-Postgres fallback → verified-with-gaps → human boots
> `./ok.sh up` at merge" framing in this entry is **disavowed**. 0003 is **`blocked`**, not
> heading to merge; its verifier verdict is **void for sign-off** pending a sanctioned live pass
> on a docker host. The reviewer's `approved f67a883` stands. Read the entry below as the cycle's
> historical record, not as current policy or status.

Branch: `feature/0003-server-auth-profile-tasks` (last code sha `f67a883`). Slice 2 of 3 of the
foundational slice 0001 — the server side of the tracer bullet, verifiable live over HTTP before
the TUI exists. The cycle ran build → review → verify and stopped at the AI-terminal
`awaiting-merge` on the branch.

What shipped (on the branch):

- **`crates/server`** (binary `organized-koalad`) with the ADR-0004 admin CLI: `run` (default
  no-arg, **never** mutates schema), `migrate` (idempotent), `rollback` (one step default,
  bounded by `--steps`, never auto-invoked). Reversible paired `*.up.sql`/`*.down.sql`
  migrations for `users`/`profiles`/`tasks` (FKs profile→user, task→profile; flat task domain),
  embedded via `sqlx::migrate!`; committed `.sqlx/` offline cache.
- **Auth:** argon2id PHC hashing (constant-time decoy verify for absent users), JWT HS256
  (sub/iat/exp, expiry enforced; secret held as `SecretString`, redacted everywhere), the
  `AuthUser` Bearer extractor. Endpoints per ADR-0005: `register` (user + named default profile
  in one transaction → 201), `login` (username-or-email → 200), `GET /api/profiles`, profile-
  scoped `GET|POST .../tasks` + `POST .../tasks/{tid}/close`, `GET /healthz`.
- **Profile isolation** via ownership-joined queries → unowned/nonexistent profile is **404
  `not_found`** (never 403, no existence leak); title trimmed+non-empty (else 400
  `validation_failed`); close idempotent (preserves original `closed_at`). The thiserror
  boundary maps each case to HTTP status + `contract::ErrorBody { code?, message }` (internal
  causes logged, never sent). `tracing` spans + INFO mutation events on every endpoint; OTLP
  export gated on `OK_OTLP_ENDPOINT`, degrading to log-only when the collector is absent.
- **`deploy/` docker stack** (platform-dev): multi-stage Dockerfile (release build off the
  committed `.sqlx/`, slim runtime as an unprivileged user), `docker-compose.yml` with the
  ADR-0004 graph — Postgres (healthcheck) → one-shot `migrate` (gated `service_healthy`) →
  `run` (gated `service_completed_successfully`) → minimal OTel collector (OTLP/gRPC receiver +
  `debug` exporter). `ok.sh` wired: `up`/`down`, dev-only `migrate`/`rollback` delegating to the
  binary, `run-server`, and a `test` verb that boots a throwaway tmpfs Postgres for the
  `#[sqlx::test]` suite. The committed stack carries **no** credential literal (a gitignored
  `deploy/.env` with DEV-ONLY placeholders is generated by `up`; secret-scan clean).
- **Tests** (tester): 28 integration tests over the public HTTP surface (`auth` 14,
  `tasks` 9, `profile_isolation` 5) driving the real `axum` router in-process via
  `tower::ServiceExt::oneshot` over a per-test `#[sqlx::test]` DB. Every error asserts the exact
  ADR-0005 `code`, not just status.

Verdicts:

- **Reviewer: REVIEW-STATUS approved `f67a883`** — mechanical gate green (`fmt --check`/`lint`/
  `build`/`sqlx prepare --check`/`secret-scan`); no contract drift (server defines no DTO, maps
  at the boundary); endpoints/CLI/compose match ADR-0004/0005; hard constraints #2–#5 held;
  secrets redacted. Two non-blocking nits (unused `app_with_ttl` expired-token helper;
  `cmd_run_server` harmlessly forwards `"$@"` to argless `run`).
- **Verifier: VERIFY-STATUS verified-with-gaps `f67a883`** — docker unavailable in the sandbox,
  so used the sanctioned binary + live-Postgres fallback (real HTTP round-trips, nothing faked).
  Verified live: `./ok.sh test` **28/28 GREEN**, CLI run/migrate/rollback, the **migrate-before-
  serve seam** proven (fresh unmigrated DB: `register`→500 since serve never creates schema;
  after `organized-koalad migrate`, the same running server served `register`→201 with no
  restart), the full ADR-0005 surface with exact codes/bodies, **profile isolation across two
  users** → 404 `not_found`, idempotent re-close, tracing spans/INFO events, and **secrets
  absent from logs**.

Two verifier gaps — environmental, docker-only, NOT code defects:

1. `./ok.sh up` full compose stack + its `service_completed_successfully` migrate→run gating was
   not booted.
2. OTLP span export to the OTel collector was not observed (ran log-only degraded mode).

> **Merge-time action for the human:** boot `./ok.sh up` once on a docker host to close 0003's
> two gaps — confirm the migrate one-shot gates the `run` service and that spans reach the OTel
> collector. The semantics are already proven via the binary + live-Postgres fallback; this is
> the live-stack confirmation the sandbox could not perform.

Process learnings captured this cycle (all on `main`):

- **Net-new infra born with a new crate rides that crate's branch (carve-out to home #1).** This
  cycle deliberately put the `deploy/` stack + the `ok.sh` `up`/`run-server`/`migrate` verbs ON
  the branch — they are net-new and only meaningful because the `server` crate doesn't exist on
  `main` yet, and the verifier needs `./ok.sh up` to work inside the worktree. Landing them on
  `main` early would be an out-of-sync bug in the *other* direction (referencing a non-existent
  crate). This is distinct from the 0002 bug class (*modifying existing* shared infra, which
  stays `main`-only). Added as a narrow, explicitly-bounded carve-out to CLAUDE.md "The Board"
  home #1 with a decision test (when unsure → main-only).
- **A binary crate that will be integration-tested needs a `[lib]` target — scaffold it lib+bin
  from the start.** `tests/` links the crate's library, not its binary; the binary-only `server`
  crate couldn't expose `app::router`/`AppState`/config for in-process tests, blocking
  `./ok.sh test` until `server-dev` added a `[lib] name = "server"` + thin `src/lib.rs`
  (re-exporting the seams) with `main.rs` reduced to a CLI shell (`f67a883`). Recorded in
  `rust-standards` (the rule) and `new-crate` (the scaffold-time action); the `new-crate`
  reference example was also refreshed off the removed `organized-koala` placeholder onto
  `contract` (library) + `server` (lib+bin) as the live exemplars.
- **Docker-unavailable sandbox is a standing verifier limitation.** Every cycle shipping
  compose/OTel infra leaves the `service_completed_successfully` gating and OTLP-export sub-items
  verified-by-reading only; the sanctioned mitigation is the binary + live-Postgres fallback
  (proves semantics) plus the human booting the full stack once at merge. Recorded in the
  `verifier` agent so future verify passes apply it consistently.

Be aware:

- 0003 is **branch-owned** on `feature/0003-server-auth-profile-tasks`; the cycle advanced the
  branch copy of the item (status, reviewer/verifier verdicts, `## Summary`). `main`'s copy stays
  frozen at the claim snapshot (`ready`, with a pointer note) until the human's merge brings it
  back atomically with the code. No new crate dev agent — `server-dev` already owns
  `crates/server`.
- With 0003 heading to merge, **0004 (TUI) becomes unblocked** (it depends-on 0003). 0004 is the
  third and final slice of the foundational 0001 umbrella.

Docs updated (all on `main` — shared/cross-cutting, home #1): `docs/handoff.md` (this entry);
`CLAUDE.md` "The Board" home #1 (the net-new-infra carve-out); the `rust-standards` +
`new-crate` skills (the lib+bin rule + refreshed reference example); the `verifier`
agent (docker-unavailable fallback); `board/README.md` regenerated (home #3, derived). The 0003
item's `## Summary` was filled **on the branch** (home #2).

---

## Handoff — 2026-06-12 (0002 re-entry — human feedback: chrono timestamps + test-layout)

Two `[human]` feedback items on the already-verified, `awaiting-merge` 0002 re-opened the cycle.
`architect` triaged both; the cycle ran forward on `feature/0002-contract-crate` and stopped at
the AI-terminal `awaiting-merge` again. Both feedback boxes are now `[x]`.

What shipped (on the branch):

- **Feedback-1 (chrono):** contract timestamps are now `chrono::DateTime<Utc>`
  (`Task.created_at`/`closed_at`, `Profile.created_at`) instead of opaque strings — consumers
  get a typed timestamp and malformed dates now fail to parse. `chrono` added pure-DTO
  (`default-features = false, features = ["std","serde"]` — no clock/IO surface). **Wire bytes
  are unchanged** (RFC 3339 `…Z`, `closed_at: null` still emitted), so it sits **inside**
  ADR-0005's frozen wire format — **no wire change, no ADR.** Commits `bc61626` (contract),
  `98d1a85` (tests); reviewer approved `98d1a85`, verifier VERIFIED — 41 integration + 12
  doctests = 53 green.
- **Feedback-2 (test layout):** resolved as a **clarification, no code change**. The
  `contract` crate is pure-DTO — its whole surface is public — so the crate-root `tests/`
  public-API suite plus doctests is the correct, complete layout; there is no private logic for
  `module/tests.rs` to cover. Captured as a durable rule in `rust-standards` on `main`
  (`8b56ed2`).

Process point worth keeping (the durable learning of this re-entry):

- **A pure-Rust-representation change on an `awaiting-merge` item, with identical wire bytes,
  does NOT need an ADR.** ADR-0005 froze the *wire format*; it explicitly delegates the Rust
  representation (chrono vs string, enum-with-catch-all, etc.) to `contract-owner`. Swapping the
  in-crate type while the serialized bytes are byte-identical stays inside that delegation.
  **Contrast:** a change to the wire shape itself (a renamed/added/removed field, a changed
  encoding the other side observes) IS an ADR event and ripples to both consumers (CLAUDE.md
  hard-constraint #2). The reviewer guarded the boundary by holding the exact-byte assertions
  (`…Z` suffix, `closed_at: null` emitted) unweakened.
- The re-entry mechanics held: the **unchecked box was the only re-entry signal**;
  `architect` triaged to the smallest re-entry point (behaviour tweak, not a redesign); the
  owning agent checked the box `[x]` only after on-branch resolution + re-review. Zero blast
  radius because 0003/0004 are not built yet.

Be aware:

- 0002 remains **branch-owned** on `feature/0002-contract-crate`; the chrono delta advanced the
  branch copy of the item (status, re-review/re-verify verdicts, Summary) — `main`'s snapshot
  stays frozen at the claim until the human's merge. 0003 (server) is still `ready` and
  unblocked once 0002 merges; 0004 (TUI) follows 0003.
- No new crate dev agent — `contract-owner` still owns `crates/contract`.

Docs updated (all on `main` — shared/cross-cutting, home #1): `docs/handoff.md` (this entry);
`.claude/skills/rust-standards/SKILL.md` (the pure-DTO test-layout rule, `8b56ed2`);
`board/README.md` regenerated (home #3, derived). The 0002 item's `## Summary` was updated for
the chrono change **on the branch** (home #2).

---

## Handoff — 2026-06-11 (0002 — contract crate + workspace restructure)

Branch: `feature/0002-contract-crate` (head `638eef1`, last code `56833a6`, linear atop `main`
`ed9510e`, fast-forward — frozen for the human to merge). Slice 1 of 3 of the foundational
slice 0001.

What shipped:

- Removed the `crates/organized-koala` placeholder; the workspace now matches the target
  `contract`/(`server`)/(`tui`) layout. `crates/contract` authored as the single source of
  truth for the foundational wire shapes per ADR-0005.
- DTOs: `RegisterRequest`, `LoginRequest`, `SessionResponse`, `Profile`, `Task`, `TaskStatus`,
  `CreateTaskRequest`, `ErrorBody { code?, message }` + the 7 stable error codes with a lossless
  `Unknown` catch-all; a `Password` newtype (transparent serialize, `[REDACTED]` Debug).
- 37 serde/wire-format integration tests + 12 doctests green; build/lint/fmt clean. Reviewer
  approved at code head `56833a6` (re-attested after the rebase); verifier confirmed the
  pure-DTO seam (live-stack E2E deferred to 0003/0004 per ADR-0003).
- Planning artifacts (ADR-0005 + the 0002/0003/0004 plan) were committed to `main` as
  `1a2540c` before the worktree was finalized.

Process learnings captured this cycle (these will bite 0003/0004 if ignored):

- **State has three homes, by which side of the `main`↔branch line it belongs on.** This is THE
  process learning of the cycle, and it supersedes the earlier (wrong)
  "Board-authoritative-on-`main`, branches code-only" framing, which added a transcription step
  and still stranded cross-cutting state on the wrong side of the line — the root cause of BOTH
  out-of-sync incidents this cycle. The corrected model (now in CLAUDE.md "The Board"):
  1. **Shared / cross-cutting → `main` only, never on a feature branch.** ADRs + the decisions
     index, infrastructure (`ok.sh`, `.githooks/`, docker/compose, OTel config), `CLAUDE.md`,
     the standards skills, and `.claude/` agent/skill defs. A change to any of these riding a
     feature branch IS the out-of-sync bug class.
  2. **Feature-local → on the feature branch, in the worktree.** The
     `board/features/NNNN-<slug>.md` item travels with the code: status flips, per-slice Log,
     reviewer/verifier verdicts, and the `## Summary` are all committed on the branch. A clean
     revert is just dropping the worktree + branch; concurrent worktrees never contend on a
     shared Board file; a verdict on the branch is immutable evidence tied to its sha.
  3. **Derived → regenerated on `main`.** `board/README.md` from item frontmatter + branch heads.
  Lifecycle: born on `main` during planning, **branch-owned on claim** (the branch copy advances,
  `main`'s copy freezes at the claim snapshot until the human's merge brings it back atomically
  with the code). reviewer/verifier are **read-only on everything** (code AND Board) and report
  verdicts back; the orchestrator commits them on the branch. A Board-only commit does not
  trigger re-review — only a new code/test commit does. Codified in `drive`/`plan`/`review` and
  the `architect`/`reviewer`/`verifier` agents.
- **The secret-scan hook fix was relocated from the 0002 branch to `main`.** This cycle
  `platform-dev`'s `.githooks/secret-scan.sh` fix was wrongly committed on the 0002 feature
  branch, leaving `main`'s scanner stale — a textbook instance of cross-cutting state (home #1)
  riding a feature branch. It has been moved to `main`; the three-home rule above exists to
  prevent the recurrence.
- **Plan/ADR must be committed to `main` before the worktree is cut.** This cycle the ADR-0005
  artifacts were left uncommitted, the worktree was cut from the pre-ADR commit, and the code's
  `(see ADR-0005)` citations dangled — contract-owner flagged it as a blocker; recovered by
  committing to `main` and rebasing. Now a corollary of the three-home model (an ADR is home #1,
  and a worktree cut from a commit that lacks it cannot see it). Codified in `plan` + `drive`,
  the `architect` agent, and CLAUDE.md.
- **secret-scan matches credential VALUES, not bare identifiers** (now `d34570c` on `main`; the
  branch's original `37b78c4` was dropped when the fix was relocated): a bare Rust field
  declaration (keyword + bare type + comma, no separator/literal) no longer false-positives;
  assigned literals still trip. One known non-blocking gap recorded for future platform-dev (the
  JSON-object quoted-key/quoted-value form is not caught). Documented in `bash-standards`
  structurally (no matchable literals, so the doc does not trip its own scanner).
- **Markdown MD004:** a wrapped prose line starting with `+`/`*`/`-` is read as a list marker;
  reflow so a symbol is never line-leading. Documented in `docs-standards`.

Be aware:

- No new crate dev agent registered — `contract-owner` already owns `crates/contract`.
- 0002 is **in-flight and branch-owned** on `feature/0002-contract-crate`; its live status lives
  on the branch (where the cycle advanced it), and `main`'s snapshot is frozen at the claim until
  the human's merge. 0003 (server) is `ready` and unblocked (depends-on 0002); 0004 (TUI) is
  `ready` but depends-on 0003. 0001 is the umbrella (`planned`), tracking its three children.
- 0003 handles real credentials/JWTs — wrap secrets so they never reach `Debug`/`Display`/logs;
  do not rely on the secret-scan as the safety net.

Docs updated (all on `main` — shared/cross-cutting state, home #1): `docs/handoff.md` (this
entry, re-corrected to the three-home model); CLAUDE.md "The Board"; `docs/build-plan.md`;
`board/README.md` regenerated; the `plan`/`drive`/`review` skills; the
`architect`/`reviewer`/`verifier` agents; the `bash-standards`/`docs-standards` skills. The
secret-scan hook fix was relocated from the 0002 branch to `main`. The 0002 item's
`## Summary` + Log live on the branch (home #2).

---

## Handoff — 2026-06-10 (Bootstrap — workflow scaffold)

Branch: `main`.
Stood up the AI development workflow per BOOTSTRAP.md: the agent team, skills, Board, and docs
system for organized-koala. No application code yet — this cycle established *how* work runs,
not *what* it does.

What shipped:

- `CLAUDE.md` constitution (purpose, stack, `ok.sh` ops, 5 hard constraints, error contract,
  ambiguity policy, Definition of done, trigger tables).
- 9 agents in `.claude/agents/` (architect, contract-owner, server-dev, tui-dev, platform-dev,
  tester, reviewer, verifier, eng-manager); read-only roles omit Write/Edit.
- Skills in `.claude/skills/`: drive, plan, grill, review, coding-/rust-/docs-/bash-standards,
  repo-map, autowork, autoreview.
- `ok.sh` operations entrypoint; `.githooks/` pre-commit secret scan (hooksPath enabled).
- `docs/adr/0001-foundational-architecture.md` + decisions index; this handoff; build-plan.
- `board/` with the dashboard and feature `0001` (foundational vertical slice) in `inbox`.

Be aware:

- `.claude/settings.json` (the permission allowlist) was **not** written by the bootstrap — the
  harness auto-mode classifier blocks an agent authoring permission rules. The human must add it
  (content is in the bootstrap conversation / README of this cycle).
- The `crates/organized-koala` placeholder still exists; feature 0001 restructures it into
  `contract` / `server` / `tui`.
- ADR-0002 (timer authority) is pending and gates Pomodoro work.

Docs updated: ADR-0001 created; CLAUDE.md authored.

---

### What works right now

- The **workflow** is in place: run `/drive` to advance the Board one item to `awaiting-merge`.
- **The `contract` crate is merged on `main`** (0002): a compile-only, pure-DTO seam carrying
  the foundational wire shapes (auth/profile/task DTOs, `ErrorBody`, error codes, the redacting
  `Password` newtype) per ADR-0005, with `chrono::DateTime<Utc>` timestamps (wire bytes
  unchanged — RFC 3339 `…Z`). The workspace matches the target layout (placeholder crate gone).
- **The server is merged on `main`** (0003): `organized-koalad` implements the full ADR-0005
  HTTP API against Postgres — argon2 + JWT auth, the atomically-created default profile,
  profile-scoped add/list/close tasks, the `{ code?, message }` error contract, the ADR-0004
  `run`/`migrate`/`rollback` CLI, reversible migrations, `tracing`/OTLP instrumentation, and the
  `deploy/` docker stack (compose `server` healthcheck on `/healthz`). Merged after a four-item
  human-feedback re-entry; reviewed + live-verified under the sanctioned docker mechanism.
- **The TUI is merged on `main`** (0004): `organized-koala` (ratatui/crossterm/reqwest) completes
  the loop — register/login (auto-selecting the single default profile), task list (newest-first,
  done/undone markers, add Title+Description, mark-done), ADR-0005 error-code branching
  (`unauthenticated`→login, `validation_failed`→inline, offline→blocking+retry), and statelessness
  (JWT + active profile id in process memory only). Built as a pure core (update fn + draw fns +
  `map_key`) behind an injected `Client` trait, so the whole interactive surface is `TestBackend`-
  tested (ADR-0003 layer 2). Reviewed + live-verified over the full reqwest path.
- **The foundational slice 0001 is CLOSED.** With 0002/0003/0004 all on `main`, the umbrella
  0001 merged too — the end-to-end tracer bullet TUI ↔ contract ↔ server ↔ Postgres is complete.
- **The TUI responsive event loop is MERGED on `main`** (0005): the TUI no longer freezes
  during an HTTP request — it keeps rendering, animates a spinner with a "working… (Esc to
  cancel)" hint, and stays interactive in flight. Per [ADR-0006][adr-0006] Model A: a synchronous
  `Client` on a worker thread, `std::sync::mpsc` request/response, a polled (`event::poll`) render
  loop — **no async runtime**. The `App` core is now client-free with two pure seams
  (`handle_event`/`apply_response`); one request in flight at a time (transient `pending:
  Option<RequestId>`), cancel is user-perceived (stale-`RequestId` response dropped). `tui::app`
  was reorganized into `auth`/`task_add`/`task_list` submodules + `protocol.rs`. TUI-only —
  `contract`/`server` unchanged. Reviewed + live-verified (code-hash
  `bc89672d4be5cdecd0bb54b340a24a5b8741cf21`); fast-forwarded to `main` at `6f9a80a`, worktree +
  branch removed.
- **The `chore` Board item type now exists** (governance, learned-0005 follow-up): a lightweight
  lane for scope-limited maintenance (refactors, doc fixes, test-only, dep bumps) with no
  behaviour/`contract`/domain change — orchestrator-mintable, on a lighter DoD (gates + an
  invariant-attesting cold review; live verifier skipped). See CLAUDE.md "Definition of done" +
  "The Board". **First trip through the pipeline complete and MERGED — `0006`** (the
  `tui/src/main.rs` stale-doc-comment fix) ran mint → claim → build → invariant-attesting cold
  review → verify skipped → `awaiting-merge`, then fast-forwarded to `main` (code-hash
  `401ad3de59c4cc7e33c3ebf8308c171d80659e4e`); the chore lane needed zero process correction.
- **The account-global Pomodoro focus timer is MERGED on `main`** (0008, the
  first Focus-phase feature; live-verified): a new `contract` `timer`
  module (`TimerConfig`, `UpdateTimerConfigRequest`, the tagged `TimerSession` enum carrying
  `ends_at` + `server_now`), five account-global `/api/timer/...` server endpoints keyed on
  `user_id` (config get/update, session get/start/stop) with a reversible migration creating
  `timer_configs` + `timer_sessions` (`ends_at` derived, not stored), and a TUI presentation whose
  live `MM:SS` countdown is **render-only** — recomputed each ~80 ms draw from the server's
  absolute `ends_at` + `server_now` + a monotonic `Instant`, never a stored counter (#1-safe;
  inside [ADR-0006][adr-0006], no per-second polling). Account-global (#4 / ADR-0002 §5), flat (#3,
  duration the only knob); the contract/domain surface carries no new/amended ADR
  ([ADR-0002][adr-0002] governs). **After the 0008-R1 feedback re-entry (TUI-only):** the timer is
  an **always-visible global widget** in the bottom-right of every post-auth screen (no dedicated
  page), toggled by a global **`p`** (start/stop) that is listed in the bottom-left help caption;
  the in-flight indicator **appends a trailing spinner** to the stable caption instead of replacing
  it (flicker fix), and the coarse session refresh loosened ~5 s → ~1 min — all governed by the
  [ADR-0006][adr-0006] **§8 amendment** (TUI presentation only; ADR-0002 authority/render model
  unchanged; no `contract`/server/migration change). Reviewed **approved** and live-**verified** at
  the 0008-R1 end state, both pinned to code-hash `3fa0adefce8cd6d67ae716dae7a24ce6dbf9defd` on
  `feature/0008-pomodoro-timer` (the original 0008 build was approved + verified at
  `708ee8d0085ce9b3af68eb7e1b76dbe56a6185da`, voided when the re-entry moved the tree).
  Fast-forwarded to `main` at `c32f0ad`; worktree + branch removed.
- **The report-only `./ok.sh coverage` verb is MERGED on `main`** (0007, a
  `chore`): `cargo llvm-cov --workspace --summary-only`, reusing
  `cmd_test`'s live-DB wiring (throwaway test Postgres booted + torn down on a `RETURN` trap), in
  the no-arg usage/help. **No threshold, not a DoD gate** — purely reported (operator-sanctioned
  shape: coverage visible, not a brittle bar). Baseline at implementation: ~66% line / ~66%
  function / ~61% region. Tooling-only (no crate source/behaviour/`contract`/domain change), so it
  ran the lighter chore DoD: gates green + a cold reviewer **approved** attesting the chore
  invariant, pinned to code-hash `3fa0adefce8cd6d67ae716dae7a24ce6dbf9defd` on
  `feature/0007-ok-coverage-verb`; the live verifier pass was correctly **skipped**. The 0003
  "sanctioned follow-up" is now consumed. Fast-forwarded to `main` at `6860b28`; worktree + branch
  removed.
- **Coverage is now captured in the cycle and recorded in each item's Summary** (0009, a
  `main`-only governance `chore`, at `awaiting-merge` on `main` after this step): `drive` step 6
  runs `./ok.sh coverage`, parses the headline workspace coverage %, and writes a `coverage: NN.N%`
  line (or `coverage: unavailable (docker)`) into the item's `## Summary` on **every** cycle
  (feature and chore). **Report-only — never a gate** (no threshold, not a DoD clause, never blocks
  `awaiting-merge`); consistent with the "How to run" `coverage` row and 0007. Three governance
  edits (drive SKILL, CLAUDE.md DoD note, eng-manager charter), applied directly on `main` with **no
  worktree**; cold reviewer **approved** with the chore invariant attested (code-hash
  `3fa0adefce8cd6d67ae716dae7a24ce6dbf9defd`), live verifier **skipped**. 0009's own Summary is the
  first to carry a coverage line: 66.36% line / 61.48% region / 66.67% function.
- **Notes — the final domain feature — is at `awaiting-merge` on `feature/0010-notes`** (0010, a
  `feature`, live-verified): the last missing flat feature, a near-exact structural clone of the
  task surface governed by [ADR-0007][adr-0007]. A new `contract` `note` module
  (`Note { id, title, content, created_at }`, `CreateNoteRequest`, `UpdateNoteRequest`, no new
  `ErrorCode`, no `updated_at` — flat #3); five profile-scoped server CRUD routes under
  `/api/profiles/{id}/notes` (create 201 / list 200 newest-first / get 200 / update 200 in-place /
  delete 204), every query ownership-joined so an unowned/missing profile or note id is `404`
  (never 403, #4), with a reversible migration `20260612163049_notes` (`ON DELETE CASCADE`,
  `(profile_id, created_at DESC)` index); and a TUI `Screen::Notes` view (list + create/edit/delete)
  opened by `n` from the task list, stateless (#1), reqwest client maps one-for-one to the wire.
  Tests in all three crates (`contract` 11, `server` 28, `tui` `TestBackend` 13). Reviewer
  **approved** + verifier **verified**, both pinned to code-hash
  `46c1c60f1eb3865eb127a72502982827ebb09d65`; coverage 68.24% line. With Notes done, all four flat
  features (TODO, Pomodoro, Notes, Profiles) exist except Profiles CRUD (0012, still `ready`).
