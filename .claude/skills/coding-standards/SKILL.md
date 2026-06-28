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
- **Focus traversal skips non-interactive elements (learned 0016).** In any per-field
  detail/form view, `Tab`/`Shift+Tab` (or arrow) focus cycling must move only between
  **interactive** panes/fields; read-only / display-only fields stay **rendered** but are
  **excluded from the focus order** (and initial + fallback focus land on the first interactive
  field). Including a read-only field in the focus ring creates a dead stop — focus lands
  somewhere that does nothing and the user must press again to reach the next editable field.
  This is a recurring UX miss: in 0016 the task Status/Created/Closed and note Created panes were
  focus stops, and the plan, ADR review, cold review, and live verify **all passed it** before
  human feedback caught it — model the focus order over the *editable* set, not the full pane
  list.

### Testing

- **High coverage**, but test the **public API / observable behaviour** — not internals.
- **Mocks only for external services** (DB, network, the server from the TUI's view). Never
  mock internal collaborators.
- **If it's hard to test, that's an architecture smell** — bubble up to `architect`; do not
  bend production code to make a test pass.
- **A claim that a behaviour is "covered" must name a test that actually exists** (learned 0003).
  A slice-5 Log entry asserted expired-token→401 was "covered by source-owned jwt unit tests"
  that were never written, so an unenforced `exp` path reached `awaiting-merge` and only human
  review caught it. Any Log/Summary line citing specific coverage must point at a real, passing
  test; do not describe coverage you intend to add, only coverage that is present and green.

### Comments

- Short. Describe **what this code does** and its expectations/gotchas.
- **No development context** (no "I changed this because…", no ticket numbers) and **nothing
  about how other layers/scopes work** — a comment stays within its own scope.

## Extending this skill

This is a living document. When a cycle or human feedback surfaces a durable rule,
`eng-manager` adds it here with a one-line rationale.
