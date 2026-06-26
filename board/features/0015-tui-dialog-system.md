---
id: 0015
title: TUI dialog system — help/add/delete/timer modals, trimmed footer caption, purple focus
type: feature      # feature | chore
status: inbox           # inbox → planned → ready → working → review → awaiting-merge → merged | blocked
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
