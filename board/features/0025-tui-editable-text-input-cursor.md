---
id: 0025
title: Editable text inputs — movable, visible cursor (stop the append-only / end-locked editing)
type: feature       # feature | chore
status: working         # inbox → planned → ready → working → review → awaiting-merge → merged | blocked
priority: medium    # high | medium | low
parent: null
depends-on: []
branch: feature/0025-tui-editable-text-input-cursor
worktree: .claude/worktrees/0025-tui-editable-text-input-cursor
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

### Acceptance criteria (firmed up by the architect's plan)

- [ ] Every text input renders a **visible caret** at the current edit position, drawn via
      `frame.set_cursor_position` for the **focused** field only (single-line dialog/auth fields
      and the multiline note-detail Content pane).
- [ ] The caret can be **moved** with **Left / Right** (by one character) and **Home / End** (to
      the start / end of the current line), and **typing / Backspace act at the caret**, inserting
      / deleting mid-buffer rather than only at the end. **Delete** (forward delete) is included.
- [ ] In the multiline note **Content** edit, **Up / Down** move the caret between lines and the
      pane **scrolls to keep the caret in view** when the buffer exceeds the pane height (absorbing
      idea 0006's scroll gap). Single-line fields keep Up/Down as field navigation (unchanged).
- [ ] Behaviour is consistent across the affected inputs: notes create/edit + note-detail edit,
      task add/edit + task-detail edit, sub-task add/edit, profile create/rename, auth
      login/register fields, and the numeric duration + window-size edits. The `f` date-filter
      spinner is out of scope (see Assumption A4).
- [ ] Caret placement and movement are **UTF-8-safe** (no panic / no `indexing_slicing`) for
      multi-byte characters and empty buffers.
- [ ] `TestBackend` regression coverage for cursor movement, mid-buffer insert/delete, and the
      rendered caret position (and multiline scroll-to-caret), plus source-owned unit tests for the
      primitive's char-boundary / cursor / scroll math.
- [ ] `./ok.sh test | lint | fmt --check` green; no new `?` help-overlay line-wrap regression.

## Plan(s)

### Plan: shared `TextInput` primitive adopted by every text field

**Design decision — shared primitive (not per-struct cursors).** Introduce one pure, deeply
tested `TextInput` primitive (a `String` buffer + a caret held as a **char index** + insert /
delete / move ops + a caret/scroll render helper) and have every free-text and numeric-text field
adopt it, replacing the ~10 copy-pasted `push_char`/`backspace` pairs and the `Option<String>`
edit buffers. Rationale (coding-standards: correctness > simplicity, deep modules, DRY): the
fiddly char-boundary and multiline scroll math is written and tested **once** rather than
hand-rolled N times, and the primitive subsumes idea 0006's scroll gap for free. The trade-off is
a larger **mechanical** blast radius — changing a field's type from `String` to `TextInput`
ripples to every render site and every test literal — but that is churn, not risk, and is
contained entirely within the `tui` crate. A read accessor (`value()`/`as_str()`) + `Default`
keep the ripple terse (Assumption A6).

**Approach (tracer-bullet then widen).** Slice 1 builds the primitive and adopts it on the
**note-detail Content pane** — the multiline + scroll case that is the operator's headline pain
and idea 0006's exact ask — proving the full pipeline end-to-end (new movement `Event`s →
`terminal/mod.rs` keymap → the note-detail handler → primitive ops → caret render + scroll). Slice
2 widens the same pattern to every remaining single-line and numeric field. Slice 3 (tester)
un-strands the harness and pins the behaviour + caret render.

