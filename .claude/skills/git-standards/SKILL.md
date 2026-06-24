---
name: git-standards
description: Git standards for organized-koala — Conventional Commits, agent co-authorship, no remote writes, and fast-forward-rebase-only linear history. Loaded by every agent that runs git. Extended over time via learnings + human feedback.
audience: dev
---

# Git standards

## When to invoke

- Before any `git commit`, branch update, or history-rewriting operation, in any crate or
  worktree.
- Whenever you integrate a branch, land review fixes, or read remote state.

## The standards

- **Conventional Commits.** Every message follows the Conventional Commits v1.0.0 spec:
  `<type>[optional scope]: <description>`, with an optional body and footers. Types:
  `feat`, `fix`, `docs`, `refactor`, `test`, `chore`, `build`, `ci`, `perf`, `style`. Add a
  scope when it sharpens intent (`feat(contract): add TaskStatus enum`). Breaking changes use
  a `!` after the type/scope or a `BREAKING CHANGE:` footer. Spec:
  <https://www.conventionalcommits.org/en/v1.0.0/#specification>.
- **Co-author footer names the committing agent.** End every commit with a trailer
  identifying the agent that authored it:

  ```text
  Co-authored-by: <agent> <agent@organized-koala.local>
  ```

  e.g. `Co-authored-by: server-dev <server-dev@organized-koala.local>`. **The top-level
  orchestrator commits as `claude` with the same domain form — `Co-authored-by: claude
  <claude@organized-koala.local>` — and this applies to its Board-only commits too** (claim
  snapshots, status flips, recorded reviewer/verifier verdicts), not only code commits
  (learned 0004: the 0004 board-claim commit used `<noreply@anthropic.com>`; the reviewer
  flagged it as a nit). This replaces any default assistant co-author trailer. **The footer
  identity is owned by this skill — never copy a trailer out of a dispatch prompt** (learned
  0003: a dispatch prompt hardcoded `<noreply@anthropic.com>`, so a fix committed with the
  wrong email). The `<agent>@organized-koala.local` form here is the only authority;
  `<noreply@anthropic.com>` is never correct in this repo. **This keeps recurring** — a dispatch
  prompt hardcoded `<noreply@anthropic.com>` again on 0009; the durable fix is on the *dispatcher*
  side (`drive` "Dispatch discipline": never write a `Co-authored-by:` line into a dispatch
  prompt). Derive the trailer from this skill; never copy one from a prompt.
- **Never write to the remote.** Agents do **not** `git push` (nor `push --force`, nor push
  tags) — this is enforced by the permission deny-list, not just convention. Reading the
  remote is fine: `git fetch`, `git log origin/<branch>`, diffing against `origin/...`. Every
  remote-mutating action is the human's.
- **Linear history — fast-forward rebase only.** Integrate a branch by **rebasing onto its
  base** (`git fetch` then `git rebase origin/main`), never by merging the base into it. No
  merge commits, no squash-merge, and no `git pull` (it can synthesize a merge). Keep the
  branch rebased so the human's final merge is always a fast-forward.

## Example

```sh
git add crates/server/src/tasks.rs
git commit -m "feat(server): scope task queries to the active profile

Co-authored-by: server-dev <server-dev@organized-koala.local>"

# integrate upstream changes — rebase, never merge
git fetch origin
git rebase origin/main
```

## Extending this skill

Living document — `eng-manager` appends durable git learnings + human feedback here.
