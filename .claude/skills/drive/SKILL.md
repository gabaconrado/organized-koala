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

### 2. Claim + isolate

For the highest-priority, oldest `ready` item, cut a worktree + branch:

```sh
git worktree add -b feature/NNNN-<slug> .claude/worktrees/NNNN-<slug> <base>
```

Record `branch` and `worktree` in frontmatter; set `status: working`; stamp a session id in
the Log.

### 3. Build (in the worktree)

Dispatch the assigned dev agent(s) — `contract-owner` → `server-dev` → `tui-dev` /
`platform-dev` per the plan's dependency order. `tester` writes tests in their own files
alongside. Append a Log entry per slice.

### 4. Review (cold)

Dispatch `reviewer` (a fresh agent that did NOT write the code) to run the `review` gate. It
posts findings + `REVIEW-STATUS: …  <sha>`. Fix-now findings go back to the owning dev agent on
the same branch. Set `status: review`. Re-review until `approved` at head.

### 5. Verify (run it)

Dispatch `verifier`: boots the stack, exercises the affected flows live, quotes what ran vs.
inferred, reports verified / verified-with-gaps / not-verified into the Log.

### 6. Learn + summarise

Dispatch `eng-manager`: update agent/skill instructions and standards skills, add CLAUDE.md
gotchas, register any new crate's dev agent, write the `docs/handoff.md` entry, fill the item's
`## Summary`, regenerate `board/README.md`.

### 7. Stop

Set `status: awaiting-merge` and **STOP** — confirm Definition of done holds (tests/lint/fmt
clean, verifier ran it, ADR for any contract change, `REVIEW-STATUS: approved` at head). The
human reads the Summary + diff and **manually merges** (→ `merged`), then removes the worktree.
The AI never merges.
