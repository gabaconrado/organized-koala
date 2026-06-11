#!/usr/bin/env bash
#
# ok.sh — the single entrypoint for every workspace operation.
# All build/test/lint/format/migrate/stack complexity is hidden here, so agents and humans
# invoke verbs (`./ok.sh test`) instead of raw cargo/docker/sqlx. Owned by `platform-dev`.
#
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
readonly ROOT_DIR
cd "${ROOT_DIR}"

# sqlx offline mode: build/test must not require a live database.
export SQLX_OFFLINE="${SQLX_OFFLINE:-true}"

# Crate/binary names (see CLAUDE.md crate layout).
readonly SERVER_BIN="organized-koalad"
readonly TUI_BIN="organized-koala"
readonly COMPOSE_FILE="${ROOT_DIR}/deploy/docker-compose.yml"

usage() {
  cat <<'EOF'
ok.sh — workspace operations

  ./ok.sh build            build the workspace
  ./ok.sh test             run all tests
  ./ok.sh lint             cargo clippy --all-targets (lint levels in Cargo.toml)
  ./ok.sh fmt [--check]    format (or verify formatting)
  ./ok.sh migrate          run sqlx migrations
  ./ok.sh prepare          regenerate the .sqlx/ offline query cache (needs a live DB)
  ./ok.sh up               bring the docker stack up (server + postgres + otel)
  ./ok.sh down             tear the docker stack down
  ./ok.sh run-server       run the server (organized-koalad)
  ./ok.sh run-tui          run the TUI (organized-koala)
  ./ok.sh secret-scan      scan staged diff + board for secrets
  ./ok.sh check            test + lint + fmt --check (the local gate)
EOF
}

cmd_build()      { cargo build --workspace; }
cmd_test()       { cargo test --workspace; }
# Lint levels (deny warnings) live in Cargo.toml [workspace.lints]; clippy takes no rules here.
# --all-targets is scope (also lint tests/benches/examples), not a lint rule.
cmd_lint()       { cargo clippy --all-targets; }

cmd_fmt() {
  if [[ "${1:-}" == "--check" ]]; then
    cargo fmt --all -- --check
  else
    cargo fmt --all
  fi
}

cmd_migrate()    { sqlx migrate run; }
cmd_prepare()    { cargo sqlx prepare --workspace; }

cmd_up() {
  require_compose
  docker compose -f "${COMPOSE_FILE}" up -d
}

cmd_down() {
  require_compose
  docker compose -f "${COMPOSE_FILE}" down
}

cmd_run_server() { cargo run --bin "${SERVER_BIN}" -- "$@"; }
cmd_run_tui()    { cargo run --bin "${TUI_BIN}" -- "$@"; }

cmd_secret_scan() { "${ROOT_DIR}/.githooks/secret-scan.sh"; }

cmd_check() {
  cmd_test
  cmd_lint
  cmd_fmt --check
}

require_compose() {
  if [[ ! -f "${COMPOSE_FILE}" ]]; then
    echo "ok.sh: ${COMPOSE_FILE} not found yet (platform-dev builds the stack)." >&2
    exit 1
  fi
}

main() {
  local verb="${1:-}"
  shift || true
  case "${verb}" in
    build)       cmd_build "$@" ;;
    test)        cmd_test "$@" ;;
    lint)        cmd_lint "$@" ;;
    fmt)         cmd_fmt "$@" ;;
    migrate)     cmd_migrate "$@" ;;
    prepare)     cmd_prepare "$@" ;;
    up)          cmd_up "$@" ;;
    down)        cmd_down "$@" ;;
    run-server)  cmd_run_server "$@" ;;
    run-tui)     cmd_run_tui "$@" ;;
    secret-scan) cmd_secret_scan "$@" ;;
    check)       cmd_check "$@" ;;
    ""|-h|--help|help) usage ;;
    *) echo "ok.sh: unknown verb '${verb}'" >&2; usage; exit 1 ;;
  esac
}

main "$@"
