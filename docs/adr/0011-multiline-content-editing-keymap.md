# ADR-0011: Multiline Content editing in the note detail view — context-dependent commit keymap

**Status:** Accepted · 2026-06-28

## Context

Board item [0018][feat-0018] makes the **Content** field of the Notes detail view a
**multiline text area** that fills the remaining pane height (panes reorder to
`Title → Created → Content`). This is a TUI-only presentation/interaction change: `Note.content`
is already a `String` in the `contract` crate, so there is **no** wire shape change and **no**
server change (hard-constraints #2/#3 untouched, exactly as [ADR-0010 §5][adr-0010] bounds the
detail-view surface to existing DTO fields).

The interaction snag is the **commit key**. [ADR-0010 §4][adr-0010] froze the detail-view
keymap as canonical: inside a detail view, "`e` enters edit on the focused pane, **`Enter`
commits that field**, and `Esc` is two-tiered." `map_key` (`crates/tui/src/terminal/mod.rs`)
honours this with an **unconditional** `KeyCode::Enter => Some(Event::Submit)`. A multiline
text area needs `Enter` to **insert a line break** while editing — but `Enter` is the very key
ADR-0010 §4 bound to commit. Something has to give, and the choice of binding is precisely the
class of cross-cutting interaction decision ADR-0010 was written to settle "once, before code."

The operator's original ask was the chat-app pattern *Enter submits / Shift+Enter breaks the
line*. **That is rejected as terminal-dependent:** distinguishing `Shift+Enter` from `Enter`
requires the **Kitty keyboard protocol** (`PushKeyboardEnhancementFlags`), which Apple Terminal,
most gnome-terminal/VTE builds, plain xterm, and bare tmux do **not** support — there
`Shift+Enter` is byte-identical to `Enter`, so no newline could ever be inserted and the feature
would silently not work for those operators. The terminal init pushes no enhancement flags today
and we will not add a terminal-capability dependency for one field.

### Forces

- **Works in every terminal** (the rejection reason above) — the binding must rely only on keys
  every terminal delivers unambiguously to a raw-mode crossterm reader. `Enter`, `Ctrl+S`, and
  `Esc` all qualify; `Shift+Enter` does not.
- **ADR-0003 layer-2 / ADR-0006 two-step seam.** The whole surface stays `TestBackend`-drivable:
  `map_key` is pure, the update core is pure, and tests construct `Event`s directly. Any new key
  must flow through this seam (a new `Event` variant, mapped by `map_key`, folded by the pure
  core) — no IO inline.
- **`map_key` context-sensitivity must stay correct.** `Enter` must remain `Submit` for the
  single-line Title pane (and everywhere else it commits today — auth, dialogs, list-open,
  create/edit forms); it may become "insert newline" **only** while the multiline Content pane's
  edit buffer is the active text-entry context. A regression here (Title stops committing on
  Enter, or Enter inserts a newline in a single-line field) is the principal risk.
- **Minimal alphabet growth (coding-standards simplicity).** Add the smallest set of `Event`
  variants that expresses the new bindings; do not overload an existing variant with a meaning
  the rest of the core does not expect.

## Decision

**1. Inside the note detail view, the commit key is context-dependent on the focused pane.**

- **Editing the multiline Content pane:** **`Enter` inserts a newline** into the edit buffer,
  and **`Ctrl+S` commits** the field (issuing the existing `UpdateNote` path).
- **Editing the single-line Title pane (and every other commit context in the app — auth,
  dialogs, create/edit forms, list-open, profile-switch):** **`Enter` still commits**,
  unchanged. `Ctrl+S` is inert outside a multiline edit.
- **`Esc` keeps its existing two-tiered behaviour** (cancel an in-progress edit → revert; else
  exit the detail view to the list). Unchanged by this ADR.

This **amends ADR-0010 §4** only for the multiline pane: §4's "`Enter` commits that field" now
reads "`Enter` commits that field, **except** while editing a multiline pane, where `Ctrl+S`
commits and `Enter` inserts a line break." Every other §4 binding stands.

