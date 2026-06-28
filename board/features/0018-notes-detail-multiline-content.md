---
id: 0018
title: Notes detail view — multiline Content text area (fills the pane), Created moved above
type: feature      # feature | chore
status: inbox          # inbox → planned → ready → working → review → awaiting-merge → merged | blocked
priority: medium    # high | medium | low
parent: null
depends-on: []      # builds on 0016 (detail views + final keymap, merged); no in-flight item gates this.
branch: null
worktree: null
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

## Log / comments

- [ ] 2026-06-28 [human] Filed from a direct operator request. Two design forks resolved up
  front (see Feature request): (1) newline binding = `Enter`-newline / `Ctrl+S`-commit in the
  Content pane (Shift+Enter rejected as terminal-dependent); (2) routed through the formal
  drive cycle. Architect: confirm the ADR-0010 keymap question and whether any ADR is needed
  before implementation.
