---
id: 0024
title: Esc does not cancel the Notes/Profiles create·edit·delete dialogs (idle, no request in flight)
type: feature       # feature | chore
status: awaiting-merge  # inbox → planned → ready → working → review → awaiting-merge → merged | blocked
priority: high      # high | medium | low
parent: null
depends-on: []
branch: feature/0024-tui-esc-cancel-notes-profiles-dialogs
worktree: .claude/worktrees/0024-tui-esc-cancel-notes-profiles-dialogs
created: 2026-07-15
updated: 2026-07-15
---

## Feature request

**Operator-flagged bug (confirmed by code trace + a throwaway `TestBackend` probe).** When a
Notes or Profiles modal dialog is open and **no request is in flight**, pressing `Esc` does
**not** close it. The dialog stays open; only `Enter`/submit gets the user out. This affects
**six** dialogs — every create/edit/delete sub-flow in the Notes and Profiles panes. The Tasks
pane and the Notes detail view are **not** affected.

### Affected dialogs

| Pane | Dialog | Handler | Esc closes? |
| --- | --- | --- | --- |
| Notes | Create | `crates/tui/src/app/notes.rs:476` `handle_create_event` | ❌ |
| Notes | Edit | `crates/tui/src/app/notes.rs:494` `handle_edit_event` | ❌ |
| Notes | Delete-confirm | `crates/tui/src/app/notes.rs:512` `handle_delete_event` | ❌ |
| Profiles | Create | `crates/tui/src/app/profiles.rs:220` `handle_create_event` | ❌ |
| Profiles | Rename | `crates/tui/src/app/profiles.rs:237` `handle_rename_event` | ❌ |
| Profiles | Delete-confirm | `crates/tui/src/app/profiles.rs:254` `handle_delete_event` | ❌ |
| Notes | Detail view / detail field-edit | `notes.rs:448` / `notes.rs:436` | ✅ (handles `Cancel`) |
| Tasks | *all* sub-flows | `crates/tui/src/app/task_list.rs` (9 handlers) | ✅ |

### Root cause

`Esc` correctly maps to `Event::Cancel` while a dialog captures input
(`crates/tui/src/terminal/mod.rs:208`). But at the App level, `Event::Cancel` is only acted on
**while a request is in flight**:

```rust
// crates/tui/src/app/mod.rs:452
Event::Cancel if self.is_pending() => { self.cancel_in_flight(); None }
```

When **idle** (the normal case — the user opens a dialog with no request pending), `Cancel`
falls through to the per-pane screen handler, which is expected to reset its own mode. The
**Tasks** pane does exactly this in all of its sub-flow handlers (e.g.
`task_list.rs:898` `Event::Cancel => self.adding = None`). The **Notes** and **Profiles**
create/edit/delete handlers instead match only `Char`/`Backspace`/`Next`/`Prev`/`Submit` with a
`_ => {}` catch-all — silently dropping `Cancel` — while a **misleading comment** (`notes.rs:517`,
`profiles.rs:259`) claims *"Cancel (Esc) is handled by the caller's cancel path,"* a path that
only exists for the in-flight case.

The note **detail** handler (`notes.rs:448` `Event::Cancel => self.mode = NotesMode::List`) shows
the intended pattern; the five idle text-entry/confirm handlers listed above are missing it.

### Why the tests didn't catch it

The existing Notes cancel tests (`crates/tui/tests/notes.rs`) exercise `Event::Cancel` only on
the **in-flight** path (stale-response-after-cancel) and on the **detail** view — never on an
**idle** create/edit/delete dialog. Profiles has no idle-cancel coverage either. So the gap is a
test blind spot, not just a source omission.

### Scoped change (sketch — the real plan is the architect's)

