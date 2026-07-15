---
id: 0025
title: Editable text inputs — movable, visible cursor (stop the append-only / end-locked editing)
type: feature       # feature | chore
status: inbox           # inbox → planned → ready → working → review → awaiting-merge → merged | blocked
priority: medium    # high | medium | low
parent: null
depends-on: []
created: 2026-07-15
related-ideas: [0006]
updated: 2026-07-15
---

## Feature request

**Operator feature request.** Every text input in the TUI is currently **append-only and
end-locked**: there is no visible cursor/caret, and edits can only happen at the end of the
buffer. To change something in the middle you must backspace away everything after it and retype.
This is a real pain in **Notes**, where a note's Content can be long — it is effectively
append-only, so correcting an early typo means destroying and re-typing the tail.

Requested behaviour: a **visible cursor** that indicates the edit position, and **cursor
movement** so the user can insert/delete anywhere in the buffer rather than only at the end.

### Current state (why it's end-locked)

Text handling is duplicated across every input as an append-only `push_char` (push to end) /
`backspace` (pop from end) pair, with **no cursor index stored** on any of them:

- `crates/tui/src/app/notes.rs` — `NoteForm` (create/edit title+content) and the note-detail edit
  buffer.
- `crates/tui/src/app/profiles.rs` — `ProfileForm` (create/rename).
- `crates/tui/src/app/task_add.rs` — add-task / add-subtask / edit fields.
- `crates/tui/src/app/task_detail.rs` — task-detail field edit buffer.
- `crates/tui/src/app/timer.rs` — duration edit (numeric).
- `crates/tui/src/app/auth.rs` — login/register fields.

Because no field tracks a caret position, both the render path (no cursor drawn) and the edit
ops (only end-of-buffer) are end-locked. The multiline note **Content** pane additionally has no
scroll affordance, so a long buffer isn't even fully viewable — see the overlap note below.

### Scope / design notes for the architect (non-binding)

- This is broad and **cross-cutting** across nearly every input. The append-only logic is
  copy-pasted N times; the architect should weigh introducing a **single shared text-input
  primitive** (buffer + cursor index + insert/delete/move ops + a render helper that places the
  caret and, for multiline, scrolls to keep it in view) that every form field adopts, versus
  bolting a cursor onto each struct. A shared primitive is the likely right call and would also
  subsume the multiline scroll gap.
- New movement keys (Left/Right, Home/End, word-jump, and for multiline Content Up/Down /
  PageUp/PageDown) must be threaded through the `Event` enum and the `terminal/mod.rs` keymap
  **without colliding** with the existing dialog/detail keymaps, and must respect the two-tier
  `Esc` and the field-`Tab` routing already in place. Mind the `?` help-overlay width gotcha when
  adding key hints (learned 0015/0019).
- `#1` (stateless TUI): the cursor index / scroll offset is **transient process-lifetime UI
  state**, never persisted — no server or wire involvement. Expected to be **`tui`-crate-only**
  with **no** `contract`/wire (#2), server, or domain-structure (#3) change. If the architect
  finds any wire implication, that's an ADR event and re-scopes the plan.
- Extends the existing `TestBackend` seam (ADR-0003): the `tester` slice pins cursor movement,
  mid-buffer insert/delete, and the rendered caret position for representative inputs; the live
  verifier confirms that suite is green for this TUI-only change.

### Relationship to idea 0006

Idea `board/ideas/0006-note-content-scroll-cursor-affordance.md` (raised out of 0018) is a
**subset** of this request — it asks specifically for a scroll + caret affordance on the note
Content pane. This item is the **broader** feature (a movable, visible cursor across **all**
inputs) and would naturally subsume 0006. **For the human to decide at triage:** whether to close
0006 as superseded-by-0025, or keep it as the narrower first slice. Left to the operator — an AI
cycle does not flip an idea's status (per the ideas lifecycle).

### Acceptance criteria (to be firmed up by the architect's plan)

- [ ] Text inputs render a **visible cursor** at the current edit position.
- [ ] The cursor can be **moved** within a buffer (at minimum Left/Right + Home/End) and typing /
      backspace act **at the cursor**, inserting/deleting mid-buffer rather than only at the end.
- [ ] The multiline note **Content** edit follows the cursor and scrolls to keep it in view when
      the buffer exceeds the pane (absorbing idea 0006's scroll gap).
- [ ] Behaviour is consistent across the affected inputs (notes, profiles, tasks, task/note
      detail edit, auth; duration edit as applicable).
- [ ] `TestBackend` regression coverage for cursor movement + mid-buffer edit + caret render.
- [ ] `./ok.sh test | lint | fmt --check` green; no new help-overlay line-wrap regressions.

## Log / comments

- 2026-07-15 [orchestrator] Filed at operator request. Broad cross-cutting `tui` feature;
  overlaps and likely subsumes idea 0006. Awaiting `architect` planning (this needs a real plan —
  probably a shared text-input primitive — and the architect should confirm no wire implication
  keeps it a single-crate feature). Operator will kick off the work manually.
