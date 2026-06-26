---
id: 0016
title: TUI detail views + final hotkey scheme — per-field task/note panes, full keymap
type: feature      # feature | chore
status: inbox           # inbox → planned → ready → working → review → awaiting-merge → merged | blocked
priority: medium    # high | medium | low
parent: null
depends-on: [0015]
branch: feature/0016-tui-detail-views-and-hotkeys
worktree: .claude/worktrees/0016-tui-detail-views-and-hotkeys
created: 2026-06-26
updated: 2026-06-26
---

## Feature request

**Goal:** Phase 3 (final) of the three-part TUI overhaul (0014 → 0015 → **0016**). Add
**task/note detail views** with one pane per field (each editable in place) and lock in the
**complete hotkey scheme**. Builds on the tab shell (0014) and dialog framework (0015).

**Context (current behaviour to change):**

- **Tasks have no detail view**; the description is entered on add/edit but never displayed.
  **Notes** have a read-only `Viewing` detail (`app/notes.rs`, `crates/tui/src/ui/mod.rs`) —
  this becomes an **editable per-field** view.
- The keymap predates this overhaul (`map_key`, `crates/tui/src/terminal/mod.rs`): `c` toggles
  done, `x` deletes, `p` toggles timer, `d` edits duration, etc. This phase replaces it with
  the scheme below.

**Surface to build (TUI only — no `contract`/server change):**

- **Detail views with per-field panes (point 9, points 5.1/5.3).** Selecting a task or note
  with **`Enter`** opens a **detail view** whose individual fields are each their own **pane**
  (task: Title, Description — plus read-only Status / Created / Closed as appropriate; note:
  Title, Content — plus read-only Created):
  - Panes are **cycled with `Tab` / `Shift+Tab`** (inside a detail view, Tab cycles *panes*,
    not top-level tabs).
  - **Edit lifecycle:** pressing **`e`** enters edit mode on the focused pane; **`Enter`
    commits the change to that field**; **`Esc` cancels the edit** (reverts the pane to its
    pre-edit value, staying in the detail view).
  - **`Esc` is two-tiered:** while a field edit is in progress it cancels *that edit* (per
    above); while no field is being edited it **exits the detail view back to the list**.
  - The focused pane shows the **purple focus border** from 0015.
- **Final hotkey scheme (point 5).** Replace the current keymap with the table below. Per-entity
  action keys (`a`/`e`/`d`/`Enter`/`Space`) are **context-scoped to the active tab**; global
  keys work anywhere a dialog/edit field is **not** capturing input.

  | Scope | Key | Action |
  | --- | --- | --- |
  | **Tasks tab** | `a` | Add task (dialog, 0015) |
  | | `e` | Edit **title only** (list); in the task detail view, enter edit on the focused pane |
  | | `Enter` | Open task detail view; **when editing a pane, commit that field** |
  | | `Space` | Mark done / undone (**replaces the old `c` toggle**) |
  | | `d` | Delete task (confirm dialog, 0015) |
  | **Notes tab** | `a` | Add note (dialog) |
  | | `e` | Edit **title only** (list); in the note detail view, enter edit on the focused pane |
  | | `Enter` | Open note detail view; **when editing a pane, commit that field** |
  | | `d` | Delete note (confirm dialog) |
  | **Profiles tab** | `Enter` | Switch active profile |
  | | `a` | Add profile (dialog) |
  | | `e` | Edit (rename) profile |
  | | `d` | Delete profile (confirm dialog; keeps 0012 last-profile guard) |
  | **Global** | `T` | Configure timer (dialog, 0015) |
  | | `t` | Start / stop timer |
  | | `r` | Refresh |
  | | `q` | Quit |
  | | arrows | Move (list selection) |
  | | `?` | Help modal (0015) |
  | | `Tab` | Next field/pane (in a dialog/detail view) **or** next top-level tab (in a list) |
  | | `Shift+Tab` | Previous field/pane, or previous top-level tab (in a list) |

  Notes on the remap: `c` (done) → **`Space`**; old delete `x` → **`d`**; old timer toggle `p`
  → **`t`**; old duration-edit `d` → **`T`** (configure). `Esc` closes a dialog (0015); in a
  detail view it cancels an in-progress field edit, else exits the view to the list (two-tiered,
  per the edit lifecycle above).

**Acceptance criteria:**

- [ ] `Enter` on a selected task opens a detail view with per-field panes; `Tab`/`Shift+Tab`
      cycle the panes; `e` enters edit on the focused pane; `Enter` commits that field; `Esc`
      cancels an in-progress edit (reverting the value), and `Esc` with no edit in progress
      returns to the list.
- [ ] `Enter` on a selected note opens the equivalent editable detail view (Title + Content
      editable; Created read-only).
- [ ] The focused pane in a detail view shows the purple focus border (0015).
- [ ] All keys behave per the table: `Space` toggles task done/undone (no `c`); `d` deletes
      (no `x`); `t` starts/stops the timer and `T` opens the timer-config dialog (no `p`/`d`
      duration); `a`/`e`/`Enter` per the active tab; `r`/`q`/`?`/arrows/Tab/Shift+Tab global.
- [ ] Per-entity action keys are scoped to the active tab and never fire while a dialog or an
      edit field is capturing input (global-suppression rule from 0015 holds).
- [ ] Full `feature` Definition of done: `./ok.sh test | lint | fmt --check` green; `reviewer`
      approved (pinned to `./ok.sh code-hash`); the detail-view + keymap behaviour covered by the
      `TestBackend` suite ([ADR-0003][adr-0003]); `verifier` confirms that suite is green and
      boots the stack to confirm reqwest paths still function (no server/contract delta).

**Out of scope (would need an ADR — #3):** new fields on tasks/notes beyond today's flat shape
(the detail view only exposes existing fields — Title/Description/Status/Created/Closed for
tasks, Title/Content/Created for notes); per-profile timer config; any profile detail view
(profiles keep switch/add/rename/delete only). No `contract`/server change.

<!-- feature: needs an `architect` plan (`plan` skill) writing a `## Plan(s)` block before code. -->

[adr-0003]: ../../docs/adr/0003-verification-layering.md
