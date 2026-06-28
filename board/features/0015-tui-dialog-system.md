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
  **Task-delete (A5): kept the two-step `x`-again affordance, now rendered as a confirmation
  dialog** — the armed `confirming_delete` state captures input (globals suppressed) but a second
  `x` still confirms and `Esc` cancels; behaviour-preserving, the smallest change.
- [x] 2026-06-26 [tui-dev] Slices 2+3 (committed together — the render-layer changes are
  interdependent): reusable `draw_dialog` widget + purple focus border + `?` help modal + trimmed
  footer. Added a deep `draw_dialog` helper (one `Dialog` struct fed by all six dialog kinds + the
  help overlay) drawn after the panes via `Clear` + `centered_rect`; `draw_field` now renders a
  **purple** (`Color::Magenta`) border on a focused field (auth form + all dialog fields) instead
  of `Modifier::BOLD`. Moved task add/edit/delete-confirm, note add/edit/delete-confirm, profile
  add/rename/delete-confirm, and the timer duration edit OUT of the 2-row message band and INTO
  dialogs; the message band now shows only the pane's transient status/error (incl. the
  `last_profile` refusal, preserved). Note "Viewing" mode left as-is (A6). Added the `?` help
  overlay (`draw_help`) listing the full hotkey reference, toggled by `Event::ToggleHelp`,
  participating in `overlay_capturing_input()`. Replaced the three long `*_CAPTION` constants with
  one short `FOOTER_CAPTION` (movement, tab switch, `?`, `q`) + the unchanged spinner/cancel
  affordance. File: `crates/tui/src/ui/mod.rs`. Build + production-target lint + fmt green.
