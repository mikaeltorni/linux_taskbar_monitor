#!/usr/bin/env bash
# gnome_extensions.sh — GNOME Shell extension management helpers
#
# Components:
#   - enable_shell_extension(ext_id): Append ext_id to enabled-extensions list.
#   - install_gnome_ext_zip(url, dest_dir, sha256, ext_id): Download zip, extract, chown.
#   - install_gnome_ext_from_src(src_dir, dest_dir, ext_id): Copy from source dir.
#   - patch_extension_metadata(ext_dir metadata_json shell_ver): Patch metadata.json.
#   - ext_gsettings(ext_dir schema args...): Run gsettings with extension schemadir.
#
# Sourced after lib/helpers.sh and lib/gsettings_helpers.sh.
# Depends on: msg, run_as_target, append_gsettings_list, need_cmd.

# Also sources lib/extension_installation.sh for installation helpers.
source "$SCRIPT_DIR/lib/extension_installation.sh"
# Also sources lib/window_rules_extension.sh for window rules extension config.
source "$SCRIPT_DIR/lib/window_rules_extension.sh"
# Also sources lib/extension_features.sh (Resource Monitor feature helpers).
source "$SCRIPT_DIR/lib/extension_features.sh"

# ── Enable GNOME Shell extension (idempotent) ────────────────────────────────
ext_gsettings() {
  local ext_dir="$1"
  shift
  run_as_target gsettings --schemadir "$ext_dir/schemas" "$@"
}

# RESOURCE_MONITOR_EXT_DIR is published by install_resource_monitor_core so the
# optional patch components (gradient colors, VRAM, per-disk) can locate the
# extracted extension after the mandatory core has installed it.
RESOURCE_MONITOR_EXT_DIR=""

# resource_monitor_refresh_interval_file - Print the target user's persisted
# refresh-interval file. The plain integer stored here is milliseconds.
resource_monitor_refresh_interval_file() {
  printf '%s\n' "$TARGET_HOME/.config/taskbar-system-status-monitor/refresh-interval-ms"
}

# resource_monitor_refresh_interval_ms - Print the configured interval in ms.
# Invalid environment/file values are ignored so installation remains bounded
# to the supported 100..2000 ms range. The clean-install default is 500 ms.
resource_monitor_refresh_interval_ms() {
  local value="${RESOURCE_MONITOR_REFRESH_INTERVAL_MS:-500}" file
  file="$(resource_monitor_refresh_interval_file)"
  if [ -f "$file" ]; then
    value="$(tr -d '[:space:]' < "$file")"
  fi
  if [[ ! "$value" =~ ^[0-9]+$ ]] || (( value < 100 || value > 2000 )); then
    msg "Invalid Resource Monitor update time '$value'; using 500 ms." >&2
    value=500
  fi
  printf '%s\n' "$value"
}

# resource_monitor_refresh_seconds - Convert the configured integer
# milliseconds to the decimal seconds expected by Resource Monitor's schema.
resource_monitor_refresh_seconds() {
  awk -v ms="$(resource_monitor_refresh_interval_ms)" 'BEGIN { printf "%.3f", ms / 1000 }'
}

# persist_resource_monitor_refresh_interval VALUE - Validate and save an
# integer millisecond interval for future standalone and master installer runs.
persist_resource_monitor_refresh_interval() {
  local value="$1" file dir
  if [[ ! "$value" =~ ^[0-9]+$ ]] || (( value < 100 || value > 2000 )); then
    msg "Update time must be a whole number from 100 to 2000 ms." >&2
    return 2
  fi
  file="$(resource_monitor_refresh_interval_file)"
  dir="$(dirname "$file")"
  run_as_target mkdir -p "$dir"
  printf '%s\n' "$value" | run_as_target tee "$file" >/dev/null
  msg "Saved Resource Monitor update time: ${value} ms."
}

# apply_resource_monitor_refresh_interval - Apply the persisted interval to an
# installed Resource Monitor schema. It is safe before installation: the core
# installer will consume the persisted value when it creates the schema.
apply_resource_monitor_refresh_interval() {
  local ext_dir seconds
  ext_dir="$(resource_monitor_ext_dir)"
  seconds="$(resource_monitor_refresh_seconds)"
  if [ -d "$ext_dir/schemas" ]; then
    if ! ext_gsettings "$ext_dir" set org.gnome.shell.extensions.resource-monitor refreshtime "$seconds"; then
      msg "Failed to apply Resource Monitor update time; the installed schema may need reconfiguration." >&2
      return 1
    fi
    msg "Applied Resource Monitor update time: $(resource_monitor_refresh_interval_ms) ms."
  else
    msg "Resource Monitor is not installed yet; saved update time will apply during installation."
  fi
}

