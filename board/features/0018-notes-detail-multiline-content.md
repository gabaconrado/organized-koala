---
id: 0018
title: Notes detail view — multiline Content text area (fills the pane), Created moved above
type: feature      # feature | chore
status: working          # inbox → planned → ready → working → review → awaiting-merge → merged | blocked
priority: medium    # high | medium | low
parent: null
depends-on: []      # builds on 0016 (detail views + final keymap, merged); no in-flight item gates this.
branch: feature/0018-notes-detail-multiline-content
worktree: .claude/worktrees/0018-notes-detail-multiline-content
created: 2026-06-28
updated: 2026-06-28
---

## Feature request

**Goal (operator):** In the **Notes detail view**, make the **Content** field a **multiline
text area** that expands to fill the **rest of the available pane area**, instead of the
fixed single-line field it is today. To keep the layout sensible once Content grows, **move
the read-only `Created` field above Content** so the pane order becomes:

```text
Title    (single-line, editable)
Created  (read-only)
Content  (multiline, editable — fills the remaining height)
```

**Context (current behaviour to change):**

- The Notes detail view renders three panes — `Title → Content → Created` — each as a fixed
  `Constraint::Length(3)` single-line `Paragraph` (`crates/tui/src/ui/mod.rs`,
  `draw_detail_panes` / `draw_note_detail`; pane order in `NotePane::ALL`,
  `crates/tui/src/app/notes.rs`). Content is edited exactly like Title: type chars, `Enter`
  commits, `Esc` cancels. Newlines are not enterable and would not render (single-line
  `Paragraph`, no wrap).
- Keymap context: `map_key` in `crates/tui/src/terminal/mod.rs` maps `Enter → Event::Submit`
  unconditionally; `Shift` is never inspected; no Kitty `KeyboardEnhancementFlags` are pushed
  at terminal init. The final keymap is governed by **ADR-0010**.

**Surface to build (TUI-only — no `contract`/server change):** `Note.content` is already a
`String` in the `contract` crate, so this changes **no** wire shape and adds **no** server
endpoint. Work is confined to `crates/tui` (`app/notes.rs`, `ui/mod.rs`, `terminal/mod.rs`)
and its tests.

1. **Reorder panes** to `Title → Created → Content` (`NotePane::ALL`). Field cycling and the
   commit/cancel lifecycle must keep working with the new order.
2. **Content fills the pane.** In the detail-pane layout, Content takes the remaining height
   (`Constraint::Min(_)`) rather than `Constraint::Length(3)`; Title and Created stay fixed.
   Render Content with newline + wrap support so multi-line text displays correctly.
3. **Multiline editing of Content.** The edit buffer already stores a `String` (can hold
   `\n`); add line-break insertion and multi-line rendering of the in-progress buffer.

**Newline / commit binding — RESOLVED design decision (operator, 2026-06-28):**

The operator's original ask was *Enter submits, Shift+Enter breaks the line* (chat-app
pattern). That pattern is **rejected as infeasible/unreliable**: distinguishing `Shift+Enter`
from `Enter` requires the **Kitty keyboard protocol** (`PushKeyboardEnhancementFlags`), which
many terminals the operator may use (Apple Terminal, most gnome-terminal/VTE builds, plain
xterm, bare tmux) do **not** support — there, Shift+Enter is byte-identical to Enter and no
newline could ever be inserted. **Decision instead (works in every terminal):**

- **While editing the multiline Content field:** **`Enter` inserts a newline**, and **`Ctrl+S`
  commits** the field.
- **Single-line fields (Title):** unchanged — **`Enter` still commits**.
- `Esc` keeps its existing two-tiered behaviour (cancel the in-progress edit → revert; or, when
  not editing, exit the detail view to the list).

This makes the commit key **context-dependent** (`Enter` for single-line panes, `Ctrl+S` for
the multiline Content pane). The new `Ctrl+S` binding and the context-dependent `Enter`
behaviour are a **keymap change** — the architect decides whether this warrants an **amendment
to ADR-0010** (final keymap) or is in-scope clarification, and records it before code.

**Acceptance:**

- Notes detail view shows panes in order `Title → Created → Content`.
- Content pane expands to fill the remaining pane height; multi-line content renders with
  wrapping, no truncation of earlier fields.
- While editing Content: `Enter` inserts a line break (visible in the rendered buffer);
  `Ctrl+S` commits the edit and persists via the existing `UpdateNote` path; `Esc` cancels and
  reverts. Title editing still commits on `Enter`.
- Behaviour is verified by `tester`'s `ratatui` `TestBackend` suite (per ADR-0003, interactive
  TUI behaviour is owned by the TestBackend suite, not the live verifier) covering: pane order,
  Content filling the pane, newline insertion, `Ctrl+S` commit, and `Esc` cancel.
- `./ok.sh test | lint | fmt --check` green.

## Plan(s)

### Plan: Multiline Content text area in the note detail view (Title → Created → Content)

