#!/usr/bin/env bash
# check.sh — Run the full local/CI verification suite for this repository.
#
# Steps (in order, each must pass before the next runs):
#   1. cargo fmt --all -- --check   (formatting)
#   2. cargo clippy --locked --all-targets -- -D warnings   (lint)
#   3. cargo test --locked   (Rust unit/integration tests)
#   4. bash -n on every shell script (install.sh, scripts/, lib/, tests/)
#   5. python3 -m pytest tests -q   (Python installer/detect tests)
#   6. bash tests/test_lifecycle.sh   (component detect lifecycle regressions)
#   7. bash tests/test_framework_override.sh (explicit framework override)
#
# Usage:
#   bash scripts/check.sh
#
# Exit codes: 0 when every step passes, 1 on the first failing step. Each
# step is logged to stderr with a `[check]` prefix so CI output stays easy to
# scan; step commands' own stdout/stderr pass through unmodified.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

log() { printf '[check] %s\n' "$*" >&2; }

# ensure_cargo_on_path — Prefer rustup cargo when present, mirroring
# scripts/build_rm_monitor.sh so this script works the same way in a fresh
# CI runner or a developer machine that only has rustup-installed cargo.
ensure_cargo_on_path() {
  if command -v cargo >/dev/null 2>&1; then
    return 0
  fi
  if [ -x "$HOME/.cargo/bin/cargo" ]; then
    # shellcheck disable=SC1091
    [ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"
    export PATH="$HOME/.cargo/bin:$PATH"
  fi
  command -v cargo >/dev/null 2>&1
}

# shell_scripts — Every tracked-style shell script this repo ships, checked
# for syntax errors with `bash -n`. Kept as an explicit list (rather than a
# recursive find) so a new top-level script must be added here deliberately.
shell_scripts() {
  local f
  for f in "$REPO_ROOT"/install.sh "$REPO_ROOT"/scripts/*.sh \
    "$REPO_ROOT"/lib/*.sh "$REPO_ROOT"/tests/*.sh; do
    [ -f "$f" ] && printf '%s\n' "$f"
  done
}

run_cargo_fmt_check() {
  log "cargo fmt --all -- --check"
  (cd "$REPO_ROOT" && cargo fmt --all -- --check)
}

run_cargo_clippy() {
  log "cargo clippy --locked --all-targets -- -D warnings"
  (cd "$REPO_ROOT" && cargo clippy --locked --all-targets -- -D warnings)
}

run_cargo_test() {
  log "cargo test --locked"
  (cd "$REPO_ROOT" && cargo test --locked)
}

run_shell_syntax_checks() {
  log "bash -n on every shell script"
  local script
  while IFS= read -r script; do
    log "  bash -n ${script#"$REPO_ROOT"/}"
    bash -n "$script"
  done < <(shell_scripts)
}

run_pytest() {
  if ! command -v python3 >/dev/null 2>&1; then
    log "python3 not found (required for installer contract tests)"
    return 1
  fi
  if ! python3 -m pytest --version >/dev/null 2>&1; then
    log "pytest module not available (pip install pytest)"
    return 1
  fi
  log "python3 -m pytest tests -q"
  (cd "$REPO_ROOT" && python3 -m pytest tests -q)
}

run_lifecycle_test() {
  log "bash tests/test_lifecycle.sh"
  bash "$REPO_ROOT/tests/test_lifecycle.sh"
}

run_framework_override_test() {
  log "bash tests/test_framework_override.sh"
  bash "$REPO_ROOT/tests/test_framework_override.sh"
}

main() {
  ensure_cargo_on_path || {
    log "cargo not found (install rustup or apt install cargo)"
    exit 1
  }

  run_cargo_fmt_check
  run_cargo_clippy
  run_cargo_test
  run_shell_syntax_checks
  run_pytest
  run_lifecycle_test
  run_framework_override_test

  log "All checks passed."
}

main "$@"
