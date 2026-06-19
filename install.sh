#!/usr/bin/env bash
# install.sh - Install and configure the Ubuntu 24.04 taskbar system status monitor
#              and related GNOME Shell extensions.
#
# Components:
#   - Resource Monitor extension (CPU/RAM/disk/GPU indicator)
#   - Window Rules extension (app-rules@local — workspace/sticky rules)
#   - Auto-move-windows extension (Wayland workspace placement)
#   - Dash-to-Panel configuration and Ubuntu Dock disabling
#   - PWA icon setup for Chrome progressive web apps
#
# Idempotent: skips already-installed extensions, detects existing desktop files.
#
# Usage: bash install.sh        # user-level setup; apt steps skipped and reported
#        sudo bash install.sh   # full setup including apt packages

set -euo pipefail

TARGET_USER="${SUDO_USER:-$USER}"
TARGET_HOME="$(getent passwd "$TARGET_USER" | cut -d: -f6)"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RUNTIME_DIR="/run/user/$(id -u "$TARGET_USER")"
DISPLAY_VAL="${DISPLAY:-:0}"
USER_BUS="unix:path=${RUNTIME_DIR}/bus"
SESSION_TYPE="${XDG_SESSION_TYPE:-unknown}"

# ── Resource Monitor extension settings ───────────────────────────────────────
RESOURCE_MONITOR_EXTENSION_ID="${RESOURCE_MONITOR_EXTENSION_ID:-Resource_Monitor@Ory0n}"
RESOURCE_MONITOR_EXTENSION_URL="${RESOURCE_MONITOR_EXTENSION_URL:-https://extensions.gnome.org/extension-data/Resource_MonitorOry0n.v27.shell-extension.zip}"
RESOURCE_MONITOR_EXTENSION_SHA256="${RESOURCE_MONITOR_EXTENSION_SHA256:-761f422933ed8e76b0c4653ae7bce5862902920cb7e9f4de2cec23b899d6d170}"

# ── Chrome PWA definitions — app-id|name|desktop_file|workspace ───────────────
CHROME_PWAS=(
  "chatgpt|ChatGPT|chatgpt.desktop|0"
  "monkeytype|Monkeytype|monkeytype.desktop|7"
)

# ── Core helpers (inline to avoid cross-repo sourcing issues) ─────────────────
msg() { printf '[%(%Y-%m-%dT%H:%M:%S%z)T] %s\n' -1 "$*"; }
need_cmd() { command -v "$1" >/dev/null 2>&1; }

run_as_target() {
  if [ "$(id -un)" = "$TARGET_USER" ]; then
    env HOME="$TARGET_HOME" USER="$TARGET_USER" LOGNAME="$TARGET_USER" \
      XDG_RUNTIME_DIR="$RUNTIME_DIR" DBUS_SESSION_BUS_ADDRESS="$USER_BUS" \
      DISPLAY="$DISPLAY_VAL" "$@"
  else
    sudo -H -u "$TARGET_USER" env \
      HOME="$TARGET_HOME" USER="$TARGET_USER" LOGNAME="$TARGET_USER" \
      XDG_RUNTIME_DIR="$RUNTIME_DIR" DBUS_SESSION_BUS_ADDRESS="$USER_BUS" \
      DISPLAY="$DISPLAY_VAL" "$@"
  fi
}

# is_root: Return success when running with root privileges.
is_root() { [ "$(id -u)" -eq 0 ]; }

# Root-only steps skipped during a non-root run; reported at the end.
SUDO_REQUIRED_STEPS=()
note_sudo_required() { SUDO_REQUIRED_STEPS+=("$1"); msg "SKIP (requires sudo): $1"; }
report_sudo_required() {
  [ "${#SUDO_REQUIRED_STEPS[@]}" -eq 0 ] && return 0
  msg "The following steps still require root and were skipped:"
  printf '    - %s\n' "${SUDO_REQUIRED_STEPS[@]}"
  msg "Apply them with: sudo bash install.sh"
}

