---
id: 0025
title: Editable text inputs — movable, visible cursor (stop the append-only / end-locked editing)
type: feature       # feature | chore
status: awaiting-merge  # inbox → planned → ready → working → review → awaiting-merge → merged | blocked
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
- 2026-07-15 [tui-dev] Slice 1 done: new `crates/tui/src/app/text_input/{mod,tests}.rs` —
  `TextInput` (String buffer + char-index caret; `insert_char`/`backspace`/`delete`,
  `move_left`/`move_right`/`home`/`end`, multiline `move_up`/`move_down`; `field_view` for a
  single-line horizontally-scrolled field and `viewport` for a hard-wrapped, vertically-scrolled
  multiline pane returning caret `(row,col)`). All ops char-index→byte via `char_indices`, no
  `indexing_slicing`. 25 source-owned unit tests (empty/multi-byte/both-ends/mid-buffer,
  single-line scroll, multiline wrap + scroll off-by-one + exact-wrap-boundary trailing row) — green
  via `cargo test -p tui --lib` + doctest. Adopted on `NoteDetail.edit`
  (`Option<String>`→`Option<TextInput>`); added movement `Event`s
  `MoveLeft/MoveRight/MoveHome/MoveEnd/MoveUp/MoveDown/Delete` + shared `text_input::apply_motion`;
  bound the keys in `map_key` (Left/Right/Home/End/Delete gated on text-entry; Up/Down→caret only
  under `editing_note_content`, unchanged elsewhere); rendered the caret via
  `frame.set_cursor_position` for the note-detail Title (single-line) and Content (multiline scroll)
  panes in `draw_detail_panes`. Gate: `cargo clippy -p tui --lib --bins` clean, `./ok.sh fmt` clean.
  `--all-targets` intentionally red (tester harness stranded by the `edit` field-type change,
  learned 0019/0020 — slice 3). No `contract`/wire/server change (still feeds the unchanged
  `UpdateNoteRequest`).
- 2026-07-15 [tui-dev] Slice 2 done: widened `TextInput` to every remaining field —
  `AuthState`'s five fields + `field_mut` (`auth.rs`); `AddTaskState`/`EditTaskState`/
  `AddSubtaskState`/`EditSubtaskState` (`task_add.rs`); `TaskDetail.edit` (`task_detail.rs`);
  `NoteForm` both fields (`notes.rs`); `ProfileForm` (`profiles.rs`); `DurationEditState.buffer`
  (`timer.rs`) and `WindowEditState.buffer` (`task_list.rs`), both numeric — digit filtering
  retained at each `push_char` call site, caret still moves. Every field handler routes the movement
  events via a `motion(&Event)`/`edit_motion(&Event)` arm delegating to `text_input::apply_motion`;
  submit paths read `.as_str().trim()`. Render: added a `caret` field to `DetailPane` and a
  `single_line_view(value, caret, width)` helper + `caret: Option<usize>` param to `draw_field`, so
  every focused single-line dialog/auth/detail field renders a horizontally-scrolled slice + caret
  via `frame.set_cursor_position` (masked password caret maps 1:1 over the visible slice); the
  date-filter spinner stays caretless (A4). Added `?`-help hint lines — `Text fields  ← → move caret
  · Home/End ends · Del delete` (56 cols) and `Content: ↑↓ move line · Enter newline · Ctrl+S
  commit` (62 cols) — both under the `HELP_DIALOG_WIDTH` inner ~70, no wrap (checked). Gate:
  `cargo clippy -p tui --lib --bins` clean, `./ok.sh fmt --check` clean, `cargo test -p tui --lib`
  (25) + `--doc` (4) green. `--all-targets` still intentionally red (tester slice 3 un-strands
  `crates/tui/tests/common/mod.rs`). No `contract`/wire/server change — all fields still feed the
  unchanged `Create*`/`Update*`/auth DTOs via `.as_str()`.
