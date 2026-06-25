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
# refresh *capability* patch lives here; the refresh *value* is configurable via
# RESOURCE_MONITOR_REFRESH_TIME (default 0.5). The visual/VRAM/per-disk tweaks
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
  # actual interval is applied below from RESOURCE_MONITOR_REFRESH_TIME.
  run_as_target python3 "$SCRIPT_DIR/scripts/patch_resource_monitor_refresh.py" "$ext_dir"

  shell_version="$(gnome-shell --version 2>/dev/null | awk '{print int($3)}')"
  if [ -n "$shell_version" ]; then
    # Pin the version high (9999) so GNOME never auto-updates the EGO-sourced
    # extension over the local patches on shell reload, which previously
    # reverted the gradient colors back to upstream's threshold coloring.
    patch_extension_metadata "$ext_dir" metadata.json "$shell_version" 9999 || true
  fi

  ext_gsettings "$ext_dir" set org.gnome.shell.extensions.resource-monitor refreshtime "${RESOURCE_MONITOR_REFRESH_TIME:-0.5}"
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
  msg "Resource Monitor core installed with a ${RESOURCE_MONITOR_REFRESH_TIME:-0.5}-second refresh interval."
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
  need_cmd node
  run_as_target node "$SCRIPT_DIR/scripts/patch_resource_monitor_colors.js" \
    "$(resource_monitor_ext_dir)/extension.js"
}

# patch_resource_monitor_vram - Show GPU VRAM usage in the panel.
patch_resource_monitor_vram() {
  msg "Applying Resource Monitor VRAM display patch"
  need_cmd node
  run_as_target node "$SCRIPT_DIR/scripts/patch_resource_monitor_vram.js" \
    "$(resource_monitor_ext_dir)/panel/containers.js"
}

# patch_resource_monitor_per_disk - Show each disk device separately in the panel.
patch_resource_monitor_per_disk() {
  msg "Applying Resource Monitor per-disk display patch"
  need_cmd node
  run_as_target node "$SCRIPT_DIR/scripts/patch_resource_monitor_disk.js" \
    "$(resource_monitor_ext_dir)/panel/containers.js"
}
