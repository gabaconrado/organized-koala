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

## Ideas backlog

[`ideas/`](./ideas/) is a calm parking lot for **out-of-scope follow-ups** captured mid-cycle — it
is **outside** the state machine above. An idea is not a work item (no DoD, blocks nothing); the
human triages each (`accepted`/`closed`), and an `accepted` idea is promoted into a Board item at the
next `drive` step 1. Spec + template: [`ideas/README.md`](./ideas/README.md). See CLAUDE.md "Ideas
backlog".

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
| [0011](./features/0011-task-update-delete-reopen.md) | Task update + delete + reopen — generalize close into PATCH (breaking) | feature | merged | medium | — | — (merged) |
| [0012](./features/0012-profiles-crud-and-switcher.md) | Profiles create/update/delete + TUI switcher (delete cascades; last-profile guard) | feature | merged | medium | — | — (merged) |
| [0013](./features/0013-session-token-debug-leak.md) | Redact the JWT in `tui` `Session` — bare `String` reachable via derived `Debug` (rust-standards secret-leak violation) | chore | merged | high | — | — (merged) |
| [0014](./features/0014-tui-layout-shell.md) | TUI layout shell — top-level tabs, centred title, centred auth form, tight footer | feature | merged | medium | — | — (merged) |
| [0015](./features/0015-tui-dialog-system.md) | TUI dialog system — help/add/delete/timer modals, trimmed footer caption, purple focus | feature | merged | medium | 0014 (merged ✓) | — (merged) |
| [0016](./features/0016-tui-detail-views-and-hotkeys.md) | TUI detail views + final hotkey scheme — per-field task/note panes, full keymap | feature | merged | medium | 0015 (merged ✓) | — (merged) |
| [0017](./features/0017-timer-completion-desktop-notification.md) | Desktop notification when the focus timer ends (cross-OS, Ubuntu-first) | feature | merged | medium | 0008 (merged ✓) | — (merged) |
| [0018](./features/0018-notes-detail-multiline-content.md) | Notes detail view — multiline Content text area (fills the pane), Created moved above | feature | merged | medium | 0016 (merged ✓) | — (merged) |
| [0019](./features/0019-task-subtasks.md) | Sub-tasks — flat title/status children of a task, with TUI list nesting + collapse | feature | ready (branch-owned: awaiting-merge after help-overlay re-entry) | medium | 0016 (merged ✓) | feature/0019-task-subtasks |