**Approach:** A `tui`-crate-only change in four cooperating seams, no `contract`/server touch.
Tracer-bullet slice: get the **new commit keymap** flowing end-to-end first — add the two
`Event` variants, teach `map_key` the context branch (`Ctrl+S`→commit, `Enter`→newline only in
the multiline Content edit), and teach the note detail handler to accept both `Submit` and
`Commit` as "commit the field" while routing `Newline` to a buffer line-break — so the
pure-seam contract (ADR-0003/0006) is proven before any layout work. Then widen to the
**layout**: reorder `NotePane::ALL` to `Title → Created → Content` and give Content
`Constraint::Min(_)` (fills remaining height) while Title/Created stay `Length(3)`, rendering
Content with newline + wrap. `tester` pins the whole surface (pane order, fill, newline, Ctrl+S
commit, Esc cancel, and the unchanged Title-commits-on-Enter fork) through the `TestBackend`
suite per ADR-0003. No live verifier exercises interactive TUI behaviour (ADR-0003); the
verifier confirms the TestBackend suite exists and is green.

**ADR:** ADR-0011 (Multiline Content editing keymap) — REQUIRED before code; authored and
committed to `main` with this plan. Amends ADR-0010 §4 for the multiline pane only.

**Slices (dependency-ordered; all in `crates/tui`):**

1. **[tui-dev] Keymap + Event alphabet (tracer-bullet).** Add `Event::Commit` (explicit
   "commit focused field") and `Event::Newline` ("insert a line break") to the `Event` enum.
   In `map_key`: map `Ctrl+S` → `Event::Commit` while a text-entry context is active; map
   `Enter` → `Event::Newline` **only** when the active text-entry context is the multiline
   Content edit, else keep the existing `Enter` → `Event::Submit`. Introduce a small predicate
   (analogous to `is_text_entry`/`detail_view_open`) that recognises "the note detail's Content
   pane is being edited" — over `Screen`, no terminal enhancement flags. Files:
   `crates/tui/src/app/mod.rs` (Event enum + variant docs), `crates/tui/src/terminal/mod.rs`
   (`map_key`, the new predicate, the `Ctrl+S`/`Enter` branches; mirror the `Ctrl+C` modifier
   check style).

2. **[tui-dev] Note detail handler: commit + newline.** In `crates/tui/src/app/notes.rs`,
   `handle_detail_event` (the `detail.is_editing()` arm): treat `Event::Commit` identically to
   `Event::Submit` (both call `submit_field`); add `Event::Newline` → push `'\n'` into the edit
   buffer (a `NoteDetail::push_newline`, or reuse `push_char('\n')`). Title pane keeps
   committing on `Submit` (Enter) unchanged; Content commits on `Commit` (Ctrl+S) and accepts
   newlines. No change to `submit_field`'s payload logic — `UpdateNoteRequest { title, content }`
   is unchanged and `content` already carries `\n`. Files: `crates/tui/src/app/notes.rs`.

3. **[tui-dev] Pane reorder + Content fills the pane.** Reorder
   `NotePane::ALL` to `[Title, Created, Content]` (`crates/tui/src/app/notes.rs`); `cycle`,
   `first_editable`, `focused_pane` already operate over `ALL` and the editable set, so they
   adapt without logic change (verify `cycle` still skips read-only `Created` and lands only on
   Title/Content). In rendering, give the detail-pane layout per-pane constraints rather than a
   uniform `Length(3)`: Title and Created stay `Length(3)`; Content takes `Constraint::Min(_)`
   (fills remaining height). Render Content with `Paragraph::wrap` (and multi-line value) so
   `\n` + wrapping display correctly with no truncation of Title/Created. This needs
   `draw_detail_panes` (or the note path) to carry a per-pane constraint/"fill" flag rather than
   mapping every pane to `Length(3)`. Files: `crates/tui/src/app/notes.rs` (`NotePane::ALL`),
   `crates/tui/src/ui/mod.rs` (`draw_detail_panes` per-pane constraints, `draw_note_detail`,
   `DetailPane` gains a fill/constraint field; Content `Paragraph` wrap).

4. **[tui-dev] Discoverability copy.** Surface `Ctrl+S` so the user can learn it: update the
   `?` help overlay `Detail` line in `draw_help` (e.g. note its commit is `Ctrl+S` in a
   multiline field) and/or the Content pane label/caption. Keep it terse; no new dialog. Files:
   `crates/tui/src/ui/mod.rs` (`draw_help`, note-pane label/caption).

5. **[tester] `TestBackend` suite.** Extend the note-detail coverage in
   `crates/tui/tests/detail.rs` (and the keybinding pins in `crates/tui/tests/keybindings.rs`)
   to cover: (a) pane order renders `Title → Created → Content`; (b) Content pane fills the
   remaining height and a multi-line value renders without truncating Title/Created; (c) while
   editing Content, `Event::Newline` inserts a `\n` visible in the rendered buffer; (d)
   `Event::Commit` commits Content via the existing `UpdateNote` path (assert payload + chained
   re-derive); (e) `Esc` cancels and reverts a Content edit; (f) **regression fork** — Title
   still commits on `Event::Submit` (Enter), and `map_key` maps `Enter`→`Submit` for Title /
   `Enter`→`Newline` for an active Content edit, and `Ctrl+S`→`Commit`. Mock only the sanctioned
   `Client` trait. Files: `crates/tui/tests/detail.rs`, `crates/tui/tests/keybindings.rs`,
   `crates/tui/tests/common/` (builders if a multiline fixture is needed).