**2. The new bindings flow through the pure seam as two `Event` variants.**

- A new **`Event::Commit`** variant carries the explicit "commit the focused field" intent.
  `Ctrl+S` maps to `Event::Commit` while a text-entry context is active. The note detail's
  field-edit handler treats **both** `Submit` and `Commit` as "commit the field" so the
  single-line Title pane keeps committing on `Enter` (`Submit`) while Content commits on `Ctrl+S`
  (`Commit`).
- A new **`Event::Newline`** variant carries the "insert a line break" intent. `Enter` maps to
  `Event::Newline` **only** when the active text-entry context is the multiline Content edit;
  otherwise `Enter` maps to `Event::Submit` exactly as today. The exact discriminant
  `map_key` uses to recognise "the multiline Content pane is being edited" (a predicate over the
  `Screen`, analogous to the existing `is_text_entry`/`detail_view_open` helpers) is a `tui-dev`
  implementation choice bounded by this ADR; it must not require terminal enhancement flags.

Whether `Newline`/`Commit` are two distinct variants or one is folded into reusing an existing
one is a `tui-dev` choice **bounded** to: (a) `Enter` keeps committing single-line fields, and
(b) the multiline buffer can receive `\n`. The plan specifies the two-variant shape as the
expected design; a narrower equivalent that preserves both invariants is acceptable.

**3. Scope boundary — presentation/interaction only, like ADR-0010 §5.**

- **No `contract`/wire change (#2).** `Note.content` is already a `String`; the multiline buffer
  is the same `edit: Option<String>` field, now allowed to hold `\n`. `UpdateNoteRequest` is
  unchanged.
- **No server change.** Commit still issues the existing `UpdateNote` over the existing client
  method.
- **No new domain structure (#3).** Notes stay `{Title, Content, Created-at}`; the change is how
  Content is *edited and rendered*, not its shape.
- **Profiles stay namespaces (#4)** and the **stateless** invariant (#1) holds — the edit buffer
  is transient process-lifetime UI state, the note is re-derived from the server on commit.

If implementation discovers it cannot meet acceptance without crossing one of these boundaries,
that is an ADR event: work stops and this ADR is amended before re-implementation — never
engineered around on the branch.

## Consequences

- **Positive.** The commit keymap is settled before code, with a terminal-independent binding
  that works everywhere the operator runs the TUI. The ADR-0003/0006 testability seam is
  preserved (new `Event` variants, pure `map_key`, pure core), so the whole multiline surface
  stays `TestBackend`-drivable with the fake `Client` as the only mock. The change is contained
  to the `tui` crate and carries no wire/server/domain risk.
- **Negative / cost.** `map_key` gains a context branch (`Ctrl+S` → `Commit`; `Enter` →
  `Newline` only in the multiline edit), and the `Event` alphabet grows by two variants. The
  detail-edit handler must accept both `Submit` and `Commit` as commit. This is a small but
  genuine increase in the keymap's context-sensitivity — pinned by `map_key`/handler tests so no
  binding silently regresses.
- **Discoverability.** `Ctrl+S` is a new affordance the user must learn; the `?` help overlay and
  the Content pane's caption/label should surface it (a `tui-dev`/plan concern). This is the only
  modifier-key binding besides `Ctrl+C` → Quit.
- **Risk.** The principal risk is the `map_key` context branch leaking — `Enter` must stay commit
  for Title and every existing commit context, becoming newline *only* inside the multiline
  Content edit. Pinned by keybinding tests covering both forks (Title commits on Enter; Content
  inserts newline on Enter and commits on Ctrl+S).
- **Reversibility.** Entirely reversible by `tui`-crate revert; nothing leaves the client and
  `main` carries no wire/server change to unwind.

[feat-0018]: ../../board/features/0018-notes-detail-multiline-content.md
[adr-0010]: ./0010-tui-navigation-and-interaction-model.md
