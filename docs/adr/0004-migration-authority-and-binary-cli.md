# ADR-0004: Migration authority and the server-binary admin CLI

**Status:** Accepted · 2026-06-11

## Context

[CLAUDE.md][claude-md]'s "How to run" table lists `./ok.sh migrate` (which today shells
out to `sqlx migrate run`) as *the* way to apply schema migrations, and the deployment
story leans on that dev script being in the loop. That couples a **running system** to a
**developer helper**: a real deployment has no checkout, no `ok.sh`, and no `sqlx` CLI on
the host. A running system must be self-contained.

Schema lifecycle (apply/rollback migrations) is an **operational capability of the
product**, not a build-time convenience. It belongs to the artifact that ships — the
server binary `organized-koalad` — and the orchestration that brings the stack up
(`./ok.sh up` → docker-compose), not to a host script that exists only in a developer's
working tree.

This reshapes (a) the **server binary's public surface** — today it is run-only; it
gains an admin CLI (`run` / `migrate` / `rollback`); (b) the **deployment/up model** —
migrations run as an orchestrated step when the stack comes up, with no host command; and
(c) the **role of `./ok.sh migrate`** — it survives only as a thin dev convenience that
delegates to the binary and is never load-bearing at runtime. That is an
architecture-shaping change, so it is recorded before implementation.

Two facts ground this ADR in current reality: the workspace is still at the placeholder
stage (no `crates/server`, no `migrations/`, no `deploy/` yet), so these are **greenfield
build directives**, not rewrites. And board item [0001 (foundational slice)][feat-0001]
already carries the acceptance criterion *"It is not necessary to run any command in the
host (like `./ok.sh migrate`) for the system to run correctly; migrations should be
handled internally by the application"* — this ADR settles **how** that is met.

### Forces

