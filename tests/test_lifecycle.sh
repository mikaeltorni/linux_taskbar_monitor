#!/usr/bin/env bash
# test_lifecycle.sh - component detection regressions for Resource Monitor.
#
# Verifies the patch-component detect functions (rm_gradient_colors, rm_per_disk,
# rm_vram) read the live extension source markers rather than relying on install
# receipts, so a core re-extract that wipes the patches is detected as absent.

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"

TARGET_HOME="$(mktemp -d)"
trap 'rm -rf "$TARGET_HOME"' EXIT

# resource_monitor_ext_dir / _rm_ext_* helpers live in gnome_extensions.sh and
# lifecycle.sh; source both so the detect functions resolve the extension paths.
RESOURCE_MONITOR_EXTENSION_ID="Resource_Monitor@Ory0n"
SCRIPT_DIR="$ROOT_DIR"
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

# --- Present state: patch markers injected ---------------------------------
printf '_gradientGetUsageColor\n' >"$EXT_DIR/extension.js"
printf 'function getDiskUsagePercentStyle() {}\n' >"$EXT_DIR/services/refreshers.js"
# Unpatched containers.js still has bracket labels; vram detect must stay false.
printf 'const separatorStart = _createBracketLabel("[", [\n' >"$EXT_DIR/panel/containers.js"
detect_rm_gradient_colors || fail "detect_rm_gradient_colors should accept a patched extension.js"
detect_rm_per_disk || fail "detect_rm_per_disk should accept a patched refreshers.js"
detect_rm_vram && fail "detect_rm_vram should reject unpatched bracket labels"

# --- VRAM patched state: bracket labels replaced by the space-separator ---
printf 'const spaceSep = new St.Label({}); // Space separator between GPU usage and VRAM (no brackets)\n' >"$EXT_DIR/panel/containers.js"
detect_rm_vram || fail "detect_rm_vram should accept the patched (bracket-free) containers.js"

printf 'test_lifecycle.sh: all assertions passed\n'
