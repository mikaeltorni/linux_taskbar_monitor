#!/usr/bin/env bash
# install.sh - Install and configure Linux Taskbar Monitor (Resource Monitor
#              patches) and related GNOME Shell extensions.
#
# Tested on Ubuntu 24.04 LTS / GNOME Shell 46. Other distros or Shell majors
# are untested — see README.md "Supported platforms".
#
# Mandatory core (always installed):
#   - Resource Monitor extension (CPU/RAM/disk/GPU indicator)
#
# Optional components (selectable; most default-on — see installer/components.sh):
#   - Resource Monitor refresh interval, gradient colors, VRAM, per-disk,
#     panel spacing, ethernet-icon hide, per-process CPU popup
#   - Window Rules extension (app-rules@local — default-off; Wayland only)
#
# Desktop-wide features that used to live here were moved to owning repos:
#   auto-move-windows → linux_workspaces_setup; Dash-to-Panel / PWA icons →
#   linux_configuration_setup.
#
# Idempotent: re-runs re-extract and re-patch cleanly; apt steps skipped without root.
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
# Prefer the installer process env; under sudo that is often empty, so fall back
# to the target user's active session type (loginctl) before defaulting unknown.
SESSION_TYPE="${XDG_SESSION_TYPE:-}"
if [ -z "$SESSION_TYPE" ]; then
  SESSION_TYPE="$(
    loginctl show-user "$TARGET_USER" -p Sessions --value 2>/dev/null \
      | tr ' ' '\n' | while read -r sid; do
          [ -n "$sid" ] || continue
          typ="$(loginctl show-session "$sid" -p Type --value 2>/dev/null || true)"
          case "$typ" in
            x11|wayland) printf '%s\n' "$typ"; break ;;
          esac
        done
  )" || true
fi
SESSION_TYPE="${SESSION_TYPE:-unknown}"

# ── Resource Monitor extension settings ───────────────────────────────────────
RESOURCE_MONITOR_EXTENSION_ID="${RESOURCE_MONITOR_EXTENSION_ID:-Resource_Monitor@Ory0n}"
RESOURCE_MONITOR_EXTENSION_URL="${RESOURCE_MONITOR_EXTENSION_URL:-https://extensions.gnome.org/extension-data/Resource_MonitorOry0n.v27.shell-extension.zip}"
RESOURCE_MONITOR_EXTENSION_SHA256="${RESOURCE_MONITOR_EXTENSION_SHA256:-761f422933ed8e76b0c4653ae7bce5862902920cb7e9f4de2cec23b899d6d170}"
# Panel refresh interval in milliseconds. The repository-owned configurator
# persists the selected value; this environment variable supports scripted
# clean installs and defaults to 500 ms.
RESOURCE_MONITOR_REFRESH_INTERVAL_MS="${RESOURCE_MONITOR_REFRESH_INTERVAL_MS:-500}"
# Panel spacing mode: "stable" reserves a tight per-value width so the taskbar
# stays put as metric values change digit count; "compact" drops the reserved
# widths so the indicator is narrower but shifts slightly as digits change. The
# repository persists the selection; this environment variable seeds a clean
# install (defaults to "stable").
RESOURCE_MONITOR_SPACING_MODE="${RESOURCE_MONITOR_SPACING_MODE:-stable}"

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

# append_gsettings_list SCHEMA KEY VALUE — Append VALUE to a GSettings `as` list.
# Reads the live list first. Never invents an empty list on read failure: a
# missing session bus / failed `gsettings get` must abort so we cannot wipe
# org.gnome.shell enabled-extensions down to only the UUID being enabled.
append_gsettings_list() {
  local schema="$1" key="$2" value="$3" current newlist
  if ! current="$(run_as_target gsettings get "$schema" "$key")"; then
    msg "ERROR: gsettings get $schema $key failed (session bus unavailable?); refusing to rewrite list"
    return 1
  fi
  if ! newlist="$(CURRENT="$current" rm_monitor gsettings-strv append "$value")"; then
    msg "ERROR: gsettings-strv append failed for $schema $key (CURRENT was not a parseable list)"
    return 1
  fi
  run_as_target gsettings set "$schema" "$key" "$newlist"
}

# remove_gsettings_list SCHEMA KEY VALUE — Remove VALUE from a GSettings `as` list.
# Same fail-hard read policy as append_gsettings_list.
remove_gsettings_list() {
  local schema="$1" key="$2" value="$3" current newlist
  if ! current="$(run_as_target gsettings get "$schema" "$key")"; then
    msg "ERROR: gsettings get $schema $key failed (session bus unavailable?); refusing to rewrite list"
    return 1
  fi
  if ! newlist="$(CURRENT="$current" rm_monitor gsettings-strv remove "$value")"; then
    msg "ERROR: gsettings-strv remove failed for $schema $key (CURRENT was not a parseable list)"
    return 1
  fi
  run_as_target gsettings set "$schema" "$key" "$newlist"
}

apt_install() {
  local missing=() pkg
  for pkg in "$@"; do dpkg -s "$pkg" >/dev/null 2>&1 || missing+=("$pkg"); done
  [ "${#missing[@]}" -eq 0 ] && return 0
  if ! is_root; then note_sudo_required "apt install ${missing[*]}"; return 0; fi
  DEBIAN_FRONTEND=noninteractive apt install -y "${missing[@]}"
}

# ensure_runtime_deps — Apt-install tools required by the Resource Monitor core
# path when running as root. Non-root runs note the missing packages instead.
ensure_runtime_deps() {
  apt_install curl unzip libglib2.0-bin
}

