---
id: 0009
title: Local-date "today" grouping in the Tasks pane (currently UTC civil day)
status: open          # open | accepted | closed   (only the human flips open → accepted/closed)
priority: low         # high | medium | low — a triage hint, never a gate
created: 2026-07-02    # absolute date
source: 0020          # Board item / cycle that surfaced it, or "adhoc"
raised-by: reviewer   # reviewer | verifier | eng-manager | architect | orchestrator | human
promoted-to: null     # NNNN of the resulting Board item, set when accepted & promoted
---

## What

0020's Tasks-pane today/older split and the top-center date header are computed against a
**UTC civil day** (epoch-seconds `div_euclid` 86400), not the operator's **local** date. This
was a deliberate, recorded smallest-change decision (0020 assumptions A5/A8): the `tui` crate
carries no timezone dependency, and pulling one in would be a #6 / ADR event, so the dev used
UTC to stay chrono-free and deterministic under test. A future refinement could compute the
operator's local date instead.

A companion **doc-consistency** fix rides with this: ADR-0014 §5 and the 0020 `## Plan(s)` S3
text still say "local date," which no longer matches the shipped UTC behaviour. Whichever way
this idea is dispositioned, those two docs should be reconciled (either updated to say "UTC
civil day" if UTC is kept, or updated alongside the local-date change).

## Why it matters

For an operator west of UTC, a task created late in their local evening can land in the "today"
group only until UTC midnight (a few hours), or a task created just after local midnight can
show as "older" — a cosmetic grouping-edge surprise near the day boundary. Low impact at
personal single-user scale, but it is a visible behaviour vs. the operator's stated "local date"
expectation. Out of scope for 0020 because closing the gap needs a **sanctioned timezone
capability** (a dependency + an ADR), which is exactly the kind of fork the AFK policy defers to
human triage rather than improvising.

## Possible approach

Non-binding sketch: (a) decide whether local-date grouping is wanted at all, or whether UTC is
acceptable and the docs should simply be corrected to match; (b) if local is wanted, an
`architect` ADR sanctioning a minimal timezone approach (a vetted crate, or a server-provided
local-day boundary so the TUI stays chrono-free and stateless per #1); (c) reconcile ADR-0014 §5
and the plan text either way.

## Disposition

- [ ] <ts> [human] decision: accept (→ promote to Board) | close (reason)
