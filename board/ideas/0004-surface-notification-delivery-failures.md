---
id: 0004
title: Surface timer-notification delivery failures to the user
status: open
priority: low
created: 2026-06-28
source: 0017
raised-by: reviewer
promoted-to: null
---

## What

In 0017 the production `DesktopNotifier` maps **every** notification delivery failure (no
daemon, unsupported platform) to a silent no-op — it writes nothing and returns nothing the
caller can observe (plan Assumption A2). So if a focus session ends and the notification
cannot be delivered (e.g. no notification daemon on the session bus), the user gets **no
feedback at all** that delivery was attempted-and-failed. This idea is to offer the user an
optional, unobtrusive signal that a notification could not be delivered.

## Why it matters

The "silent and non-fatal" requirement was the binding acceptance criterion, and dropping the
log avoided both a new logging dependency and alt-screen terminal corruption (writing to
stdout/stderr corrupts the ratatui display). That was the right call for the feature's scope.
But a user on a bare TTY / headless / SSH session would never learn their timer-end
notifications are silently being discarded — they'd just think the feature is broken. Surfacing
the failure in-band (not via the terminal stream) would close that gap. It was explicitly out
of scope of 0017 (A2 named it a follow-up).

## Possible approach

A non-binding sketch (the architect writes any real plan): surface the last delivery outcome
**in-band in the TUI** rather than via a terminal write — e.g. a one-line, transient status hint
in the timer widget ("notification not delivered") when `.show()` fails, derived from a new
best-effort return on the `Notifier` trait. This keeps the alt-screen safe (no stdout/stderr)
and adds no logging dependency. Care needed: it must stay #1-safe (transient in-memory only),
must not become noisy, and must not change the wire/contract (#2) or domain (#3).

## Disposition

- [ ] <ts> [human] decision: accept (→ promote to Board) | close (reason)
