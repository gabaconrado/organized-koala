---
name: autoreview
description: (Autonomous mode) Always-on reviewer loop. On each wake, review any item in status:review, post a machine-readable verdict + reviewed sha, re-review when the branch head advances. Never merges.
audience: dev
---

# autoreview

## When to invoke

- Only in autonomous mode, alongside `autowork`. Runs the `review` skill as the `reviewer`.

## Procedure

### Each wake

1. **Recompute** from the Board: find items in `status: review`.
2. For each, run the `review` skill **cold** (you did not write the code) and post into the
   item's `## Log / comments` a verdict line plus the reviewed commit sha:

   ```text
   REVIEW-STATUS: approved        <sha>
   REVIEW-STATUS: changes-requested   <sha>
   ```

3. **Re-review when the branch head advances** — an `approved` verdict is only valid while
   `<sha>` == current head. If the dev pushed a fix after `changes-requested`, review the new head.
4. **Re-arm `ScheduleWakeup`** (~270s while waiting for work, ~120s when actively re-reviewing)
   and end the turn.

### Constraints

- **Never merges.** Approval is a signal for `autowork`/the human, not an action.
- Findings are handed back via the Log; the reviewer edits no code.
