#!/usr/bin/env bash
# test_lifecycle.sh - component detection regressions for Resource Monitor.
#
# Verifies the patch-component detect functions read the live extension source
# markers rather than relying on install receipts, so a core re-extract that
# wipes the patches is detected as absent.

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"

TARGET_HOME="$(mktemp -d)"
trap 'rm -rf "$TARGET_HOME"' EXIT

# resource_monitor_ext_dir / _rm_ext_* helpers live in gnome_extensions.sh and
# lifecycle.sh; source both so the detect functions resolve the extension paths.
RESOURCE_MONITOR_EXTENSION_ID="Resource_Monitor@Ory0n"
SCRIPT_DIR="$ROOT_DIR"
msg() { :; }
source "$ROOT_DIR/lib/gnome_extensions.sh"
source "$ROOT_DIR/lib/lifecycle.sh"

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

EXT_DIR="$TARGET_HOME/.local/share/gnome-shell/extensions/$RESOURCE_MONITOR_EXTENSION_ID"
mkdir -p "$EXT_DIR/panel" "$EXT_DIR/services"

# run_as_target: detect functions only read files, so dispatch plain commands.
run_as_target() { "$@"; }

# --- Absent state: clean core re-extract has no patch markers ---------------
detect_rm_gradient_colors && fail "detect_rm_gradient_colors should reject a clean extension.js"
detect_rm_per_disk && fail "detect_rm_per_disk should reject a clean refreshers.js"
detect_rm_vram && fail "detect_rm_vram should reject unpatched bracket labels"
detect_rm_hide_eth_icon && fail "detect_rm_hide_eth_icon should reject clean mainGui.js"
detect_rm_process_popup && fail "detect_rm_process_popup should reject clean extension.js"
detect_window_rules && fail "detect_window_rules should reject missing app-rules@local"

# --- Present state: patch markers injected ---------------------------------
printf '_gradientGetUsageColor\nProcess popup: total CPU/RAM aggregated per process name\n' >"$EXT_DIR/extension.js"
printf 'function getDiskUsagePercentStyle() {}\n' >"$EXT_DIR/services/refreshers.js"
# Unpatched containers.js still has bracket labels; vram detect must stay false.
printf 'const separatorStart = _createBracketLabel("[", [\n' >"$EXT_DIR/panel/containers.js"
printf '// clean ethernet wiring\n' >"$EXT_DIR/panel/mainGui.js"
detect_rm_gradient_colors || fail "detect_rm_gradient_colors should accept a patched extension.js"
detect_rm_per_disk || fail "detect_rm_per_disk should accept a patched refreshers.js"
detect_rm_vram && fail "detect_rm_vram should reject unpatched bracket labels"
detect_rm_process_popup || fail "detect_rm_process_popup should accept the process-popup marker"
detect_rm_hide_eth_icon && fail "detect_rm_hide_eth_icon should reject unpatched mainGui.js"

# --- VRAM / eth-icon patched state ----------------------------------------
printf 'const spaceSep = new St.Label({}); // Space separator between GPU usage and VRAM (no brackets)\n' >"$EXT_DIR/panel/containers.js"
detect_rm_vram || fail "detect_rm_vram should accept the patched (bracket-free) containers.js"
printf '// Ethernet icon removed: value/unit kept, icon omitted\n' >"$EXT_DIR/panel/mainGui.js"
detect_rm_hide_eth_icon || fail "detect_rm_hide_eth_icon should accept the eth-icon marker"

# --- Panel spacing detection is mode-aware ----------------------------------
# Stable is present when either reservation marker is in containers.js: the
# disk-space secondary activity marker and/or the GPU VRAM width-split marker
# (GPU-only happens when rm_per_disk was not selected). Compact requires both gone.
DISK_MARKER='Space separator between disk-space activity percent and its unit (stable width)'
GPU_MARKER='VRAM value (0-99 GB, 2 digits) gets its own tighter reserved'
printf '%s\n' "$DISK_MARKER" >"$EXT_DIR/panel/containers.js"
RESOURCE_MONITOR_SPACING_MODE=stable detect_rm_panel_spacing || fail "detect_rm_panel_spacing should report installed via disk marker"
rm -f "$EXT_DIR/panel/containers.js"
RESOURCE_MONITOR_SPACING_MODE=compact detect_rm_panel_spacing && fail "detect_rm_panel_spacing should reject a missing containers.js"
printf '%s\n' "$DISK_MARKER" >"$EXT_DIR/panel/containers.js"
RESOURCE_MONITOR_SPACING_MODE=stable detect_rm_panel_spacing || fail "detect_rm_panel_spacing should still detect the disk marker in stable mode"
RESOURCE_MONITOR_SPACING_MODE=compact detect_rm_panel_spacing && fail "detect_rm_panel_spacing must treat a present disk marker as NOT installed in compact mode"
# GPU-only stable (soft-skip disk path when rm_per_disk was not selected).
printf '%s\n' "$GPU_MARKER" >"$EXT_DIR/panel/containers.js"
RESOURCE_MONITOR_SPACING_MODE=stable detect_rm_panel_spacing || fail "detect_rm_panel_spacing should report installed via GPU marker alone"
RESOURCE_MONITOR_SPACING_MODE=compact detect_rm_panel_spacing && fail "detect_rm_panel_spacing must treat a present GPU marker as NOT installed in compact mode"
# Compact with neither marker: considered applied.
printf '// no stable-width markers\n' >"$EXT_DIR/panel/containers.js"
RESOURCE_MONITOR_SPACING_MODE=compact detect_rm_panel_spacing || fail "detect_rm_panel_spacing should report compact when both markers are absent"
RESOURCE_MONITOR_SPACING_MODE=stable detect_rm_panel_spacing && fail "detect_rm_panel_spacing must reject marker-free containers in stable mode"

# --- Window rules directory presence ---------------------------------------
mkdir -p "$TARGET_HOME/.local/share/gnome-shell/extensions/app-rules@local"
detect_window_rules || fail "detect_window_rules should accept an installed app-rules@local"

# --- Refresh interval detect compares schema value to configured ms --------
mkdir -p "$EXT_DIR/schemas"
ext_gsettings() {
  # $1 = ext_dir; remaining args are gsettings argv. Stub refreshtime get.
  shift
  if [ "${1:-}" = get ] && [ "${3:-}" = refreshtime ]; then
    printf '%s\n' "${STUB_REFRESHTIME:-0.500}"
    return 0
  fi
  return 1
}
STUB_REFRESHTIME=0.500 RESOURCE_MONITOR_REFRESH_INTERVAL_MS=500 \
  detect_rm_refresh_interval || fail "detect_rm_refresh_interval should match 500 ms -> 0.500 s"
# gsettings often prints `0.5` for the same double; string equality must not fail.
STUB_REFRESHTIME=0.5 RESOURCE_MONITOR_REFRESH_INTERVAL_MS=500 \
  detect_rm_refresh_interval || fail "detect_rm_refresh_interval should treat 0.5 and 0.500 as equal"
STUB_REFRESHTIME=0.500 RESOURCE_MONITOR_REFRESH_INTERVAL_MS=1000 \
  detect_rm_refresh_interval && fail "detect_rm_refresh_interval should reject a mismatched refreshtime"
rm -rf "$EXT_DIR/schemas"
detect_rm_refresh_interval && fail "detect_rm_refresh_interval should reject a missing schemas dir"

printf 'test_lifecycle.sh: all assertions passed\n'
