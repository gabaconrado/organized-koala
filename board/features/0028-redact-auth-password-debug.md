---
id: 0028
title: Redact the auth password entry buffer so it is not reachable via derived Debug
type: feature       # feature | chore
status: inbox           # inbox → planned → ready → working → review → awaiting-merge → merged | blocked
priority: low       # high | medium | low
parent: null
depends-on: []
branch: null
worktree: null
created: 2026-07-15
updated: 2026-07-15
---

## Feature request

**Goal:** The login/register password entry buffer must not be reachable through a derived
`Debug` implementation, so a plaintext password cannot leak into a log/trace/panic message.

**Why:** Promoted from idea [`board/ideas/0012-redact-auth-password-entry-buffer.md`][idea-0012]
(surfaced by the reviewer during 0025, operator-accepted 2026-07-15). `AuthState` derives `Debug`
and holds the login/register password as a plaintext entry buffer (a `TextInput` after 0025, a
`String` before — the leak surface is identical either way; **not** a 0025 regression). The
rust-standards secret rule warns against a secret being reachable from a `Debug` impl. The JWT
session bearer is already wrapped (`SessionToken`) and the buffer is wrapped in `Password::new(…)`
at submit, so the only remaining leak surface is the *entry* buffer while typing. Low severity
(transient, process-lifetime UI state) but the kind of thing worth closing.

**Scope note (architect to settle at plan/triage):** the idea defers the scope decision to the
`architect` — whether this is a `feature` or a scope-limited `chore`, and which redaction shape
(a redacting newtype around the entry field with a manual `Debug` printing `"***"`, or a manual
`Debug` impl on `AuthState` that redacts the password field(s)). Promoted as `feature` pending
that triage. `tui`-crate-only; no wire/contract change expected.

**Acceptance criteria (provisional — architect confirms at plan time):**

- [ ] The auth password entry buffer is **not** reachable as plaintext through any `Debug` output
      (`format!("{:?}", …)` on `AuthState` / the field prints a redacted placeholder, not the
      typed password).
- [ ] Login/register still function unchanged — the password is still submitted correctly
      (wrapped in `Password::new(…)`); only the `Debug` reachability changes.
- [ ] A test pins that the redacted `Debug` output does not contain the typed password.
- [ ] `tui`-crate-only; no `contract`/wire shape (#2) change, no domain structure (#3) change.
- [ ] `./ok.sh test | lint | fmt --check` green.

[idea-0012]: ../ideas/0012-redact-auth-password-entry-buffer.md
