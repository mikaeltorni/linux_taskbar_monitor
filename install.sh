#!/usr/bin/env bash
# install.sh - Install and configure the Ubuntu 24.04 taskbar system status monitor
#              and related GNOME Shell extensions.
#
# Mandatory core (always installed):
#   - Resource Monitor extension (CPU/RAM/disk/GPU indicator)
#
# Optional components (selectable; all default-on):
#   - Resource Monitor gradient indicator colors
#   - Resource Monitor GPU VRAM display
#   - Resource Monitor per-disk display
#   - Window Rules extension (app-rules@local — workspace/sticky rules)
#   - Dash-to-Panel configuration and Ubuntu Dock disabling
#
# Auto-move-windows placement (linux_workspaces_setup) and Chrome PWA icons
# (linux_configuration_setup) used to be selectable here too; they were removed
# to avoid duplicating components owned by those repositories.
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
# Panel refresh interval in milliseconds. The repository-owned configurator
# persists the selected value; this environment variable supports scripted
# clean installs and defaults to 500 ms.
RESOURCE_MONITOR_REFRESH_INTERVAL_MS="${RESOURCE_MONITOR_REFRESH_INTERVAL_MS:-500}"

# Dash-to-Panel now lives in linux_configuration_setup (lib/dash_to_panel.sh),
# which owns desktop layout/panel behavior; its EGO download settings moved with
# it, so this system-monitor repo no longer defines them.


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
  newlist="$(CURRENT="$current" python3 "$SCRIPT_DIR/scripts/gsettings_strv.py" append "$value")"
  run_as_target gsettings set "$schema" "$key" "$newlist"
}

remove_gsettings_list() {
  local schema="$1" key="$2" value="$3" current newlist
  current="$(run_as_target gsettings get "$schema" "$key" 2>/dev/null || echo "[]")"
  newlist="$(CURRENT="$current" python3 "$SCRIPT_DIR/scripts/gsettings_strv.py" remove "$value")"
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

# ensure_node: Guarantee the `node` interpreter used by the Resource Monitor JS
# patch scripts is available. The gradient/VRAM/per-disk patches transform the
# extension's JavaScript with Node and have no GJS-runtime equivalent, so a
# clean machine needs Node.js installed before they can run. Installs the Ubuntu
# `nodejs` package (which ships /usr/bin/node) when missing and root is present;
# under a non-root run it records the skipped apt step and reports failure so the
# dependent component is not falsely marked installed.
#
# Returns:
#   0 when `node` is available, 1 when it could not be provided.
ensure_node() {
  need_cmd node && return 0
  apt_install nodejs
  need_cmd node
}

# ── Source extension library modules ──────────────────────────────────────────
source "$SCRIPT_DIR/lib/extension_installation.sh"
source "$SCRIPT_DIR/lib/window_rules_extension.sh"
source "$SCRIPT_DIR/lib/gnome_extensions.sh"
source "$SCRIPT_DIR/lib/extension_features.sh"
source "$SCRIPT_DIR/lib/lifecycle.sh"

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
  --list-components|--list-configurable-components|--list-component-config-values|--configure-component|--configure-component=*|--export-selection|--detect|--help|-h|--uninstall|--uninstall=*) component_main "$@"; exit $? ;;
esac

msg "=== Taskbar System Status Monitor & GNOME Extensions Setup ==="
# Mandatory core: the Resource Monitor indicator always installs so the program
# works regardless of which optional components the user selects below.
install_resource_monitor_core
component_main "$@"
msg "=== Taskbar Setup Complete ==="
msg "Log out and back in before testing GNOME Shell extension changes."
