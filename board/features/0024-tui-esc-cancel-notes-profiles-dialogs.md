---
id: 0024
title: Esc does not cancel the Notes/Profiles create·edit·delete dialogs (idle, no request in flight)
type: feature       # feature | chore
status: inbox           # inbox → planned → ready → working → review → awaiting-merge → merged | blocked
priority: high      # high | medium | low
parent: null
depends-on: []
created: 2026-07-15
updated: 2026-07-15
---

## Feature request

**Operator-flagged bug (confirmed by code trace + a throwaway `TestBackend` probe).** When a
Notes or Profiles modal dialog is open and **no request is in flight**, pressing `Esc` does
**not** close it. The dialog stays open; only `Enter`/submit gets the user out. This affects
**six** dialogs — every create/edit/delete sub-flow in the Notes and Profiles panes. The Tasks
pane and the Notes detail view are **not** affected.

### Affected dialogs

| Pane | Dialog | Handler | Esc closes? |
| --- | --- | --- | --- |
| Notes | Create | `crates/tui/src/app/notes.rs:476` `handle_create_event` | ❌ |
| Notes | Edit | `crates/tui/src/app/notes.rs:494` `handle_edit_event` | ❌ |
| Notes | Delete-confirm | `crates/tui/src/app/notes.rs:512` `handle_delete_event` | ❌ |
| Profiles | Create | `crates/tui/src/app/profiles.rs:220` `handle_create_event` | ❌ |
| Profiles | Rename | `crates/tui/src/app/profiles.rs:237` `handle_rename_event` | ❌ |
| Profiles | Delete-confirm | `crates/tui/src/app/profiles.rs:254` `handle_delete_event` | ❌ |
| Notes | Detail view / detail field-edit | `notes.rs:448` / `notes.rs:436` | ✅ (handles `Cancel`) |
| Tasks | *all* sub-flows | `crates/tui/src/app/task_list.rs` (9 handlers) | ✅ |

### Root cause

`Esc` correctly maps to `Event::Cancel` while a dialog captures input
(`crates/tui/src/terminal/mod.rs:208`). But at the App level, `Event::Cancel` is only acted on
**while a request is in flight**:

```rust
// crates/tui/src/app/mod.rs:452
Event::Cancel if self.is_pending() => { self.cancel_in_flight(); None }
```

When **idle** (the normal case — the user opens a dialog with no request pending), `Cancel`
falls through to the per-pane screen handler, which is expected to reset its own mode. The
**Tasks** pane does exactly this in all of its sub-flow handlers (e.g.
`task_list.rs:898` `Event::Cancel => self.adding = None`). The **Notes** and **Profiles**
create/edit/delete handlers instead match only `Char`/`Backspace`/`Next`/`Prev`/`Submit` with a
`_ => {}` catch-all — silently dropping `Cancel` — while a **misleading comment** (`notes.rs:517`,
`profiles.rs:259`) claims *"Cancel (Esc) is handled by the caller's cancel path,"* a path that
only exists for the in-flight case.

The note **detail** handler (`notes.rs:448` `Event::Cancel => self.mode = NotesMode::List`) shows
the intended pattern; the five idle text-entry/confirm handlers listed above are missing it.

### Why the tests didn't catch it

The existing Notes cancel tests (`crates/tui/tests/notes.rs`) exercise `Event::Cancel` only on
the **in-flight** path (stale-response-after-cancel) and on the **detail** view — never on an
**idle** create/edit/delete dialog. Profiles has no idle-cancel coverage either. So the gap is a
test blind spot, not just a source omission.

### Scoped change (sketch — the real plan is the architect's)

Add an `Event::Cancel => self.mode = <List>` arm to the five idle handlers that lack it
(mirroring the note-detail handler at `notes.rs:448` and the Tasks handlers), and fix the two
misleading doc-comments. `tui`-crate-only: no `contract`/wire (#2), no server, no
domain-structure (#3) change. Because it changes observable interactive behaviour it is a
**`feature`**, not a chore — the `tester` slice must add idle-`Esc`-cancels regression coverage
for each of the six dialogs (owned by `tester`'s `TestBackend` suite per ADR-0003; the live
verifier confirms that suite exists and is green for this TUI-only change).

### Acceptance criteria

- [ ] Pressing `Esc` on an **idle** (no in-flight request) Notes **Create** dialog closes it back
      to the list, discarding the draft; asserted by a `TestBackend` test.
- [ ] Same for the Notes **Edit** and **Delete-confirm** dialogs.
- [ ] Same for the Profiles **Create**, **Rename**, and **Delete-confirm** dialogs.
- [ ] The in-flight `Esc`→cancel-request behaviour is unchanged (existing stale-response tests
      still green).
- [ ] The misleading "handled by the caller's cancel path" comments are corrected.
- [ ] `./ok.sh test | lint | fmt --check` green.

## Log / comments

- 2026-07-15 [orchestrator] Filed at operator request after a confirmed investigation (code
  trace of the `Esc`→`Cancel` routing in `terminal/mod.rs` + `app/mod.rs`, cross-checked against
  the Tasks handlers, and empirically confirmed with a throwaway `TestBackend` probe that failed
  for both the Notes-create and Profiles-create dialogs — probe removed). Awaiting `architect`
  planning. Operator will kick off the fix manually.
