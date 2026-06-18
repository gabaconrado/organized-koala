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
| [0003](./features/0003-server-auth-profile-tasks.md) | Server — auth, default profile, tasks, migrations, docker stack (slice 2 of 0001) | merged | high | 0002 | — (merged) |
| [0004](./features/0004-tui-foundational.md) | TUI — register/login, default profile, task add/list/close (slice 3 of 0001) | ready | high | 0003 | feature/0004-tui-foundational |

> **Dependency chain (slice 0001):** `0002` (contract) → `0003` (server) → `0004` (TUI). `0001`
> is the umbrella tracking the three. `0002` and `0003` are **merged**. `0004` (TUI) is
> **in-flight and branch-owned** on `feature/0004-tui-foundational`: built, reviewed
> **approved**, and live-**`verified`** at code sha `8fb0505` (Docker 29.5.3 / Compose v5.1.4 —
> full reqwest path, ADR-0005 error contract, profile-scoping, persistence, OTel spans; the
> ADR-0003 layer-2 `TestBackend` suite green). Its live status is `awaiting-merge` on the branch;
> `main`'s snapshot stays frozen at the claim (`ready`, with a pointer note) until the human's
> merge brings it back atomically with the code. **Merging 0004 closes the foundational slice** —
> all three children land on `main`, so `0001`'s end-to-end acceptance is closeable and only
> `0001` (umbrella) remains open.
>
> **Sanctioned follow-up (not yet a Board item):** a reported-only `./ok.sh coverage` verb over
> `cargo-llvm-cov` (no hard threshold, not a DoD gate) — `architect` to plan it as a new item.