- 2026-07-15 [tester] Slice 3 done: un-stranded the `crates/tui/tests/` harness for the
  `String`/`Option<String>` → `TextInput` field-type change (learned 0019/0020) and added caret
  coverage. **Un-stranded:** `common/mod.rs` — imported `TextInput`, converted the 7 stranded
  state-struct literals (`AuthState` 5 fields, `AddTaskState`, `EditTaskState`, `NoteForm` ×2,
  `ProfileForm` ×2) to `TextInput::default()` / `TextInput::new(…)`; and the read sites across the
  test files: `notes.rs` (`form.title/.content.as_str()`), `profiles.rs` (`form.name.as_str()`),
  `tasks.rs` / `subtasks.rs` (`.title.as_str().to_owned()`), `date_window.rs`
  (`editor.buffer.as_str()`), `detail.rs` (×3 `.edit.as_deref()` → `.edit.as_ref().map(|t|
  t.as_str())`). No source touched (A7 held — no `Client`/`ClientRequest`/`Outcome` surface change,
  only field-type + read-site churn). **Coverage added:** new `crates/tui/tests/text_input.rs` (8
  tests) driven through the public two-step `App` API — Left/Right + mid-buffer insert, Home/End,
  mid-buffer Backspace + forward Delete (incl. end-of-buffer no-op), the **rendered caret cell** via
  a new `common::render_cursor` helper reading the `TestBackend` terminal cursor (dialog field +
  masked-password 1:1 mapping), multiline Up/Down line-move, multiline **scroll-to-caret** on the
  note-detail Content pane (tail visible / head scrolled off, then caret walked to top → head
  visible / tail off + caret row rises), and UTF-8 multi-byte caret safety end-to-end (café/naïve
  insert + forward-delete around `é`/`ï`, no panic, correct string, well-defined render). Plus two
  anti-wrap regression tests in `dialogs.rs` pinning the two new `?`-help hint lines (`Text fields …
  Del delete` and `Content: … Ctrl+S commit`) do not reflow against `HELP_DIALOG_WIDTH` inner ~70
  (learned 0015/0019/0020/0023). **Gates (all green):** `./ok.sh fmt --check` clean; `./ok.sh lint`
  (clippy `--all-targets`) clean; `./ok.sh test` all green — workspace-wide incl. the server
  DB-backed integration tests against the script's throwaway Postgres (no capability gap), and the
  `tui` `TestBackend` suite (`text_input` 8, `dialogs` 28, and every pre-existing file). Full
  `--all-targets` build is green again.
- 2026-07-15 [orchestrator] `working` → `review`; dispatched cold `reviewer` (read-only).
- 2026-07-15 [reviewer] Cold review of diff `main@0b6ef88 .. 0108053` (`tui`-crate-only feature).
  **Gate (all green):** `./ok.sh test` green (workspace-wide incl. server DB-backed integration
  tests + contract/tui doctests); `./ok.sh lint` (clippy `--all-targets`) clean; `./ok.sh fmt
  --check` clean. **Substantive:** (1) UTF-8/char-boundary correctness sound — caret char-index →
  byte-offset via `char_indices`/`byte_offset`, end-guarded, no `indexing_slicing`, no cast/overflow
  panic; covered by 25 unit tests + multibyte e2e. (2) Hard constraints hold — #1 caret/scroll are
  transient UI state (scroll derived on demand, nothing persisted/sent); **#2 NO contract/wire
  change** (diff touches only `crates/tui/{src,tests}` + `board/`; no `contract`/server/protocol
  surface; every field still `.as_str().trim/.to_owned()` into unchanged DTOs); #3 no domain
  structure. (3) Keymap additive/non-colliding — Left/Right/Home/End/Delete only under `text_entry`;
  Up/Down → MoveUp/MoveDown only under `editing_note_content`, single-line arrow nav + date spinner
  unchanged. (4) `?` help-overlay: two new lines (~56 / ~62 cols) fit `HELP_DIALOG_WIDTH=72` inner
  ~70; two `dialogs.rs` anti-wrap regression tests pin them (learned 0015/0019). (5) Tests
  public-API-driven, real caret/scroll/render coverage, harness un-stranding field-type-only (A7).
  (6) No unjustified `#[allow]` (only sanctioned `cfg_attr(test, …)`); Conventional Commits with
  correct `@organized-koala.local` co-author trailers (no `noreply@anthropic.com` leak). (7) Scope:
  DateFilter excluded (A4), note dialog Content single-line (A5), word-jump/PageUp-Down deferred
  (A3, non-gating). **fix-now: none.** **Out-of-scope (→ idea, not dragged in):** pre-existing
  `AuthState` password *entry* buffer reachable via derived `Debug` (unchanged by 0025, not a
  regression; JWT wrapped in `SessionToken`, buffer wrapped in `Password` at submit) — candidate for
  a redacting holder. **Verdict:** `REVIEW-STATUS: approved` pinned to code-hash `5175b549` (commit
  `0108053`).
