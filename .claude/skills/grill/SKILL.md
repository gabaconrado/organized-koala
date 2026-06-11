---
name: grill
description: Adversarially stress-test a design BEFORE coding — interview one question at a time to surface hidden assumptions and failure modes. Use for risky or contract-shaping features.
audience: dev
---

# grill

## When to invoke

- By `architect` before committing to a risky plan — especially anything touching the
  contract, auth, profile isolation, or the timer-authority decision.

## Procedure

### 1. One question at a time

Ask a single sharp question, get the answer (from the request, the code, or a resolved
assumption), then ask the next. Do not batch — each answer shapes the next probe.

### 2. Attack these surfaces

- **Contract:** does this add or change a wire shape? What breaks on the other side?
- **Hard constraints:** does it sneak in client-side state, cross-profile access, domain
  bloat, or external auth? (Each is forbidden without an ADR.)
- **Failure modes:** server offline (TUI is stateless — what does the user see?), DB error,
  expired JWT, concurrent profile switch.
- **Testability:** can the public API be tested without mocking internals? If not, redesign.
- **Simplicity:** is there a smaller design that still satisfies the criteria?

### 3. Record the outcome

Fold surviving assumptions into the plan's "Assumptions"; turn any decision into an ADR.
If grilling reveals a genuine fork, block with a precise question.
