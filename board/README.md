# Board

Coordination state for organized-koala (this replaces a ticket tracker). One file per work
item in `features/`; the `status:` frontmatter **is** the state machine. This dashboard is
**derived** — `eng-manager` regenerates it from item frontmatter; do not hand-edit as truth.

> The Board is committed and potentially public. **Never write secrets or sensitive payloads
> into any item.** Describe behaviour and shape.

## State machine

```text
inbox → planned → ready → working → review → awaiting-merge → merged | blocked
```

The AI cycle is terminal at `awaiting-merge`; only the human merges (→ `merged`). An item is
born on `main` during planning, then becomes **branch-owned on claim**: its live status
advances on the feature branch while `main`'s snapshot stays frozen at the claim until the
human's merge (see CLAUDE.md "The Board"). The `Status` column below shows `main`'s snapshot;
for an in-flight item the authoritative live status is on its branch.

## Items

| ID | Title | Status (main snapshot) | Priority | Depends on | Branch |
| --- | --- | --- | --- | --- | --- |
| [0001](./features/0001-foundational-slice.md) | Foundational vertical slice (auth + profile + minimal TODO) | planned | high | umbrella → 0002, 0003, 0004 | — |
| [0002](./features/0002-contract-crate.md) | Contract crate + workspace restructure (slice 1 of 0001) | merged | high | — | — (merged) |
| [0003](./features/0003-server-auth-profile-tasks.md) | Server — auth, default profile, tasks, migrations, docker stack (slice 2 of 0001) | ready (frozen; live on branch) | high | 0002 | feature/0003-server-auth-profile-tasks |
| [0004](./features/0004-tui-foundational.md) | TUI — register/login, default profile, task add/list/close (slice 3 of 0001) | ready | high | 0003 | — |

> **Dependency chain (slice 0001):** `0002` (contract) → `0003` (server) → `0004` (TUI). `0001`
> is the umbrella tracking the three. `0002` is **merged**. `0003` (server) is **in-flight and
> branch-owned** on `feature/0003-server-auth-profile-tasks`: the cycle ran build → review →
> verify and the live item advanced to `awaiting-merge` on the branch (reviewer **approved** and
> verifier **verified-with-gaps** at last code sha `f67a883`; two docker-only gaps — compose
> `service_completed_successfully` gating + OTLP export — await the human's `./ok.sh up` at merge).
> `main`'s snapshot stays frozen at the claim (`ready`, with a pointer note) until the human's
> fast-forward merge brings the finished item back atomically with the code. `0004` (TUI) is
> `ready` but **blocked behind 0003** (depends-on 0003) and becomes claimable once 0003 merges.
