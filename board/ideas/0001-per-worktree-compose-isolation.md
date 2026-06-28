---
id: 0001
title: Isolate each worktree's docker stack with a per-worktree COMPOSE_PROJECT_NAME
status: open
priority: medium
created: 2026-06-26
source: 0011
raised-by: eng-manager
promoted-to: null
---

## What

Every worktree's `./ok.sh up` uses the **same** docker compose project name (`deploy`) and therefore
the **same** persistent named volume `deploy_postgres-data`. Concurrent branches share one Postgres
volume and one migration history. Give each worktree its own isolated stack — e.g. derive
`COMPOSE_PROJECT_NAME` from the worktree slug — so concurrent branches never share a volume or a
migration history.

## Why it matters

This shared-stack coupling is the root cause of a recurring, documented failure (CLAUDE.md gotcha,
learned 0011): a `verifier` booting the stack on worktree X inherits the migration history left by
worktree Y. If Y applied a migration absent from X's tree, sqlx's strict migration-history
consistency check fails (*"migration NNNN was previously applied but is missing in the resolved
migrations"*), the one-shot `migrate` errors, and the `run` service (gated on it) never comes up.
Today this is an **environment conflict** that **blocks** the verifier (per hard constraint #6 it is
not worked around): the only fix is the operator authorizing a destructive `docker compose down -v`,
which destroys another branch's local data. A per-worktree project name removes the failure mode
entirely. It is out of scope of any single feature cycle (it's `platform-dev` infra, home #1) — hence
parked here rather than minted, since it changes no product behaviour and wants deliberate design.

## Possible approach

Two complementary fixes for the same root cause — *state surviving a run*. They are sequenced: the
hermetic-teardown discipline fixes the **serial** case we actually run today; per-worktree isolation
is its **parallel** generalization. A `platform-dev` concern; net-new isolation wiring on existing
shared infra, so it lands on `main`. Non-binding — the architect/platform-dev settle the exact shape
if accepted.

**(1) Near-term: make the verifier hermetic (`up` → verify → `down -v`, always).** The
migration-history conflict only exists because state survives a run. If every verifier tears down its
own volume on exit, there is never a leftover migration history for the next run to inherit — in serial
execution (our reality today: dev/verify sessions are never run in parallel) this eliminates the failure
mode entirely. Note the authorization consequence: today `down -v` needs operator sign-off only because
it destroys *another branch's* data; a verifier tearing down state **it just created itself** is cleaning
up its own mess and needs no authorization — so this *removes* a human-in-the-loop block rather than
adding one. Two design requirements for robustness:

- **Teardown must run on failure too** — a `trap`/`finally` so `down -v` fires on *any* exit (success,
  failure, signal). Otherwise the failing runs most likely to strand state are the ones that skip it.
- **A hard crash (reboot, OOM-kill) still strands the volume** — the trap can't fire then, so this
  reduces the conflict to a rare residual with the operator-authorized reset as fallback. It does
  **not** make the failure structurally impossible — only (2) does.

The cost is a fresh migrate-from-scratch per verifier run; for a correctness gate that trade is right
and should not be optimized away.

**(2) Parallel generalization: per-worktree `COMPOSE_PROJECT_NAME`.** Have `ok.sh` set
`COMPOSE_PROJECT_NAME` from the current worktree (e.g. a sanitized basename of the worktree path or
the `feature/NNNN-<slug>` branch) for `up`/`down`/`run-server` and the test/coverage boots, so each
worktree gets its own project + volume. Confirm teardown (`down`/`down -v`) targets the same name. This
is the only fix that makes the failure mode *structurally* impossible under concurrent worktrees, and
closes the hard-crash residual left by (1).

## Disposition

- [ ] [human] decision: accept (→ promote to Board) | close (reason)
