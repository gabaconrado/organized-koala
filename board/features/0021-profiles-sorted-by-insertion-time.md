---
id: 0021
title: Profiles sorted by insertion time (not alphabetically) in the Profile list
type: feature      # feature | chore
status: inbox           # inbox → planned → ready → working → review → awaiting-merge → merged | blocked
priority: medium    # high | medium | low
parent: null
depends-on: [0012]  # profiles CRUD + switcher (merged)
branch: null
worktree: null
created: 2026-07-02
updated: 2026-07-02
---

## Feature request

**Goal:** In the **Profile list** (and switcher), sort profiles by their **insertion
(creation) time** instead of **alphabetically**.

**Context (current behaviour to change):** the profile list/switcher (from 0012) presents
profiles in alphabetical order.

### Behaviour (acceptance)

1. The Profile list and the profile switcher render profiles in **insertion-time order**
   (**oldest-first** — see assumption below), **not** alphabetically.
2. The ordering is consistent everywhere profiles are listed.

### Assumptions / notes for the architect

- **Direction.** The request says "sorted by their insertion time" without a direction;
  **assumption: oldest-first (ascending insertion order)** — the order profiles were created.
  Flag for the operator if descending is wanted instead.
- **Ordering source + possible ADR (#2).** Prefer a deterministic **server `ORDER BY
  created_at`** over a TUI-side sort (keeps ordering authoritative server-side; #1 stateless
  TUI). Confirm the profile row/DTO already carries a creation timestamp to order by:
  - If the profile row has `created_at` and it need not appear on the wire, this is a
    **server-query-only** change with **no `contract` change** → it may be a **`chore`** (no
    behaviour-visible wire delta; ordering is observable but no shape changes — architect to
    judge whether the visible reorder counts as behaviour under the chore scope guard).
  - If ordering must be exposed on the DTO (e.g. a `created_at` field added to the profile wire
    type), that is a **`contract` change → ADR event (#2)** and it stays a `feature`.
- **Scope guard.** Left as `feature` pending the architect's sizing; downgradable to `chore` if
  it is purely a server `ORDER BY` with no wire/behaviour-contract change.

## Plan(s)

*None yet — awaiting `architect` (`plan` skill) to confirm the ordering source (server vs
wire) and whether an ADR is required.*

## Log / comments

- [ ] 2026-07-02 [human] Filed from an operator interface-improvements request; see acceptance above.
