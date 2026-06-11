---
name: docs-standards
description: Documentation/Markdown standards for organized-koala. Extended over time via learnings + human feedback.
audience: dev
---

# Documentation standards

## When to invoke

- Before writing or editing any Markdown (ADRs, handoff, board, READMEs, skills, agents).

## The standards

### Markdown links use reference style

Define the target once at the bottom and reference it by label inline — not inline URLs.

```markdown
See the [axum docs][axum] and [ADR-0001][adr-0001].

[axum]: https://docs.rs/axum
[adr-0001]: ./adr/0001-foundational-architecture.md
```

Rationale: keeps prose readable, makes link targets auditable in one place, and avoids
duplicated URLs.

When adding a reference-style link, write the inline `[text][label]` **and** its
`[label]: <target>` definition in the **same edit** — never split them across two edits. A
file with an inline reference but no matching definition is transiently invalid and trips
markdownlint **MD052** ("reference links should use a label that is defined"), which gates the
edit.

### General

- Prose is concise; ADRs follow the §4.2 template (Context / Decision / Consequences).
- The Board and docs are potentially public — never include secrets or sensitive payloads.
- **Do not start a wrapped prose line with `+`, `*`, or `-`** (learned 0002). The linter reads a
  line-leading `+`/`*`/`-` as a list marker (`rumdl`/markdownlint MD004), so a continuation line
  of a Board Log bullet that wraps onto a leading `+` (e.g. a commit-count "37 + 12") trips the
  unordered-list-style rule. Reflow so the operator/symbol is not the first character of a line.

## Extending this skill

Living document — `eng-manager` appends durable documentation learnings here.