**Assumptions (ambiguity policy — smallest change that satisfies acceptance):**

- A1. **Two new `Event` variants** (`Commit`, `Newline`) is the chosen shape (ADR-0011 §2). The
  detail handler accepts both `Submit` and `Commit` as commit so Title (Enter) and Content
  (Ctrl+S) share one `submit_field` path. A narrower equivalent preserving both invariants
  (Enter commits single-line; buffer takes `\n`) is acceptable to `tui-dev`, but the plan
  expects the two-variant design.
- A2. **`Ctrl+S` maps to `Commit` only in a text-entry context** (an active field edit). Outside
  text entry it is inert (returns `None`), so it never collides with a global hotkey. `Ctrl+C`
  stays the unconditional Quit and is checked first, as today.
- A3. **`Enter` → `Newline` is gated strictly to the note detail Content edit.** In the Title
  edit, the create/edit note forms, auth, dialogs, and the duration edit, `Enter` stays
  `Submit`. (The note **create/edit forms** — `Creating`/`Editing` `NoteForm` — are out of scope
  for multiline; only the **detail view's** Content pane becomes multiline. Acceptance names the
  detail view only.)
- A4. **`Content` Min-height constraint** uses `Constraint::Min(3)` (a sensible floor matching
  the others' 3-row box) so a short Content still shows a usable box and grows to fill. The
  exact floor is a `tui-dev` choice within "fills remaining, never truncates Title/Created."
- A5. **Cursor/scroll for very long Content** is out of scope: render with wrap; if content
  exceeds the pane, simple top-anchored rendering is acceptable (no scroll affordance is in the
  acceptance criteria). Capture any follow-up as an idea, not scope creep.
- A6. **No terminal enhancement flags** are pushed at init (ADR-0011 rejection of Shift+Enter);
  the binding relies only on keys every terminal delivers.

**Risks / blast radius:**

- R1. **`map_key` context-sensitivity leak (principal risk).** `Enter` must stay `Submit` for the
  single-line Title pane and *every* existing commit context (auth, dialogs, create/edit forms,
  list-open, profile-switch) and become `Newline` **only** inside the multiline Content edit. A
  too-broad predicate breaks committing across the app; a too-narrow one leaves Content unable to
  insert newlines. Mitigation: the new predicate is scoped to exactly the note-detail Content-edit
  state, pinned by keybinding tests on both forks (slice 5f).
- R2. **Layout truncation.** Giving Content `Min(_)` while Title/Created keep `Length(3)` must not
  let a growing/multi-line Content push Title/Created out or truncate them. Mitigation: fixed
  `Length(3)` for Title/Created + `Min` for Content in a vertical `Layout`; test asserts Title and
  Created remain fully rendered with a multi-line Content (slice 5b).
- R3. **`draw_detail_panes` is shared with the task detail view.** Adding per-pane constraints must
  not change the task detail layout (all task panes stay `Length(3)`). Mitigation: default the
  per-pane constraint to `Length(3)`; only the note Content pane opts into `Min`. The task path is
  unchanged; existing task-detail tests must stay green.
- R4. **`cycle`/`focused_pane` over reordered `ALL`.** Reordering `ALL` must keep focus skipping
  read-only `Created` and landing only on Title/Content; `first_editable` still resolves to Title
  (index 0). Mitigation: these helpers iterate the editable set, not fixed indices — verify and
  pin by the existing cycle test (slice 5).
- R5. **Discoverability of `Ctrl+S`** is a learnable-affordance risk, not correctness — mitigated by
  the help-overlay/caption copy (slice 4).

**Out of scope (file an idea if surfaced):** multiline editing of the note **create/edit forms**
(`NoteForm`), a Content scroll/cursor affordance for content exceeding the pane, and any task-detail
Description multiline change.

## Log / comments

- 2026-06-28 [drive] Claimed `0018`; cut worktree `feature/0018-notes-detail-multiline-content`
  from `main@7a96ee1` (plan + ADR-0011 present in base). Status `ready`→`working`. Branch copy
  is now authoritative; `main`'s copy frozen at the claim snapshot.
- [x] 2026-06-28 [human] Filed from a direct operator request. Two design forks resolved up
  front (see Feature request): (1) newline binding = `Enter`-newline / `Ctrl+S`-commit in the
  Content pane (Shift+Enter rejected as terminal-dependent); (2) routed through the formal
  drive cycle. Architect: confirm the ADR-0010 keymap question and whether any ADR is needed
  before implementation. Resolved (architect, 2026-06-28): the `Ctrl+S`-commit
  binding plus context-dependent `Enter` (newline in the multiline Content edit, commit
  elsewhere) **is** a keymap change to ADR-0010 §4, not in-scope clarification — recorded in
  **ADR-0011** (amends §4 for the multiline pane); plan written, item `planned`→`ready`.