- [x] 2026-06-26 [tui-dev] Slice 4: doc / module-comment touch-ups. `ui/mod.rs` module doc and
  the message-line/dialog rustdoc describe dialogs-overlay-panes + message-band-carries-only-status;
  `map_key` rustdoc describes the `?` key and the unified overlay-suppression rule and keeps the
  note that `t` stays unbound for 0016 (0015 does not remap); `app/mod.rs` module doc names the
  unified `overlay_capturing_input()` overlay model. Files: `crates/tui/src/app/mod.rs`,
  `crates/tui/src/ui/mod.rs`, `crates/tui/src/terminal/mod.rs`. Production build/lint/fmt green
  (existing `tests/` call the old `map_key` signature — those land in tester's slice 5).
- [x] 2026-06-26 [tester] Slice 5: extended the `TestBackend` suite for the dialog surface. Fixed the
  broken 4-arg `map_key(screen, overlay_capturing, editing_duration, key)` call sites (`keybindings.rs`
  `map`/`map_editing` now derive `overlay_capturing` from a new `common::screen_overlay_capturing`;
  `navigation.rs` passes `app.overlay_capturing_input()`). Added `common` builders/helpers:
  `screen_overlay_capturing`, `render_buffer`, `row_fg_count`, and screen builders for the
  task/note/profile delete-confirm + note create/edit dialogs. New `tests/dialogs.rs` (16 tests):
  centred-dialog rendering for add/edit/delete/timer/help (title + magenta border, centred, NOT in the
  2-row message band); trimmed-footer asserts (movement + tab-switch + `?` + `q`, no per-pane keys);
  global-hotkey suppression end-to-end through `map_key` (a typed char — incl. `q`/`r`/`p`/`d`/`?` —
  lands in the focused field, no global fires); two-tiered `Esc` (cancels an open dialog, still quits
  idle post-auth, still cancels an in-flight request); `?` help modal (opens, lists the full
  reference, closes on `Esc`); purple `Color::Magenta` focus border on the auth form + a dialog field
  (focused vs non-focused row contrast); and a behaviour-preserved add-task submit→chained-refresh.
  Extended `keybindings.rs` with the unified suppression pins (every dialog kind across the three
  tabs, the task-delete `x`-again confirm, `?`-opens-only-when-idle, Esc-cancels-in-every-dialog).
  Updated the stale footer-caption assertions in `rendering.rs`/`tasks.rs`/`timer.rs`/`navigation.rs`
  to the trimmed `FOOTER_CAPTION`, and the armed-task-delete render assertion to the new confirmation
  dialog. **Finding flagged for review (not worked around — no src edit):** a live `?` keypress is
  suppressed by the open help overlay at the keymap, so today only `Esc` closes help via the keyboard,
  yet `draw_help`'s footer hint advertises `?/Esc: close`. The app core *does* fold `Event::ToggleHelp`
  into a close (`handle_event`), so the gap is purely the keymap not emitting it while an overlay
  captures input. Pinned by `question_mark_keypress_is_suppressed_while_help_is_open` +
  `help_modal_toggle_close_event_is_supported_by_the_core`. `./ok.sh test` 380 passed / 0 failed;
  `./ok.sh lint` clean (`--all-targets`); `./ok.sh fmt --check` clean.
- [x] 2026-06-26 [tui-dev] Fix-now (tester's pre-review finding): make `?` close the help overlay
  so the advertised `?/Esc: close` affordance works. Threaded `help_open` distinctly into `map_key`
  and special-cased `?` to **toggle** help — it opens from an idle post-auth screen and closes
  while the help overlay is the active overlay (the core already folds `Event::ToggleHelp` into a
  close). `?` stays suppressed while a *non-help* dialog (add/edit/confirm/duration) captures input
  (A3); the two-tiered `Esc` and all other suppression are unchanged. New signature:
  `map_key(screen, overlay_capturing, help_open, editing_duration, key)`. File:
  `crates/tui/src/terminal/mod.rs`. Production build/lint/fmt green; the two `map_key` call sites
  in the test suite and the two pinning tests
  (`question_mark_keypress_is_suppressed_while_help_is_open`,
  `help_modal_toggle_close_event_is_supported_by_the_core`) need updating by tester for the new
  arity + flipped `?`-closes-help behaviour.
- [x] 2026-06-26 [tester] Slice 5 follow-up: updated the suite for tui-dev's `4b1a1b7` fix. All six
  `map_key` call sites moved to the 5-arg `(screen, overlay_capturing, help_open, editing_duration,
  key)` form (`keybindings.rs` `map`/`map_editing` pass `help_open=false`; `navigation.rs` and the
  `dialogs.rs` `press` helper + in-flight-Esc call pass `app.help_open()`). Re-pinned the
  `?`-closes-help behaviour: `question_mark_keypress_is_suppressed_while_help_is_open` →
  `question_mark_closes_help_while_open` (a second `?` now closes via the keymap end-to-end), and
  `help_modal_toggle_close_event_is_supported_by_the_core` → `help_close_is_reachable_from_the_keyboard_via_question_mark`
  (asserts `map_key` returns `Some(ToggleHelp)` while help is open, then the core folds it into a
  close). Kept the A3 pin `question_mark_is_inert_while_another_dialog_is_open` (a non-help dialog
  open ⇒ `?` is typed text, no help). `./ok.sh test` 380 passed / 0 failed; `./ok.sh lint` clean
  (`--all-targets`); `./ok.sh fmt --check` clean.
- [x] 2026-06-26 [reviewer] Cold review — `REVIEW-STATUS: approved`
  pinned to code-hash `b9884943f36f3ac6c9d56fd2be46e31057a9060a` (last code commit `22c7e92`).
  Gates green (test 380 pass / 0 fail, lint `--all-targets` clean, fmt clean). ADR-0010 §5
  presentation-only boundary holds (`contract`/`server` untouched; diff is
  `crates/tui/src/{app,terminal,ui}/mod.rs` plus tests only). #1 statelessness preserved
  (`help_open` transient; state machines/error routing untouched —
  `last_profile` guard intact). No 0016 creep (`t`/`Space`/detail views absent; only `?` is new).
  All six acceptance criteria met; `?`-closes-help fix verified. No fix-now findings, no
  out-of-scope ideas.
- [x] 2026-06-26 [verifier] Live verify — **VERIFIED** pinned to code-hash
  `b9884943f36f3ac6c9d56fd2be46e31057a9060a` (head commit `ad0dd70`). No-delta premise confirmed
  (`contract`/`server`/`migrations`/`deploy`/`ok.sh` byte-identical to main; diff is
  `crates/tui/src/{app,terminal,ui}/mod.rs` plus tui tests). Clause-4 part 1: `TestBackend` suite
  green — `tests/dialogs.rs` 21/0 covering all six acceptance criteria + supporting suites all
  0-fail. Clause-4 part 2: `./ok.sh up` booted clean (postgres healthy, one-shot `migrate` exit 0
  — no history conflict, server healthy on :8080); exercised live the
  reqwest/API paths the dialogs drive — auth register/login (+401 invalid_credentials), profiles list,
  tasks/notes create(201)/list/delete(204), timer config(GET/PUT)+session start/stop, error contract
  (400 validation_failed, 401 unauthenticated), profile-scoping (404 not_found, no cross-profile read),
  OTel per-handler spans. All shapes/status/error-contract/scoping unchanged. Left `deploy_postgres-data`
  intact (no `down -v`).

## Summary

Phase 2 of the three-part TUI overhaul (0014 → **0015** → 0016): a **`tui`-crate-only** dialog
system, with **no** `contract`/server/domain change (the presentation-only boundary binds per
[ADR-0010][adr-0010] §5; reviewer + verifier both confirmed `contract`/`server`/`migrations`/
`deploy`/`ok.sh` byte-identical to `main`).

What shipped (on the branch, `crates/tui/` only):

- **Reusable dialog framework.** A deep, narrow `draw_dialog` helper in `ui/mod.rs` — one
  `Dialog` struct fed by all six dialog kinds + the help overlay — drawn after the panes via
  `Clear` + `centered_rect` so it floats centred over the tabbed view, carrying a title, fields
  and/or a confirm/cancel prompt, and an optional inline error line.
- **`?` help modal + trimmed footer.** A transient `App.help_open` flag toggled by
  `Event::ToggleHelp` (`?`) renders a centred help modal listing the full hotkey reference; the
  three long `*_CAPTION` constants collapse into one short `FOOTER_CAPTION` (movement, tab switch,
  `?`, `q`) plus the unchanged in-flight spinner + "(Esc to cancel)" affordance.
- **Add / delete / timer dialogs.** Task add/edit (title+description), note add/edit
  (title+content), profile add/rename (name), the three delete-confirmations, and the timer
  duration edit all moved **out of the 2-row message band and into dialogs**; the message band now
  carries only the pane's transient status/error (the `last_profile` refusal preserved). State
  machines + submit/cancel + chained-refresh + error routing are untouched — only the render site
  moved.
- **Purple focus border.** `draw_field` renders a focused field's border in `Color::Magenta`
  (replacing `Modifier::BOLD`), applied to the auth form fields + all dialog fields; non-focused
  fields keep the plain border.
- **Unified suppression rule + two-tiered Esc.** A single `App::overlay_capturing_input()`
  predicate replaces the scattered `adding.is_some()`/`in_sub_flow()`/`editing_duration` gates: while
  any overlay captures input the globals (`q`/`r`/`?`/`p`/`d`/tab-switch) are suppressed and `Esc`
  cancels the overlay; `Esc` with no overlay on a post-auth screen still quits, and in-flight
  `Esc`-cancel is preserved.
- **`?`-closes-help fix (in-cycle).** `map_key` gained a distinct `help_open` param (5-arg
  `(screen, overlay_capturing, help_open, editing_duration, key)`) so a second `?` closes the help
  overlay end-to-end through the keymap — the advertised `?/Esc: close` affordance now works; `?`
  stays suppressed while a non-help dialog captures input (A3).

Agents involved: **tui-dev** (slice 1 overlay/suppression seam, slices 2+3 dialog framework + help
modal + footer trim, slice 4 docs, + the `?`-closes-help fix-now) and **tester** (slice 5
`TestBackend` suite — new `tests/dialogs.rs` + extensions across the existing suites, plus the
follow-up updating the suite for the 5-arg `map_key` and flipped `?`-closes-help behaviour).

Gate results: `./ok.sh test` **380 passed / 0 failed**; `./ok.sh lint` clean (`--all-targets`);
`./ok.sh fmt --check` clean. **reviewer approved** + **verifier VERIFIED**, both pinned to
code-hash `b9884943f36f3ac6c9d56fd2be46e31057a9060a`.

coverage: 73.80% line (the headline `TOTAL` line-coverage from a fresh `./ok.sh coverage` in the
worktree after the footer-fix re-entry; docker plus the throwaway test Postgres booted cleanly. The
footer fix realigned five in-flight asserts and added two pins but left the headline `TOTAL`
line-coverage unchanged at 73.80%). Report-only — never a gate.

- [x] 2026-06-26 [orchestrator] Step-7 freshen: rebased onto `main@a739006` (docs/handoff +
  dashboard advance only). `./ok.sh code-hash HEAD` = `b9884943f36f3ac6c9d56fd2be46e31057a9060a`,
  **unchanged** from the attested hash — code byte-identical, so the reviewer `approved` + verifier
  `VERIFIED` verdicts carry forward untouched (no relabelling). Re-ran gates on the rebased tree:
  test 0 failures (32 `test result: ok`), lint clean, fmt --check clean. Branch is current on `main`.
  → `review` → `awaiting-merge`. Cycle terminal; human performs the ff-merge.
- [x] 2026-06-26 [human] The footer (hotkey caption) sits too high — 2 blank rows of bottom
  margin in the terminal; want 0. Root cause (orchestrator triage): 0015 trimmed the caption to
  a single non-wrapping line but left `BOTTOM_BAND_ROWS = 3` (sized for the OLD wrapping captions
  per ADR-0006 §8.3 / learned 0010), so the top-aligned caption leaves 2 dead rows. This is a
  0015 loose end (the trim created it) and completing it furthers ADR-0010 §2 "tight footer".
  Folded back into 0015 (operator decision): `awaiting-merge` → `working`; prior approved+verified
  verdicts VOID once code changes (code-hash will differ); re-runs review + verify. tui-dev shrinks
  the band to fit the single-line caption + timer widget; tester pins "no blank trailing rows".
- [x] 2026-06-26 [tui-dev] Footer fix per ADR-0006 §8.3 (amended 2026-06-26, on disk in this
  worktree): the in-flight spinner still appends to the stable caption and `Esc` still cancels in
  flight (keymap unchanged), but the textual `(Esc to cancel)` affordance is removed from
  `caption_with_spinner` and documented in the `?` help modal instead (`draw_help` gains an
  `Esc cancel an in-flight / loading request` line). With the affordance gone the footer is one
  flush row: `BOTTOM_BAND_ROWS 3 → 1` (doc comment rewritten — no stale 3-row/wrapping rationale).
  Pending caption now 60 cols (was 76), so no wrap; on the rare wide-timer state the trailing
  spinner glyph may clip at the row edge — accepted per the single-row decision (no wrap-growth).
  Pure `tui::ui` presentation; #1/#2/#3 untouched. File: `crates/tui/src/ui/mod.rs`. Production
  build + `cargo clippy --lib --bins -p tui` + `fmt --check` green. The `--all-targets` suite is
  red on the stale `Esc to cancel` footer asserts (tester's next): `rendering.rs`
  `auth_in_flight_appends_spinner_without_replacing_the_caption`,
  `task_list_in_flight_appends_spinner_without_replacing_the_caption`,
  `offline_retry_in_flight_appends_spinner_without_replacing_the_caption`; `tasks.rs`
  `delete_in_flight_renders_spinner_and_keeps_caption`; `timer.rs`
  `in_flight_appends_a_spinner_without_replacing_the_caption` (its comment cites the old wrap).
  The negative pin `timer.rs idle_caption_has_no_spinner_or_cancel_affordance` still passes.
- [x] 2026-06-26 [tester] Footer-fix test re-entry: realigned the five stale in-flight asserts to
  the amended ADR-0006 §8.3 — `rendering.rs` (`auth_/task_list_/offline_retry_in_flight_…`),
  `tasks.rs` `delete_in_flight_renders_spinner_and_keeps_caption`, `timer.rs`
  `in_flight_appends_a_spinner_without_replacing_the_caption` — each now asserts the in-flight
  render APPENDS the spinner glyph + KEEPS the base caption and that `"Esc to cancel"` is NOT in
  the footer; module doc in `rendering.rs` rewritten. Added two positive pins: `navigation.rs`
  `footer_is_a_single_flush_row_with_no_blank_trailing_rows` (caption AND timer on the terminal's
  last row, last row non-empty — the operator's zero-bottom-margin ask) and `dialogs.rs`
  `help_modal_documents_that_esc_cancels_an_in_flight_request` (affordance's new home). Test-only;
  no `src/` touched. `./ok.sh test | lint | fmt --check` all green (tui suites: dialogs 22,
  navigation 21, rendering 21, tasks 17, timer 21). Commit `a714a83`.
- [x] 2026-06-26 [verifier] Live re-verify at code-hash `542f19aa…` — **VERIFIED**. TestBackend
  TUI suite green (ADR-0003 owner of footer/dialog/keybinding behaviour); booted the stack
  (`./ok.sh up`, migrate exit 0 — no 0011 conflict) and smoke-exercised the reqwest paths: auth
  register/login (201/200), error contract `{code,message}` (401 invalid_credentials/unauthenticated,
  400 validation_failed, 404 not_found), tasks CRUD (201/200/204 + closed_at on done), notes
  (201/200), profiles, timer config+session (idle/running/idle), profile-scoping (#4) confirmed,
  OTel spans live (`service.name: organized-koalad`). Reopened diff is pure presentation — wire
  surface byte-identical to the earlier VERIFIED run; nothing regressed. Stack down clean.
- [x] 2026-06-26 [reviewer] Re-review at code-hash `542f19aa…` — `REVIEW-STATUS: changes-requested`.
  Mechanical gate green; behaviour change matches amended ADR-0006 §8.3; #1/#2/#3 hold; new tests
  pin the behaviour correctly. ONE blocking finding: stale `FOOTER_CAPTION` doc comment
  (`crates/tui/src/ui/mod.rs:31-33`) still described the removed `(Esc to cancel)` affordance +
  old multi-row band (adjacent doc comments were updated in `a934599`, this one missed). Fix-now.
- [x] 2026-06-26 [tui-dev] Fixed the reviewer's finding: rewrote the `FOOTER_CAPTION` doc comment
  to describe current behaviour (single non-wrapping line; spinner glyph only — no textual
  affordance; single flush row `BOTTOM_BAND_ROWS == 1`; Esc-cancel affordance in the `?` help
  modal; ref → ADR-0006 §8.3 amended 2026-06-26). Comment-only (4 ins / 3 del, all within the
  `///` block — no logic/value change). `./ok.sh test | lint | fmt --check` green. Commit
  `cf66137`; new code-hash `b4bc0cdb93086adb620ffbe66bc5d66a524e4ffd`.
- [x] 2026-06-26 [reviewer] Re-review of `cf66137` — **`REVIEW-STATUS: approved`**, pinned to
  code-hash `b4bc0cdb93086adb620ffbe66bc5d66a524e4ffd` (commit `cf66137`; HEAD `95314d1`
  Board-only). Single prior finding genuinely resolved (comment now matches the code exactly).
  Confirmed comment-only vs. `542f19aa` (only `///` lines changed; `FOOTER_CAPTION` value
  unchanged); no `contract`/wire (#2), no domain (#3), TUI stateless (#1), no `#[allow]`. Lint +
  fmt clean; `./ok.sh test` green with DB-backed integration tests serialized (`RUST_TEST_THREADS=1`).
  No code findings. Out-of-scope env note (not blocking): default parallel `./ok.sh test` is flaky
  under throwaway-Postgres pool contention (intermittent `register → 500 internal` in server DB
  suites) — a `platform-dev` infra concern; filed as a `board/ideas/` follow-up.
- [x] 2026-06-26 [verifier] Re-attest at code-hash `b4bc0cdb93086adb620ffbe66bc5d66a524e4ffd`
  (commit `cf66137`; HEAD `95314d1` Board-only) — **VERIFIED**. Independently confirmed the sole
  source delta since the live pass is the `///` doc comment (`git diff a714a83..cf66137 -- crates/`
  = one hunk; `FOOTER_CAPTION` value unchanged), and `./ok.sh test` green (TestBackend suite same
  counts as the prior VERIFIED run). Live server API + reqwest-path attestation carries forward
  from the byte-identical `542f19aa` tree (no wire/reqwest delta; stack not re-booted — correct).
- [x] 2026-06-27 [orchestrator] Step-7 freshen: rebased onto `main@0c9d827` (which added only
  docs/board — idea 0002 + handoff + dashboard regen; no `crates/` change). `./ok.sh code-hash` at
  the rebased head is still `b4bc0cdb93086adb620ffbe66bc5d66a524e4ffd` — byte-identical code, so
  per verdict pinning the `approved` + `VERIFIED` attestations carry forward untouched (no
  relabelling; commit shas in the verdict lines above are now stale pointers, which is expected).
  Footer-fix re-entry complete: all 7 feature DoD clauses met. `working` → `awaiting-merge`.
- [ ] 2026-06-27 [human] Bug in the `?` help dialog: the `q  quit` entry is jammed onto the same
  row as the `? / Esc  close help` entry, and `close help` is not tab-aligned to the description
  column. Both should be their own properly-tabbed Global lines.
- [x] 2026-06-27 [orchestrator] Triage: `draw_help`'s `? / Esc  close help    q  quit` line
  (`crates/tui/src/ui/mod.rs`) crams two entries on one row, breaking the `{key:<18}{desc}`
  layout every other Global line follows (desc aligned at col 21). Pure-presentation defect in
  0015's own help modal — no behaviour/wire (#2)/domain (#3) change; no ADR needed (ADR-0010 /
  ADR-0006 §8.3 already govern the help modal). Folded back into 0015 (same precedent as the
  footer fix): `awaiting-merge` → `working`; prior approved+verified verdicts VOID once code
  changes (code-hash will differ); re-runs review + verify. tui-dev splits the line into two
  tabbed Global entries; tester pins the help layout.
- [x] 2026-06-27 [tui-dev] Fixed the help-modal layout bug: split `draw_help`'s malformed
  `? / Esc  close help    q  quit` row into two properly-tabbed Global lines — key `q` →
  `quit`, and key `? / Esc` → `close help` — each following the sibling
  `{key:<18}{desc}` layout so descriptions align at col 21. Pure presentation; keymap (`?`/`Esc`
  close-help), wire, contract, domain all unchanged. No existing test referenced the old string
  (the `q: quit` / `?: help` grep hits are footer-caption asserts on a different surface).
  `./ok.sh test | lint | fmt --check` green. Commit `8c25b97`; code-hash
  `c49cb87a15514abab9c84d01f69833eda4b3b98e`.
- [x] 2026-06-27 [tester] Added a positive regression test
  `help_modal_global_block_lists_quit_and_close_help_as_separate_aligned_rows` (`dialogs.rs`):
  opens the `?` modal via the real keymap and asserts (1) `quit` and `close help` are on separate
  rows (the `close help` row does not also contain `quit`, and vice-versa — guards the crammed row
  from returning) and (2) the `close help` description starts at the same column as the `quit` and
  `r refresh` sibling rows (alignment invariant, asserted relative to siblings, not a magic
  constant). Test-only; no `src/`. `dialogs` suite 22→23, full workspace green; lint + fmt clean.
  Commit `397d759`; code-hash `00b1cb162b4c8c9bea9ce1e3eb840c0c50ebafcc`.
- [x] 2026-06-27 [verifier] Re-attest at code-hash `00b1cb162b4c8c9bea9ce1e3eb840c0c50ebafcc`
  (commit `397d759`; HEAD `8477b35` Board-only) — **VERIFIED**. Independently confirmed (`git diff
  cf66137..397d759 -- crates/`) the only source deltas since the live wire pass are the `draw_help`
  help-text edit + the new `dialogs.rs` test — no server/contract/reqwest code. `./ok.sh test` green
  (`dialogs` 22→23, no flake). Live server API + reqwest-path attestation carries forward from the
  byte-identical `542f19aa` tree (no wire delta; stack not re-booted — correct).
- [x] 2026-06-27 [reviewer] Re-review of `8c25b97`+`397d759` — **`REVIEW-STATUS: approved`**, pinned
  to code-hash `00b1cb162b4c8c9bea9ce1e3eb840c0c50ebafcc` (commit `397d759`; HEAD `8477b35`
  Board-only). Measured the rendered Global block: all eight rows' descriptions align at column 21
  (index 20) — the two new rows match the sibling `{key:<18}{desc}` layout; the crammed-row defect
  is fully corrected. Presentation-only (only two `Line::from` literals changed); no behaviour/keymap,
  no wire (#2), no domain (#3), TUI stateless (#1), no `#[allow]`. New test genuinely pins the
  corrected layout (separate rows + column alignment). Gate green: lint + fmt clean; `./ok.sh test`
  green run serialized (`RUST_TEST_THREADS=1`) from clean state — auth 16, dialogs 23, 0 failures.
  No code findings. Process note: re-review hit the idea-0002 parallel-Postgres flake; overlapping
  background test runs poisoned the shared throwaway test PG (transient auth-suite failures), which
  the clean serialized re-run cleared — reviewer did NOT reset the shared volume (correctly outside
  read-only authority); orphaned `pgrep` waiter shells were the churn, not a code defect.
