# ADR-0012: Sub-tasks — a bounded exception to the flat-domain constraint (#3)

**Status:** Accepted · 2026-06-29

## Context

Board item [0019 (sub-tasks)][feat-0019] adds **sub-tasks** to the TODO feature. This directly
reverses a hard constraint: `CLAUDE.md` #3 lists **subtasks** as structure the flat domain
deliberately does *not* have — *"TODO = {Title, Description, Status, Created-at, Closed-at} …
**Do not** add structure (subtasks, tags, categories, …) without an ADR."* It also reopens
[ADR-0010][adr-0010] §5, whose presentation-only boundary for the 0014–0016 TUI arc explicitly
forbade *"No subtasks/tags/categories."*

The operator has **decided to add sub-tasks anyway** — the 0019 card is that decision, with an
authored `[human]` log line: *"I know I said no subtasks at the start — I want them now; treat
the ADR amending hard-constraint #3 as part of this work."* This is authoritative operator
direction (CLAUDE.md ambiguity policy). The question this ADR settles is therefore **not whether
to add sub-tasks, but exactly how much structure #3 now admits**, so that #3 stays a meaningful,
enforceable constraint rather than a dead letter. The concrete wire/contract shape is a separate
ADR event (#2) and is settled in [ADR-0013][adr-0013]; this ADR fixes the *domain* decision and
its boundaries.

### Forces

- **#3 must stay meaningful.** Admitting "sub-tasks" without a precise boundary would licence
  the very proliferation (deep nesting, per-sub-task descriptions, categories) #3 exists to
  prevent. The exception must be *narrow and stated*, so a reviewer can still block creep.
- **Flatness / simplicity** (coding-standards priority order). The smallest structure that
  satisfies the 0019 acceptance wins; a sub-task is the minimal child — title + status only.
- **Profiles are namespaces (#4).** A sub-task is part of the TODO domain, which #4 scopes to a
  profile. It must inherit its parent's profile and never be reachable cross-profile.
- **The TUI is stateless (#1).** Sub-tasks are server-owned state like tasks; the only
  presentation state they introduce (collapse/expand) must not become client-side persistence.
- **No `updated_at`, no extra timestamps (operator-locked, #3 / ADR-0008).** The flat shape's
  timestamp discipline is preserved — a sub-task is *more* minimal than a task, not less.

## Decision

### 1. Sub-tasks are admitted as a bounded child of a task — and nothing more

Hard-constraint #3 is **amended** to admit exactly one new structure: a **sub-task**, a child of
a task carrying **only a Title and a Status** (`open` / `done`). Precisely what is now allowed:

- A task **may have zero or more sub-tasks**.
- A sub-task has **exactly two fields**: `title` (non-empty after trimming) and `status` (the
  same two-value `open` / `done` lifecycle as a task). It belongs to exactly one parent task.
- A sub-task has **no `description`**, **no `created_at`**, **no `closed_at`**, **no
  `updated_at`**, and **no detail view of its own** — selecting one never opens a per-field pane.
- Sub-tasks are **ordered by creation order** within their parent (the only ordering).

### 2. What remains forbidden — #3 still bites

The exception is deliberately narrow. The following remain **forbidden without a further ADR**,
exactly as before:

- **Exactly one level of nesting.** A sub-task **cannot itself have sub-tasks.** There is no
  recursion, no tree of arbitrary depth.
- **No fields beyond title + status** on a sub-task (no description, no timestamps, no labels).
- **No tags, categories, labels, priorities, or due dates** on tasks *or* sub-tasks.
- **No per-profile timer config**, no cross-profile structure — every other clause of #3 and #4
  stands unchanged.

A sub-task is therefore the *single, minimal* structural addition the flat domain now carries; #3
remains the rule that any further structure is an ADR event.

### 3. Profile-scoping is inherited, structural, and enforced by the query (#4)

A sub-task is **not** independently profile-scoped: it inherits its parent task's profile. The
persistence and every query enforce this **structurally** — a sub-task is reached only through its
parent task, and the parent task is reached only through the caller's owned profile. There is **no
path** by which a caller can read or mutate a sub-task whose parent task is not in a profile they
own; an unowned/cross-profile reach is `404 not_found` exactly as for tasks (ADR-0005 §4). This is
a query-shape obligation on `server-dev`, settled concretely in [ADR-0013][adr-0013].

### 4. Cascade delete — no orphans

Deleting a task **deletes all of its sub-tasks**; deleting a profile (which already cascades its
tasks, ADR-0009 / the `notes`/`tasks` FK cascades) therefore transitively removes their sub-tasks.
No orphaned sub-task can exist. This is enforced at the persistence layer (a foreign-key cascade),
not left to handler discipline.

### 5. Collapse/expand is presentation state, not domain state (#1)

Whether a parent's sub-tasks are shown expanded or hidden in the TUI list is **presentation
state**, owned by the TUI, **derived** and **transient** — never persisted server-side and never a
wire field. The *initial* expand/collapse state is **derived from the parent task's status each
render** (an **open** parent shows its sub-tasks expanded; a **closed** parent shows them
collapsed); the user's `x` toggle is an in-session, process-lifetime override (the same category as
list selection and the in-memory JWT, ADR-0010 §4 / hard-constraint #1). No sub-task or task DTO
gains a `collapsed`/`expanded` field, and the server stores no collapse state. This keeps #1 intact:
every list still derives from a server response; collapse is a view decoration computed over it.

## Consequences

- **Hard-constraint #3 is amended, not abandoned.** It now reads: the domain is flat *except* a
  task may carry title+status-only sub-tasks, one level deep; every other structure remains an ADR
  event. `eng-manager` updates the #3 wording in `CLAUDE.md` to cite this ADR so the constraint
  stays self-documenting (a handoff task; the prose change is shared/cross-cutting state on `main`).
- **ADR-0010 §5 is superseded only on the "no subtasks" clause**, and only for the TODO domain;
  the rest of that boundary (no tags/categories, no per-profile timer config, profiles stay
  namespaces, TUI stays stateless) is untouched. The TUI work for 0019 is no longer
  "presentation-only" — it renders a new DTO over new client methods — which is why 0019 is a
  `feature`, not a chore.
- **The wire/persistence shape is a separate ADR event** ([ADR-0013][adr-0013], #2): a new
  sub-task DTO, its create/edit/toggle/list endpoints, a reversible migration with an FK cascade.
- **Reversibility.** The domain exception is reversible: dropping the sub-tasks table (its
  `down.sql`) and removing the DTO + endpoints returns the domain to its pre-0019 flat shape, with
  no change to the `tasks`/`notes`/`profiles` schema.

[feat-0019]: ../../board/features/0019-task-subtasks.md
[adr-0010]: ./0010-tui-navigation-and-interaction-model.md
[adr-0013]: ./0013-subtasks-wire-contract.md
