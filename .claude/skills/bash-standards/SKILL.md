---
name: bash-standards
description: Bash standards for organized-koala (ok.sh, hooks, scripts). Extended over time via learnings + human feedback.
audience: dev
---

# Bash standards

## When to invoke

- Before writing or editing any shell script (`ok.sh`, `.githooks/*`, `deploy/*` scripts).

## The standards

- **Always brace-and-quote variable expansions:** `"${VAR}"`, never `$VAR` or `${VAR}`
  unquoted. This applies to every expansion, including `"$@"` → keep array forms quoted.
- Start scripts with `set -euo pipefail`.
- Resolve the script's own directory rather than assuming a CWD:
  `ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"`.
- Keep operational complexity inside `ok.sh` verbs; call sites invoke verbs, not raw tools.
- **Never download/install/run an external binary without the operator's approval** (CLAUDE.md
  hard constraint #6). `ok.sh` and any script must not fetch+run a tool to satisfy a step: a
  missing tool (docker, a live DB, any required binary) **fails loudly and escalates** — it does
  not silently acquire and run something. Detect the missing capability, print a precise message,
  and exit non-zero; do not `curl … | sh`, do not pull an embedded/throwaway DB image, do not
  reuse a leftover binary.
- `readonly` for constants; lowercase for locals, UPPER_CASE for exported/global config.

```bash
#!/usr/bin/env bash
set -euo pipefail
readonly NAME="${1:-default}"
echo "hello ${NAME}"
```

## Gotchas

- **`secret-scan.sh` matches credential VALUES, not bare identifiers** (learned 0002).
  Described structurally so this doc does not trip its own scanner: the credential-keyword
  group is the four words for a password, an abbreviated password, a generic secret, and an API
  key (case-insensitive). A line matches **only** when one of those keywords is immediately
  followed by an assignment or key separator (an equals sign or a colon) and then **either** a
  quoted string literal **or** an unquoted token of eight-plus characters. So a bare Rust field
  declaration — the keyword followed by a bare type identifier and a comma, with no separator
  and no literal — does **not** false-positive, while an assigned literal does. Known
  non-blocking gaps for future platform-dev hardening: the value-pattern does **not** catch a
  JSON-object form where a quoted key is followed by a quoted value, because a quote (not a
  separator) directly follows the keyword. The dedicated token signatures (private keys,
  AWS/Slack/GitHub tokens, Bearer, JWT-shaped) are independent of this group and always fire.
  Server work (0003) handles real credentials/JWTs — author code so secrets never reach a
  `Debug`/`Display`/log, not merely so the scan passes.

## Extending this skill

Living document — `eng-manager` appends durable bash learnings here.
