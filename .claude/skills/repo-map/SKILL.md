---
name: repo-map
description: A short tour of the repo — which crate/path is what, and which agent owns it.
audience: dev
---

# Repo map

## When to invoke

- On waking into any task, to recompute "where things are and who owns them."

## Layout & ownership

| Path | What | Owner agent |
| --- | --- | --- |
| `crates/contract/` | shared wire types (DTOs, error payload) — single source of truth | `contract-owner` |
| `crates/server/` | package `server`, binary `organized-koalad` — axum/sqlx/auth/OTel | `server-dev` |
| `crates/tui/` | package `tui`, binary `organized-koala` — ratatui/reqwest | `tui-dev` |
| `crates/<shared>/` | narrowly-scoped shared crates (e.g. observability) | its own dev agent (or shared if trivial) |
| `tests/`, `*_test.rs` | tests (own files, public API) | `tester` |
| `ok.sh` | the operations entrypoint | `platform-dev` |
| `deploy/` | docker-compose stack + OTel collector config | `platform-dev` |
| `.githooks/` | pre-commit secret scan | `platform-dev` |
| `docs/` | ADRs, decisions index, handoff, build-plan | `architect` (ADRs) / `eng-manager` (rest) |
| `board/` | coordination state (the state machine) | every agent appends; `eng-manager` regenerates README |
| `.claude/agents/`, `.claude/skills/` | the team + its skills | `eng-manager` |

## Key facts

- All ops via `./ok.sh <verb>` (sqlx offline mode). See CLAUDE.md "How to run".
- The TUI is stateless; `contract` is authoritative; the domain is flat; profiles namespace
  TODOs/Notes; auth is local. (CLAUDE.md "Hard constraints".)
- Status state machine: `inbox → planned → ready → working → review → awaiting-merge → merged | blocked`.
  The AI stops at `awaiting-merge`; the human merges.
