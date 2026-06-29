---
id: 0007
title: TUI affordance to delete a single sub-task
status: open          # open | accepted | closed   (only the human flips open → accepted/closed)
priority: low         # high | medium | low — a triage hint, never a gate
created: 2026-06-29    # absolute date
source: 0019          # Board item / cycle that surfaced it, or "adhoc"
raised-by: reviewer   # reviewer | verifier | eng-manager | architect | orchestrator | human
promoted-to: null     # NNNN of the resulting Board item, set when accepted & promoted
---

## What

0019 added sub-tasks with create (`A`), edit-title (`e`), toggle (`Space`), and collapse (`x`)
from the Tasks tab. There is **no key to delete a single sub-task** from the TUI. The server
`DELETE /api/profiles/{pid}/tasks/{tid}/subtasks/{sid}` endpoint and the matching
`delete_subtask` client method + `apply_delete_subtask` fold all exist and are tested, but
nothing in the keymap reaches them — the only way a sub-task disappears today is the cascade when
its parent task is deleted.

## Why it matters

This is **in-scope-correct** for 0019: the acceptance points (1–9) specified create/edit/toggle/
collapse and cascade-on-task-delete, but no delete-single-sub-task key, so the absence is faithful
to the card rather than a defect. The cold reviewer flagged it as a follow-up, not a blocking
finding. The usability gap: a user who adds a sub-task by mistake, or finishes one and wants it
gone without deleting the whole task, has no direct way to remove it. The server/client plumbing
already being present makes this a small, TUI-only follow-up if the operator wants it.

## Possible approach

Non-binding sketch: bind a delete key (e.g. `d` already deletes a *task* when a task row is
selected, per the 0016 scheme — route `d` to delete the selected *sub-task* when a sub-task row is
selected, mirroring how `e`/`Space` already route by row type) to the existing
`Client::delete_subtask` path, with the same confirm/commit lifecycle as task delete. No wire,
server, or domain change — the endpoint and client method already exist — so this is `tui`-crate
only and extends the same `TestBackend` seam 0019's sub-task suite uses. Whether it needs a
confirmation step is for the `architect` if accepted.

## Disposition

- [ ] <ts> [human] decision: accept (→ promote to Board) | close (reason)
