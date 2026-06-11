---
name: review
description: The pre-merge gate — run build/test/lint/format, hunt contract drift and hard-constraint violations, post findings + a machine-readable verdict into the Board Log.
audience: dev
---

# review

## When to invoke

- By `reviewer` when an item is in `status: review`. The reviewer must be **cold** — it did
  not write the code under review.

## Procedure

### 1. Mechanical gate

Run, and quote the result of, each:

- `./ok.sh test`
- `./ok.sh lint`
- `./ok.sh fmt --check`

Any failure ⇒ `changes-requested`.

### 2. Substantive review (read the diff cold)

- **Contract drift:** is any DTO/error shape defined outside the `contract` crate? Do server
  and TUI agree with `contract`? A contract change without an ADR is a blocker.
- **Hard-constraint violations:** client-side/local state in the TUI; non-profile-scoped
  query; domain bloat (subtasks/tags/categories/per-profile timer); external auth. Any ⇒ blocker.
- **Error contract:** errors return HTTP status + `{ code?, message }`.
- **Sensitive-data leaks:** secrets (passwords, tokens, JWT/session keys, DB credentials)
  must be wrapped in `secrecy` and must never be reachable from a `Debug`/`Display` impl, a
  log/trace line, or an auto-instrumented span/endpoint field — e.g. a `#[tracing::instrument]`
  capturing a secret argument, or a handler logging request bodies/headers. Any leak ⇒ blocker.
- **Lints:** any `#[allow]` without a documented justification.
- **Standards:** deep modules, tests in own files + public-API-only, `thiserror` (lib) /
  `anyhow` (bin), short scoped comments, reference-style markdown, `"${VAR}"` bash.
- **Blast radius & simplicity:** is this the smallest correct change?

### 3. Report the verdict

You are **read-only on everything — code AND Board.** Hand findings + a final verdict line back
to the orchestrator; the orchestrator commits the verdict onto the item **on the branch** (the
item is feature-local and travels with the code). Do **not** edit or commit the Board yourself,
and leave no scratch (`*.tmp`) files behind:

```text
REVIEW-STATUS: approved        <commit-sha>
REVIEW-STATUS: changes-requested   <commit-sha>
```

`<commit-sha>` names the reviewed **code** sha and must equal the current branch head. A
Board-only commit (status flip / a verdict the orchestrator records) does **not** invalidate an
approval — only a new code/test commit does; re-review when a code commit advances the head.
Fix-now findings go back to the owning dev agent (the reviewer does not edit code).