# configure_resource_monitor_refresh_interval - Open a typeable field prefilled
# with the current value. Re-prompts until an integer from 100 through 2000 is
# entered, persists it, and applies it live when the schema is installed.
configure_resource_monitor_refresh_interval() {
  local current value
  current="$(resource_monitor_refresh_interval_ms)"
  while true; do
    value=""
    read -r -e -i "$current" -p "Resource Monitor update time in ms (100-2000): " value </dev/tty || return 1
    if persist_resource_monitor_refresh_interval "$value"; then
      apply_resource_monitor_refresh_interval
      return 0
    fi
  done
}

# resource_monitor_refresh_interval_status - Print the menu-friendly current
# interval without changing desktop or repository state.
resource_monitor_refresh_interval_status() {
  printf '%s ms\n' "$(resource_monitor_refresh_interval_ms)"
}

# resource_monitor_ext_dir - Echo the installed Resource Monitor extension dir.
# Falls back to the canonical path derived from the extension id when the core
# has not exported it yet (e.g. a patch component invoked in isolation).
resource_monitor_ext_dir() {
  if [ -n "$RESOURCE_MONITOR_EXT_DIR" ]; then
    printf '%s\n' "$RESOURCE_MONITOR_EXT_DIR"
  else
    printf '%s\n' "$TARGET_HOME/.local/share/gnome-shell/extensions/$RESOURCE_MONITOR_EXTENSION_ID"
  fi
}

# install_resource_monitor_core - Mandatory core: download, verify, extract and
# configure the Resource Monitor extension so the taskbar indicator works on its
# own regardless of which optional components the user selects. The sub-second
# refresh *capability* patch lives here; the refresh *value* is configurable in
# milliseconds through the installer (default 500). The visual/VRAM/per-disk tweaks
# are split into separately selectable components below.
install_resource_monitor_core() {
  msg "Installing Resource Monitor taskbar CPU/RAM/disk/ethernet/GPU indicator (core)"
  need_cmd curl
  need_cmd unzip
  need_cmd python3
  need_cmd gsettings
  need_cmd glib-compile-schemas

  local ext_id="$RESOURCE_MONITOR_EXTENSION_ID"
  local ext_dir="$TARGET_HOME/.local/share/gnome-shell/extensions/$ext_id"
  local tmpdir zip_file gpu_devices shell_version
  tmpdir="$(mktemp -d)"
  zip_file="$tmpdir/resource-monitor.zip"

  curl -fL "$RESOURCE_MONITOR_EXTENSION_URL" -o "$zip_file"
  if [ -n "$RESOURCE_MONITOR_EXTENSION_SHA256" ]; then
    printf "%s  %s\n" "$RESOURCE_MONITOR_EXTENSION_SHA256" "$zip_file" | sha256sum -c -
  fi

  run_as_target rm -rf "$ext_dir"
  run_as_target mkdir -p "$ext_dir"
  unzip -q "$zip_file" -d "$ext_dir"
  if [ "$(id -u)" -eq 0 ]; then
    chown -R "$TARGET_USER:$TARGET_USER" "$ext_dir"
  fi
  rm -rf "$tmpdir"

  # Sub-second refresh capability (schema/type widening + GPU poll floor). The
  # actual interval is applied below from the persisted installer setting.
  run_as_target python3 "$SCRIPT_DIR/scripts/patch_resource_monitor_refresh.py" "$ext_dir"

  shell_version="$(gnome-shell --version 2>/dev/null | awk '{print int($3)}')"
  if [ -n "$shell_version" ]; then
    # Pin the version high (9999) so GNOME never auto-updates the EGO-sourced
    # extension over the local patches on shell reload, which previously
    # reverted the gradient colors back to upstream's threshold coloring.
    patch_extension_metadata "$ext_dir" metadata.json "$shell_version" 9999 || true
  fi

  ext_gsettings "$ext_dir" set org.gnome.shell.extensions.resource-monitor refreshtime "$(resource_monitor_refresh_seconds)"
  ext_gsettings "$ext_dir" set org.gnome.shell.extensions.resource-monitor extensionposition "'right'"
  ext_gsettings "$ext_dir" set org.gnome.shell.extensions.resource-monitor displaymode "'primary'"
  ext_gsettings "$ext_dir" set org.gnome.shell.extensions.resource-monitor iconsstatus true
  ext_gsettings "$ext_dir" set org.gnome.shell.extensions.resource-monitor itemsposition "['cpu', 'ram', 'stats', 'space', 'eth', 'wlan', 'gpu']"
  ext_gsettings "$ext_dir" set org.gnome.shell.extensions.resource-monitor cpustatus true
  ext_gsettings "$ext_dir" set org.gnome.shell.extensions.resource-monitor cpufrequencystatus false
  ext_gsettings "$ext_dir" set org.gnome.shell.extensions.resource-monitor cpuloadaveragestatus false
  ext_gsettings "$ext_dir" set org.gnome.shell.extensions.resource-monitor ramstatus true
  ext_gsettings "$ext_dir" set org.gnome.shell.extensions.resource-monitor ramunit "'numeric'"
  ext_gsettings "$ext_dir" set org.gnome.shell.extensions.resource-monitor rammonitor "'used'"
  ext_gsettings "$ext_dir" set org.gnome.shell.extensions.resource-monitor swapstatus false
  ext_gsettings "$ext_dir" set org.gnome.shell.extensions.resource-monitor diskstatsstatus false
  ext_gsettings "$ext_dir" set org.gnome.shell.extensions.resource-monitor diskspacestatus true
  run_as_target python3 "$SCRIPT_DIR/scripts/configure_resource_monitor.py" \
    --disk-space-gb \
    --schema-dir "$ext_dir/schemas"
  ext_gsettings "$ext_dir" set org.gnome.shell.extensions.resource-monitor netethstatus true
  ext_gsettings "$ext_dir" set org.gnome.shell.extensions.resource-monitor netunit "'bits'"
  ext_gsettings "$ext_dir" set org.gnome.shell.extensions.resource-monitor netunitmeasure "'m'"
  ext_gsettings "$ext_dir" set org.gnome.shell.extensions.resource-monitor netwlanstatus false
  ext_gsettings "$ext_dir" set org.gnome.shell.extensions.resource-monitor gpustatus true
  ext_gsettings "$ext_dir" set org.gnome.shell.extensions.resource-monitor gpumemoryunit "'numeric'"
  ext_gsettings "$ext_dir" set org.gnome.shell.extensions.resource-monitor gpumemoryunitmeasure "'auto'"
  ext_gsettings "$ext_dir" set org.gnome.shell.extensions.resource-monitor gpumemorymonitor "'used'"
  ext_gsettings "$ext_dir" set org.gnome.shell.extensions.resource-monitor gpudisplaydevicename false

  gpu_devices="$(run_as_target python3 "$SCRIPT_DIR/scripts/report_cuda_devices.py")"
  if [ -n "$gpu_devices" ]; then
    ext_gsettings "$ext_dir" set org.gnome.shell.extensions.resource-monitor gpudeviceslist "$gpu_devices"
  else
    msg "No NVIDIA GPU reported by nvidia-smi; Resource Monitor GPU list left empty."
  fi

  enable_shell_extension "$ext_id"
  RESOURCE_MONITOR_EXT_DIR="$ext_dir"
  msg "Resource Monitor core installed with a $(resource_monitor_refresh_interval_ms) ms refresh interval."
}

