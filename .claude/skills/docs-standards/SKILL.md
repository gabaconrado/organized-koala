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
- **Do not start a wrapped prose line with `#` or a list-like token** (learned 0003). A
  continuation line inside a Board list item that wraps onto a leading `#` (e.g. "constraints
  #1–#6" breaking so a line begins `#1`) is misparsed by `rumdl` as a block boundary → MD032,
  and `rumdl fmt`'s auto-fix "resolves" it by **inserting a blank line that splits the prose
  paragraph** — corrupting it. Leading `(1)`-style tokens are similarly risky. Reword (e.g.
  "constraints 1–6") so no wrapped line begins with `#` or a list marker. **Never blindly accept
  `rumdl fmt` output on prose** — re-read the diff.
- **A successful commit does NOT prove markdown is lint-clean** (learned 0003). `.githooks/pre-commit`
  is a **secret-scan only**; markdown linting (`rumdl`, line-length 100) is the PostToolUse
  `.claude/lint.sh` hook, which does **not** gate `git commit`. So a long line can commit cleanly.
  After editing any Markdown, run `rumdl check --config .claude/rumdl.toml <file>` explicitly
  before considering it done.

## Extending this skill

Living document — `eng-manager` appends durable documentation learnings here.
