---
id: 0003
title: Server — auth, default profile, tasks, migrations, docker stack (slice 2 of 0001)
status: working      # inbox → planned → ready → working → review → awaiting-merge → merged | blocked
priority: high       # high | medium | low
parent: 0001
depends-on: [0002]
branch: feature/0003-server-auth-profile-tasks
worktree: .claude/worktrees/0003-server-auth-profile-tasks
created: 2026-06-11
updated: 2026-06-12
---

## Feature request

**Goal:** `organized-koalad` serves the full ADR-0005 API against Postgres — register/login
(argon2 + JWT), the atomically-created default profile, profile-scoped add/list/close tasks,
the `{ code?, message }` error contract — with the ADR-0004 admin CLI
(`run`/`migrate`/`rollback`), reversible migrations, tracing/OTel instrumentation, and a
docker stack such that `./ok.sh up` needs **no host command** to leave the system runnable.

**Why:** Slice 2 of [0001][feat-0001]: the server side of the tracer
bullet, verifiable live over HTTP before the TUI exists.

**Acceptance criteria:**

- [ ] `crates/server` (binary `organized-koalad`) exists; clap CLI with `run` (default
      no-arg), `migrate`, `rollback` per [ADR-0004][adr-0004].
- [ ] Reversible migrations (paired `*.up.sql`/`*.down.sql`) for users/profiles/tasks,
      embedded via `sqlx::migrate!`; `.sqlx/` offline cache committed (`./ok.sh prepare`).
- [ ] Endpoints per [ADR-0005][adr-0005]:
      `POST /api/auth/register` (creates user + named default profile in one transaction),
      `POST /api/auth/login`, `GET /api/profiles`, `GET|POST /api/profiles/{pid}/tasks`,
      `POST /api/profiles/{pid}/tasks/{tid}/close`, `GET /healthz`.
- [ ] argon2 password hashing; JWT HS256 sessions (secret via env as `SecretString`,
      TTL default 24 h env-configurable); unowned profile → 404 `not_found`.
- [ ] Every error maps to HTTP status + `{ code?, message }` with the ADR-0005 code set.
- [ ] All endpoints instrumented with `tracing` spans; INFO events on mutations; errors
      recorded; OTLP export wired to a collector in the compose stack.
- [ ] `deploy/` compose stack (Postgres + one-shot `migrate` + `run` + OTel collector);
      `./ok.sh up` brings it up with migrations applied automatically; `ok.sh`
      `migrate`/`rollback`/`run-server` delegate to the binary per ADR-0004.
- [ ] `./ok.sh test|lint|fmt --check` green; integration tests cover the public HTTP API.

**Out of scope:** the TUI (0004), Notes/Pomodoro endpoints, task update/delete, refresh
tokens/logout, multi-profile creation endpoint, dashboards/sampling (backlog
"Observability wiring").

<!-- written by `architect` via the `plan` skill -->
## Plan(s)

### Plan: server + platform (2026-06-11, architect)

**Approach:** Build the server as a thin vertical over the frozen 0002 contract: schema →
auth → profile scoping → tasks → error mapping → instrumentation, then wrap it in the
ADR-0004 compose stack so the verifier can exercise everything as live HTTP round-trips
(per ADR-0003 the verifier owns this layer). Widening order inside the item: `healthz` +
CLI + migrations first (proves the boot/migrate seam), then auth, then tasks.

**ADR:** [ADR-0004][adr-0004] +
[ADR-0005][adr-0005] (both accepted; no new ADR
needed).

**Slices:**

1. [server-dev] Scaffold `crates/server` (`new-crate` skill); clap CLI `run`/`migrate`/
   `rollback` per ADR-0004 §1–3; reversible migrations for `users`, `profiles`, `tasks`
   (FKs: profile → user, task → profile; task status/closed-at per the flat domain) —
   files: `crates/server/**`, `crates/server/migrations/**`, root `Cargo.toml` member.
2. [server-dev] Auth: argon2 (PHC string storage) register/login, register creates user +
   default profile in one transaction, JWT issue/verify middleware (extractor), `healthz` —
   files: `crates/server/src/**` (auth/session modules).
3. [server-dev] Profile + task handlers, ownership-joined queries (404 on unowned), typed
   error → `{ code?, message }` mapping at the boundary, `tracing` instrumentation (spans on
   all endpoints, INFO mutation events, error events) + OTLP layer init — files:
   `crates/server/src/**`; `.sqlx/` refresh.
