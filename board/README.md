# Board

Coordination state for organized-koala (this replaces a ticket tracker). One file per work
item in `features/`; the `status:` frontmatter **is** the state machine. This dashboard is
**derived** — `eng-manager` regenerates it from item frontmatter; do not hand-edit as truth.

> The Board is committed and potentially public. **Never write secrets or sensitive payloads
> into any item.** Describe behaviour and shape.

## State machine

```text
inbox → planned → ready → working → review → awaiting-merge → merged | blocked
```

The AI cycle is terminal at `awaiting-merge`; only the human merges (→ `merged`).

## Items

| ID | Title | Status | Priority | Branch |
| --- | --- | --- | --- | --- |
| [0001](./features/0001-foundational-slice.md) | Foundational vertical slice (auth + profile + minimal TODO) | inbox | high | — |
