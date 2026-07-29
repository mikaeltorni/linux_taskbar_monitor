#!/usr/bin/env bash
# rm_monitor_bin.sh — Resolve and invoke the rm-monitor Rust CLI.
#
# Sourced by install.sh / lib/*.sh after SCRIPT_DIR is set. Builds the binary
# on demand via scripts/build_rm_monitor.sh (local cargo or container), then
# runs subcommands that replace the former Node/Python helpers.
#
# Public functions:
#   ensure_rm_monitor_bin  — build/locate dist/rm-monitor; sets RM_MONITOR_BIN
#   rm_monitor             — ensure_rm_monitor_bin then exec it with "$@"

# ensure_rm_monitor_bin — Locate or build dist/rm-monitor; export RM_MONITOR_BIN.
#
# Returns non-zero when the binary cannot be produced. Logs via msg when
# available, otherwise stderr.
ensure_rm_monitor_bin() {
  local build_script bin
  build_script="${SCRIPT_DIR:?SCRIPT_DIR unset}/scripts/build_rm_monitor.sh"
  if [ -n "${RM_MONITOR_BIN:-}" ] && [ -x "$RM_MONITOR_BIN" ]; then
    return 0
  fi
  if [ ! -x "$build_script" ]; then
    if declare -F msg >/dev/null 2>&1; then
      msg "Missing build helper: $build_script"
    else
      printf 'Missing build helper: %s\n' "$build_script" >&2
    fi
    return 1
  fi
  bin="$(bash "$build_script" --print)" || return 1
  RM_MONITOR_BIN="$bin"
  export RM_MONITOR_BIN
}

# rm_monitor — Run the rm-monitor CLI with the given arguments as the target user.
#
# Uses run_as_target when that function exists (installer context); otherwise
# runs directly. Arguments after the function name are passed through unchanged.
rm_monitor() {
  ensure_rm_monitor_bin || return 1
  if declare -F run_as_target >/dev/null 2>&1; then
    run_as_target "$RM_MONITOR_BIN" "$@"
  else
    "$RM_MONITOR_BIN" "$@"
  fi
}
