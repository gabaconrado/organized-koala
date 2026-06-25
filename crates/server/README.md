# server (`organized-koalad`)

The organized-koala HTTP server: an `axum` + `sqlx` (Postgres) service implementing the
ADR-0005 wire contract — local auth (argon2 password hashing, JWT HS256 sessions),
atomically-bootstrapped default profiles, and profile-scoped TODO tasks. All wire shapes come
from the `contract` crate; this crate never redefines a DTO.

## Binary and admin CLI

The crate ships one binary, `organized-koalad`, whose `clap` CLI is the artifact's full
operational surface (ADR-0004):

- `organized-koalad run` — run the long-running HTTP server (the default no-arg behaviour).
  The serve path never mutates schema.
- `organized-koalad migrate` — apply all pending migrations and exit (idempotent).
- `organized-koalad rollback [--steps N]` — revert applied migrations (one step by default;
  an explicit admin action, never automated).

The `migrations/` tree is embedded into the binary via `sqlx::migrate!`, so the shipped image
carries its own schema and needs no `sqlx` CLI or checkout at runtime.

## Endpoints

| Method | Path | Auth | Purpose |
| --- | --- | --- | --- |
| `GET` | `/healthz` | no | liveness probe |
| `POST` | `/api/auth/register` | no | create user + named default profile (one transaction) |
| `POST` | `/api/auth/login` | no | exchange credentials for a JWT |
| `GET` | `/api/profiles` | yes | list the caller's profiles |
| `POST` | `/api/profiles/{pid}/tasks` | yes | create a task in an owned profile |
| `GET` | `/api/profiles/{pid}/tasks` | yes | list a profile's tasks, newest-first |
| `PATCH` | `/api/profiles/{pid}/tasks/{tid}` | yes | update a task (title/description/status; `done` sets `closed_at`, `open` reopens) |
| `DELETE` | `/api/profiles/{pid}/tasks/{tid}` | yes | delete a task (`204`; second/unowned → `404`) |

Every query is ownership-joined: a profile the caller does not own is indistinguishable from a
nonexistent one (`404 not_found`, never `403`). Errors map to the standard HTTP status plus a
`contract::ErrorBody` (`{ code?, message }`).

## Configuration (environment)

| Variable | Default | Meaning |
| --- | --- | --- |
| `OK_DATABASE_URL` | — (required) | Postgres connection string |
| `OK_JWT_SECRET` | — (required for `run`) | HS256 signing secret (held as `SecretString`) |
| `OK_JWT_TTL_SECONDS` | `86400` | session token lifetime (24 h) |
| `OK_BIND_ADDR` | `0.0.0.0:8080` | HTTP listen address |
| `OK_OTLP_ENDPOINT` | unset | OTLP collector endpoint; tracing exports there when set |
| `OK_AUTO_MIGRATE` | `0` | dev-only: when `1`, `run` applies pending migrations on boot |
