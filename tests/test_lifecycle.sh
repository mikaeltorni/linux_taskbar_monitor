#!/usr/bin/env bash
# test_lifecycle.sh - component detection regressions for Resource Monitor.

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"

TARGET_HOME="$(mktemp -d)"
trap 'rm -rf "$TARGET_HOME"' EXIT

source "$ROOT_DIR/lib/lifecycle.sh"

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

GPUMEMORYMONITOR="'used'"
GPUSTATUS="true"

run_as_target() {
  if [ "$1" = "gsettings" ] && [ "$2" = "get" ]; then
    case "$3 $4" in
      "$RM_SCHEMA gpustatus") printf '%s\n' "$GPUSTATUS" ;;
      "$RM_SCHEMA gpumemorymonitor") printf '%s\n' "$GPUMEMORYMONITOR" ;;
      *) return 1 ;;
    esac
    return 0
  fi
  "$@"
}

detect_rm_vram || fail "detect_rm_vram should accept the schema's string monitor value"

GPUMEMORYMONITOR="'disabled'"
if detect_rm_vram; then
  fail "detect_rm_vram should reject disabled GPU memory monitoring"
fi

GPUMEMORYMONITOR="'used'"
GPUSTATUS="false"
if detect_rm_vram; then
  fail "detect_rm_vram should reject disabled GPU status"
fi

printf 'test_lifecycle.sh: all assertions passed\n'
