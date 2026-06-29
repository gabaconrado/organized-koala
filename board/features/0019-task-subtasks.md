---
id: 0019
title: Sub-tasks — flat title/status children of a task, with TUI list nesting + collapse
type: feature      # feature | chore
status: inbox          # inbox → planned → ready → working → review → awaiting-merge → merged | blocked
priority: medium    # high | medium | low
parent: null
depends-on: [0016]  # builds on the task detail view + final hotkey scheme (merged)
branch: null
worktree: null
created: 2026-06-29
updated: 2026-06-29
---

## Feature request

**Goal:** Add **sub-tasks** to the TODO feature. A sub-task is a deliberately-minimal child
of a task: it has **only a Title and a Status** — *no* Description, *no* Created-at, and *no*
detailed view of its own. Sub-tasks are created, edited, toggled, and collapsed from the
**Tasks tab** and the **Task Detail page**.

> **⚠️ Hard-constraint #3 (ADR-worthy).** `CLAUDE.md` explicitly lists **subtasks** as
> structure the flat domain does *not* have — *"TODO = {Title, Description, Status,
> Created-at, Closed-at} … **Do not** add structure (subtasks, …) without an ADR."* The
> operator has decided to add it anyway (this card is that decision). The `architect` must
> therefore **author an ADR** that amends the flat-domain constraint before any code, and the
> `contract` change it implies is itself an ADR event (#2). Planning starts there.

### Behaviour (acceptance)

1. **Create — `A` (capital A).** With a task selected (Tasks-tab list **or** its Task Detail
   page), pressing **`A`** creates a new sub-task **for that task**. (`a` remains "add task";
   `A` is the new "add sub-task" key.) The new sub-task starts at the open/not-done status.
2. **Shape — title + status only.** A sub-task carries **only** `title` and `status`. It has
   **no** description, **no** `created_at`, and **no** detail view — selecting one never opens
   a per-field pane.
3. **Edit — `e`.** With a sub-task selected, **`e`** edits its **title** (same edit lifecycle
   as a task title: commit / cancel per the 0016 scheme).
4. **Toggle — `Space`.** With a sub-task selected, **`Space`** toggles its status open ↔ closed
   (done/undone), mirroring the task toggle.
5. **Sub-tasks section in the Task Detail page.** The Task Detail page (0016) gains a **new
   "Sub-tasks" section** listing the task's sub-tasks, each showing its **title and status**.
6. **Indentation in the list.** Sub-tasks are rendered **indented one level** under their
   parent task in the Tasks-tab list.
7. **Collapse / expand — `x`.** Sub-tasks can be **collapsed** (hidden in the list) or
   **expanded** under their parent. **`x`** toggles collapse/expand for the selected task.
   **Defaults:** an **open** parent task shows its sub-tasks **expanded**; a **closed** parent
   task shows them **collapsed**.
   - *(Note: `x` was the old "delete" key pre-0016; under the 0016 final scheme delete is `d`.
     This card binds `x` = toggle-collapse. Planning confirms there is no live `x` collision.)*
8. **List indicator.** A task's list indicator is **`+`** when it has sub-tasks **and** they
   are **collapsed**; otherwise (expanded, or no sub-tasks) it stays **`>`**.
9. **Cascade delete.** Deleting a task **automatically deletes all of its sub-tasks**. No
   orphaned sub-tasks remain.

### Surface this is expected to touch (for planning, not binding)

- **`contract`** — a new sub-task wire type (`{ id, title, status, parent task id }`) plus the
  request/response shapes for create / edit-title / toggle / list. Single source of truth (#2).
- **`server`** — persistence (a `subtasks` table scoped under its parent task, itself
  profile-scoped per #4), endpoints for the four operations, and **`ON DELETE CASCADE`** (or
  equivalent in the delete handler) so removing a task removes its sub-tasks. Reversible
  migration (`*.up.sql` / `*.down.sql`).
- **`tui`** — `A` / `e` / `Space` / `x` handling scoped to the Tasks context, indented list
  rendering, the `+` vs `>` indicator, collapse state, and the Task Detail "Sub-tasks" section.

### Open questions for the architect / `grill`

- **Are sub-tasks themselves profile-scoped, or scoped purely via their parent task?** (#4 says
  every TODO is profile-scoped; a sub-task inherits its parent's profile — confirm the query
  shape enforces no cross-profile reach.)
- **Where does collapse state live?** It is presentation state — does it belong server-side, or
  is it TUI-local? (#1 says the TUI is stateless and holds *no* local persistence; reconcile
  the per-task collapse default with that constraint — likely derived from parent status each
  render rather than persisted.)
- **Can a sub-task have sub-tasks?** Assumed **no** — exactly one level of nesting. Confirm.
- **Does closing a parent auto-collapse already-expanded children, or only set the initial
  default?** ("By default" suggests the latter — an initial state, with `x` then free to
  override — but this needs to be pinned down.)
- **Ordering** of sub-tasks within a parent (creation order assumed).

## Log / comments

- [ ] 2026-06-29 [human] Filed: add sub-tasks. See acceptance points 1–9 above. I know I
  said no subtasks at the start — I want them now; treat the ADR amending hard-constraint #3
  as part of this work.
