#!/usr/bin/env bash
# install.sh - Install and configure the Ubuntu 24.04 taskbar system status monitor.
#
# Components:
#   - msg(): Print timestamped installer messages.
#   - need_cmd(): Validate required commands.
#   - run_as_target(): Execute user-scoped commands as the desktop user.
#   - resource_monitor_gsettings(): Apply Resource Monitor GSettings with its schema dir.
#   - configure_resource_monitor_extension(): Download, patch, configure, and enable the extension.
#
# Usage:
#   sudo bash install.sh

set -euo pipefail

TARGET_USER="${SUDO_USER:-$USER}"
TARGET_HOME="$(getent passwd "$TARGET_USER" | cut -d: -f6)"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RESOURCE_MONITOR_EXTENSION_ID="${RESOURCE_MONITOR_EXTENSION_ID:-Resource_Monitor@Ory0n}"
RESOURCE_MONITOR_EXTENSION_URL="${RESOURCE_MONITOR_EXTENSION_URL:-https://extensions.gnome.org/extension-data/Resource_MonitorOry0n.v27.shell-extension.zip}"
RESOURCE_MONITOR_EXTENSION_SHA256="${RESOURCE_MONITOR_EXTENSION_SHA256:-761f422933ed8e76b0c4653ae7bce5862902920cb7e9f4de2cec23b899d6d170}"

msg() {
  printf '[%(%Y-%m-%dT%H:%M:%S%z)T] %s\n' -1 "$*"
}

need_cmd() {
  local command_name="$1"
  if ! command -v "$command_name" >/dev/null 2>&1; then
    msg "Missing required command: $command_name"
    return 1
  fi
}

run_as_target() {
  sudo -H -u "$TARGET_USER" "$@"
}

resource_monitor_gsettings() {
  local ext_dir="$TARGET_HOME/.local/share/gnome-shell/extensions/$RESOURCE_MONITOR_EXTENSION_ID"
  run_as_target gsettings --schemadir "$ext_dir/schemas" "$@"
}

enable_shell_extension() {
  local ext_id="$1"
  run_as_target python3 - "$ext_id" <<'PY'
import ast
import subprocess
import sys

ext_id = sys.argv[1]
schema = "org.gnome.shell"
key = "enabled-extensions"
current = subprocess.check_output(["gsettings", "get", schema, key], text=True).strip()
try:
    enabled = ast.literal_eval(current)
except (SyntaxError, ValueError):
    enabled = []
if ext_id not in enabled:
    enabled.append(ext_id)
subprocess.check_call(["gsettings", "set", schema, key, repr(enabled)])
PY
}

