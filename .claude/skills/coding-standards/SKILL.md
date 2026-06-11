---
name: coding-standards
description: General coding standards every developer agent follows. Extended over time via learnings + human feedback.
audience: dev
---

# Coding standards

## When to invoke

- Before writing or reviewing any code in any crate.

## The standards

### Priority order (when forces conflict, decide in this order)

1. **Correctness** — it does the right thing.
2. **Security** — no injection, no leaked secrets, profile isolation holds.
3. **Simplicity** — the smallest design that is correct and secure.
4. **Performance** — fast enough; optimize only with evidence.
5. **Others** — everything else (style, ergonomics, cleverness).

### Design

- **Deep modules.** A module exposes a small, simple interface over substantial hidden
  implementation. Favor a narrow public API and a thick private body; avoid shallow
  pass-through layers.
- **Tracer-bullet development.** Build a thin end-to-end slice first (a real request flowing
  through every layer), then widen it. Prove the seams early; don't build a layer in isolation.

### Testing

- **High coverage**, but test the **public API / observable behaviour** — not internals.
- **Mocks only for external services** (DB, network, the server from the TUI's view). Never
  mock internal collaborators.
- **If it's hard to test, that's an architecture smell** — bubble up to `architect`; do not
  bend production code to make a test pass.

### Comments

- Short. Describe **what this code does** and its expectations/gotchas.
- **No development context** (no "I changed this because…", no ticket numbers) and **nothing
  about how other layers/scopes work** — a comment stays within its own scope.

## Extending this skill

This is a living document. When a cycle or human feedback surfaces a durable rule,
`eng-manager` adds it here with a one-line rationale.
