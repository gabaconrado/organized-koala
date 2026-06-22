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
commit does. The verdict **pins to the code-tree hash** (`./ok.sh code-hash`), recorded with
the last code sha for reference; it stays valid as long as `./ok.sh code-hash HEAD` equals the
attested hash (CLAUDE.md "Verdict pinning") — this is what survives the step-7 rebase untouched.

### 5. Verify (run it)

Dispatch `verifier`: boots the stack, exercises the affected flows live, quotes what ran vs.
inferred, reports verified / verified-with-gaps / not-verified. The verifier is **read-only on
everything** (code AND Board): as with review, the verdict is **reported back** and the
orchestrator commits it onto the item **on the branch**, pinned to the same **code-tree hash**
(`./ok.sh code-hash`) as the review verdict. (Learned 0002: reviewer/verifier
running in the worktree must never edit/commit the Board copy and must leave no stray `*.tmp` —
both are read-only and report; the orchestrator does the branch-side Board commit.)

### 6. Learn + summarise

Dispatch `eng-manager`: update agent/skill instructions and standards skills, add CLAUDE.md
gotchas, register any new crate's dev agent, write the `docs/handoff.md` entry, and fill the
item's `## Summary`. Those shared/cross-cutting edits (`docs/**`, `.claude/**`) land on
**`main`** (home #1), while the item's `## Summary` is committed **on the branch** (home #2, it
travels with the item). The derived `board/README.md` dashboard is regenerated on **`main`**
from item frontmatter + active branch heads (home #3).

### 7. Freshen against `main` (rebase current — so the human reviews up-to-date)

`main` has just advanced in step 6 (eng-manager's shared learnings + the regenerated
dashboard), and other cycles may have merged since the worktree was cut. Before stopping,
bring the branch up to date so the human reviews **exactly what will merge**. A rebase rewrites
shas, so the move is gated on its effect on **code content**, not done blindly — and since
verdicts pin to the **code-tree hash** (not the sha), the gate is a hash comparison and the
code-identical case needs **no relabelling**.

Recompute first: is `main` already an ancestor of the branch head
(`git merge-base --is-ancestor main feature/NNNN-<slug>`)?

- **Already current** → nothing to do; go to step 8.
- **Behind** → the worktree must be clean (the per-slice commits in step 3 ensure this; if not,
  abort and surface). **Record the attested verdict hash** as `OLD_HASH = ./ok.sh code-hash`
  (at the approved head, before rebasing), then `git rebase main` in the worktree. The
  expected-and-only conflict is the feature-local Board file (`main`'s frozen-pointer note vs.
  the branch's authoritative copy) — resolve in favour of the **branch** (drop the pointer
  note). Then classify by the **decision gate** below:

  **Did the rebase change the code? — `./ok.sh code-hash` (rebased head) vs. `OLD_HASH`**

  - **Equal (code byte-identical)** — `main` moved only where the branch doesn't (`docs/`,
    `.claude/`, the Board file). The approved+verified attestation **carries forward untouched**:
    the verdict already pins to this exact hash, so there is **nothing to relabel**. Re-run the
    gates on the rebased tree (`./ok.sh test|lint|fmt --check`) and append a **one-line freshen
    Log note** (the rebase happened; the code-hash is unchanged at `<hash>`, so verdicts hold).
    Do **not** rewrite the verdict lines. This is a Board-only commit — it does **not** retrigger
    review (CLAUDE.md: only a new code/test commit does). Item stays `awaiting-merge`. (The
    commit-sha pointer in the verdict line may now be stale; that is fine — the binding key is
    the hash, not the sha.)
  - **Differ (rebase changed code)** — real code conflicts were resolved, or a `main` change
    to a shared crate altered this branch's compiled code. **The `approved`/`verified` verdicts
    are now void** — they attest a code-hash the live tree no longer has. Do **not** stay at
    `awaiting-merge`: set the item back to `review` (or `working` if gates now fail and code must
    change) and **re-enter at step 4** (review) → step 5 (verify) on the rebased head.
    Carrying a stale `approved` onto rebased-changed code is the exact failure this gate prevents.

> This freezes drift only at the instant you stop: a parallel cycle may merge to `main`
> afterwards, so the human's final ff-merge can still need a re-rebase. The step minimises
> review-time surprise; it does not guarantee a permanently-current branch.

### 8. Stop

Set `status: awaiting-merge` **on the branch** and **STOP** — confirm Definition of done holds
(tests/lint/fmt clean, verifier ran it, ADR for any contract change, `REVIEW-STATUS: approved`
at the last code sha, **branch rebased current on `main` per step 7**). The human reads the
Summary + diff and **manually merges** the branch (→ `merged`), which brings the finished item
back to `main` atomically with the code, then removes the worktree. The AI never merges.
