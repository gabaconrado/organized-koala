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
- **Lints:** any `#[allow]` without a documented justification.
- **Standards:** deep modules, tests in own files + public-API-only, `thiserror` (lib) /
  `anyhow` (bin), short scoped comments, reference-style markdown, `"${VAR}"` bash.
- **Blast radius & simplicity:** is this the smallest correct change?

### 3. Post the verdict

Append findings to the item's `## Log / comments`, then a final line:

```text
REVIEW-STATUS: approved        <commit-sha>
REVIEW-STATUS: changes-requested   <commit-sha>
```

Approval requires `<commit-sha>` == current branch head; re-review when the head advances.
Fix-now findings go back to the owning dev agent (the reviewer does not edit code).