4. [platform-dev] `deploy/`: server Dockerfile, compose (Postgres healthcheck → one-shot
   `organized-koalad migrate` → `organized-koalad run` gated on
   `service_completed_successfully` → OTel collector with a logging/debug exporter);
   `ok.sh`: `up`/`down` wiring, `migrate`/`rollback` delegating to the binary, `run-server`
   → `-- run`, `prepare` verb if missing — files: `deploy/**`, `ok.sh`, `Dockerfile`.
5. [tester] Integration tests against the public HTTP surface: auth happy/failure paths,
   profile isolation (user B cannot see/write user A's profile → 404), task add/list/close
   incl. idempotent re-close, error-body shape/codes, validation rules — files:
   `crates/server/tests/**` (+ unit `tests.rs` siblings where modules warrant).
6. [verifier] Live pass per ADR-0003: boot `./ok.sh up`, exercise endpoints, confirm
   status codes, error contract, profile scoping, migration one-shot ordering, OTel spans.

**Assumptions:**

- Integration tests need a live Postgres (DB is an external service — mocking it is
  permitted but a real test DB gives more value): mechanism (`sqlx::test` /
  compose-provided test DB wired through `./ok.sh test`) is chosen by `tester` +
  `platform-dev`; if it turns hard, bubble up rather than bend source.
- JWT env names (`OK_JWT_SECRET`, TTL var), bind address/port, and DB URL var naming are
  `server-dev`'s call; compose may carry an obviously-dev-only default secret, documented
  as such — never a real secret in the committed stack (Board/docs stay secret-free).
- The ADR-0004 dev-only auto-migrate hatch (`OK_AUTO_MIGRATE=1`) is optional this slice;
  `server-dev` decides implement-now vs defer (contract: default-off, env-gated, dev-only).
- OTel collector config is minimal (OTLP receiver + logging/debug exporter) — enough for
  the verifier to observe spans; richer pipelines belong to the backlog observability item.
- Validation limits beyond ADR-0005's load-bearing rules (username `@` ban, non-empty
  trimmed title) — e.g. max lengths — are `server-dev`'s choice, surfaced as
  `validation_failed`.
- Passwords/secrets are `secrecy::SecretString` end-to-end per rust-standards; no bare
  secret in any `Debug`/log/span.

**Risks:**

- Largest item of the three (auth + schema + stack). Mitigation: strict internal ordering
  (CLI/migrate seam first) so failures localize; review gate on each concern.
- Compose one-shot gating (`service_completed_successfully`) is version-sensitive in
  compose specs — verifier must observe the ordering for real, not infer it.
- sqlx offline cache drift (`.sqlx/` not refreshed after query edits) breaks clean builds —
  review-blocking; refresh via `./ok.sh prepare` is part of slice 3.
- JWT/argon2 misconfiguration is a security risk — reviewer attention on hashing params,
  token validation (exp enforced), and 404-not-403 scoping.

<!-- append-only; dated, AUTHORED entries. Human feedback = an UNCHECKED box: [ ] unhandled, [x] addressed + re-reviewed (the ONLY re-entry signal). -->
## Log / comments

- 2026-06-11 [architect] item created as slice 2/3 of 0001 via the `plan` skill; plan
  authored; governed by ADR-0004 + ADR-0005; status `planned` → `ready`. **Dependency
  gate:** do not start until [0002][feat-0002] is `awaiting-merge`/`merged`;
  if 0002 is unmerged, cut the worktree stacked on 0002's branch (linear history,
  fast-forward rebase per git-standards).
- 2026-06-12 [drive] claimed: 0002 is `merged`, so worktree cut from `main` (affa85d) which
  carries ADR-0004/0005 + the merged `contract` crate. Branch
  `feature/0003-server-auth-profile-tasks`; session `drive-0003-20260612`. Status
  `ready` → `working`. Branch copy is now authoritative.
- 2026-06-12 [server-dev] slice 1 (a7c9b81): scaffolded `crates/server` (binary
  `organized-koalad`) as a workspace member via the new-crate conventions; clap admin CLI
  per ADR-0004 — `run` (default no-arg, never mutates schema), `migrate` (idempotent), and
  `rollback` (one step by default, bounded via `--steps`, never auto-invoked). Reversible
  migrations (paired `*.up.sql`/`*.down.sql`) for `users`, `profiles`, `tasks` with FKs
  (profile->user, task->profile) and the flat task status/closed-at domain, embedded via
  `sqlx::migrate!`. OTLP export layer gated on `OK_OTLP_ENDPOINT`, degrading to log-only when
  the collector is absent/unreachable. Env names chosen: `OK_DATABASE_URL`, `OK_JWT_SECRET`,
  `OK_JWT_TTL_SECONDS` (default 86400 = 24h), `OK_BIND_ADDR` (default `0.0.0.0:8080`),
  `OK_OTLP_ENDPOINT`, `OK_AUTO_MIGRATE` (dev-only hatch, default off — implemented now).