**ADR:** none required. This shapes **no** wire type (#2 — the primitive still yields a `String`
that is `.trim().to_owned()` into the unchanged `Create*`/`Update*` DTOs; no `contract` change),
adds **no** domain structure (#3), and **preserves** the stateless-TUI invariant (#1 — the caret
index and scroll offset are transient process-lifetime UI state, exactly like the existing
`pending`/`selected`/edit-buffer state, never persisted and never sent). No auth (#5),
cross-profile (#4), or new-binary (#6) surface. The keymap additions are **additive**,
non-colliding (Left/Right/Home/End/PageUp/PageDown are currently unbound; Up/Down are repurposed
for caret line-move **only** under the existing `editing_note_content` discriminant, so
single-line field arrow-navigation is unchanged), and **extend** — not re-decide — the ADR-0010
hotkey scheme. Should any wire implication surface mid-build (not expected), that re-scopes to an
ADR event per the operator's framing and #2; the dev agent sets the item `blocked` and routes back
to `architect`.

**Slices:**

1. [tui-dev] **The `TextInput` primitive + tracer-bullet adoption on the note-detail Content
   pane.** New module `crates/tui/src/app/text_input/mod.rs` (+ source-owned unit tests in
   `crates/tui/src/app/text_input/tests.rs`, declared `#[cfg(test)] mod tests;` per rust-standards;
   module-directory layout to satisfy `self_named_module_files`). The primitive: buffer + char-index
   caret; `insert_char` / `backspace` / `delete` at the caret; `move_left` / `move_right` /
   `home` / `end`; multiline `move_up` / `move_down`; and a render helper returning the caret's
   (row, col) plus the vertical scroll offset that keeps the caret line within a given viewport
   height. All ops are UTF-8-safe (convert char index → byte offset via `char_indices`; no
   `indexing_slicing`). Adopt it on `NoteDetail.edit` (`crates/tui/src/app/notes.rs`); add the
   movement `Event` variants + their app-core routing (`crates/tui/src/app/mod.rs`); bind the
   movement keys in `crates/tui/src/terminal/mod.rs` (`map_key`); and render the caret + multiline
   scroll for the Content fill-pane in `crates/tui/src/ui/mod.rs` (`draw_detail_panes` +
   `frame.set_cursor_position`). Files: `crates/tui/src/app/text_input/{mod,tests}.rs`,
   `crates/tui/src/app/{mod,notes}.rs`, `crates/tui/src/terminal/mod.rs`,
   `crates/tui/src/ui/mod.rs`. **DoD for this slice = lib+bins build + `clippy --lib --bins`
   green** (the `--all-targets` suite goes red until slice 3 un-strands the harness — expected per
   the learned-0019/0020 gotcha; do **not** read a `--lib --bins`-green Log entry as passing DoD
   clause 1/2).

2. [tui-dev] **Widen the primitive to every remaining field.** Adopt `TextInput` on: `AuthState`'s
   five fields + `field_mut` (`auth.rs`); `AddTaskState` / `EditTaskState` / `AddSubtaskState` /
   `EditSubtaskState` (`task_add.rs`); `TaskDetail.edit` (`task_detail.rs`); `NoteForm` (`notes.rs`,
   both dialog fields); `ProfileForm` (`profiles.rs`); `DurationEditState.buffer` (`timer.rs`,
   digit filtering retained at the call site); `WindowEditState.buffer` (`task_list.rs`, numeric).
   Render the caret for all single-line fields (`draw_field` / `draw_auth` in `ui/mod.rs`) and add
   the movement-key hint line(s) to the `?` help overlay (`draw_help` in `ui/mod.rs`) — **check the
   new hint line width against `HELP_DIALOG_WIDTH` inner ~70 so it does not wrap** (learned
   0015/0019). Route the movement `Event`s in each field handler. Files:
   `crates/tui/src/app/{auth,task_add,task_detail,notes,profiles,timer,task_list}.rs`,
   `crates/tui/src/ui/mod.rs`. Depends on slice 1.

3. [tester] **Un-strand the harness + pin the behaviour and caret render.** Update every stranded
   struct literal and `.value()`/read site in `crates/tui/tests/common/mod.rs` and the test files
   (the field-type change from `String` to `TextInput` re-strands the harness exactly per learned
   0019/0020 — this slice **must land in the same cycle** or the `--all-targets` suite stays red).
   Add coverage (a new `crates/tui/tests/text_input.rs` and/or extensions to `dialogs.rs` /
   `detail.rs` / `keybindings.rs` / `notes.rs`): caret movement (Left/Right/Home/End, multiline
   Up/Down), mid-buffer insert + Backspace + forward Delete, the **rendered caret position** via
   the `TestBackend` cursor position, multiline **scroll-to-caret** when Content exceeds the pane,
   and an anti-wrap assertion pinning the new help-overlay hint line(s). Files:
   `crates/tui/tests/common/mod.rs`, `crates/tui/tests/*.rs`. Depends on slices 1–2.

**Agent involvement:** `tui-dev` (all of `crates/tui/src/`) and `tester` (all of
`crates/tui/tests/`) only. **`platform-dev`, `contract-owner`, and `server-dev` are NOT
involved** — no `ok.sh`/infra change, no `contract`/wire change, no server change. No new crate.

**File ownership:** `tui-dev` owns `crates/tui/src/**` (the primitive, the state structs, the
keymap, the render path, and the primitive's source-owned unit tests). `tester` owns
`crates/tui/tests/**` (the `TestBackend` behaviour + caret-render coverage and the harness
un-stranding).

**Dependency order:** slice 1 (primitive + tracer field) → slice 2 (widen to all fields) → slice 3
(tester harness + coverage). Slices 1–2 are one mergeable `tui-dev` unit; the crate's
`--all-targets` build is only green once slice 3 lands (harness stranding), so all three land in
the same cycle.

**Assumptions:**

- **A1 — Shared primitive over per-struct cursors.** One `TextInput` module is adopted everywhere;
  cursor/scroll logic is not hand-rolled per struct (DRY, single tested implementation, subsumes
  the multiline scroll gap).
- **A2 — Caret stored as a char index; ops convert to byte offsets via `char_indices`.** UTF-8-safe,
  avoids the denied `indexing_slicing`, and remains transient process-lifetime UI state, never
  persisted (#1).
- **A3 — Movement key scheme.** Required: Left/Right (one char), Home/End (line start/end), forward
  Delete (all text fields), and Up/Down for multiline caret line-move gated on the existing
  `editing_note_content` discriminant so single-line field arrow-navigation is unchanged.
  **Optional / deferred:** word-jump (Ctrl+Left / Ctrl+Right) and PageUp/PageDown — included only
  if low-cost on the primitive, otherwise deferred and captured as an idea; they do **not** gate
  acceptance.
- **A4 — `DateFilterState` (day/month/year) is out of scope.** It is a numeric spinner (Up/Down
  increment, Tab to switch component), not a free-text buffer, and keeps its current model. The
  numeric **text** buffers (`DurationEditState`, `WindowEditState`) do adopt the primitive, with
  digit filtering retained at the call site.
- **A5 — The note create/edit DIALOG Content field keeps its current single-line dialog rendering**
  (cursor-navigable, but not re-laid-out as a multiline box); the multiline scroll-to-caret applies
  to the note **DETAIL** Content fill-pane, which is idea 0006's exact ask. Not expanding the
  dialog layout keeps the change to the smallest that satisfies acceptance.
- **A6 — The primitive exposes a terse read accessor** (`value()` / `as_str()`) + `Default` so
  render and test call sites migrate with minimal churn; the caret screen position is rendered via
  `frame.set_cursor_position` for the focused field only and is asserted in tests through the
  `TestBackend` cursor position.
- **A7 — No new `ClientRequest` / `Outcome` / `Client`-trait surface.** The tester harness's
  worker-analogue `ClientRequest` match is untouched; the stranding is limited to state-struct
  field-type changes + read sites — still requiring the tester slice in the same cycle
  (learned 0019/0020).

**Risks:**

- **UTF-8 / char-boundary correctness** (the primary correctness risk): mis-converting the char
  index to a byte offset would panic or trip `indexing_slicing`. Mitigated by A2 and the primitive's
  source-owned unit tests (empty buffer, multi-byte chars, caret at both ends).
- **Keymap regressions:** repurposing Up/Down for caret line-move must not break the existing arrow
  field-switch in single-line forms. Mitigated by gating on `editing_note_content` and by the
  `keybindings` suite; Left/Right/Home/End/PageUp/PageDown are currently unbound so no collision.
- **Tester-harness stranding** (learned 0019/0020): the `String`→`TextInput` field-type change
  re-strands `crates/tui/tests/common/mod.rs` (~10 struct literals + ~61 read sites). Expected and
  budgeted in slice 3; not mergeable until that slice lands. Blast radius: `tui` crate only.
- **`?` help-overlay wrap** (learned 0015/0019): a new movement-key hint line can overflow the box
  and wrap un-indented. Mitigated by checking the line width against `HELP_DIALOG_WIDTH` inner ~70
  and a tester anti-wrap assertion.
- **Multiline scroll off-by-one** (caret parked at the pane's last visible row): the scroll offset
  must keep the caret line within `[scroll, scroll + visible_rows)`. Covered by primitive unit
  tests and a `TestBackend` scroll-to-caret assertion.
- **Masked password caret:** the caret column maps 1:1 to the char index even when the value renders
  as asterisks — low risk, covered by a redaction/caret assertion.

**ADR implications:** **None.** Reasoning recorded under **ADR:** above — no wire (#2), no domain
structure (#3), and #1 preserved (transient UI state only). This conclusion holds against the code:
the primitive feeds the same unchanged DTOs, and the keymap change is additive and confined to
text-entry contexts. If a wire implication is discovered during implementation, it becomes an ADR
event and the item is re-scoped (blocked → `architect`).

**Relationship to idea 0006:** this plan **subsumes** idea 0006 (the note Content scroll + caret
affordance is delivered by slice 1's multiline path). Whether to close 0006 as
superseded-by-0025 or keep it as a narrower record is the **human's** call at triage — this plan
does not flip the idea's status (ideas lifecycle).

## Log / comments

- 2026-07-15 [orchestrator] Filed at operator request. Broad cross-cutting `tui` feature;
  overlaps and likely subsumes idea 0006. Awaiting `architect` planning (this needs a real plan —
  probably a shared text-input primitive — and the architect should confirm no wire implication
  keeps it a single-crate feature). Operator will kick off the work manually.
- 2026-07-15 [orchestrator] Claimed `ready`->`working`; cut worktree
  `.claude/worktrees/0025-tui-editable-text-input-cursor` + branch
  `feature/0025-tui-editable-text-input-cursor` from `main@0b6ef88` (carries the plan). Session
  `drive-0025`. Branch copy is now authoritative (home #2).
- 2026-07-15 [architect] Planned (inbox→planned→ready). Design decision: **single shared
  `TextInput` primitive** adopted by every text/numeric field, over per-struct cursors (DRY,
  one tested char-boundary/scroll implementation, subsumes idea 0006's multiline scroll).
  **Contract/ADR conclusion: no ADR — `tui`-crate-only**, no `contract`/wire (#2), server, or
  domain-structure (#3) change; #1 preserved (caret index + scroll offset are transient UI state).
  Confirmed against the code: primitive still feeds the unchanged `Create*`/`Update*` DTOs, and the
  movement keys are additive/non-colliding (Left/Right/Home/End/PageUp/PageDown currently unbound;
  Up/Down repurposed only under `editing_note_content`). Three slices: tui-dev primitive +
  tracer-bullet Content pane → tui-dev widen to all fields → tester un-strand harness + pin caret
  render (must land same cycle, learned 0019/0020). `grill` considered and **declined** — the
  residual risks are implementation-correctness (UTF-8/char-boundary, scroll off-by-one) handled by
  the primitive's unit tests, not genuine architecture forks. Set `status: ready`.
