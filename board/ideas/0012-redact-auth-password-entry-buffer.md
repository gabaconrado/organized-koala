---
id: 0012
title: Redact the auth password entry buffer so it is not reachable via derived Debug
status: open          # open | accepted | closed   (only the human flips open → accepted/closed)
priority: low         # high | medium | low — a triage hint, never a gate
created: 2026-07-15    # absolute date
source: 0025          # Board item / cycle that surfaced it, or "adhoc"
raised-by: reviewer   # reviewer | verifier | eng-manager | architect | orchestrator | human
promoted-to: null     # NNNN of the resulting Board item, set when accepted & promoted
---

## What

`AuthState` derives `Debug` and holds the login/register password as a plaintext entry buffer.
Before 0025 this buffer was a `String`; after 0025 it is a `TextInput` — either way the plaintext
password typed at the login/register screen is reachable through the derived `Debug`. The JWT
session bearer is already properly wrapped (`SessionToken`), and the buffer is wrapped in
`Password::new(...)` at submit, so the leak surface is only the *entry* buffer while typing, and it
is **identical before and after 0025** (not a 0025 regression).

## Why it matters

The rust-standards secret rule warns against a secret being reachable from a `Debug`
implementation (it can end up in a log/trace/panic message). This is exactly that class, on the
password *entry* buffer. It is low severity (transient, process-lifetime UI state; the token and
submitted password are already wrapped) but is the kind of thing worth closing. It was out of scope
of 0025 because 0025 is a purely mechanical `String`→`TextInput` field-type change with no security
delta — fixing the `Debug` reachability is an independent, pre-existing concern.

## Possible approach

Non-binding sketch: give the auth password entry field a redacting holder (a newtype around
`TextInput` / `String` with a manual `Debug` that prints `"***"`), or add a manual `Debug` impl on
`AuthState` that redacts the password field(s). Keep it `tui`-crate-only; no wire/contract change.
The architect decides scope at triage.

## Disposition

- [ ] <ts> [human] decision: accept (→ promote to Board) | close (reason)
