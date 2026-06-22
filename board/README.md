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
| [0001](./features/0001-foundational-slice.md) | Foundational vertical slice (auth + profile + minimal TODO) | merged | high | umbrella → 0002, 0003, 0004 | — (merged) |
| [0002](./features/0002-contract-crate.md) | Contract crate + workspace restructure (slice 1 of 0001) | merged | high | — | — (merged) |
| [0003](./features/0003-server-auth-profile-tasks.md) | Server — auth, default profile, tasks, migrations, docker stack (slice 2 of 0001) | merged | high | 0002 | — (merged) |
| [0004](./features/0004-tui-foundational.md) | TUI — register/login, default profile, task add/list/close (slice 3 of 0001) | merged | high | 0003 | — (merged) |
| [0005](./features/0005-tui-responsive-event-loop.md) | TUI — responsive (non-blocking) event loop + `tui::app` submodule reorg | ready | high | 0004 | — (not yet claimed) |

> **Foundational slice 0001 — CLOSED.** All three children are **merged** on `main`:
> `0002` (contract) → `0003` (server) → `0004` (TUI). The umbrella `0001` is therefore **merged**
> too — its end-to-end acceptance was satisfied collectively at 0004's live verification (full
> reqwest path, ADR-0005 error contract with exact wire strings, profile-scoping, persistence
> across restart, OTel spans; the ADR-0003 layer-2 `TestBackend` suite green). The tracer bullet
> TUI ↔ `contract` ↔ server ↔ Postgres is complete.
>
> **Next up — `0005` (`ready`):** make the TUI responsive while requests are in flight (spinner +
> cancel, no UI freeze) and reorganize `tui::app` into feature submodules. Governed by
> [ADR-0006](../docs/adr/0006-tui-concurrency-and-responsiveness.md) (**Model A**: synchronous
> `Client` on a worker thread + `mpsc` + polled render loop; no async runtime). Depends on `0004`
> (now merged), so it is claimable on the next `/drive`.
>
> **Sanctioned follow-up (not yet a Board item):** a reported-only `./ok.sh coverage` verb over
> `cargo-llvm-cov` (no hard threshold, not a DoD gate) — `architect` to plan it as a new item.
