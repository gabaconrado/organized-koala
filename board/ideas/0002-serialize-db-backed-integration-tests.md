---
id: 0002
title: Stabilize the default parallel test run against throwaway-Postgres pool contention
status: open
priority: medium
created: 2026-06-27
source: 0015
raised-by: reviewer
promoted-to: null
---

## What

The default parallel `./ok.sh test` is **intermittently flaky** under throwaway-Postgres
connection-pool contention. Server DB-backed integration suites (auth / notes / profiles)
occasionally fail at a `register` step with **HTTP 500 `{"code":"internal"}`** when many tests
hit the database concurrently. The failure is non-deterministic and **vanishes when the
DB-backed tests are serialized** (`RUST_TEST_THREADS=1`). It is an **environment/test-harness
concern, not a product defect** — the server code under test is correct; the flakiness comes
from too many concurrent connections against the test Postgres.

## Why it matters

Flaky tests erode trust in the green/red signal and can intermittently red a DoD gate
(`./ok.sh test`) for reasons unrelated to the change under review — exactly what happened during
the 0015 footer-fix re-entry, where the reviewer had to serialize DB tests
(`RUST_TEST_THREADS=1`) to get a clean, deterministic pass. A deterministic test run is
foundational; intermittent infra-induced failures cost cycles chasing non-bugs and risk masking
a real regression behind "probably just the flake." This is out of scope of any single feature
cycle: it is `platform-dev` infra (the test-harness / pool wiring inside `ok.sh` and the test
Postgres config), home #1 (shared / cross-cutting → `main`), and wants deliberate design rather
than being smuggled into a feature branch.

## Possible approach

Non-binding sketch for `platform-dev` (the architect/platform-dev settle the exact shape if
accepted). Options, roughly in order of locality:

- **Serialize the DB-backed integration tests** — run the server DB suites single-threaded (a
  per-suite test-thread cap, or a shared serialization guard around DB-touching tests) while
  leaving pure-unit/TUI `TestBackend` suites parallel, so the wall-clock cost is contained.
- **Bound the server's test connection pool** — cap `max_connections` on the pool the test
  harness builds so concurrent tests cannot exhaust the backend.
- **Raise the throwaway test Postgres `max_connections`** — give the test DB enough headroom for
  the parallel test fan-out.

Likely the right answer is a combination (bound the pool *and* size the DB to match the harness's
parallelism), measured rather than guessed. Note this interacts with idea 0001 (per-worktree
compose isolation): both touch how `ok.sh` boots the throwaway/stack Postgres for tests, so if
both are accepted they are best designed together.

## Disposition

- [ ] [human] decision: accept (→ promote to Board) | close (reason)
