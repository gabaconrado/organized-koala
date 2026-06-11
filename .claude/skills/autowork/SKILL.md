---
name: autowork
description: (Autonomous mode) Self-pacing worker loop. On each wake, recompute position from the Board + git, advance the highest-priority claimable item by ONE phase using the drive cycle, then re-arm ScheduleWakeup. Never merges.
audience: dev
---

# autowork

## When to invoke

- Only in autonomous mode. Wraps the `drive` cycle in a `ScheduleWakeup` re-arm — it does
  **not** re-implement the cycle; per-phase logic is `drive`'s.

## Procedure

### Each wake

1. **Recompute position** from the Board + `git` (no scratch file). Re-verify ownership of any
   `working` item by matching the session id stamped in its Log; rediscover its worktree with
   `git worktree list` before cutting a new one. If a human moved/merged/blocked it, abort that
   item cleanly and pick another.
2. **Apply the highest-priority wake-rule** (table below) — advance the item by exactly **one
   phase** using `drive`'s logic for that phase.
3. **Re-arm `ScheduleWakeup` and end the turn.** Re-arming is mandatory on every non-terminal
   path and at **every phase boundary** — skipping it silently kills the loop.

### Wake-rules (highest priority first)

| Situation | Action | Re-arm |
| --- | --- | --- |
| any non-merged item has an unchecked `[human]` box (**top priority**) | `architect` triage → re-enter at smallest phase; may pull an item back out of `awaiting-merge` | ~120s |
| `ready` item exists, none claimed | claim highest-priority, oldest-first | ~120s |
| board empty | idle | ~1800s |
| `working` | advance one build phase | ~120s |
| `review`, no verdict yet | poll for `autoreview` verdict | ~270s |
| `review`, `changes-requested` | fix + commit on same branch (no push) | ~120s |
| `review`, `approved` + sha==head | set `awaiting-merge`, **STOP for human** | pick next ~60s |
| `awaiting-merge` / `merged` | nothing (human-owned) | pick next |
| `blocked` | drop it (human-owned) | pick next |

### Cold restart

On relaunch, read the Board for your own `working` item, rediscover the worktree by name,
re-derive the phase from the Log, re-arm, and resume at the matching wake-rule. Re-verify
ownership before each expensive phase.

**The AI is terminal at `awaiting-merge` — never merge.**
