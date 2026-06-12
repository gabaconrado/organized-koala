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
| [0003](./features/0003-server-auth-profile-tasks.md) | Server — auth, default profile, tasks, migrations, docker stack (slice 2 of 0001) | blocked (live on branch) | high | 0002 | feature/0003-server-auth-profile-tasks |
| [0004](./features/0004-tui-foundational.md) | TUI — register/login, default profile, task add/list/close (slice 3 of 0001) | ready | high | 0003 | — |

> **Dependency chain (slice 0001):** `0002` (contract) → `0003` (server) → `0004` (TUI). `0001`
> is the umbrella tracking the three. `0002` is **merged**. `0003` (server) is **`blocked` on its
> branch** (`feature/0003-server-auth-profile-tasks`): code is written and the reviewer
> **approved `f67a883`**, but docker is unavailable in the sandbox so the live verifier pass
> cannot run. Per CLAUDE.md hard constraint #6 a missing capability blocks rather than being
> engineered around (the prior "embedded-Postgres" fallback verification is **disavowed and void
> for sign-off**). **Re-entry:** operator sets up docker → re-verify under the sanctioned
> mechanism (real `./ok.sh up`) → `awaiting-merge`. `main`'s snapshot stays frozen at the claim
> until the human's merge. `0004` (TUI) is `ready` but **blocked behind 0003** (depends-on 0003)
> and becomes claimable once 0003 merges.
