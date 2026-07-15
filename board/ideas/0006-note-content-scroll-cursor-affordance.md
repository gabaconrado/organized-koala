---
id: 0006
title: Note Content scroll/cursor affordance for content exceeding the pane
status: closed        # open | accepted | closed   (only the human flips open → accepted/closed)
priority: low         # high | medium | low — a triage hint, never a gate
created: 2026-06-28    # absolute date
source: 0018          # Board item / cycle that surfaced it, or "adhoc"
raised-by: reviewer/plan   # reviewer | verifier | eng-manager | architect | orchestrator | human
promoted-to: null     # NNNN of the resulting Board item, set when accepted & promoted
---

## What

0018 made the Notes detail view's **Content** field a multiline text area that fills the
remaining pane height and renders with wrapping. There is currently **no scroll or cursor
affordance**: when a note's Content is longer than the visible pane can show, rendering is
simply top-anchored — the overflow below the pane is not viewable, and there is no visible
cursor/caret indicating the edit position within the buffer. A scroll mechanism (and/or a
visible cursor that the view follows) would let the user view and edit Content that exceeds the
pane height.

## Why it matters

The 0018 change is correct and complete for its stated acceptance — fill the pane, wrap, no
truncation of Title/Created — but content longer than the pane height is editable yet not fully
viewable, and the lack of a caret makes it hard to tell where typing/newlines land in a long
buffer. This is a usability gap for genuinely long notes. It was **deliberately out of scope**
of 0018: the plan recorded it as non-blocking Assumption A5 ("Cursor/scroll for very long
Content is out of scope ... no scroll affordance is in the acceptance criteria") and the
out-of-scope list named it explicitly; the cold reviewer independently flagged the same gap as a
follow-up, not a blocking finding.

## Possible approach

Non-binding sketch: track a scroll offset (and possibly a cursor position) as transient
process-lifetime UI state on the note detail edit buffer (#1-safe — no persistence), have the
render path scroll the Content `Paragraph` to keep the cursor in view, and bind a small set of
movement keys within the multiline edit (e.g. arrows / PageUp/PageDown) that do not collide with
the existing detail-view keymap. This stays `tui`-crate-only with no wire/server/domain change,
and would extend the same `TestBackend` seam 0018 uses. The real plan (and whether a caret is in
scope) is for the `architect` if this is accepted as a feature.

## Disposition

- [x] 2026-07-15 [human] decision: **close** — superseded by feature **0025** (editable text-input
  cursor), which shipped the shared `TextInput` primitive with multiline scroll-to-caret on the
  note-detail Content pane — exactly this idea's ask. Kept as a record (calm log), not deleted.
