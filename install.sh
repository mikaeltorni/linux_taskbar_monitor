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
# Usage: sudo bash install.sh

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

run_as_target() { sudo -H -u "$TARGET_USER" env \
  HOME="$TARGET_HOME" USER="$TARGET_USER" LOGNAME="$TARGET_USER" \
  XDG_RUNTIME_DIR="$RUNTIME_DIR" DBUS_SESSION_BUS_ADDRESS="$USER_BUS" \
  DISPLAY="$DISPLAY_VAL" "$@"; }

DESKTOP_DIRS=(/usr/share/applications /var/lib/snapd/desktop/applications "$TARGET_HOME/.local/share/applications")

find_desktop_file_path() {
  local name="$1" dir path="${1}.desktop"
  [[ "$name" == *.desktop ]] && path="$name"
  for dir in "${DESKTOP_DIRS[@]}"; do [ -f "${dir}/${path}" ] && { printf '%s\n' "${dir}/${path}"; return 0; }; done
  return 1
}

find_desktop_entry() {
  local c; for c in "$@"; do find_desktop_file_path "$c" >/dev/null && { [[ "$c" == *.desktop ]] && printf '%s\n' "$c" || printf '%s\n' "${c}.desktop"; return 0; }; done
  return 1
}

join_as_gsettings_array() {
  local out="[" first=1 v; for v in "$@"; do [ -n "$v" ] || continue; [ "$first" -eq 0 ] && out+=", "; out+="'$v'"; first=0; done; out+="]"; printf "%s" "$out"
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

user_gsettings_set_if_key_exists() {
  local schema="$1" key="$2" value="$3"
  if gsettings_key_exists "$schema" "$key"; then run_as_target gsettings set "$schema" "$key" "$value" || true; fi
}

apt_install() { DEBIAN_FRONTEND=noninteractive apt install -y "$@"; }

# ── Source extension library modules ──────────────────────────────────────────
source "$SCRIPT_DIR/lib/extension_installation.sh"
source "$SCRIPT_DIR/lib/window_rules_extension.sh"
source "$SCRIPT_DIR/lib/window_manager.sh"
source "$SCRIPT_DIR/lib/gnome_extensions.sh"
source "$SCRIPT_DIR/lib/extension_features.sh"

# ── Main installer logic ─────────────────────────────────────────────────────
if [ "$(id -u)" -ne 0 ]; then
  msg "Run this installer with sudo: sudo bash install.sh"
  exit 1
fi

msg "=== Taskbar System Status Monitor & GNOME Extensions Setup ==="

# Resource Monitor extension (core taskbar component) — idempotent via install_gnome_ext_zip
configure_resource_monitor_extension

# Window rules extension (workspace/sticky assignment for Wayland) — idempotent
if [[ "$SESSION_TYPE" =~ ^(x11|xorg)$ ]]; then
  msg "X11 session detected; skipping custom window-rules extension."
else
  configure_window_rules_extension
fi

# Auto-move-windows extension (Wayland workspace placement) — idempotent via install_auto_move_windows_extension
configure_auto_move_windows

# Dash-to-Panel configuration and PWA icons — idempotent
configure_dash_and_switchers
configure_pwa_icons

msg "=== Taskbar Setup Complete ==="
msg "Log out and back in before testing GNOME Shell extension changes."