- 2026-07-15 [verifier] Verified end-to-end (ADR-0003 layering). code-hash `5175b549` matches the
  attested pin (live HEAD shares the digest — only Board-only commits on top of code `0108053`).
  docker 29.5.3 + compose present; no new binaries; hermetic `verify-boot` boot + self-teardown
  (clean — no lingering containers, no stranded `deploy_postgres-data`). **TestBackend suite
  present + green:** `crates/tui/tests/text_input.rs` 8/8 (Left/Right + mid-buffer insert, Home/End,
  multiline Up/Down, Backspace + forward Delete, masked-password 1:1 caret, multibyte caret safety,
  rendered caret cell, scroll-to-caret) and `dialogs.rs` 28/28 (incl. the Text-fields + Content
  anti-wrap regression tests). **Live stack exercised** (12/12 pass) over the reqwest/HTTP path for
  the now-`TextInput`-backed fields: register→201+JWT, login→200+JWT, create profile→201, create
  task→201 (`status:open`), patch task→200 (`closed_at` set), create note with embedded newlines→201
  (multiline round-tripped exactly), update note→200; error contract `{code,message}` (401
  invalid_credentials / 401 unauthenticated / 422 deserialize); profile-scoping #4 (cross-profile
  read→404, owner scoped read→200); OTel spans observed at the collector for all exercised paths.
  Server API + reqwest wire shapes behaviourally unchanged (no contract/wire/server change), as
  planned. **Verdict: `verified`** pinned to code-hash `5175b549` (commit `0108053`).
- 2026-07-15 [orchestrator] Step-7 freshen: rebased the branch onto `main` (which had advanced with
  the eng-manager learnings + regenerated dashboard + idea 0012, all docs/board/`.claude` only). The
  only conflict was this feature-local Board file (main's frozen pointer vs. the branch's
  authoritative copy) — resolved in favour of the branch. `./ok.sh code-hash HEAD` is **unchanged at
  `5175b549`**, so the code is byte-identical and the `approved` + `verified` verdicts carry forward
  untouched (no relabelling). Re-ran the gates on the rebased tree: `./ok.sh fmt --check | lint |
  test` all green (exit 0). `working`/`review` → `awaiting-merge`.

## Summary

coverage: 73.79%

Ships **editable text inputs** across the TUI: every text field — previously append-only and
end-locked with no visible caret — now renders a **movable, visible cursor** and supports mid-buffer
insert / delete. The operator's headline pain (correcting an early typo in a long Notes Content meant
destroying and re-typing the tail) is resolved, and the multiline note Content pane now scrolls to
keep the caret in view.

