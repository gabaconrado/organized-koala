---
name: drive
description: The manual-cycle orchestrator and human entrypoint. Reads the Board, dispatches the agent subagents in sequence, advances ONE work item to awaiting-merge, then stops. Run /drive to start a cycle.
audience: dev
---

# drive

## When to invoke

- The human runs `/drive` in a session to advance the workflow. This top-level session **is**
  the orchestrator — it dispatches the agents as subagents (via the Agent/Task tool) in order.

This skill is the **single home of the cycle definition**. The `autowork` loop wraps this same
logic in a `ScheduleWakeup` re-arm; it does not re-define the cycle.

## Procedure

> Every step **recomputes state** from the Board (`board/features/*.md` frontmatter + Log) and
> `git` (`git worktree list`, branch heads). There is no scratch state file.

### 0. Feedback sweep (FIRST — outranks claiming new work)

Scan **all non-merged** items for an unchecked `- [ ] … [human]` line in `## Log / comments`.
If any exists, re-enter it **before** picking a new `ready` item:

- Dispatch `architect` to triage the feedback to the **smallest** re-entry point (see CLAUDE.md
  "Feedback re-entry" table). Scope/approach feedback ⇒ `architect` writes/amends an ADR first,
  item → `planned` → `ready`. Behaviour tweak ⇒ `working`. Review/verify concern ⇒ `review`.
  Doc/process ⇒ `eng-manager` only. Clarification ⇒ check the box with a note.
- Run the cycle **forward from the re-entry point**; `eng-manager` always runs at the tail.
- The owning agent checks the box `[x]` (with resolution + commit) only once resolved at head
  and re-reviewed; the item returns to `awaiting-merge`.
- Feedback on a `merged` item does NOT re-enter — `architect` creates a new linked Board item.

### 1. Triage → plan

For the highest-priority `inbox` item: dispatch `architect` (runs the `plan` skill). It writes
plan(s), sets `status: planned`, and after self-acceptance (optional `grill`) → `ready`. A large
request may fan into several items.

**Then COMMIT the planning artifacts to `main` before leaving this step** — the ADR(s),
`docs/decisions.md` index, and the planned/ready Board item(s). Planning lives on `main`, not on
a feature branch. (Learned 0002: an ADR left uncommitted in the working tree does not exist in a
worktree cut from the prior commit, so the code's `(see ADR-NNNN)` citations dangle and the dev
agent blocks. The worktree in step 2 **must** be cut from the commit that carries the plan.)

### 2. Claim + isolate

For the highest-priority, oldest `ready` item, cut a worktree + branch **from the `main` commit
that carries the item's plan + ADR** (the commit from step 1 — verify the ADR is present there,
not just in the working tree):

```sh
git worktree add -b feature/NNNN-<slug> .claude/worktrees/NNNN-<slug> <base>
```

Record `branch` and `worktree` in frontmatter; set `status: working`; stamp a session id in
the Log — **committed on the branch**. From the claim onward the **branch's copy of the item is
authoritative**: status flips, per-slice Log entries, verdicts, and the `## Summary` are all
committed on the branch (feature-local state, CLAUDE.md home #2). Leave `main`'s copy frozen at
the claim snapshot with a one-line pointer note; the human's merge brings the finished item back
to `main` atomically with the code. **Never commit shared/cross-cutting state (ADRs, the
decisions index, `ok.sh`, `.githooks/`, docker/OTel config, `CLAUDE.md`, standards skills,
`.claude/` agent-skill defs) onto the branch** — those live on `main` only (home #1); committing
one on a branch is the out-of-sync bug class.

### 3. Build (in the worktree)

Dispatch the assigned dev agent(s) — `contract-owner` → `server-dev` → `tui-dev` /
`platform-dev` per the plan's dependency order. `tester` writes tests in their own files
alongside. Append a Log entry per slice.

### 4. Review (cold)

Dispatch `reviewer` (a fresh agent that did NOT write the code) to run the `review` gate. The
reviewer is **read-only on everything** (code AND Board): it **reports** its findings +
`REVIEW-STATUS: … <sha>` back to the orchestrator, which commits the verdict onto the item **on
the branch** (feature-local). Fix-now findings go back to the owning dev agent on the same
branch. Set `status: review` (committed on the branch). Re-review until `approved` at head — a
Board-only commit (status flip / verdict) does **not** require re-review; only a new code/test
commit does. The approved `<sha>` names the last **code** sha; verify no code commit follows it.

### 5. Verify (run it)

Dispatch `verifier`: boots the stack, exercises the affected flows live, quotes what ran vs.
inferred, reports verified / verified-with-gaps / not-verified. The verifier is **read-only on
everything** (code AND Board): as with review, the verdict is **reported back** and the
orchestrator commits it onto the item **on the branch**. (Learned 0002: reviewer/verifier
running in the worktree must never edit/commit the Board copy and must leave no stray `*.tmp` —
both are read-only and report; the orchestrator does the branch-side Board commit.)

### 6. Learn + summarise

Dispatch `eng-manager`: update agent/skill instructions and standards skills, add CLAUDE.md
gotchas, register any new crate's dev agent, write the `docs/handoff.md` entry, and fill the
item's `## Summary`. Those shared/cross-cutting edits (`docs/**`, `.claude/**`) land on
**`main`** (home #1), while the item's `## Summary` is committed **on the branch** (home #2, it
travels with the item). The derived `board/README.md` dashboard is regenerated on **`main`**
from item frontmatter + active branch heads (home #3).

### 7. Stop

Set `status: awaiting-merge` **on the branch** and **STOP** — confirm Definition of done holds
(tests/lint/fmt clean, verifier ran it, ADR for any contract change, `REVIEW-STATUS: approved`
at the last code sha). The human reads the Summary + diff and **manually merges** the branch
(→ `merged`), which brings the finished item back to `main` atomically with the code, then
removes the worktree. The AI never merges.
