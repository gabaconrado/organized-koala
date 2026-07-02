---
id: 0011
title: Fix stale confirming_delete doc-comment (modal-confirm, not "any navigation")
status: open          # open | accepted | closed   (only the human flips open → accepted/closed)
priority: low         # high | medium | low — a triage hint, never a gate
created: 2026-07-02    # absolute date
source: 0020          # Board item / cycle that surfaced it, or "adhoc"
raised-by: reviewer   # reviewer | verifier | eng-manager | architect | orchestrator | human
promoted-to: null     # NNNN of the resulting Board item, set when accepted & promoted
---

## What

The `confirming_delete` field in `crates/tui/src/app/task_list.rs` (~line 116) carries a
doc-comment saying the armed delete is "cleared on confirm or on any other navigation." That
wording never matched the actual behaviour: delete confirmation is a **modal confirm** — while it
is armed, `Enter` confirms, `Esc` cancels, and every other key (including navigation) is **inert**
and does **not** disarm (matching the notes/profiles confirm dialog, ADR-0010 §3). The comment's
"any other navigation [disarms]" claim describes an affordance the code never had. The drift
predates this cycle (the field's type changed `Option<String>` → `Option<DeleteTarget>` in 0020,
but the misleading comment rode along untouched).

## Why it matters

Pure doc-comment accuracy — no behaviour, wire, or domain implication, so it was out of scope of
0020's operator-feedback re-entry (which fixed the three interaction adjustments, not a comment).
Left as-is, the comment actively misleads a future reader about the delete-confirm lifecycle,
inviting a "fix" that introduces a disarm-on-navigation the modal design deliberately omits. A
one-line correction removes that trap. This is a natural **`chore`** candidate if the human
accepts it (test-/comment-only, no behaviour change, no `contract`/wire (#2), no domain (#3)).

## Possible approach

Non-binding sketch: reword the doc-comment to state the real modal-confirm lifecycle — armed by
`d` (by selected-row kind, `DeleteTarget::Task` | `Subtask`), confirmed by `Enter`, cancelled by
`Esc`, all other keys inert while armed. `tui`-crate comment-only; no test change strictly needed,
though the existing "non-confirm key issues no delete" pin in `crates/tui/tests/tasks.rs` already
documents the true behaviour and can be cited.

## Disposition

- [ ] <ts> [human] decision: accept (→ promote to Board) | close (reason)
