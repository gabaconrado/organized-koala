#!/usr/bin/env bash
#
# lint.sh — auto-lint a single file that Claude just wrote.
#
# Invoked by the PostToolUse (Write|Edit) hook in .claude/settings.json, which pipes the
# hook payload as JSON on stdin. We pull the written file's path out with `jq`, dispatch by
# extension to the right linter, and surface any findings on stderr with exit code 2 — the
# PostToolUse contract that feeds stderr back to Claude so it reads the diagnostics and
# fixes the file. These linters only REPORT; they are not run in fix mode.
#
#   .sh            -> shellcheck
#   .md/.markdown  -> rumdl check --config <this dir>/rumdl.toml
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
readonly SCRIPT_DIR
readonly RUMDL_CONFIG="${SCRIPT_DIR}/rumdl.toml"

# The hook payload arrives as JSON on stdin; jq extracts the written file's path.
# Write and Edit both populate tool_input.file_path.
payload="$(cat)"
file="$(printf '%s' "${payload}" | jq -r '.tool_input.file_path // empty')"

# Nothing actionable: no path supplied, or the file no longer exists on disk.
[[ -n "${file}" && -f "${file}" ]] || exit 0

case "${file}" in
  *.sh)
    if ! out="$(shellcheck "${file}" 2>&1)"; then
      printf 'shellcheck found issues in %s — fix them:\n%s\n' "${file}" "${out}" >&2
      exit 2
    fi
    ;;
  *.md | *.markdown)
    if ! out="$(rumdl check --config "${RUMDL_CONFIG}" "${file}" 2>&1)"; then
      printf 'rumdl found issues in %s — fix them:\n%s\n' "${file}" "${out}" >&2
      exit 2
    fi
    ;;
  *)
    # Extension we do not lint — nothing to do.
    ;;
esac
