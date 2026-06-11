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
- `readonly` for constants; lowercase for locals, UPPER_CASE for exported/global config.

```bash
#!/usr/bin/env bash
set -euo pipefail
readonly NAME="${1:-default}"
echo "hello ${NAME}"
```

## Extending this skill

Living document — `eng-manager` appends durable bash learnings here.
