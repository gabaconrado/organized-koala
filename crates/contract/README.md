# contract

Shared wire types for organized-koala: the `serde`-derived DTOs that the HTTP server
(`organized-koalad`) and the TUI (`organized-koala`) exchange over the wire. This crate is
the **single source of truth** for every wire shape — neither consumer redefines a DTO, and a
change here is an ADR event (see `CLAUDE.md` and ADR-0005).

It is a **pure data crate**: no HTTP, no database, no I/O. It carries shapes and documents the
conventions; validation rules (non-empty title, `@`-free usernames) are enforced server-side.

## Wire conventions

These are fixed by ADR-0005 and bind every DTO here:

- JSON object fields are `snake_case`.
- Ids are server-generated **UUID strings**.
- Timestamps are **RFC 3339 UTC strings** (e.g. `2026-06-11T12:00:00Z`).
- Enums serialize as **lowercase strings** (`open`, `done`).
- A task's `closed_at` is **nullable** (`null` while the task is open).

UUIDs and timestamps cross the wire as plain strings: the contract carries the shape, while
parsing and validation happen at the server and TUI boundaries.

## What's here

- Auth: [`RegisterRequest`], [`LoginRequest`], [`SessionResponse`].
- Profiles: [`Profile`].
- Tasks: [`Task`], [`TaskStatus`], [`CreateTaskRequest`].
- Errors: [`ErrorBody`] and [`ErrorCode`] — the standard error payload `{ code?, message }`
  with the stable, machine-matchable code identifiers.

The error `code` is forward-compatible: known codes deserialize to their [`ErrorCode`]
variant, and any unrecognized string is preserved as [`ErrorCode::Unknown`] rather than
failing — so a consumer built against an older code set still parses newer responses.
