---
id: 0027
title: Correct the stale confirming_delete doc-comment (modal-confirm, not "any navigation")
type: chore         # feature | chore
status: inbox           # inbox → planned → ready → working → review → awaiting-merge → merged | blocked
priority: low       # high | medium | low
parent: null
depends-on: []      # tui-crate doc-comment only; no crate behaviour, no contract, no domain
branch: null
worktree: null
created: 2026-07-15
updated: 2026-07-15
---

## Feature request

**Goal:** Correct the misleading doc-comment on the `confirming_delete` field in
`crates/tui/src/app/task_list.rs` so it describes the real modal-confirm lifecycle.

**Why:** Promoted from idea [`board/ideas/0011-confirming-delete-doccomment-drift.md`][idea-0011]
(surfaced by the reviewer during 0020, operator-accepted 2026-07-15). The `confirming_delete`
field (~line 116) carries a doc-comment saying the armed delete is "cleared on confirm or on any
other navigation." That wording never matched the actual behaviour: delete confirmation is a
**modal confirm** — while armed, `Enter` confirms, `Esc` cancels, and every other key (including
navigation) is **inert** and does **not** disarm (matching the notes/profiles confirm dialog,
ADR-0010 §3). The "any other navigation [disarms]" claim describes an affordance the code never
had. The drift predates 0020 (the field's type changed `Option<String>` → `Option<DeleteTarget>`
in 0020, but the misleading comment rode along untouched). Left as-is, the comment invites a
future "fix" that adds a disarm-on-navigation the modal design deliberately omits.

**Acceptance criteria:**

- [ ] The `confirming_delete` doc-comment states the real modal-confirm lifecycle: armed by `d`
      (by selected-row kind, `DeleteTarget::Task` | `Subtask`), confirmed by `Enter`, cancelled by
      `Esc`, all other keys inert while armed (no disarm-on-navigation).
- [ ] **Chore invariant holds:** doc-comment-only. No `tui` behaviour change, no `contract`/wire
      shape (#2), no domain structure (#3). The reviewer's approval must attest this invariant.
- [ ] `./ok.sh test | lint | fmt --check` green (unchanged by this change).

**Notes:** Comment-only; no test change strictly needed. The existing "non-confirm key issues no
delete" pin in `crates/tui/tests/tasks.rs` already documents the true behaviour and can be cited
in the corrected comment.

[idea-0011]: ../ideas/0011-confirming-delete-doccomment-drift.md
