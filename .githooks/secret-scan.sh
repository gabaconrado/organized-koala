#!/usr/bin/env bash
#
# secret-scan.sh — refuse a commit if the staged diff (or Board files) contain credential
# patterns. The Board is committed and potentially public, so this is mandatory (see
# CLAUDE.md "The Board"). Invoked by .githooks/pre-commit and by `./ok.sh secret-scan`.
#
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
readonly ROOT_DIR
cd "${ROOT_DIR}"

# Credential signatures. Each is an extended-regex matched against staged additions.
readonly PATTERNS=(
  '-----BEGIN [A-Z ]*PRIVATE KEY-----'   # private keys
  'AKIA[0-9A-Z]{16}'                      # AWS access key id
  'xox[bpars]-[0-9A-Za-z-]{10,}'          # slack tokens
  'gh[pousr]_[0-9A-Za-z]{30,}'            # github tokens
  'Bearer [A-Za-z0-9._~+/-]{20,}'         # bearer tokens
  # Assigned secret VALUES, not bare Rust declarations. A field/type decl such as
  #   `pub password: Password,`  or  `password: Password::new(...)`
  # is `<key>: <Type>` — the RHS is an identifier or a `Type::method(...)` form, which is
  # source code, not a leaked credential. We therefore match only two genuine value shapes
  # (the key, then `=` or `:`, then):
  #   (A) a quoted string-literal RHS  (a "..." or '...' literal)
  #   (B) an unquoted RHS of >=8 secret-like chars containing no `(` and no `::`
  # (B)'s length floor and char class exclude short type names, and forbidding `(`/`::` excludes
  # the `Type::method(...)` construction form. This still catches real `key=value` / `key: value`
  # leaks (env files, configs, source) while passing legitimate typed-field declarations.
  '(password|passwd|secret|api[_-]?key)[[:space:]]*[=:][[:space:]]*("[^"]+"|'"'"'[^'"'"']+'"'"')'
  '(password|passwd|secret|api[_-]?key)[[:space:]]*[=:][[:space:]]*[A-Za-z0-9._+/=-]{8,}([[:space:]]|$)'
  'eyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}'  # JWT-shaped
)

# Only scan ADDED lines in the staged diff (leading '+', excluding the +++ header).
staged_additions="$(git diff --cached --no-color -U0 | grep -E '^\+' | grep -Ev '^\+\+\+' || true)"

hits=0
for pattern in "${PATTERNS[@]}"; do
  if matches="$(printf '%s\n' "${staged_additions}" | grep -nEi -e "${pattern}" || true)"; then
    if [[ -n "${matches}" ]]; then
      echo "secret-scan: possible credential matching /${pattern}/:" >&2
      printf '%s\n' "${matches}" >&2
      hits=1
    fi
  fi
done

if [[ "${hits}" -ne 0 ]]; then
  echo "" >&2
  echo "secret-scan: commit refused. Remove the secret or, if a false positive, commit with --no-verify after review." >&2
  exit 1
fi

echo "secret-scan: clean."
