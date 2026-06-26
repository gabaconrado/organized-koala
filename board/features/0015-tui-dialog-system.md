---
id: 0015
title: TUI dialog system — help/add/delete/timer modals, trimmed footer caption, purple focus
type: feature      # feature | chore
status: working         # inbox → planned → ready → working → review → awaiting-merge → merged | blocked
priority: medium    # high | medium | low
parent: null
depends-on: [0014]
branch: feature/0015-tui-dialog-system
worktree: .claude/worktrees/0015-tui-dialog-system
created: 2026-06-26
updated: 2026-06-26
---

## Feature request

**Goal:** Phase 2 of the three-part TUI overhaul (0014 → **0015** → 0016). Introduce a
reusable **modal/dialog framework** and move every create/delete/config sub-flow into it,
trim the footer caption, and add purple focus styling. Builds on the tab shell from
[0014](./0014-tui-layout-shell.md).

**Context (current behaviour to change):**

- There are **no dialogs/modals**. Every add/edit/delete/duration sub-flow renders as **inline
  text in a 2-row "message band"** above the footer (`crates/tui/src/ui/mod.rs`; per-screen
  sub-flow state in `app/task_list.rs`, `app/notes.rs`, `app/profiles.rs`, `app/timer.rs`).
- The **footer caption lists every hotkey** for the current screen (long strings in
  `crates/tui/src/ui/mod.rs`).
- **Focus is shown with a bold border**, not colour (`draw_field`, `crates/tui/src/ui/mod.rs`).

**Surface to build (TUI only — no `contract`/server change):**

- **Reusable dialog framework (point 6).** A centered, floating modal widget (overlay on a
  dimmed/!inert background) that the rest of this phase reuses. While a dialog is open, **global
  hotkeys are suppressed** (typing in a field never fires `q`/`t`/`r`/etc.) and **`Esc` closes
  the dialog** (cancel). This replaces the inline message-band sub-flows.
- **Help modal (points 4 + 6.1).** Pressing **`?`** opens a modal listing the full hotkey
  reference. The footer caption is **trimmed to essentials only**: movement (arrows), tab
  switch (Tab/Shift+Tab), quit (`q`), help (`?`), plus the **server-loading spinner** (the
  existing in-flight spinner + "Esc to cancel" affordance). Everything else moves into the `?`
  modal.
- **Add dialogs (point 6.2).** Adding a **task**, **note**, or **profile** opens a dialog with
  the relevant fields (task: title + description; note: title + content; profile: name) instead
  of editing inline fields above the footer. Submit creates; `Esc` cancels.
- **Delete-confirmation dialogs (point 6.3).** Deleting a task, note, or profile opens a
  **confirmation dialog** (confirm/cancel) rather than the inline confirm prompt.
- **Timer-config dialog (point 6.4).** Configuring the timer duration opens a dialog (single
  duration field) instead of the inline duration-edit in the message band.
- **Purple focus border (point 2).** Any focused field — in the centered auth form (0014) and
  in every dialog — renders its **border in purple** to signal focus, replacing the bold-border
  cue. Applies wherever a field can hold focus.

**Acceptance criteria:**

- [ ] A shared modal widget renders centered/floating; while open, global hotkeys are
      suppressed and `Esc` cancels/closes it.
- [ ] `?` opens a help modal showing the full hotkey reference; the footer caption shows only
      movement + tab switch + quit + help + the in-flight spinner.
- [ ] Add task / add note / add profile each open a dialog (correct fields per entity); submit
      creates the entity; `Esc` cancels without creating.
