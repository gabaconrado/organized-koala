---
id: 0008
title: Drop the commit-sha citation from the help-overlay regression test comment
status: open          # open | accepted | closed   (only the human flips open → accepted/closed)
priority: low         # high | medium | low — a triage hint, never a gate
created: 2026-06-29    # absolute date
source: 0019          # Board item / cycle that surfaced it, or "adhoc"
raised-by: reviewer   # reviewer | verifier | eng-manager | architect | orchestrator | human
promoted-to: null     # NNNN of the resulting Board item, set when accepted & promoted
---

## What

The 0019 help-dialog-fix re-entry added the regression test
`help_modal_tasks_line_renders_intact_without_wrapping_d_delete` in
`crates/tui/tests/dialogs.rs`. Its explanatory comment cites the fixing commit sha (`5fc5021`)
inline. `coding-standards` discourages development context (commit shas, PR numbers, "fixed in …")
in comments — the comment should explain the *invariant being pinned* (the Tasks reference line
must not wrap `d delete` to an un-indented continuation), not the commit that introduced the fix.

## Why it matters

Purely a comment-hygiene nit — no behaviour, no test logic. The cold reviewer flagged it
non-blocking and explicitly routed it here rather than requesting a change: fixing it now would
itself alter the `crates/` code-tree hash and **void the just-issued `approved`/`verified`
verdicts**, forcing another full review+verify pass for a one-line comment edit — disproportionate
mid-cycle. It is out of scope of the help-dialog *rendering* fix the cycle was driving, so it is
parked here for the human to fold into a future `tui`-tests touch (or a batched comment-cleanup
chore) rather than churning 0019.

## Possible approach

Non-binding: when `crates/tui/tests/dialogs.rs` is next edited for another reason, reword the
comment to describe the wrap invariant and drop the `5fc5021` reference. Too small to warrant its
own cycle; best as a free pickup riding a future `tui`-tests change, or rolled into a broader
comment-hygiene `chore` if one is ever minted.

## Disposition

- [ ] <ts> [human] decision: accept (→ promote to Board) | close (reason)