# ensure_rm_monitor_tools: Build or locate the rm-monitor Rust CLI used for every
# Resource Monitor patch/config helper. Prefers an existing dist/ binary, then
# local cargo, then a Docker/Podman rust image (see scripts/build_rm_monitor.sh).
# When root is available and cargo is missing, apt-install cargo as a fallback
# so a clean Ubuntu install can compile without containers.
#
# Returns:
#   0 when dist/rm-monitor is ready, 1 when it could not be produced.
ensure_rm_monitor_tools() {
  ensure_runtime_deps
  if ensure_rm_monitor_bin; then
    return 0
  fi
  if ! need_cmd cargo; then
    apt_install cargo
  fi
  ensure_rm_monitor_bin
}

# ── Source extension library modules ──────────────────────────────────────────
source "$SCRIPT_DIR/lib/rm_monitor_bin.sh"
source "$SCRIPT_DIR/lib/extension_installation.sh"
source "$SCRIPT_DIR/lib/window_rules_extension.sh"
source "$SCRIPT_DIR/lib/gnome_extensions.sh"
source "$SCRIPT_DIR/lib/lifecycle.sh"

# ── Component selection runtime and manifest ─────────────────────────────────
# The manifest maps each component id to a configure_*/install_* function from
# the lib files sourced above.
# Prefer the shared linux_installation_scripts_functions framework (sibling
# checkout or on-demand download). When that framework is unreachable — private
# GitHub raw URLs, offline host, missing sibling — load the built-in fallback so
# this repository remains a working standalone installer for core +
# list/detect/default/select/uninstall/reconfigure.
ISC_FRAMEWORK_ACTIVE=0
if [ -n "${ISC_FUNCTIONS_DIR:-}" ]; then
  # Explicit override is exclusive so callers can force the built-in fallback
  # (e.g. ISC_FUNCTIONS_DIR=/nonexistent) without the sibling search winning.
  __isc_search_paths=("$ISC_FUNCTIONS_DIR")
else
  __isc_search_paths=(
    "$SCRIPT_DIR/../linux_installation_scripts_functions"
    "$HOME/projects/linux_installation_scripts_functions"
  )
fi
for __isc_d in "${__isc_search_paths[@]}"; do
  if [ -n "$__isc_d" ] && [ -f "$__isc_d/component_loader.sh" ]; then
    # shellcheck source=/dev/null
    source "$__isc_d/component_loader.sh"
    if declare -F isc_activate_components >/dev/null 2>&1 \
       && isc_activate_components; then
      ISC_FRAMEWORK_ACTIVE=1
      break
    fi
  fi
done
unset __isc_search_paths
if [ "$ISC_FRAMEWORK_ACTIVE" -eq 0 ]; then
  __isc_ref="${ISC_FUNCTIONS_REF:-master}"
  __isc_url="https://raw.githubusercontent.com/mikaeltorni/linux_installation_scripts_functions/${__isc_ref}/component_loader.sh"
  if need_cmd curl && __isc_body="$(curl -fsSL "$__isc_url" 2>/dev/null)" \
     && [ -n "$__isc_body" ]; then
    # shellcheck source=/dev/null
    source <(printf '%s\n' "$__isc_body")
    if declare -F isc_activate_components >/dev/null 2>&1 \
       && isc_activate_components; then
      ISC_FRAMEWORK_ACTIVE=1
    fi
  fi
  unset __isc_ref __isc_url __isc_body
fi
if [ "$ISC_FRAMEWORK_ACTIVE" -eq 0 ]; then
  # Warnings on stderr only — stdout is reserved for --list-components / --detect.
  msg "WARN: installer component framework unavailable; using built-in standalone fallback." >&2
  msg "      Clone linux_installation_scripts_functions as a sibling (or set ISC_FUNCTIONS_DIR) for the full menu." >&2
  # shellcheck source=/dev/null
  source "$SCRIPT_DIR/lib/standalone_component_fallback.sh"
fi
source "$SCRIPT_DIR/installer/components.sh"

# ── Main installer logic ─────────────────────────────────────────────────────
# Readonly / uninstall / reconfigure must not wipe the installed extension.
# Listing/help print only their own output (the master installer parses
# --list-components). --reconfigure re-runs selected install functions against
# the already-extracted tree; core re-extract is reserved for fresh install
# modes (--default / --all / --select / interactive) so deselected patches
# revert cleanly on a full install.
case "${1:-}" in
  --list-components|--list-configurable-components|--list-select-configure-components|--list-component-config-values|--configure-component|--configure-component=*|--export-selection|--detect|--help|-h|--uninstall|--uninstall=*|--reconfigure|--reconfigure=*)
    component_main "$@"
    exit $?
    ;;
  --default|--all|--select|--select=*|"")
    ;;
  -*)
    # Unknown dash-args must not fall through into a fresh core re-extract.
    # The shared framework may "ignore" unknown flags after selection; by then
    # the mandatory core install has already wiped the live extension tree.
    msg "ERROR: unknown argument: $1"
    msg "       Use --help for supported flags (e.g. --list-configurable-components)."
    exit 1
    ;;
esac

msg "=== Linux Taskbar Monitor & GNOME Extensions Setup ==="
# Build the Rust helper CLI before any patch/config step. Listing/detect modes
# above already returned, so this never pollutes --list-components output.
ensure_rm_monitor_tools || {
  msg "ERROR: could not build rm-monitor (install cargo or docker/podman, then re-run)."
  exit 1
}
# Mandatory core: the Resource Monitor indicator always installs so the program
# works regardless of which optional components the user selects below.
install_resource_monitor_core
component_main "$@"
msg "=== Linux Taskbar Monitor Setup Complete ==="
msg "On X11, reload the Shell with Alt+F2, type r, Enter (or log out/in). On Wayland, log out and back in."