configure_resource_monitor_extension() {
  msg "Installing Resource Monitor taskbar CPU/RAM/disk/ethernet/GPU indicator"
  need_cmd curl
  need_cmd unzip
  need_cmd node
  need_cmd python3
  need_cmd gsettings

  local ext_id="$RESOURCE_MONITOR_EXTENSION_ID"
  local ext_dir="$TARGET_HOME/.local/share/gnome-shell/extensions/$ext_id"
  local tmpdir zip_file gpu_devices
  tmpdir="$(mktemp -d)"
  zip_file="$tmpdir/resource-monitor.zip"

  curl -fL "$RESOURCE_MONITOR_EXTENSION_URL" -o "$zip_file"
  if [ -n "$RESOURCE_MONITOR_EXTENSION_SHA256" ]; then
    printf "%s  %s\n" "$RESOURCE_MONITOR_EXTENSION_SHA256" "$zip_file" | sha256sum -c -
  fi

  run_as_target rm -rf "$ext_dir"
  run_as_target mkdir -p "$ext_dir"
  unzip -q "$zip_file" -d "$ext_dir"
  chown -R "$TARGET_USER:$TARGET_USER" "$ext_dir"
  rm -rf "$tmpdir"

  # Install GSettings schema so gsettings can find it without --schemadir
  run_as_target mkdir -p "$TARGET_HOME/.local/share/glib-2.0/schemas/"
  run_as_target cp "$ext_dir/schemas/org.gnome.shell.extensions.resource-monitor.gschema.xml" \
    "$TARGET_HOME/.local/share/glib-2.0/schemas/"
  run_as_target glib-compile-schemas "$TARGET_HOME/.local/share/glib-2.0/schemas/"

  run_as_target node "$SCRIPT_DIR/scripts/patch_resource_monitor_vram.js" "$ext_dir/panel/containers.js"
  run_as_target node "$SCRIPT_DIR/scripts/patch_resource_monitor_disk.js" "$ext_dir/panel/containers.js"
  run_as_target node "$SCRIPT_DIR/scripts/patch_resource_monitor_colors.js" "$ext_dir/extension.js"

  resource_monitor_gsettings set org.gnome.shell.extensions.resource-monitor refreshtime 2
  resource_monitor_gsettings set org.gnome.shell.extensions.resource-monitor extensionposition "'right'"
  resource_monitor_gsettings set org.gnome.shell.extensions.resource-monitor displaymode "'primary'"
  resource_monitor_gsettings set org.gnome.shell.extensions.resource-monitor iconsstatus true
  resource_monitor_gsettings set org.gnome.shell.extensions.resource-monitor itemsposition "['cpu', 'ram', 'stats', 'space', 'eth', 'wlan', 'gpu']"
  resource_monitor_gsettings set org.gnome.shell.extensions.resource-monitor cpustatus true
  resource_monitor_gsettings set org.gnome.shell.extensions.resource-monitor cpufrequencystatus false
  resource_monitor_gsettings set org.gnome.shell.extensions.resource-monitor cpuloadaveragestatus false
  resource_monitor_gsettings set org.gnome.shell.extensions.resource-monitor ramstatus true
  resource_monitor_gsettings set org.gnome.shell.extensions.resource-monitor ramunit "'numeric'"
  resource_monitor_gsettings set org.gnome.shell.extensions.resource-monitor rammonitor "'used'"
  resource_monitor_gsettings set org.gnome.shell.extensions.resource-monitor swapstatus false
  resource_monitor_gsettings set org.gnome.shell.extensions.resource-monitor diskstatsstatus false
  resource_monitor_gsettings set org.gnome.shell.extensions.resource-monitor diskspacestatus true
  resource_monitor_gsettings set org.gnome.shell.extensions.resource-monitor diskspaceunit "'perc'"
  resource_monitor_gsettings set org.gnome.shell.extensions.resource-monitor diskspacemonitor "'used'"
  run_as_target python3 "$SCRIPT_DIR/scripts/configure_resource_monitor.py" \
    --disk-space-perc-home-only \
    --schema-dir "$ext_dir/schemas"
  resource_monitor_gsettings set org.gnome.shell.extensions.resource-monitor netethstatus true
  resource_monitor_gsettings set org.gnome.shell.extensions.resource-monitor netwlanstatus false
  resource_monitor_gsettings set org.gnome.shell.extensions.resource-monitor netunitmeasure "'m'"
  resource_monitor_gsettings set org.gnome.shell.extensions.resource-monitor netethdecimals 1
  resource_monitor_gsettings set org.gnome.shell.extensions.resource-monitor gpustatus true
  resource_monitor_gsettings set org.gnome.shell.extensions.resource-monitor gpumemoryunit "'numeric'"
  resource_monitor_gsettings set org.gnome.shell.extensions.resource-monitor gpumemoryunitmeasure "'auto'"
  resource_monitor_gsettings set org.gnome.shell.extensions.resource-monitor gpumemorymonitor "'used'"
  resource_monitor_gsettings set org.gnome.shell.extensions.resource-monitor gpudisplaydevicename false

  gpu_devices="$(run_as_target python3 "$SCRIPT_DIR/scripts/report_cuda_devices.py")"
  if [ -n "$gpu_devices" ]; then
    resource_monitor_gsettings set org.gnome.shell.extensions.resource-monitor gpudeviceslist "$gpu_devices"
  else
    msg "No NVIDIA GPU reported by nvidia-smi; Resource Monitor GPU list left empty."
  fi

  enable_shell_extension "$ext_id"
  msg "Resource Monitor installed. Log out and back in before testing GNOME Shell extension changes."
}

if [ "$(id -u)" -ne 0 ]; then
  msg "Run this installer with sudo: sudo bash install.sh"
  exit 1
fi

configure_resource_monitor_extension