Add an `Event::Cancel => self.mode = <List>` arm to the five idle handlers that lack it
(mirroring the note-detail handler at `notes.rs:448` and the Tasks handlers), and fix the two
misleading doc-comments. `tui`-crate-only: no `contract`/wire (#2), no server, no
domain-structure (#3) change. Because it changes observable interactive behaviour it is a
**`feature`**, not a chore — the `tester` slice must add idle-`Esc`-cancels regression coverage
for each of the six dialogs (owned by `tester`'s `TestBackend` suite per ADR-0003; the live
verifier confirms that suite exists and is green for this TUI-only change).

### Acceptance criteria

- [ ] Pressing `Esc` on an **idle** (no in-flight request) Notes **Create** dialog closes it back
      to the list, discarding the draft; asserted by a `TestBackend` test.
- [ ] Same for the Notes **Edit** and **Delete-confirm** dialogs.
- [ ] Same for the Profiles **Create**, **Rename**, and **Delete-confirm** dialogs.
- [ ] The in-flight `Esc`→cancel-request behaviour is unchanged (existing stale-response tests
      still green).
- [ ] The misleading "handled by the caller's cancel path" comments are corrected.
- [ ] `./ok.sh test | lint | fmt --check` green.

## Plan(s)

### Diagnosis — confirmed

The architect confirmed the root cause by tracing the full `Esc` routing and reading the five
divergent handlers against the two that work:

1. **Key mapping (`terminal/mod.rs:207-214`)** — `KeyCode::Esc` maps to `Event::Cancel` when
   `in_overlay || pending`, else `Event::Quit`. `in_overlay` is `overlay_capturing ||
   editing_duration`, and `overlay_capturing` comes from `App::overlay_capturing_input`
   (`app/mod.rs:349-357`), which is `true` for **all six** target dialogs — Notes
   `in_sub_flow()` covers `Creating | Editing | ConfirmingDelete` (`notes.rs:318-323`) and
   Profiles `in_sub_flow()` is `!matches!(List)`, covering `Creating | Renaming |
   ConfirmingDelete` (`profiles.rs:129-131`). So with any of the six open, `Esc` **does** reach
   the app as `Event::Cancel` (not `Quit`).
2. **App routing (`app/mod.rs:452`)** — `Event::Cancel if self.is_pending()` fires only while a
   request is outstanding; idle `Cancel` falls through to `handle_screen_event` →
   `notes.handle_event` / `profiles.handle_event`.
3. **The bug** — the five idle handlers (`notes.rs` create/edit/delete, `profiles.rs`
   create/rename/delete) match only mutating events with a `_ => {}` catch-all, silently
   dropping `Cancel`. The dialog never resets its mode, so it stays open. The Notes detail
   handler (`notes.rs:448`) and every Tasks sub-flow handler (e.g. `task_list.rs:898`) already
   carry the correct `Event::Cancel => self.mode = <List>` arm — this is the intended,
   already-decided pattern the five stragglers must adopt.

**Borrow-check is already proven.** Three of the five handlers hold a live `&mut self.mode`
binding (`let NotesMode::Creating(form) = &mut self.mode else …`) across the `match`. Assigning
`self.mode = NotesMode::List` inside a `Cancel` arm that does **not** use that binding is exactly
what `notes.rs:448` (`detail` bound, `self.mode` reassigned) and `task_list.rs:898` (`add` bound
via `self.adding`, reassigned) already do and compile under NLL. The two delete-confirm handlers
hold no mode borrow, so the assignment is trivial there.

### Slice 1 — `tui-dev` (source fix)

**Owns:** `crates/tui/src/app/notes.rs`, `crates/tui/src/app/profiles.rs` (source only).

Add the missing `Event::Cancel` arm to the five idle handlers, mirroring `notes.rs:448` and the
Tasks handlers exactly:

- `notes.rs` `handle_create_event` (~476): add `Event::Cancel => self.mode = NotesMode::List,`.
- `notes.rs` `handle_edit_event` (~494): add `Event::Cancel => self.mode = NotesMode::List,`.
- `notes.rs` `handle_delete_event` (~512): convert the `if matches!(event, Event::Submit)` body
  to a `match event` with a `Event::Submit => …` arm and an `Event::Cancel => self.mode =
  NotesMode::List` arm (the delete handler holds no mode borrow, so this is a plain reassign).
- `profiles.rs` `handle_create_event` (~220): add `Event::Cancel => self.mode =
  ProfilesMode::List,`.
- `profiles.rs` `handle_rename_event` (~237): add `Event::Cancel => self.mode =
  ProfilesMode::List,`.
- `profiles.rs` `handle_delete_event` (~254): same shape as the notes delete handler — a `match`
  with `Submit` and `Cancel` arms resetting to `ProfilesMode::List`.

Fix the two misleading doc-comments (`notes.rs:517`, `profiles.rs:259`) that claim *"Cancel (Esc)
is handled by the caller's cancel path"* — replace with a comment reflecting that the handler
itself resets to the list on `Cancel` (matching the detail/Tasks handlers), so the comment no
longer describes a path that only exists for the in-flight case.

**Discarding the draft is inherent, not extra work:** resetting `self.mode` to `List` drops the
owned `NoteForm` / `ProfileForm` / `ConfirmingDelete` payload, so the draft is discarded exactly
as the acceptance criteria require — no separate clear step. The `message` field is left as-is
(consistent with the detail/Tasks cancel, which do not touch `message`).

**Out of scope (do NOT touch):** `terminal/mod.rs` (key mapping is correct), `app/mod.rs`
(routing is correct — the in-flight `Cancel` arm stays exactly as is), any `contract` type, any
server code, the `?` help overlay, and the Tasks/detail handlers that already work.

**Dev gate:** `cargo clippy --lib --bins` green. Per learned-0019, extending only handler match
arms (no new `Client`/`ClientRequest`/`Outcome`/state-field surface) does **not** strand the
tester harness — but the item is not mergeable until Slice 2 lands, and this slice's lib+bins
green is not a DoD clause-1/2 pass on its own.

### Slice 2 — `tester` (regression coverage)

**Owns:** `crates/tui/tests/notes.rs`, `crates/tui/tests/profiles.rs` (and `tests/common/` only
if a helper is genuinely needed; none is anticipated — the existing `enter_notes` / login
helpers suffice).

Add one idle-`Esc`-cancel regression test per dialog (six total), each: drive the app into the
dialog from an **idle** list (no request dispatched, so `is_pending()` is `false`), optionally
type a few chars into the draft, feed `Event::Cancel`, then assert (a) the mode is back to the
list (`NotesMode::List` / `ProfilesMode::List` — assert via the observable state or a rendered
`TestBackend` frame, consistent with how the suite currently asserts mode), (b) **no**
`ClientRequest` was emitted by the cancel (`handle_event(Event::Cancel).is_none()` / no dispatch
scripted on the fake client), and (c) the draft is gone (re-opening the dialog shows an empty
form). Mirror the existing detail-cancel and Tasks-cancel test style already in the suite.

Keep the existing in-flight cancel tests (`stale_delete_response_after_cancel_is_dropped`,
`superseded_response_after_new_request_is_dropped`, `notes.rs:477/513`) green and untouched —
they exercise the `is_pending()` path, which this fix does not alter.

**Order:** Slice 1 and Slice 2 land in the **same cycle**; the item reaches `review` only with
both in. `./ok.sh test | lint | fmt --check` must be green over `--all-targets` (both slices
present).

### Verification

TUI-only feature. Per ADR-0003 the interactive behaviour (Esc→cancel branching) is owned by the
`tester` `TestBackend` suite; the **live `verifier`** confirms that suite exists and is green and
that the server-API / reqwest path is unaffected (this change issues no new requests and touches
no wire shape, so there is nothing new on the live server side to exercise — the verifier states
that explicitly rather than inventing an exercise).

### Risks

- **Borrow-checker (LOW).** Assigning `self.mode` inside a `match` that holds a `&mut self.mode`
  binding — de-risked: identical pattern already compiles at `notes.rs:448` and
  `task_list.rs:898`. If NLL unexpectedly complains, drop the binding before the match arm (bind
  only in the arms that use it), matching the working handlers; no design change.
- **Over-reach into routing (LOW).** Temptation to "fix" the app-level `Cancel` arm to also
  handle the idle case centrally. Rejected: that would diverge from the established per-pane
  pattern (Tasks/detail reset their own mode) and risk swallowing `Cancel` where a pane wants it
  live. Keep the fix local to the five handlers.
- **Help-overlay width (NONE — does not apply).** No hotkey is added or renamed; `Esc` already
  maps to `Cancel`. The `?` help reference lines are untouched, so the learned-0015 / 0019
  help-width-overflow gotcha is not in play. Confirmed.
- **Tester-harness strand (learned-0019) (NONE).** No `Client` trait method,
  `ClientRequest`/`Outcome` variant, or state-struct field is added; only existing handler match
  arms gain a case. The `tests/common/` fake surface is unaffected.

### Assumptions (AFK ambiguity policy)

- **A1 — Cancel resets to the list, discarding the draft.** Matches the note-detail
  (`notes.rs:448`) and Tasks (`task_list.rs:898`) behaviour and the acceptance criteria ("closes
  it back to the list, discarding the draft"). No "are you sure?" on cancel — none of the working
  handlers has one.
- **A2 — `message` is left untouched on cancel.** The detail/Tasks cancel arms do not clear
  `message`; the five fixed handlers follow suit for consistency. (Opening a dialog already
  clears `message` via `begin_*`.)
- **A3 — the two doc-comments are corrected, not deleted.** They currently mislead; the fix
  replaces them with an accurate one-liner rather than removing the comment.
- **A4 — no new `Event` variant or key.** The fix consumes the existing `Event::Cancel`; the
  key-map and `Event` enum are unchanged.
- **A5 — smallest change.** Only the five handlers + two comments in `src/`, plus six tests. No
  refactor of the handler shape beyond what the new arm requires (the two delete handlers become
  `match` blocks, the minimal change to host a second arm).

### ADR determination — NOT needed

This change makes **no contract-shaping or scope decision**. It is `tui`-crate-only: **no**
`contract`/wire change (hard-constraint #2), **no** server change, **no** domain-structure change
(#3), **no** cross-profile access, **no** client-side state, **no** auth change. It does not
introduce a new design decision — it aligns five divergent handlers with the **already-decided**
`Event::Cancel => reset-to-list` pattern that the note-detail and Tasks handlers already embody.
It is a `feature` (not a chore) purely because it restores observable interactive behaviour and
therefore requires the tester regression slice, but "changes behaviour" is not "shapes a
contract/scope" — the ADR trigger. **No ADR is written or amended for 0024.**

## Log / comments

- 2026-07-15 [orchestrator] Filed at operator request after a confirmed investigation (code
  trace of the `Esc`→`Cancel` routing in `terminal/mod.rs` + `app/mod.rs`, cross-checked against
  the Tasks handlers, and empirically confirmed with a throwaway `TestBackend` probe that failed
  for both the Notes-create and Profiles-create dialogs — probe removed). Awaiting `architect`
  planning. Operator will kick off the fix manually.
- 2026-07-15 [architect] Planned. Confirmed the diagnosis end-to-end by reading the full
  `Esc`→`Cancel` route (`terminal/mod.rs:207`, `overlay_capturing_input` at `app/mod.rs:349`, the
  idle fall-through at `app/mod.rs:452`) and the five divergent handlers against the working
  note-detail (`notes.rs:448`) and Tasks (`task_list.rs:898`) handlers. Wrote the plan: Slice 1
  `tui-dev` (five `Event::Cancel` arms + two comment fixes in `src/app/notes.rs` +
  `src/app/profiles.rs`), Slice 2 `tester` (six idle-Esc-cancel regression tests in
  `tests/notes.rs` + `tests/profiles.rs`). **No ADR** — `tui`-only, no contract/wire (#2), no
  server, no domain-structure (#3); the fix adopts an already-decided pattern, making no new
  contract/scope decision. Help-overlay width gotcha (learned-0015/0019) does **not** apply (no
  hotkey added/renamed; `Esc` already maps to `Cancel`). Grill skipped — no design fork; the fix
  is contained and the pattern is pre-existing. Set `planned` → `ready`.
- 2026-07-15 [orchestrator] Claimed `ready`->`working`; cut worktree
  `.claude/worktrees/0024-tui-esc-cancel-notes-profiles-dialogs` + branch
  `feature/0024-tui-esc-cancel-notes-profiles-dialogs` from `main@97fc2a4` (carries the plan).
  Session `drive-0024`. Branch copy is now authoritative (home #2).
- 2026-07-15 [tui-dev] Slice 1 (source fix) done. Added `Event::Cancel => self.mode = <List>`
  arms to the five idle handlers that dropped it: `notes.rs` `handle_create_event` +
  `handle_edit_event` (each gained one match arm) and `handle_delete_event` (converted its
  `if matches!(event, Event::Submit)` body to a `match` with `Submit` + `Cancel` arms);
  `profiles.rs` `handle_create_event` + `handle_rename_event` (each one arm) and
  `handle_delete_event` (same `if`→`match` conversion). Corrected the two misleading
  "handled by the caller's cancel path" comments (`notes.rs`/`profiles.rs` delete handlers) to
  state the handler itself resets to the list on `Cancel`. Out-of-scope surfaces untouched:
  `terminal/mod.rs`, `app/mod.rs` routing (the in-flight `Cancel if self.is_pending()` arm is
  unchanged), the `Client` trait, `ClientRequest`/`Outcome`, all state-struct fields, `contract`,
  server, and the `?` help overlay — no hotkey added/renamed, so no tester-harness strand
  (learned-0019) and no help-width concern (learned-0015/0019). `./ok.sh fmt` clean; `./ok.sh lint`
  (`--all-targets`) clean; `./ok.sh build` green. Awaiting Slice 2 (`tester` regression coverage).
- 2026-07-15 [tester] Slice 2 (regression coverage) done. Added six idle-`Esc`-cancel `TestBackend`
  tests, one per affected dialog — three in `crates/tui/tests/notes.rs`
  (`esc_cancels_idle_create_dialog_discarding_draft`, `esc_cancels_idle_edit_dialog_discarding_draft`,
  `esc_cancels_idle_delete_confirm_without_deleting`) and three in `crates/tui/tests/profiles.rs`
  (`esc_cancels_idle_create_dialog_discarding_draft`, `esc_cancels_idle_rename_dialog_discarding_draft`,
  `esc_cancels_idle_delete_confirm_without_deleting`). Each drives the app into the dialog's idle open
  state (no request in flight), feeds `Event::Cancel`, and asserts: (a) `handle_event(Cancel)` returns
  `None` and the recorded call count is unchanged (no `Create*`/`Update*`/`Delete*` request emitted);
  (b) the mode returns to `NotesMode::List` / `ProfilesMode::List`; (c) the draft/confirm is discarded
  — create/rename re-open to an empty/re-prefilled form, delete leaves the note/profile in place. The
  in-flight cancel tests (`stale_delete_response_after_cancel_is_dropped`,
  `superseded_response_after_new_request_is_dropped` in both suites) and the detail-view cancel test
  are untouched and remain green — the `is_pending()` path is unaltered. No harness update needed:
  the fix touched no `Client`/`ClientRequest`/`Outcome`/state-field surface, so `tests/common/mod.rs`
  was not stranded (learned-0019 confirmed not to apply). `./ok.sh fmt --check` clean; `./ok.sh test`
  green (all suites, `--all-targets`); `./ok.sh lint` (`--all-targets`) clean.
- 2026-07-15 [reviewer] Cold review complete. Gates green (`./ok.sh test | lint | fmt --check` all
  exit 0; the six new idle-Esc-cancel tests pass and genuinely fail against the pre-fix source).
  Correctness confirmed: five `Event::Cancel => self.mode = <List>` arms drop the owned form/confirm
  payload (draft discard inherent); the two `if`→`match` delete-confirm conversions preserve the
  Submit path; the in-flight cancel path (`app/mod.rs:452`) is untouched (empty diff on
  `app/mod.rs` and `terminal/mod.rs`). `tui`-crate-only — empty diff on `contract`/`server`/`app/mod.rs`/
  `terminal/mod.rs`/`tests/common`; no contract/wire (#2), server, domain-structure (#3), or auth
  (#5) change; TUI stays stateless (#1). No hotkey added/renamed → help-overlay width gotcha N/A
  (help lines untouched). No ADR needed. No out-of-scope nits.
  **REVIEW-STATUS: approved** — code-hash `fd2bd1508506786d0127a1005317a4852201351d` (last code
  commit `79467a9`).
- 2026-07-15 [verifier] **verified** — code-hash `fd2bd1508506786d0127a1005317a4852201351d`.
  Step 1 (owns the interactive behaviour per ADR-0003): `./ok.sh test` full workspace green; the six
  `esc_cancels_idle_*` regression tests ran and passed (notes 16, profiles 19), in-flight cancel
  tests still green. Step 2 (live boot smoke via hermetic `./ok.sh verify-boot`, docker 29.5.3):
  stack booted (postgres→migrate one-shot clean→server `/healthz` 200→otel healthy); ran live
  against :8080 — register 201, profiles CRUD + profile-scoping (#4) isolation, note create 201
  with exact `{id,title,content,created_at}` shape, error contract `401 unauthenticated` /
  `400 validation_failed` both match `{code?,message}`. Hermetic `down --volumes` teardown fired;
  no lingering `deploy-*` containers / `deploy_postgres-data` volume. TUI-only diff did not break
  the live path; nothing new server-side to exercise (empty server/contract diff), Esc→Cancel
  behaviour owned by the TestBackend suite.

## Summary

coverage: 73.25%

Fixes a confirmed TUI bug: on an **idle** Notes/Profiles create·edit·delete dialog (no request in
flight) `Esc` did not close it. The five idle per-pane handlers matched only the mutating events
(`Char`/`Backspace`/`Next`/`Prev`/`Submit`) with a `_ => {}` catch-all that silently dropped
`Event::Cancel`, while a misleading comment claimed cancel was "handled by the caller's cancel
path" — a path that only fires `if self.is_pending()` (`app/mod.rs:452`). The note-detail handler
and every Tasks sub-flow handler already carried the correct `Event::Cancel => reset-to-list` arm;
this cycle aligns the five stragglers with that already-decided pattern.

- **Source fix (`tui`-crate-only).** Five `Event::Cancel => self.mode = <List>` arms added:
  in `notes.rs`, `handle_create_event` and `handle_edit_event` (one arm each) plus
  `handle_delete_event` (its `if matches!(event, Event::Submit)` body converted to a `match` with
  `Submit`/`Cancel` arms); in `profiles.rs`, `handle_create_event` and `handle_rename_event` (one
  arm each) plus `handle_delete_event` (same `if`→`match` conversion). Resetting `self.mode` to
  `List` drops the
  owned `NoteForm`/`ProfileForm`/`ConfirmingDelete` payload, so the draft is discarded inherently
  (no separate clear); `message` is left untouched, matching the detail/Tasks cancel arms. The two
  misleading doc-comments were corrected to state the handler resets to the list on `Cancel`.
- **No routing/wire/domain change.** The in-flight `Event::Cancel if self.is_pending()` arm
  (`app/mod.rs`) and the key mapping (`terminal/mod.rs`) are untouched; no `contract`/wire (#2), no
  server, no domain-structure (#3) change; TUI stays stateless (#1). No hotkey added or renamed, so
  the help-overlay width gotcha (learned-0015/0019) does not apply, and no `Client`/`ClientRequest`/
  `Outcome`/state-field surface changed, so the tester harness (learned-0019) was not stranded.
  No ADR — the fix adopts an existing pattern, making no contract/scope decision.
- **Regression coverage.** Six `TestBackend` tests, one per affected dialog — three in
  `crates/tui/tests/notes.rs` and three in `crates/tui/tests/profiles.rs`. Each drives the app into
  the dialog's idle-open state, feeds `Event::Cancel`, and asserts (a) `handle_event(Cancel)`
  returns `None` with no `Create*`/`Update*`/`Delete*` request emitted, (b) the mode returns to
  `NotesMode::List`/`ProfilesMode::List`, and (c) the draft/confirm is discarded. The in-flight
  cancel tests remain untouched and green. The new tests genuinely fail against the pre-fix source.

DoD (`feature` track): clauses 1–3 green (`./ok.sh test | lint | fmt --check`); clause 4 the
`verifier` confirmed the `TestBackend` suite exists and is green and live-boot-smoked the unchanged
server/reqwest path (nothing new server-side to exercise on a TUI-only diff); clause 5 N/A (no
contract change, no new gotcha); clause 6 reviewer **approved** + clause-4 verifier **verified**,
both pinned to code-hash `fd2bd1508506786d0127a1005317a4852201351d` (last code commit `79467a9`).
- 2026-07-15 [orchestrator] Step-7 freshen: rebased branch onto `main@f7f6ccf` (governance-only
  advance — eng-manager learnings + handoff + dashboard). `./ok.sh code-hash HEAD` unchanged at
  `fd2bd1508506786d0127a1005317a4852201351d` (== attested hash), so the code is byte-identical and
  the reviewer-approved + verifier-verified attestations **carry forward untouched** (no
  relabelling). Re-ran gates on the rebased tree: `./ok.sh fmt --check | lint | test` all green.
  Board-only commit — does not retrigger review. Set `review`→`awaiting-merge`. **DoD (feature,
  7/7) satisfied; terminal for the AI cycle — awaiting human merge.**
