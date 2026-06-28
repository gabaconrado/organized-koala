---
id: 0003
title: Fix stale `NotesMode::Viewing` doc comment in notes.rs after 0016 rename
status: open          # open | accepted | closed   (only the human flips open → accepted/closed)
priority: low         # high | medium | low — a triage hint, never a gate
created: 2026-06-28    # absolute date
source: 0016          # Board item / cycle that surfaced it, or "adhoc"
raised-by: reviewer   # reviewer | verifier | eng-manager | architect | orchestrator | human
promoted-to: null     # NNNN of the resulting Board item, set when accepted & promoted
---

## What

The `open_selected` rustdoc at `crates/tui/src/app/notes.rs:341` still says it "folds the result
into `NotesMode::Viewing`", but 0016 renamed `NotesMode::Viewing` to `NotesMode::Detail` (the
read-only viewer became the editable per-field detail view). The comment is the only remaining
`Viewing` reference left in `crates/tui/src/`.

## Why it matters

Cosmetic doc drift only — no behaviour, contract, or domain impact, and the code itself is
correct. It was out of scope of 0016 (which is presentation behaviour, not doc-comment hygiene)
and is not review-blocking, so it was parked here rather than folded into the feature. Left
unfixed it is a small future-reader trip hazard (a comment naming an enum variant that no longer
exists).

## Possible approach

A one-line `chore` (doc-comment fix): update the `notes.rs:341` rustdoc to say `NotesMode::Detail`.
Scope-limited, no behaviour/contract/domain change — a textbook chore the orchestrator can mint
directly if the human accepts.

## Disposition

- [ ] <ts> [human] decision: accept (→ promote to Board) | close (reason)