# ── Optional Resource Monitor tweaks (selectable components) ──────────────────
# Each patcher edits the extension files extracted by install_resource_monitor_core.
# The core re-extracts a clean copy on every run, so deselecting a tweak on a
# later run reverts it. Re-running with the tweak selected is idempotent (the
# patch scripts skip already-applied edits).

# patch_resource_monitor_gradient_colors - Replace upstream threshold coloring
# with a smooth value-proportional gradient on the panel indicators.
patch_resource_monitor_gradient_colors() {
  msg "Applying Resource Monitor gradient colors patch"
  ensure_node || { msg "Node.js unavailable; skipping gradient colors patch (install nodejs and re-run)"; return 1; }
  run_as_target node "$SCRIPT_DIR/scripts/patch_resource_monitor_colors.js" \
    "$(resource_monitor_ext_dir)/extension.js"
  _isc_mark_installed "rm_gradient_colors" || true
}

# patch_resource_monitor_vram - Show GPU VRAM usage in the panel.
patch_resource_monitor_vram() {
  msg "Applying Resource Monitor VRAM display patch"
  ensure_node || { msg "Node.js unavailable; skipping VRAM display patch (install nodejs and re-run)"; return 1; }
  run_as_target node "$SCRIPT_DIR/scripts/patch_resource_monitor_vram.js" \
    "$(resource_monitor_ext_dir)/panel/containers.js"
}

# patch_resource_monitor_per_disk - Show each disk device separately in the panel.
patch_resource_monitor_per_disk() {
  msg "Applying Resource Monitor per-disk display patch"
  ensure_node || { msg "Node.js unavailable; skipping per-disk display patch (install nodejs and re-run)"; return 1; }
  run_as_target node "$SCRIPT_DIR/scripts/patch_resource_monitor_disk.js" \
    "$(resource_monitor_ext_dir)/panel/containers.js"
  _isc_mark_installed "rm_per_disk" || true
}


# Integrate system-status extension with Dash-to-Panel bottom panel
# When both extensions are active, ensure proper positioning and interaction
setup_dash_to_panel_integration() {
  # Ensure dash-to-panel is configured for bottom panel position  
  dconf write /org/gnome/shell/extensions/dash-to-panel/panel-position "'BOTTOM'" || true
  
  # Configure system-status to work with dash-to-panel layout
  gsettings set org.gnome.shell disable-user-extensions false 2>/dev/null || true
  
  msg "System status extension configured for Dash-to-Panel integration"  
}
