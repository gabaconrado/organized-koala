---
id: 0005
title: Move notification .show() off the poll loop if it ever blocks materially
status: open
priority: low
created: 2026-06-28
source: 0017
raised-by: reviewer
promoted-to: null
---

## What

In 0017 the notification effect (`notify-rust` `.show()`) is fired **synchronously on the poll
loop / edge thread**, right after a worker response is drained and folded (plan Assumption A6).
`.show()` returns quickly (it sends a D-Bus message), so the synchronous call was accepted
rather than threading it through the worker request protocol. This idea is to move the effect
onto the worker thread **if** `.show()` is ever found to block materially on some platform.

## Why it matters

The TUI render loop is responsive precisely because all I/O lives on the worker thread
(ADR-0006); a synchronous effect on the poll loop is only safe while it stays fast. On Ubuntu's
default backend the call is a cheap async D-Bus message and the assumption holds. But a future
platform/backend where `.show()` blocks (e.g. waiting on a slow or absent daemon with a
timeout) would stall the render loop for that duration — a UI-responsiveness regression. It was
out of scope of 0017 because no such blocking was observed; A6 named it a follow-up, contingent
on evidence.

## Possible approach

A non-binding sketch (the architect writes any real plan if accepted): add a fire-and-forget
notification step to the worker protocol so the effect runs off the render thread, or spawn a
short-lived detached thread for the `.show()` call. Either keeps ADR-0006's request protocol's
intent (no I/O on the render thread) and changes no wire/contract (#2) or domain (#3). Only
worth doing **with evidence** that `.show()` blocks materially somewhere — optimize on measured
blocking, not speculatively (coding-standards priority order).

## Disposition

- [ ] <ts> [human] decision: accept (→ promote to Board) | close (reason)
