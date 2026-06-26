---
id: 0013
title: Redact the JWT in tui `Session` — bare `String` reachable via derived `Debug`
type: chore         # feature | chore
status: working         # inbox → planned → ready → working → review → awaiting-merge → merged | blocked
priority: high      # high | medium | low
parent: null
depends-on: []
created: 2026-06-25
updated: 2026-06-25
---

## Feature request

**Security defect (operator-flagged high priority).** `crates/tui/src/app/mod.rs` defines

    #[derive(Debug, Clone)]
    pub struct Session {
        pub token: String,        // ← the bearer JWT, a secret, held bare
        pub profile_id: String,
        pub profile_name: String,
    }

The bearer JWT is held as a **bare `String`** inside a struct that **derives `Debug`**. This is a
direct violation of `rust-standards` → *Sensitive data*: *"**Never** `#[derive(Debug)]` a struct …
that holds a bare secret — hold it as a `Secret<_>` (or hand-write a redacting `Debug`). A bare
secret reachable from a `Debug` impl, a log, or auto-instrumentation is a **review-blocking
leak**."* The token can leak through any `{:?}` of `Session` (or anything embedding it) — a debug
log line, a `tracing` span field, a panic message, or future auto-instrumentation.

The redacting pattern is already established in-repo: `contract::Password` is a newtype whose
hand-written `Debug` renders `[REDACTED]` (`crates/contract/src/auth/mod.rs`). `rust-standards`
additionally prefers `secrecy::SecretString` (redacts **and** zeroizes on drop).

**Scoped change.** Stop the JWT being reachable from any `Debug`/log/trace path, with the smallest
correct change. The token's *value and usage are unchanged* — it still flows over the wire
identically; only its in-memory representation and `Debug` rendering change. Two acceptable shapes
(implementer picks the smaller correct one):

1. Hold `Session.token` as `secrecy::SecretString` (rust-standards' preferred — redacts + zeroizes),
   exposing the inner value only at the point of use (`expose_secret()`) where the request worker
   needs the bearer string. Requires adding the `secrecy` dependency to the `tui` crate.
2. A local redacting newtype (e.g. `SessionToken(String)` with a hand-written `Debug` → `[REDACTED]`),
   mirroring `contract::Password`, if pulling in `secrecy` is judged disproportionate for one field.

Audit the rest of `crates/tui/src/` for any **other** bare secret reachable from `Debug` (e.g. the
`ClientRequest`/`Outcome` protocol variants and any worker struct that carry `token`) and apply the
same redaction so the secret is not merely moved from one derived `Debug` to another.

**Acceptance criteria:**

- [ ] The bearer JWT is **not reachable** from any derived/`{:?}` `Debug` in `crates/tui/` — a test
      asserts `format!("{:?}", session)` (and any protocol/worker type that carries the token) does
      **not** contain the token and renders `[REDACTED]` (mirroring `contract`'s `Password` doctest
      `assert_eq!(format!("{:?}", req.password), "[REDACTED]")`).
- [ ] The token still flows correctly: register/login → authenticated task/note/profile/timer calls
      all still carry the correct bearer string (exposed only at the point of use). No behavioural
      change to the wire surface.
- [ ] No `contract`/wire change (#2): `Session` is a `tui`-internal struct, **not** a `contract` DTO;
      `SessionResponse`/`Password` and all wire shapes are untouched. No domain-structure change (#3),
      no observable product-behaviour change beyond `Debug` rendering.
- [ ] Lighter `chore` DoD: `./ok.sh test | lint | fmt --check` green; cold `reviewer` **approved**
      with the **chore-invariant attestation** (no behaviour / no `contract`-wire (#2) / no
      domain-structure (#3) change), pinned to `./ok.sh code-hash`. Live verifier pass **skipped**
      (chore track — no live-observable change).

**Scope guard.** If implementing this is found to require a `contract`/wire change (it should not —
`SessionResponse.token` over the wire is unchanged; only the TUI's in-memory holding changes) or to
alter observable behaviour, it is **no longer a chore**: set `blocked` and route to `architect` to
re-type it `feature` (with an ADR if a wire change is involved), per the CLAUDE.md scope guard.

## Log / comments

- 2026-06-26 [orchestrator] Claimed minted chore; cut worktree
  `feature/0013-session-token-debug-leak` from `main@3b60289`; status → working. Session: drive-0013.

- [x] 2026-06-25 [operator] High-priority: this is a serious problem — a session JWT reachable
      from a derived `Debug`. Why did it pass our guidelines (secrecy / manual `Debug`)?
      **Root cause (answered):** the `Session` struct was introduced in 0004 (`4b9eda0`,
      2026-06-22) — **after** both the `rust-standards` secret rule (`e50b48f`, 2026-06-11) and
      the `contract::Password` redacting template (`3501468`, 2026-06-12) already existed. So it
      was a violation of a **pre-existing documented rule**, missed by the 0004 author and the
      0004 cold reviewer, then carried silently through 0005/0008/0010/0011 because cold review is
      **diff-scoped** (pre-existing code is out of each cycle's review scope) until 0012's
      reviewer flagged it as a pre-existing nit. Contributing factors: (1) the rule is
      **prose-only**, with no clippy/lint enforcement (no mechanical guard for "bare secret
      reachable from `Debug`"); (2) `[workspace.lints] rust.missing_debug_implementations =
      "deny"` actively pushes devs to add `#[derive(Debug)]` to **every** struct, colliding with
      the secret rule — and by default the bare derive wins. Possible `eng-manager` follow-up
      (separate from this fix): a mechanical guard and/or a `rust-standards` callout on the
      `missing_debug_implementations`-vs-secret-redaction tension.