- **Self-containment (operator directive #1).** Nothing outside a dev context may assume
  `ok.sh` exists. The runtime migrate path must live inside the shipped image.
- **`up` owns migrations (directive #2).** Bringing the stack up must leave the schema
  current without a separate human step.
- **Admin control (directive #3).** Admins need migrate **and rollback** from the binary
  itself — a deliberate, invokable surface, not a silent side effect buried in run.
- **Multi-replica safety.** If migrations run on the run path, every replica boot races
  to migrate the same database. We must not bake a migration race into the run path.
- **Rollback requires reversibility.** A `rollback` subcommand is meaningful only if
  migrations carry `down` scripts.
- **The `.sqlx/` offline cache is orthogonal.** Hard-constraint sqlx **offline mode**
  ([ADR-0001][adr-0001] §7) concerns the committed **query cache** that lets the workspace
  *compile* without a DB. Migration files define **schema** and run **against a live DB**.
  These must not be muddled: adopting reversible migrations changes the `migrations/` tree,
  not `.sqlx/`; refreshing `.sqlx/` (`./ok.sh prepare`) still needs a live DB and stays a
  dev step.
- **Dev ergonomics.** Developers still want a one-word "make my DB current" verb.

## Decision

### 1. The migrate/rollback surface lives on the server binary

`organized-koalad` gains an **admin CLI** over the existing run behaviour, with three
subcommands at minimum:

- **`run`** — run the HTTP server. **It remains the default no-arg behaviour**: bare
  `organized-koalad` (and `cargo run --bin organized-koalad` with no trailing args) still
  runs, preserving back-compat with how `ok.sh run-server` invokes it. `run` is also
  accepted explicitly.
- **`migrate`** — apply all pending migrations and exit. Idempotent; a no-op when the
  schema is already current.
- **`rollback`** — revert applied migrations (see §3 for bounding semantics) and exit.

Arg parsing uses **`clap`** (derive). The subcommand *shape* is fixed here; exact flag
names/defaults are left to `server-dev` within this shape. The migration runner is
`sqlx::migrate!` against the configured database, embedding the `migrations/` tree into
the binary so the shipped artifact carries its own schema and needs no `sqlx` CLI or
checkout at runtime.

### 2. `up` runs migration as an explicit one-shot step BEFORE run — NOT auto-migrate-on-run

We **refute** silent auto-migration inside the run path and **affirm** the operator's
reading: the binary exposes an explicit `migrate` subcommand, and orchestration invokes it
as a distinct step ordered before `run`.

Concretely, the docker-compose stack runs migration as a **one-shot `migrate` service**
(`organized-koalad migrate`, same image) that runs to completion, gated on Postgres being
healthy; the long-running server service (`organized-koalad run`) **depends on that
one-shot completing successfully** (`depends_on: condition: service_completed_successfully`).
`./ok.sh up` triggers this whole graph — so "migrations are part of `up`" holds with **no
host command** and nothing on the run path mutating schema.

Reasoning on the auto-migrate fork (the load-bearing one):

- **Multi-replica safety.** Auto-migrate-on-run means N replicas race the same DDL on
  boot. sqlx's migrator takes an advisory lock so it will not corrupt, but it makes every
  replica boot contingent on migration timing and turns a schema change into an implicit,
  unobservable run-time event. A single one-shot step runs migration **exactly once**,
  before any server replica starts, with its own exit code — the natural shape for a
  multi-replica future even though the default deployment is single-replica today.
- **Explicit > implicit for a schema mutation.** Migration is a privileged, occasionally
  destructive operation. It deserves its own invocation and its own success/failure signal,
  not a hidden prologue to "start serving." This also keeps the model symmetric with
  `rollback`, which is unambiguously a deliberate admin act.
- **Operability.** A failed migrate surfaces as a failed one-shot service that blocks
  run from starting — fail-fast and legible — rather than a half-migrated replica
  flapping in a restart loop.

**Dev-only auto-migrate escape hatch.** Because `migrate` is a separate process, the inner
dev loop (`cargo run`) would otherwise need two invocations. We permit `run` to run
pending migrations on boot **only when explicitly opted in** via an env flag
(suggested: `OK_AUTO_MIGRATE=1`). It is **off by default**, intended for local dev, and
must be documented as dev-only. The default run path **never** migrates. (Whether to
implement this hatch now or defer it is `server-dev`'s call within the foundational slice;
the contract is: default-off, env-gated, dev-only.)

### 3. Reversible migrations are the project standard; rollback is bounded and explicit

The project **adopts reversible migrations as a standard**: every migration is created with
sqlx's reversible form (`sqlx migrate add -r <name>`, producing paired
`*.up.sql` / `*.down.sql` files). A migration without a `down` is a review-blocking gap.

`rollback` semantics:

- **Default: one step.** Bare `organized-koalad rollback` reverts exactly the **single most
  recent** applied migration. One-step-at-a-time is the safe default for a destructive
  operation; it avoids "rollback the world" footguns.
- **Optional target.** `rollback` MAY accept a bounded target (e.g. `--to <version>` /
  `--steps N`); `server-dev` chooses the precise flag surface within "explicitly bounded,
  never unbounded-by-default."
- **Manual/admin only.** Rollback is **never** wired into `up` or any automated path. It is
  an admin action invoked deliberately against a running deployment's database.

This is independent of the `.sqlx/` offline cache (see Forces): reversibility lives in the
`migrations/` tree; `.sqlx/` remains the compile-time query cache, refreshed via
`./ok.sh prepare` against a live DB. The ADR does not change offline-mode.

### 4. `ok.sh migrate` becomes a dev-only delegating convenience; `run-server` invokes `run`

- **`./ok.sh migrate`** survives **only** as a developer convenience and **delegates to the
  binary** — `cargo run --bin organized-koalad -- migrate` (replacing today's direct
  `sqlx migrate run`). It is explicitly documented as **dev-only and non-load-bearing at
  runtime**; a real deployment never calls it.
- **`./ok.sh rollback`** is added as a parallel dev convenience delegating to
  `organized-koalad rollback` (dev-only, same framing). Optional but recommended for parity.
- **`./ok.sh run-server`** is the **dev wrapper** that invokes the binary's `run`
  subcommand: it shells to `cargo run --bin "${SERVER_BIN}" -- run` (equivalently
  `cargo run --bin organized-koalad -- run`). Note the two names are distinct: the
  *binary subcommand* is `organized-koalad run`; `./ok.sh run-server` is the host-side
  dev convenience that calls it. (Because bare invocation also defaults to `run`, a
  no-arg `cargo run --bin organized-koalad -- "$@"` form stays back-compatible; passing
  `run` explicitly is the clear form.)
- **`./ok.sh up` / `down`** continue to wrap `docker compose`; the migrate-before-run
  ordering lives in the **compose file**, not in `ok.sh` shell logic, keeping the runtime
  self-contained inside the stack definition.

## Consequences

- The shipped image is **self-contained**: `organized-koalad migrate|rollback|run` is the
  complete operational surface; no host `ok.sh` or `sqlx` CLI at runtime. Directive #1 met.
- Bringing the stack up (`./ok.sh up`) leaves the schema current with **no separate human
  step** and **no schema mutation on the run path**. Directives #2 and #3 met; board item
  [0001][feat-0001]'s "no host command needed" criterion is satisfiable.
- **Multi-replica migration is safe by construction**: a single one-shot migrate runs
  before any replica runs; replicas never race DDL on boot.
- The project commits to **reversible migrations** — a small ongoing authoring discipline
  (every migration needs a `down`) bought for admin rollback capability.
- The binary now depends on `clap` and embeds the `migrations/` tree via `sqlx::migrate!`.
  This is a binary-crate concern only; no `contract` change, no wire-shape change.
- The dev-only auto-migrate hatch is a documented, default-off ergonomic affordance; the
  burden of proof is on turning it **on**, never off.
- `.sqlx/` offline-mode is untouched and explicitly kept distinct from schema migrations.

## Downstream edits (mandated; designed here, implemented by the named owners)

### `server-dev` — `crates/server` (binary `organized-koalad`)

1. **Add a `clap` (derive) CLI** over the binary with subcommands `run` (default no-arg),
   `migrate`, `rollback`. Bare invocation and `cargo run -- ` with no args MUST still run the
   server.
2. **Migration runner.** Embed the `migrations/` tree with `sqlx::migrate!` and wire
   `migrate` (apply all pending, idempotent) and `rollback` (revert; default one step,
   optional bounded `--to`/`--steps`; never unbounded-by-default; never auto-invoked).
3. **Adopt reversible migrations** as the standard: author every migration with
   `sqlx migrate add -r` (paired `*.up.sql`/`*.down.sql`). For the foundational slice this
   is the auth/profile/tasks schema. A migration lacking a `down` is a review-blocking gap.
4. **Dev-only auto-migrate hatch (optional, default-off):** `run` MAY run pending
   migrations on boot **only** when an env flag (suggested `OK_AUTO_MIGRATE=1`) is set;
   off by default; documented as dev-only. The default run path never migrates.
5. Keep this binary-only: `anyhow` at the top per rust-standards; **no `contract` change**.

### `platform-dev` — `deploy/**` and `ok.sh`

6. **Compose: migrate-before-run.** In `deploy/docker-compose.yml` add a **one-shot
   `migrate` service** (`organized-koalad migrate`, same server image) gated on Postgres
   health (`depends_on: postgres: condition: service_healthy`). The long-running server
   service (`organized-koalad run`) MUST gate on it via
   `depends_on: migrate: condition: service_completed_successfully`. No host command, no
   `ok.sh` inside the container. This is what makes "migrations are part of `up`" true.
7. **Reframe `./ok.sh migrate`** to delegate to the binary — replace
   `cmd_migrate() { sqlx migrate run; }` with
   `cmd_migrate() { cargo run --bin "${SERVER_BIN}" -- migrate; }` — and document it in
   `ok.sh --help` as **dev-only, non-load-bearing at runtime**.
8. **Add `./ok.sh rollback`** delegating to `cargo run --bin "${SERVER_BIN}" -- rollback`
   (dev convenience, same dev-only framing). Document in `--help`.
9. **`run-server` reframed to the `run` subcommand** — the dev wrapper invokes
   `cargo run --bin "${SERVER_BIN}" -- run` (the binary subcommand `organized-koalad run`,
   distinct from this `./ok.sh run-server` wrapper); the no-arg default still runs the
   server. `up`/`down` keep
   wrapping `docker compose`.

### `eng-manager` — `CLAUDE.md` and agent defs

10. **CLAUDE.md "How to run" table:** reframe the `migrate` row as a **dev convenience that
    delegates to `organized-koalad migrate`** (dev-only); add a `rollback` row likewise.
    Add a note that **migrations run automatically as part of `./ok.sh up`** (a compose
    one-shot) and that the **server binary owns migrate/rollback** at runtime — no host
    script is required by a deployment.
11. **CLAUDE.md deployment wording / hard constraints:** add a line that a running system is
    **self-contained** (no `ok.sh` at runtime; the binary carries migrate/rollback and the
    embedded `migrations/` tree), and clarify that **`.sqlx/` offline-mode (query cache) is
    distinct from schema migrations** so the two are never conflated.
12. **`.claude/agents/server-dev.md`:** its responsibilities already name "migrations";
    extend to note the binary owns the **migrate/rollback admin CLI** and the
    **reversible-migration standard** per this ADR.
13. **`.claude/agents/platform-dev.md`:** note that `up` orchestrates the migrate step via
    the binary (compose one-shot) and that `ok.sh migrate`/`rollback` are **dev-only
    delegating conveniences**, not runtime requirements.

### `architect` — `docs/decisions.md`

14. Add the ADR-0004 index row and resolve its link reference (done with this ADR, per the
    ADR-0003 precedent that `architect` owns `decisions.md`).

[claude-md]: ../../CLAUDE.md
[adr-0001]: ./0001-foundational-architecture.md
[feat-0001]: ../../board/features/0001-foundational-slice.md