> **0019 — Sub-tasks — back at `awaiting-merge` on `feature/0019-task-subtasks` after a
> help-overlay re-entry (re-approved + re-verified).** The **first admitted structural exception to
> the deliberately-flat domain (#3)**: a task may have **one level** of **title+status-only**
> sub-tasks (no description, no `created_at`, no detail view), per
> [ADR-0012](../docs/adr/0012-subtasks-domain-exception.md) (amends #3) +
> [ADR-0013](../docs/adr/0013-subtasks-wire-contract.md) (wire contract). `contract`
> `Subtask`/`CreateSubtaskRequest`/`UpdateSubtaskRequest` (existing task DTOs byte-untouched, #2);
> five profile+parent-scoped server endpoints under `/api/profiles/{pid}/tasks/{tid}/subtasks`
> (+ a per-profile list) joined `subtasks → tasks` on `task_id` AND `tasks.profile_id` (cross-reach
> `404`, #4/A1), reversible `subtasks` migration with `task_id` FK `ON DELETE CASCADE` (no-orphans
> R4). TUI: `A` create / `e` edit-title / `Space` toggle / `x` collapse (Tasks-context, routed by
> row type), a two-call tree load, indented render + `+`/`>` indicator, collapse derived from
> parent status (transient override map, #1), and a read-only Task Detail "Sub-tasks" section.
> Tests: contract 14, server 21 (incl. task- and profile-delete cascade), tui `TestBackend` 16.
> **Post-`awaiting-merge` re-entry (TUI-presentation-only):** an operator-reported `?` help-overlay
> bug — the Tasks reference line wrapped `d delete` to an un-indented continuation once the 0019
> sub-task hotkeys overflowed the shared 64-col box — was fixed by giving the help overlay its own
> `HELP_DIALOG_WIDTH = 72` (a `width` field on the `Dialog` struct; the other five dialogs keep
> `DIALOG_WIDTH = 64`) + a regression test in `crates/tui/tests/dialogs.rs`; no `contract`/server/
> domain change, no ADR. The code-changing fix voided the original `8c500ca0…` verdicts. Reviewer
> **re-approved** + verifier **re-verified**, both pinned to code-hash
> `da5b04634dcedc3a6df38ef65958548981d83775` (commit `54fea75`; server/contract diff empty ⇒
> live-boot N/A, prior endpoint verification carries forward); coverage 71.23% line (report-only).
> New CLAUDE.md gotcha this re-entry: the `?` help overlay packs key·action pairs into a fixed-width
> box, so a new/renamed hotkey can silently overflow a reference line and wrap with no indent
> (second occurrence — 0015 Global block, 0019 Tasks line). The earlier-cycle gotcha (extending the
> `tui` `Client`/`ClientRequest`+`Outcome`/screen-`State` surface strands the tester-owned
> `crates/tui/tests/` harness — `--lib --bins` green, `--all-targets` red until the tester slice
> un-strands it) still stands. Two out-of-scope follow-ups filed as ideas on `main`:
> [`ideas/0007`](./ideas/0007-delete-single-subtask-affordance.md) (a TUI key to delete a single
> sub-task — client/server plumbing + tests exist, but no keymap reaches them) and
> [`ideas/0008`](./ideas/0008-drop-commit-sha-from-help-regression-test-comment.md) (the new
> regression test's comment cites the fixing commit sha — a `coding-standards` nit). The main
> snapshot is frozen at the `ready` claim; the authoritative live status is on the branch.
>
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
> **0011 — task update/delete/reopen — MERGED.** The one-way task
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
> Coverage 68.24% line (report-only). This parallel-feature re-verify is recorded as a new CLAUDE.md
> gotcha (alongside the cross-worktree volume gotcha + a `platform-dev` per-worktree-isolation
> follow-up). After re-verify, the operator authorized a doc-only README fix (`close` → `update/delete`
> in the `tui` crate README) with the re-review/re-verify cycle **explicitly waived** — that commit
> moved the code-hash `ee5047c9…` → `97cbc025523bdff1907e9552fd3636d3a874b589`, so the verdicts are
> carried forward by operator authorization. Operator authorized the close; fast-forwarded to `main`
> at `9635608`; worktree + branch removed.
>
> **0012 — Profiles CRUD + switcher — MERGED.** The **last domain feature** — organized-koala is now
> functionally complete. Governed by [ADR-0009](../docs/adr/0009-profile-mutations.md) (profile
> mutations, ref ADR-0005 §2/§4/§6 — two **append-only** error codes `ProfileNameTaken`/`LastProfile`).
> `contract` `CreateProfileRequest`/`UpdateProfileRequest`; server `POST` (201) / `PATCH` (200) /
> `DELETE` (204) under `/api/profiles`, owner-scoped: race-safe DB unique-violation → `409
> profile_name_taken` (no TOCTOU), atomic last-profile guard → `409 last_profile` (account keeps ≥1
> namespace), delete **cascades** tasks **and** notes via FK `ON DELETE CASCADE` (#4), reversible
> `UNIQUE (user_id, name)` migration ordered after 0010. TUI `Screen::Profiles` switcher (`s`) where
> **switch is client-side only** — rebinds the in-memory `active_profile_id`, no server endpoint, no
> persistence (#1); deleting the active profile re-points to the first remaining. Tests: contract
> 8 + 16, server 20 (headline cascade asserts BOTH task and note gone), tui 16 + keybindings 25. Reviewer
> **approved** + verifier **verified** (live cascade DB-confirmed `tasks=0, notes=0, profile=0`), both
> pinned to code-hash `71fb7ecf327fbd42a14cb19456207885c782fe49`; coverage 66.91% line (report-only).
> Load-bearing learning this cycle: `./ok.sh prepare` is now self-contained (`3e0094b` on `main`),
> completing the "every DB-needing `ok.sh` verb self-boots the shared test PG" pattern
> (`test`/`coverage`/`prepare`). Operator authorized the close; fast-forwarded to `main` at
> `685b4de` (linear, no merge commit); worktree + branch removed. The reviewer's pre-existing
> `Session.token` bare-`String`/derived-`Debug` JWT-leak nit was promoted to **0013** (high chore).
>
> **0013 — Session JWT `Debug` leak — MERGED (high `chore`).** The `tui` bearer JWT, held as
> a bare `String` reachable from a derived `Debug` on `Session`, all 17 `ClientRequest::*` variants,
> and `Outcome::ListProfiles` (a `rust-standards` *Sensitive data* review-blocking leak, introduced
> in 0004 after the rule + `contract::Password` template existed, carried silently through
> 0005/0008/0010/0011 under diff-scoped cold review until 0012's reviewer flagged it), is now held in
> a `SessionToken(String)` newtype (`crates/tui/src/app/token.rs`) with a hand-written `Debug` →
> `[REDACTED]` and an `expose()` accessor used only at the point the `Authorization: Bearer` header
> is attached. Local redacting newtype chosen over `secrecy::SecretString` (the in-repo `Password`
> pattern; no new dependency for one field); the wire bearer string is byte-identical. `tui`-only —
> no `contract`/wire (#2), no domain (#3),
> no behaviour change beyond `Debug` rendering. Lighter chore DoD: gates green + cold `reviewer`
> **approved** with the chore invariant attested, pinned to code-hash
> `e5925c5139e52846d8593c4be3ab2d0516d49fa0`; live verifier **skipped** (chore track). Coverage 66.90%
> line. This cycle sharpened `rust-standards` with a callout on the
> `missing_debug_implementations`-lint-vs-secret-redaction tension. Operator approved the code;
> fast-forwarded to `main` (linear, no merge commit); worktree + branch removed.
>
> **0014 — TUI layout shell — MERGED (Phase 1 of 3).** A **`tui`-crate-only**
> reshape of the structural shell — navigation, auth screen, title, footer — with **no
> `contract`/server/domain change** (boundary binding per
> [ADR-0010](../docs/adr/0010-tui-navigation-and-interaction-model.md) §5). `Screen::TaskList`/
> `Notes`/`Profiles` collapsed into one `Screen::Main(Box<MainState>)` holding the active
> `Tab{Tasks,Notes,Profiles}` + all three live panes (new `crates/tui/src/app/main_view.rs`);
> `Tab`/`Shift+Tab` cycle the tabs (arrows move list selection), each switch re-derives the pane
> from a fresh server load for the active profile (#1, #4) preserving the selected row; removed
> `n`/`s`/idle-`Esc`-back and the old cross-screen events; `t` left unbound for 0016. `Session`/
> `AuthState` gained a client-captured `account: String` (no new wire); centred bounded auth form,
> centred verbatim title `organized koala - <user> @ [<profile>]`, footer flushed to the bottom.
> Tests: new `crates/tui/tests/navigation.rs` (14) + re-pointed `TestBackend` suites. Reviewer
> **approved** + verifier **verified**, both pinned to code-hash
> `bf65aa9612bf1633bf75e64f66a3dfddcfb4aa10` (commit `c8b1217`); coverage 72.96% line (report-only).
> ADR-0010 binds 0015/0016 — those phases inherit and cite it. Fast-forward merged into `main`
> (linear history); **0015 is now unblocked.**
>
> **0016 — TUI detail views + final hotkey scheme — MERGED (Phase 3 / final of 3).**
> The three-part TUI overhaul (0014 → 0015 → **0016**) is complete. A **`tui`-crate-only**,
> presentation-only cycle implementing [ADR-0010](../docs/adr/0010-tui-navigation-and-interaction-model.md)
> §3–§5 with **no new ADR** and **no** `contract`/server/domain delta (reviewer + verifier confirmed
> `crates/contract/**`, `crates/server/**`, `Cargo.toml`/`Cargo.lock` byte-identical to `main`). Two
> things landed: per-field **task & note detail views** — each field its own bordered pane, opened with
> `Enter`, panes cycled with `Tab`/`Shift+Tab` **between editable panes only** (read-only panes stay
> rendered but are skipped; initial/fallback focus on the first editable pane), `e`→edit /
> `Enter`→commit-one-field / two-tiered
> `Esc` — and the **canonical hotkey remap** (`c`(done)→`Space`, `x`(delete)→`d`, `p`(timer)→`t`,
> duration-edit `d`→`T`). Task detail is a new `crates/tui/src/app/task_detail.rs` (`TaskDetail`
> sub-state, a `Screen::Main` sub-mode, not a new `Screen` variant); note detail converted the
> read-only `NotesMode::Viewing` into editable `NotesMode::Detail`; the existing `Event` alphabet was
> reused (no new variants). Commits re-derive from the server response (#1); the note per-field commit
> re-sends the snapshot's untouched field (R5, wire unchanged). A7 contract: an open non-editing detail
> view captures action keys + `Tab` but keeps `?` reachable; two-tiered `Esc` modelled via an
> `Option<String>` edit buffer (its presence is the tier discriminant); all gating folded into the
> existing unified `overlay_capturing_input` predicate (no parallel gate). Tests: new
> `tests/detail.rs` (25, incl. read-only-skip / initial-focus / A6 seams) + re-pinned keymap
> regressions (old `c`/`x`/`p`/duration-`d` asserted gone). Reviewer **approved** + verifier
> **verified** (booted `./ok.sh up`,
> exercised the existing `UpdateTask`/`UpdateNote`/`GetNote` reqwest routes — per-field PATCH leaving
> other fields intact, GetNote+UpdateNote round-trip, 400/401/404/profile-scoping, error contract, OTel
> spans; no server/contract delta) — both re-pinned to code-hash
> `18d6445a05b7834320186551a6ee72e1972c3a08` after a **human-feedback focus-cycling re-entry**
> (read-only panes were `Tab` focus stops — excluded from cycling; the earlier `59ab3172` verdicts
> were voided by the code change); coverage 72.05% line (report-only). The re-entry added one durable
> rule to the `coding-standards` skill (focus traversal skips non-interactive fields, learned 0016);
> no new gotcha/agent change; no new crate. One out-of-scope cosmetic nit
> (stale `Viewing` doc comment in `notes.rs`) filed as
> [`ideas/0003`](./ideas/0003-stale-viewing-doccomment-notes.md) on `main`. At the AI-terminal
> `awaiting-merge` on its branch; awaiting the human's merge.
>
> **0018 — Notes detail multiline Content text area — MERGED.** The Notes detail view's
> **Content** field becomes a **multiline text area that fills the rest of the pane** (panes
> reorder to `Title → Created → Content`). **`tui`-crate-only**, **no** `contract`/server/migration
> change (`Note.content` is already a `String`; the `crates/contract`/`crates/server` diff against
> `main` is empty), implementing [ADR-0011](../docs/adr/0011-multiline-content-editing-keymap.md)
> which **amends [ADR-0010](../docs/adr/0010-tui-navigation-and-interaction-model.md) §4** for the
> multiline pane only. Two new `Event` variants (`Commit`/`Newline`) drive a **context-dependent
> commit keymap**: `Ctrl+S` commits while a text-entry context is active, `Enter` inserts a newline
> **only** in the multiline Content edit (predicate `editing_note_content`) and stays `Submit`
> everywhere else; `Ctrl+C` still wins as Quit; **no terminal enhancement flags** (Shift+Enter
> rejected as terminal-dependent). Content fills via an opt-in `DetailPane.fill` flag →
> `Constraint::Min(3)` + `Wrap { trim: false }` (the task detail path defaults to `Length(3)`,
> unchanged); focus cycling still skips read-only `Created`. Tests live in `tester`'s `TestBackend`
> suite per [ADR-0003](../docs/adr/0003-verification-layering.md) (`tests/detail.rs` 31,
> `tests/keybindings.rs` 38 — pane order, fill, `Enter`→newline, `Ctrl+S` commit via the
> `UpdateNote` path, `Esc` revert, and the Title-still-commits-on-`Enter` regression fork). Reviewer
> **approved** + verifier **verified**, both pinned to code-hash
> `1f9db5c40754afb83857a67b71313fd9d2db7ba8`; coverage 72.47% line (report-only). One out-of-scope
> follow-up filed as [`ideas/0006`](./ideas/0006-note-content-scroll-cursor-affordance.md) (a
> Content scroll/cursor affordance for content exceeding the pane height). No CLAUDE.md gotcha or
> standards/agent change this cycle. Audit clean (all 11 branch commits authored by the human with
> well-formed agent co-author trailers); already current on `main` (no re-rebase needed); DoD
> re-checked green at code-hash `1f9db5c40754afb83857a67b71313fd9d2db7ba8`. Operator-authorised
> ff-merge to `main` at `e4cddb2` (linear, no merge commit); worktree + branch removed.
>
> **0017 — Timer-completion desktop notification — MERGED.** When the
> TUI observes a focus session transition into `Completed`, it fires **exactly one** desktop
> notification (title `"Focus timer"`, body `"Your focus session has ended."`; no sound, no
> actions). **`tui`-crate-only**, **no** `contract`/server/migration change and **no ADR**
> (Decision 2 — the only new state is a transient in-memory marker on `Timer`, #1-blessed; #2/#3
> untouched; inside [ADR-0006](../docs/adr/0006-tui-concurrency-and-responsiveness.md)). An
> injected `Notifier` seam (`crates/tui/src/client/notify.rs`, modelled on the sanctioned
> `Client` boundary, [ADR-0003](../docs/adr/0003-verification-layering.md)): production
> `DesktopNotifier` wraps `notify-rust` (default `zbus` pure-Rust backend, C `dbus` feature off)
> and maps every delivery failure to a silent no-op (A2). A pure fire-once core on `Timer`
> (`notified_for_session` guard + `notify_pending` one-shot signal) detects the Running→Completed
> edge, arms+signals once, re-arms on a new `Running`/`Idle`, and only arms the initial post-login
> `Completed` (A4 — no stale replay); `terminal::run<N: Notifier>` fires after draining each
> worker response (no new request/poll). **A1 confirmed:** the default backend compiled on Ubuntu
> with **no apt package**. Tests: `crates/tui/tests/notifications.rs` (13). Reviewer **approved**
> (no fix-now) + verifier **verified** (13/13 + live `./ok.sh up`), both pinned to code-hash
> `d3fa1fc5b3ed5ac0770085809aac150e25012849`; coverage 72.18% line (report-only). The notification's
> **visual appearance** on a real Ubuntu desktop is the operator's manual confirmation (criterion 4
> / R2; not a capability gap). Two out-of-scope follow-ups filed as ideas on `main`
> ([`ideas/0004`](./ideas/0004-surface-notification-delivery-failures.md),
> [`ideas/0005`](./ideas/0005-move-notification-show-off-poll-loop.md)). Operator-authorised
> ff-merge; the operator's real-Ubuntu visual confirmation (criterion 4) remains a manual check.
>
> **0015 — TUI dialog system — MERGED (Phase 2 of 3).** A **`tui`-crate-only**
> modal framework with **no** `contract`/server/domain change (the
> [ADR-0010](../docs/adr/0010-tui-navigation-and-interaction-model.md) §5 presentation-only boundary,
> confirmed byte-identical by reviewer + verifier). A deep `draw_dialog` helper (one `Dialog` fed by
> all six dialog kinds + the help overlay) floats centred over the tabbed view via `Clear` +
> `centered_rect`; task/note/profile add+edit+delete-confirm and the timer duration edit all moved off
> the 2-row message band into dialogs (state machines/error routing untouched — `last_profile` refusal
> preserved); a `?` help modal (transient `App.help_open`) lists the full hotkey reference and the
> three long `*_CAPTION` constants collapse into one short `FOOTER_CAPTION`; `draw_field` renders a
> focused field's border in `Color::Magenta` (auth + all dialog fields). A single
> `App::overlay_capturing_input()` predicate unifies the scattered text-entry/sub-flow gates (globals
> suppressed while any overlay captures input; two-tiered `Esc`). A tester-flagged fix-now made `?`
> close the help overlay end-to-end (distinct `help_open` param in the 5-arg `map_key`) so the
> advertised `?/Esc: close` affordance works.
> **Footer-fix re-entry (2026-06-27):** operator feedback re-opened 0015 to drop two dead bottom-margin
> rows — the trimmed single-line caption sat too high because `BOTTOM_BAND_ROWS` was still `3` (sized
> for the old wrapping captions). [ADR-0006](../docs/adr/0006-tui-concurrency-and-responsiveness.md)
> §8.3 was **amended** (single flush footer row; the textual `(Esc to cancel)` affordance moved to
> the `?` help modal — the keymap is unchanged, `Esc` still cancels in flight); tui-dev shrank
> `BOTTOM_BAND_ROWS 3 → 1` and dropped the textual affordance from `caption_with_spinner`; tester
> realigned five in-flight asserts + added a single-flush-row pin and a help-modal-documents-Esc-cancel
> pin; one cold-review nit (stale `FOOTER_CAPTION` doc comment) was fixed.
> **Help-modal layout re-entry (2026-06-27):** operator feedback re-opened 0015 again — the `?` help
> dialog crammed `q  quit` onto the `? / Esc  close help` row, and `close help` was not tab-aligned
> to the description column. tui-dev split the malformed `Line` into two properly-tabbed Global rows
> (key
> `q` → `quit`; key `? / Esc` → `close help`) following the sibling `{key:<18}{desc}` layout (desc at
> col 21); tester added a regression pin
> (`help_modal_global_block_lists_quit_and_close_help_as_separate_aligned_rows`, `dialogs` 22 → 23).
> Pure presentation; #1/#2/#3 untouched. The prior verdicts were void once code changed; final
> **approved** + **VERIFIED** at code-hash `00b1cb162b4c8c9bea9ce1e3eb840c0c50ebafcc`; coverage 73.81%
> line (report-only). Fast-forward merged into `main` (`8138f8c..ede004d`, linear history, no merge
> commit) at that attested hash; worktree + branch cleaned up. **0016 (TUI detail views + final
> hotkey scheme) is now unblocked.**
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