- **Design — one shared `TextInput` primitive** (`crates/tui/src/app/text_input/`), not per-struct
  cursors (DRY, deep module: the char-boundary + scroll math is written and tested **once**). A
  `String` buffer + a **char-index caret** + `insert_char` / `backspace` / `delete` /
  `move_left` / `move_right` / `home` / `end` + multiline `move_up` / `move_down`, plus two render
  helpers — `field_view` (single-line horizontally-scrolled slice) and `viewport` (hard-wrapped,
  vertically-scrolled multiline pane returning the caret `(row, col)` + scroll offset). All ops are
  UTF-8-safe (char index → byte offset via `char_indices`, no `indexing_slicing`). Adopted across
  ~10 fields, replacing the copy-pasted `push_char`/`backspace` pairs and `Option<String>` edit
  buffers: auth (5 fields), task add/edit + task-detail, sub-task add/edit, notes (form + detail),
  profiles, and the numeric duration + window buffers (digit filtering retained at each call site).
  A terse `value()` / `as_str()` accessor + `Default` kept the migration mechanical.
- **Keys + render.** Left/Right/Home/End/Delete bound under a **text-entry** context; Up/Down
  repurposed to caret line-move **only** under `editing_note_content` (single-line arrow-nav + the
  `f` date-filter spinner unchanged). The caret is drawn via `frame.set_cursor_position` for the
  **focused** field only; the note-detail Content pane scrolls to keep the caret in view. Two
  `?`-help hint lines added (`Text fields  ← → move caret · …` and `Content: ↑↓ move line · …`),
  width-checked against the inner ~70 and pinned by anti-wrap tests. Word-jump + PageUp/PageDown were
  deferred (A3, non-gating); the note create/edit *dialog* Content stays single-line (A5); the
  `DateFilter` numeric spinner is out of scope (A4).
- **No wire/domain change.** `tui`-crate-only — **no** `contract`/wire (#2), server, or
  domain-structure (#3) change: every field still `.as_str().trim()` into the unchanged
  `Create*`/`Update*`/auth DTOs; **#1 preserved** (caret index + scroll offset are transient
  process-lifetime UI state, never persisted/sent). **No ADR** — the primitive shapes no wire type
  and the keymap additions are additive and non-colliding.
- **DoD (`feature` track).** `./ok.sh test | lint | fmt --check` all green. Reviewer
  `REVIEW-STATUS: approved` + verifier **verified** (TestBackend suite green + live-boot of the
  unchanged server/reqwest path), both pinned to code-hash
  `5175b54974233e04218f5c2a6eac8d8bc1aece42` (last code commit `0108053`). **ADR: none** (tui-only,
  no contract/domain decision). **coverage: 73.79%** headline region — **report-only, not a gate**;
  the primitive itself is 96.69%-covered.
- **Tests.** 25 source-owned primitive unit tests (empty / multi-byte / both-ends / mid-buffer,
  single-line scroll, multiline wrap + scroll off-by-one) + 8 `crates/tui/tests/text_input.rs`
  `TestBackend` tests (movement, mid-buffer insert / Backspace / forward Delete, the rendered caret
  cell incl. masked-password 1:1 mapping, multiline scroll-to-caret, UTF-8 e2e) + 2 anti-wrap
  regression tests pinning the new help lines.
- **Relationship to idea 0006.** **Subsumed** — the note-detail Content scroll-to-caret is exactly
  idea [`0006`][idea-0006]'s ask. Whether to **close** it as superseded-by-0025 or keep it as a
  narrower record is the **human's** call (an AI cycle does not flip an idea's status). Idea `0012`
  (`board/ideas/0012-redact-auth-password-entry-buffer.md`, filed on `main` this cycle) was also
  **filed** from the reviewer's out-of-scope finding: the pre-existing `AuthState` password-*entry*
  buffer is reachable via derived `Debug` (unchanged by 0025, **not** a regression — the JWT is
  `SessionToken`-wrapped and the buffer becomes `Password` at submit); a candidate for a redacting
  holder.

[idea-0006]: ../ideas/0006-note-content-scroll-cursor-affordance.md
