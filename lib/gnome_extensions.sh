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
  msg "Installing Resource Monitor from ubuntu_2404_taskbar_system_status_monitor"
  if [ ! -x "$TASKBAR_SYSTEM_STATUS_MONITOR_DIR/install.sh" ]; then
    log "warn" "Taskbar system status monitor installer not found at $TASKBAR_SYSTEM_STATUS_MONITOR_DIR/install.sh"
    return 1
  fi

  RESOURCE_MONITOR_EXTENSION_ID="$RESOURCE_MONITOR_EXTENSION_ID" \
  RESOURCE_MONITOR_EXTENSION_URL="$RESOURCE_MONITOR_EXTENSION_URL" \
  RESOURCE_MONITOR_EXTENSION_SHA256="$RESOURCE_MONITOR_EXTENSION_SHA256" \
    bash "$TASKBAR_SYSTEM_STATUS_MONITOR_DIR/install.sh"
}
