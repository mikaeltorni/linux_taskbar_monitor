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
# Also sources lib/extension_features.sh for PWA icons, monitor hotkeys, workspace popup, and dash config.
source "$SCRIPT_DIR/lib/extension_features.sh"

# ── Enable GNOME Shell extension (idempotent) ────────────────────────────────
ext_gsettings() {
  local ext_dir="$1"
  shift
  run_as_target gsettings --schemadir "$ext_dir/schemas" "$@"
}

configure_resource_monitor_extension() {
  msg "Installing Resource Monitor taskbar CPU/RAM/disk/ethernet/GPU indicator"
  need_cmd curl
  need_cmd unzip
  need_cmd node
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

  run_as_target node "$SCRIPT_DIR/scripts/patch_resource_monitor_vram.js" "$ext_dir/panel/containers.js"
  run_as_target node "$SCRIPT_DIR/scripts/patch_resource_monitor_disk.js" "$ext_dir/panel/containers.js"
  run_as_target node "$SCRIPT_DIR/scripts/patch_resource_monitor_colors.js" "$ext_dir/extension.js"
  run_as_target python3 "$SCRIPT_DIR/scripts/patch_resource_monitor_refresh.py" "$ext_dir"

  shell_version="$(gnome-shell --version 2>/dev/null | awk '{print int($3)}')"
  if [ -n "$shell_version" ]; then
    # Pin the version high (9999) so GNOME never auto-updates the EGO-sourced
    # extension over the local patches on shell reload, which previously
    # reverted the gradient colors back to upstream's threshold coloring.
    patch_extension_metadata "$ext_dir" metadata.json "$shell_version" 9999 || true
  fi

  ext_gsettings "$ext_dir" set org.gnome.shell.extensions.resource-monitor refreshtime 0.5
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
  msg "Resource Monitor installed with a 0.5-second refresh interval."
}