append_gsettings_list() {
  local schema="$1" key="$2" value="$3" current newlist
  current="$(run_as_target gsettings get "$schema" "$key" 2>/dev/null || echo "[]")"
  newlist="$(VAL="$value" CURRENT="$current" python3 - <<'PY'
import ast, os
cur_raw = os.environ.get("CURRENT", "").strip()
if cur_raw.startswith("@as "): cur_raw = cur_raw[4:].strip()
try: cur = ast.literal_eval(cur_raw) if cur_raw else []
except Exception: cur = []
if not isinstance(cur, list): cur = []
cur = list(dict.fromkeys(str(item) for item in cur))
val = os.environ.get("VAL", "")
if val and val not in cur: cur.append(val)
print("[" + ", ".join(repr(str(item)) for item in cur) + "]")
PY
)"
  run_as_target gsettings set "$schema" "$key" "$newlist"
}

remove_gsettings_list() {
  local schema="$1" key="$2" value="$3" current newlist
  current="$(run_as_target gsettings get "$schema" "$key" 2>/dev/null || echo "[]")"
  newlist="$(VAL="$value" CURRENT="$current" python3 - <<'PY'
import ast, os
cur_raw = os.environ.get("CURRENT", "").strip()
if cur_raw.startswith("@as "): cur_raw = cur_raw[4:].strip()
try: cur = ast.literal_eval(cur_raw) if cur_raw else []
except Exception: cur = []
if not isinstance(cur, list): cur = []
val = os.environ.get("VAL", "")
cur = [str(item) for item in cur if str(item) != val]
cur = list(dict.fromkeys(cur))
print("[" + ", ".join(repr(str(item)) for item in cur) + "]")
PY
)"
  run_as_target gsettings set "$schema" "$key" "$newlist"
}

gsettings_key_exists() {
  local schema="$1" key="$2"
  run_as_target gsettings list-keys "$schema" 2>/dev/null | grep -qx "$key"
}

apt_install() {
  local missing=() pkg
  for pkg in "$@"; do dpkg -s "$pkg" >/dev/null 2>&1 || missing+=("$pkg"); done
  [ "${#missing[@]}" -eq 0 ] && return 0
  if ! is_root; then note_sudo_required "apt install ${missing[*]}"; return 0; fi
  DEBIAN_FRONTEND=noninteractive apt install -y "${missing[@]}"
}

# ── Source extension library modules ──────────────────────────────────────────
source "$SCRIPT_DIR/lib/extension_installation.sh"
source "$SCRIPT_DIR/lib/window_rules_extension.sh"
source "$SCRIPT_DIR/lib/window_manager.sh"
source "$SCRIPT_DIR/lib/gnome_extensions.sh"
source "$SCRIPT_DIR/lib/extension_features.sh"

# ── Component selection runtime and manifest ─────────────────────────────────
# The manifest maps each component id to a configure_*/install_* function from
# the lib files sourced above.
# Load the shared installer component framework. Its single source of truth is
# the linux_installation_scripts_functions repository (cloned as a sibling by
# installation_scripts, or downloaded on demand) -- no per-repo vendored copy.
for __isc_d in "${ISC_FUNCTIONS_DIR:-}" \
               "$SCRIPT_DIR/../linux_installation_scripts_functions" \
               "$HOME/projects/linux_installation_scripts_functions"; do
  [ -n "$__isc_d" ] && [ -f "$__isc_d/component_loader.sh" ] && { source "$__isc_d/component_loader.sh"; break; }
done
declare -F isc_activate_components >/dev/null 2>&1 || \
  source <(curl -fsSL "https://raw.githubusercontent.com/mikaeltorni/linux_installation_scripts_functions/${ISC_FUNCTIONS_REF:-master}/component_loader.sh")
isc_activate_components
source "$SCRIPT_DIR/installer/components.sh"

# ── Main installer logic ─────────────────────────────────────────────────────
# Listing/help must print only their own output (the master installer parses
# --list-components); bypass the surrounding messages for those.
case "${1:-}" in
  --list-components|--help|-h) component_main "$@"; exit $? ;;
esac

msg "=== Taskbar System Status Monitor & GNOME Extensions Setup ==="
component_main "$@"
msg "=== Taskbar Setup Complete ==="
msg "Log out and back in before testing GNOME Shell extension changes."