- 2026-06-12 [server-dev] slice 2 (1a2cd5b): argon2 PHC password hashing (constant-time decoy
  verify for absent users), JWT HS256 issue/verify (sub/iat/exp, expiry enforced; secret held
  as `SecretString`, redacted everywhere), the `AuthUser` Bearer extractor, the thiserror
  boundary error mapping each case to HTTP status + `contract::ErrorBody { code?, message }`
  (internal causes logged, never sent), and the router + state. Endpoints: `GET /healthz`,
  `POST /api/auth/register` (user + named default profile in one transaction → 201),
  `POST /api/auth/login` (username-or-email → 200 token).
- 2026-06-12 [server-dev] slice 3 (9d234ce): `GET /api/profiles` and the task surface
  `GET|POST /api/profiles/{pid}/tasks` + `POST .../tasks/{tid}/close`. Every query is
  ownership-joined on the authenticated user, so unowned/nonexistent profiles return
  **404 `not_found`** (never 403); title is trimmed + non-empty (else 400
  `validation_failed`); close is idempotent (preserves `closed_at`). All endpoints
  `tracing`-instrumented (spans + INFO mutation events). Committed the `.sqlx/` offline cache;
  `build`/`lint`/`fmt --check`/`prepare` all green. Smoke-tested live over HTTP against a
  local Postgres: register/login, profile isolation (user B → 404 on user A's profile),
  task add/list/close incl. idempotent re-close, all error codes (`invalid_credentials`,
  `unauthenticated`, `not_found`, `username_taken`, `email_taken`, `validation_failed`), and
  confirmed no secret in any log line. Defers slices 4 (deploy/ok.sh), 5 (tests), 6 (verify).
- 2026-06-12 [platform-dev] slice 4 (deploy stack + ok.sh wiring): added `deploy/Dockerfile`
  (multi-stage; build stage compiles `organized-koalad` in release with `SQLX_OFFLINE=true`
  off the committed `.sqlx/` cache, runtime stage is `debian:bookworm-slim` running as an
  unprivileged user, entrypoint the binary), `.dockerignore` (lean context; keeps the two
  `include_str!` crate READMEs), and `deploy/docker-compose.yml` with the ADR-0004 graph:
  Postgres (pg_isready healthcheck) -> one-shot `migrate` (`command: ["migrate"]`, gated
  `depends_on postgres: service_healthy`) -> `server` (`command: ["run"]`, gated
  `depends_on migrate: service_completed_successfully`) -> minimal OTel collector
  (`deploy/otel/collector-config.yaml`: OTLP/gRPC receiver on 4317 + `debug` exporter); the
  server's `OK_OTLP_ENDPOINT` points at the collector over gRPC. The migrate-before-serve
  ordering lives entirely in the compose file (no host command, no `ok.sh` at runtime).
  `ok.sh` wired: `up`/`down` (compose, migrations auto-applied via the one-shot),
  `migrate`/`rollback` as dev-only delegating conveniences shelling to the binary,
  `run-server` -> `organized-koalad run`, `test` boots a throwaway tmpfs Postgres
  (`deploy/docker-compose.test.yml`) and points `sqlx::test` at it via `DATABASE_URL`
  (honours a caller-provided `DATABASE_URL` if set); `--help` documents the dev-only framing.
  Credentials live only in a gitignored `deploy/.env` that `up` generates with obvious
  DEV-ONLY placeholders — the committed stack carries no credential literal (secret-scan
  clean). `build`/`lint`/`fmt --check`/`secret-scan` green; shellcheck clean. Docker is
  unavailable in this sandbox, so the stack is WIRING-ONLY pending the verifier's live boot.

<!-- written at end of cycle; what the human reviews -->
## Summary

[adr-0004]: ../../docs/adr/0004-migration-authority-and-binary-cli.md
[adr-0005]: ../../docs/adr/0005-foundational-wire-contract.md
[feat-0001]: ./0001-foundational-slice.md
[feat-0002]: ./0002-contract-crate.md