- [ ] Delete of a task / note / profile opens a confirmation dialog; confirm deletes, cancel
      aborts (preserving today's delete semantics, incl. profile-delete guards from 0012).
- [ ] Timer duration is configured via a dialog (not the inline message band).
- [ ] Focused fields (auth form + all dialogs) show a **purple border**; non-focused fields do
      not.
- [ ] Full `feature` Definition of done: `./ok.sh test | lint | fmt --check` green; `reviewer`
      approved (pinned to `./ok.sh code-hash`); TUI behaviour covered by the `TestBackend` suite
      ([ADR-0003][adr-0003]); `verifier` confirms that suite is green and boots the stack to
      confirm reqwest paths still function (no server/contract delta to exercise).

**Out of scope (later phase / would need an ADR):** task/note **detail views** with per-field
panes and the **complete hotkey remap** (`a`/`e`/`d`/Enter/Space/`t`/`T`/`r` etc.) — those are
0016; this phase opens add/delete/timer dialogs on whatever trigger keys 0014 left in place and
0016 finalizes the scheme. No `contract`/server change; no new domain structure (#3).

<!-- feature: needs an `architect` plan (`plan` skill) writing a `## Plan(s)` block before code. -->

[adr-0003]: ../../docs/adr/0003-verification-layering.md

## Plan(s)

### Plan: TUI dialog system — modal framework, help/add/delete/timer dialogs, purple focus

**ADR:** **None required.** [ADR-0010][adr-0010] (TUI navigation and interaction model)
already settles this phase. §3 is written *for 0015* and binds every invariant this feature
needs — a reusable centred floating modal overlaying the tabbed view; global-hotkey suppression
while an overlay captures input (generalising today's `is_text_entry` gate to one
"input-capturing overlay" rule); a two-tiered `Esc` (close/cancel the overlay, never a hard quit
while one is open); add/delete/timer-config sub-flows moving from the inline message band into
modals; the footer trimmed to essentials with the full reference in a `?` help modal; and a
purple focus border replacing the bold-border cue. §5 makes the **presentation-only** boundary
binding for the whole 0014–0016 arc: no `contract`/wire change (#2), no server change, no new
domain structure (#3) — every dialog renders **existing** DTOs (`Task`/`Note`/`Profile`/
`TimerConfig`/`TimerSession`) over **existing** client methods. The feature request's "TUI only —
no `contract`/server change" assertion is therefore already an accepted, reviewer-enforceable
decision; writing a fresh ADR would only restate §3. ADR-0010 §3 explicitly defers "detailed
design" to "0015's plan" — i.e. **this** block. If implementation discovers it cannot meet
acceptance without crossing a §5 boundary (it should not — this is pure render + pure state over
existing wire), that is an ADR event: the work stops and ADR-0010 is amended before
re-implementation, never engineered around on the branch.

**Confirmed single-agent scope.** This is **`tui-dev`** (production) + **`tester`** (the
`TestBackend` suite) only. No `contract-owner` and no `server-dev` work: there is no new DTO,
no new field, no new route, no changed response shape or status code. `platform-dev` is not
involved (no `ok.sh`/docker/OTel change). The blast radius is contained to `crates/tui/`.

**Approach (tracer-bullet first, then widen).** The existing sub-flow **state** already exists
and is correct — `AddTaskState`/`EditTaskState`, `NotesMode::{Creating,Editing,ConfirmingDelete}`,
`ProfilesMode::{Creating,Renaming,ConfirmingDelete}`, and `Timer.editing` (`DurationEditState`).
0015 does **not** rebuild those state machines; it changes **where they render** (message band →
centred modal), adds the **purple focus cue**, adds a **`?` help overlay**, **trims the footer
caption**, and **tightens the global-hotkey-suppression rule** so an open overlay swallows
`q`/`r`/`?`/tab-switch. The seam (`Event` → `handle_event` → `Dispatch`, `apply_response`, pure
`ui::draw`, pure `map_key`, fake `Client`) is preserved exactly (ADR-0010 forces this; ADR-0003
layer-2). Trigger keys stay **as 0014 left them** (`a` add, `e` edit/rename, `x` delete, `d`
duration, Enter open/submit, `p` timer) — the full hotkey remap (`c`→`Space`, `x`→`d`, `p`→`t`,
detail views) is **0016**, out of scope here.

Tracer bullet = **slice 2 end-to-end for the add-task dialog**: a reusable `draw_dialog` helper +
purple `draw_field`, wired so the *existing* add-task sub-flow renders as a centred modal instead
of the message-band line, with global hotkeys suppressed while it is open and `Esc` cancelling.
Once that one flow is proven through the `TestBackend` harness, every other dialog (note/profile
add, the three delete confirmations, timer-config) reuses the same `draw_dialog` + suppression
rule, and the `?` help modal + footer trim land on top.

**Slices (dependency-ordered):**

1. **[tui-dev] Overlay model + global-hotkey suppression rule (the framework seam).** — files:
   `crates/tui/src/app/mod.rs`, `crates/tui/src/terminal/mod.rs`.
   - Introduce a single predicate on `App` — e.g. `App::overlay_capturing_input()` — that is true
     whenever **any** dialog/overlay owns input: a task add/edit form, a notes
     create/edit/confirm-delete, a profiles create/rename/confirm-delete, the duration edit, **or**
     the new help modal (slice 3). This generalises the scattered `adding.is_some()` /
     `in_sub_flow()` / `editing_duration` checks into one "input-capturing overlay" rule
     (ADR-0010 §3). Confirmation dialogs count as overlays too (they suppress global hotkeys and
     are `Esc`-cancelled), even though they capture no text.
   - In `terminal::map_key`, route on that one predicate: while an overlay captures input, **global
     hotkeys are suppressed** (`q`/`r`/`?`/`p`/`d`/tab-switch do not fire), `Esc` → `Event::Cancel`
     (never `Quit`), and text/`Tab`-field-switch/`Submit` still flow to the focused dialog. This
     keeps the **two-tiered `Esc`** (ADR-0010 §3): `Esc` with an overlay open cancels it; `Esc`
     with none open on a post-auth screen still quits, as today. Preserve the in-flight `Esc`→
     cancel behaviour. **No new trigger keys** beyond `?` (slice 3).
   - `Event` enum: add **`Event::ToggleHelp`** (the `?` key) and **`Event::CloseHelp`** if a
     distinct close is cleaner than reusing `Cancel` (tui-dev's call — prefer reusing `Cancel`
     to keep the alphabet small, per Assumption A2). No other `Event` variants are needed: add/
     edit/delete/duration already have theirs.
   - This slice is the tracer seam: it changes routing, not yet rendering. After it, behaviour is
     unchanged except `?` is recognised and suppression is unified — verified by the keybinding
     suite (slice 6).

2. **[tui-dev] Reusable modal widget + purple focus border + move add/delete/timer sub-flows into
   it.** — files: `crates/tui/src/ui/mod.rs` (the bulk), with the pure render derivations it needs.
   - Add a private **`draw_dialog`** helper in `ui/mod.rs`: a centred floating box (reuse/extend
     `centered_rect`) over the active tabbed view, drawn **after** the main panes so it overlays
     them, carrying a title, a body of fields and/or a confirmation prompt, and an optional inline
     error line. Keep it a **deep, narrow** helper (coding-standards): one function the six
     dialog kinds feed, not six near-duplicate widgets. The background tabbed view is still drawn
     underneath (dimmed/inert is a nice-to-have, not required for acceptance — see Assumption A4).
   - **Purple focus border (ADR-0010 §3, acceptance criterion 6):** change `draw_field` so a
     focused field renders its border in **purple** (`Style::default().fg(Color::Magenta)` on the
     block border) instead of `Modifier::BOLD`. Apply the same focused-border styling to the
     centred **auth** form fields (criterion 6 covers "auth form + all dialogs"). Non-focused
     fields keep the plain border. (Confirmation dialogs have no editable field, so no purple
     border applies there — they show a confirm/cancel prompt.)
   - Re-route rendering so the **existing** sub-flows draw as dialogs, **removing them from the
     message band**:
     - Task **add**/**edit** (`TaskListState.adding`/`.editing`) → a two-field dialog
       (Title/Description) with the focused field bordered purple; inline error in the dialog.
     - Task **delete** (`TaskListState.confirming_delete`) → a **confirmation dialog**
       (confirm/cancel). Today's task delete is a two-step `x`-again affordance; render the
       *armed* state (`confirming_delete.is_some()`) as the confirmation dialog and keep the
       confirm/cancel semantics intact (see Risk R3).
     - Note **add**/**edit** (`NotesMode::Creating`/`Editing`) → two-field dialog (Title/Content);
       note **delete** (`NotesMode::ConfirmingDelete`) → confirmation dialog.
     - Profile **add**/**rename** (`ProfilesMode::Creating`/`Renaming`) → single-field dialog
       (Name); profile **delete** (`ProfilesMode::ConfirmingDelete`) → confirmation dialog
       (preserving the 0012/ADR-0009 last-profile guard: the `last_profile` refusal still surfaces;
       see Risk R4).
     - Timer **duration** (`Timer.editing`) → single-field dialog (Duration minutes), replacing
       the message-band duration overlay (`main_message_line`'s `timer.editing` branch and
       `main_caption_base`'s duration branch).
   - The message-line / caption-base functions (`task_message_line`, `note_message_line`,
     `profile_message_line`, `main_message_line`, `*_caption_base`) lose their sub-flow branches:
     the **message band now shows only the pane's transient status/error message** (e.g. the
     `last_profile` refusal, a list-load error), and the per-sub-flow caption strings
     ("Enter: save Tab: switch field Esc: cancel") move **into the dialog** as the dialog's own
     footer hint. The note **Viewing** mode (read-only single note) is **not** a 0015 concern —
     it is the 0016 detail view — so leave it rendering as it does today (Assumption A6).

3. **[tui-dev] `?` help modal + trimmed footer caption.** — files:
   `crates/tui/src/app/mod.rs` (help-open state + toggle), `crates/tui/src/ui/mod.rs` (render +
   captions), `crates/tui/src/terminal/mod.rs` (`?` key, already added in slice 1).
   - Add a small **help-overlay flag** to `App` (or to `MainState`) — transient process-lifetime
     UI state (#1), e.g. `App.help_open: bool` — toggled by `Event::ToggleHelp` on a post-auth
     screen and closed by `Esc`/`Cancel`. It is mutually sensible with dialogs: opening `?` is a
     post-auth, no-other-overlay action (Assumption A3 — `?` is inert while another dialog is open,
     keeping one overlay at a time and the suppression rule simple). The help flag participates in
     `overlay_capturing_input()` (slice 1).
   - Render the help modal via `draw_dialog` (or a sibling `draw_help`): a centred box listing the
     **full** hotkey reference for the post-auth surface (the keys currently spelled out in
     `TASK_LIST_CAPTION`/`NOTES_CAPTION`/`PROFILES_CAPTION` plus `?`, arrows, tab-switch, `q`).
     Closed with `Esc`/`?`.
   - **Trim the footer caption** (criterion 2) to essentials only: movement (arrows), tab switch
     (`Tab`/`Shift+Tab`), quit (`q`), help (`?`) — plus the **existing** in-flight spinner +
     "(Esc to cancel)" affordance, which stays exactly as `caption_with_spinner` appends it today
     (ADR-0006 §8.3). Replace the three long `*_CAPTION` constants with one short shared caption
     (the per-pane action keys move into the `?` modal). The timer widget bottom-right is
     unchanged.

4. **[tui-dev] Crate docs / module-comment touch-ups.** — files: the doc comments at the heads of
   the touched modules. Keep `ui/mod.rs`'s module doc accurate ("dialogs overlay the panes; the
   message band carries only the transient status message"), and update the `map_key` rustdoc
   list to describe the `?` key and the unified overlay-suppression rule (the current rustdoc
   enumerates the per-tab keys and explicitly notes `t` is left unbound for 0016 — keep that note;
   0015 does not remap). This satisfies `rust.missing_docs` and keeps the ADR-0003 seam
   documentation honest.

5. **[tester] Extend the `TestBackend` suite for the dialog surface.** — files:
   `crates/tui/tests/common/mod.rs` (builders only if needed), and the relevant existing suites:
   `tests/keybindings.rs`, `tests/rendering.rs`, `tests/navigation.rs`, `tests/tasks.rs`,
   `tests/notes.rs`, `tests/profiles.rs`, `tests/timer.rs`, `tests/in_flight.rs`. (New suite file
   `tests/dialogs.rs` only if a flow does not fit an existing file — tester's call.)
   - **Modal rendering:** assert each add/edit/delete/timer/help flow renders a **centred dialog**
     (buffer-snapshot: the dialog title/border appears, centred; the sub-flow text is **no longer**
     in the 2-row message band). Assert the trimmed footer caption shows **only** movement +
     tab-switch + `q` + `?` (+ spinner when pending) and **not** the per-pane action keys.
   - **Global-hotkey suppression:** with a dialog open, assert `q`/`r`/`p`/`d`/`Tab`/`?` do **not**
     fire their global action (the keymap returns the dialog-scoped event or `None`), and that a
     typed character lands in the focused field — directly exercising
     `overlay_capturing_input()` via `map_key`.
   - **Two-tiered `Esc`:** `Esc` with a dialog open cancels the dialog (no `Quit`); `Esc` with no
     overlay on a post-auth screen still quits. `Esc` with a request in flight still cancels the
     request (unchanged).
   - **`?` help modal:** `?` opens it on a post-auth screen, lists the full reference, `Esc`/`?`
     closes it; `?` is inert while another dialog is open (Assumption A3).
   - **Purple focus border:** assert the focused field's border cell carries the magenta fg style
     (auth form + at least one dialog), and a non-focused field's does not (criterion 6).
   - **Behaviour preserved:** the existing add/edit/delete/timer **submit/cancel + chained
     refresh** assertions (the `drive`/`execute` worker-analogue paths) must stay green — moving
     the render does not change the request/response folding. The profile-delete **last-profile
     guard** still surfaces its refusal (Risk R4). Keep the keybinding suite's pin that
     `t`/`n`/`p`/`s` are not tab hotkeys (0016 territory).

**Order / dependencies.** 1 → 2 (2 is the tracer bullet, depends on the slice-1 seam) → 3
(help modal + footer trim, depends on the slice-1 `?` key and the slice-2 `draw_dialog`) → 4 (docs,
trails 1–3) → 5 (tester, after each production slice is in; tester can write red tests against the
seam early but the suite goes green as 2/3 land). All five are within `crates/tui/` and owned by
`tui-dev` (1–4) and `tester` (5); no inter-crate ordering.

**Assumptions (human AFK — smallest change satisfying acceptance, every fork recorded):**

- **A1 — Trigger keys are 0014's, unchanged.** Dialogs open on the keys 0014 left in place
  (`a`/`e`/`x`/`d`/Enter/`p`); the only new key is `?` (help). The full remap (`c`→`Space`,
  `x`→`d`, `p`→`t`, detail views) is **0016** and explicitly out of scope (feature request
  "Out of scope"; ADR-0010 §4). This is the smallest change that satisfies 0015's acceptance.
- **A2 — Reuse `Event::Cancel` to close the help modal** rather than adding a distinct
  `Event::CloseHelp`, keeping the input alphabet minimal; `?` toggles open via `Event::ToggleHelp`.
  (If tui-dev finds a distinct close materially clearer, a `CloseHelp` variant is acceptable — both
  satisfy acceptance; prefer the smaller alphabet.)
- **A3 — One overlay at a time; `?` is inert while a dialog is open.** The suppression rule and the
  two-tiered `Esc` are simplest with a single active overlay. Opening `?` is only allowed when no
  add/edit/delete/timer dialog is capturing input; while one is, `?` is suppressed like the other
  globals. No stacking.
- **A4 — Background dim is optional, not gated.** Acceptance criterion 1 says "overlay on a
  dimmed/inert background"; the *inert* part (global hotkeys suppressed) is gated and implemented
  in slice 1. A literal visual dim of the underlying buffer is a nice-to-have; if a clean ratatui
  dim is cheap (`Clear` + dimmed style behind the box) include it, otherwise the centred overlay
  drawn atop an unmodified background satisfies acceptance. Do **not** block on dimming.
- **A5 — Task delete keeps its two-step affordance, now rendered as a confirmation dialog.** Today
  task-delete arms on first `x` and confirms on second `x` (`confirming_delete`); notes/profiles
  use an explicit `Esc`/Enter confirm dialog. To preserve behaviour and keep the change minimal,
  render the *armed* task-delete state as the confirmation dialog and keep confirm = second `x`
  (or Enter — tui-dev's call to unify on Enter-confirm/`Esc`-cancel across all three deletes if it
  is a clean, behaviour-preserving simplification; the acceptance criterion only requires
  "confirm deletes, cancel aborts"). Record whichever is chosen in the slice Log.
- **A6 — Note read-only "Viewing" mode is left as-is.** It is the 0016 detail view, not a 0015
  dialog. 0015 does not move it into a modal. (Opening a note with Enter still works as 0014 left
  it.)
- **A7 — Help-modal content is derived from the existing caption strings.** The full reference
  listed in `?` is exactly the per-pane keys 0014 documents (the three `*_CAPTION` constants) plus
  the global keys; no new behaviour is documented, so the modal cannot drift from reality. It is
  rendered from constants, asserted by the rendering suite.
- **A8 — Purple = `Color::Magenta`.** "Purple" maps to ratatui's `Color::Magenta` on the border
  fg (the standard 16-colour purple; no truecolor dependency). Applied to focused field borders
  only (auth + dialog fields). Confirmation dialogs have no field, so no purple border there.

**Risks (and containment):**

- **R1 — Suppression-rule regressions.** Unifying the scattered text-entry/sub-flow gates into one
  `overlay_capturing_input()` is the highest-blast-radius change: get it wrong and a global key
  fires inside a dialog (e.g. `q` quits mid-edit) or a dialog swallows a key it should not.
  *Containment:* slice 1 is the tracer seam, behaviour-unchanged except `?`; the keybinding suite
  (slice 5) pins every global key as suppressed-with-overlay and live-without, and the existing
  keybinding pins (no `t`/`n`/`p`/`s` tab hotkeys; `Tab` switches fields in a sub-flow, tabs
  otherwise) must stay green.
- **R2 — Footer caption width / wrap at 80×24.** The trimmed caption + appended spinner +
  "(Esc to cancel)" must still fit the `BOTTOM_BAND_ROWS` band at the 80×24 test viewport
  (learned 0010, ADR-0006 §8.3). Trimming *shortens* the caption so this is lower-risk than 0014,
  but the rendering suite must keep a snapshot assertion that the cancel affordance is not clipped.
- **R3 / R4 — Behaviour drift moving render off the message band.** The submit/cancel + chained
  refresh folding (`apply_*`) and the error routing (`handle_*_error`, including the
  profile `last_profile` refusal and inline `validation_failed`) must be **untouched** — only the
  *render site* of the sub-flow moves. *Containment:* the existing flow/error suites
  (`tests/tasks.rs`, `notes.rs`, `profiles.rs`, `error_branches.rs`, `in_flight.rs`) stay green;
  the inline error now renders inside the dialog, asserted by the rendering suite. This is the
  ADR-0010 §5 presentation-only boundary — a reviewer blocks any wire/server/domain creep.
- **R5 — Scope creep toward 0016.** The temptation to start the remap or detail views while in
  these files. *Containment:* ADR-0010 §4 fences 0016; the plan and the keybinding suite hold the
  line (trigger keys unchanged, `t` stays unbound, no detail view). A reviewer checks 0015 against
  §3 only.
- **R6 — Verifier handshake.** This is a TUI-touching feature with **no server/contract delta**, so
  the live verifier pass (DoD #4) has nothing new to exercise on the wire; per ADR-0003 §4 the
  verifier confirms the `TestBackend` suite **exists and is green** and boots the stack only to
  confirm the reqwest paths still function. The acceptance criteria already state this. *No risk to
  containment, but the plan flags it so the cycle does not expect new live-API assertions.*

[adr-0010]: ../../docs/adr/0010-tui-navigation-and-interaction-model.md

## Log / comments

- [x] 2026-06-26 [orchestrator] Claimed `ready` → `working`. Worktree
  `.claude/worktrees/0015-tui-dialog-system` cut from `main@c9ccd4f` (carries the plan +
  ADR-0010). Session: drive cycle for 0015. tui-dev builds slices 1–4, tester slice 5.
- [x] 2026-06-26 [tui-dev] Slice 1: overlay model + unified global-hotkey-suppression rule.
  Added `App::overlay_capturing_input()` (one predicate true for any add/edit/confirm-delete
  task/note/profile dialog, the duration edit, or the new help overlay), `Event::ToggleHelp`
  (`?`), and a transient `App.help_open` flag. `map_key` now takes the unified `overlay_capturing`
  predicate: globals (`q`/`r`/`?`/`p`/`d`/tab-switch) are suppressed while an overlay captures
  input, `Esc`→`Cancel` (two-tiered Esc preserved; idle post-auth Esc still quits; in-flight
  Esc-cancel preserved). Help reuses `Event::Cancel` to close (Assumption A2). Files:
  `crates/tui/src/app/mod.rs`, `crates/tui/src/terminal/mod.rs`. Build green.
