#!/usr/bin/env bash
# lifecycle.sh - Detection and uninstall functions for the taskbar system
# monitor repo. detect_* report live state for --detect; uninstall_* reset the
# Resource Monitor feature toggles and remove bundled extensions. The mandatory
# Resource Monitor core install is not a component and is never removed here.
#
# The patch components (rm_gradient_colors, rm_per_disk, rm_vram) edit the
# extension's JavaScript source. Their detect functions inspect the live
# extension files for the patch markers the patchers inject, so detection
# reflects the actual on-disk state rather than a stale install receipt. This
# matters because install_resource_monitor_core re-extracts a clean unpatched
# copy on every run: receipt-based detection would report "already installed"
# and skip re-applying the patches, leaving the extension unpatched.

# Source-patch components have no GSettings uninstall (core re-extract reverts them).
rm_ext_dir() { printf '%s\n' "$TARGET_HOME/.local/share/gnome-shell/extensions/$1"; }

# Resolve the installed Resource Monitor extension's source files through the
# same helper the patch components use, so detection matches the live core state
# (RESOURCE_MONITOR_EXT_DIR when the core has run, the canonical path otherwise).
_rm_extension_js() { printf '%s/extension.js\n' "$(resource_monitor_ext_dir)"; }
_rm_containers_js() { printf '%s/panel/containers.js\n' "$(resource_monitor_ext_dir)"; }
_rm_refreshers_js() { printf '%s/services/refreshers.js\n' "$(resource_monitor_ext_dir)"; }
_rm_main_gui_js() { printf '%s/panel/mainGui.js\n' "$(resource_monitor_ext_dir)"; }

# --- Detection ---------------------------------------------------------------
# detect_rm_refresh_interval: live check that the installed schema refreshtime
# matches the configured millisecond interval (converted to seconds). Compare
# numerically so gsettings' `0.5` still matches the helper's `0.500`.
detect_rm_refresh_interval() {
  local ext_dir expected actual
  ext_dir="$(resource_monitor_ext_dir)"
  [ -d "$ext_dir/schemas" ] || return 1
  expected="$(resource_monitor_refresh_seconds)"
  actual="$(ext_gsettings "$ext_dir" get org.gnome.shell.extensions.resource-monitor refreshtime 2>/dev/null | tr -d "[:space:]'")" || return 1
  [ -n "$actual" ] || return 1
  awk -v a="$actual" -v e="$expected" 'BEGIN { exit !(a + 0 == e + 0) }'
}

# detect_rm_gradient_colors: the colors patcher injects the _gradientGetUsageColor
# override into extension.js. Its presence is the deterministic live signal.
detect_rm_gradient_colors() {
  local js; js="$(_rm_extension_js)"
  [ -f "$js" ] && grep -q "_gradientGetUsageColor" "$js"
}

# detect_rm_per_disk: the per-disk patcher injects the getDiskUsagePercentStyle
# helper into services/refreshers.js. Its presence is the deterministic live signal.
detect_rm_per_disk() {
  local js; js="$(_rm_refreshers_js)"
  [ -f "$js" ] && grep -q "getDiskUsagePercentStyle" "$js"
}

# detect_rm_vram: the VRAM patcher removes the GPU memory bracket labels and
# leaves a "Space separator between GPU usage and VRAM" comment in containers.js.
# The gsettings toggle is set by the mandatory core regardless of the source
# patch, so it is not a reliable signal on its own.
detect_rm_vram() {
  local js; js="$(_rm_containers_js)"
  [ -f "$js" ] && grep -q "Space separator between GPU usage and VRAM" "$js"
}

# detect_rm_panel_spacing: reflects the configured panel-spacing mode.
# Stable mode is present when either stable-width reservation is in
# containers.js: the disk-space secondary activity marker (when rm_per_disk
# applied secondary labels) and/or the GPU VRAM width-split marker (always
# attempted by patch-stable-width). Compact mode requires both markers gone.
detect_rm_panel_spacing() {
  local js disk_marker gpu_marker mode
  js="$(_rm_containers_js)"
  disk_marker="Space separator between disk-space activity percent and its unit (stable width)"
  gpu_marker="VRAM value (0-99 GB, 2 digits) gets its own tighter reserved"
  mode="$(resource_monitor_spacing_mode 2>/dev/null || echo stable)"
  [ -f "$js" ] || return 1
  case "$mode" in
    compact)
      ! grep -q "$disk_marker" "$js" && ! grep -q "$gpu_marker" "$js"
      ;;
    *)
      grep -q "$disk_marker" "$js" || grep -q "$gpu_marker" "$js"
      ;;
  esac
}
# detect_rm_hide_eth_icon: the eth-icon patcher wires the ethernet group to
# _appendSimpleChildren with a null icon ("Ethernet icon removed: value/unit
# kept, icon omitted"). Its presence in mainGui.js is the deterministic signal.
detect_rm_hide_eth_icon() {
  local js; js="$(_rm_main_gui_js)"
  [ -f "$js" ] && grep -q "Ethernet icon removed: value/unit kept, icon omitted" "$js"
}
# detect_rm_process_popup: the process-popup patcher injects marker-guarded
# _toggleProcessMenu/_refreshProcessMenu methods ("Process popup: total CPU/RAM
# aggregated per process name"). Its presence in extension.js is the
# deterministic live signal.
detect_rm_process_popup() {
  local js; js="$(_rm_extension_js)"
  [ -f "$js" ] && grep -q "Process popup: total CPU/RAM aggregated per process name" "$js"
}
detect_window_rules() {
  # Installed extension tree, or an explicit X11 skip marker so --detect does
  # not keep reporting absent after a successful X11 no-op configure.
  [ -d "$(rm_ext_dir app-rules@local)" ] && return 0
  [ -f "$TARGET_HOME/.config/taskbar-system-status-monitor/window-rules-skipped-x11" ] && return 0
  return 1
}

# --- Uninstall ---------------------------------------------------------------
# uninstall_rm_panel_spacing - Revert the panel to the default stable spacing
# (reserved widths, no taskbar shift) and clear the persisted compact choice.
uninstall_rm_panel_spacing() {
  msg "Reverting Resource Monitor panel spacing to stable (default)"
  if persist_resource_monitor_spacing_mode stable; then
    apply_resource_monitor_spacing_mode
  fi
}

# Source patches (rm_vram, rm_gradient_colors, rm_per_disk, …) have no uninstall
# handlers: install_resource_monitor_core re-extracts a clean zip every run, so
# deselecting a patch and re-installing reverts it. Do not reset unrelated
# GSettings keys as a fake uninstall.

uninstall_window_rules() {
  msg "Removing app window-rules extension"
  run_as_target gnome-extensions disable app-rules@local 2>/dev/null || true
  # Drop the UUID from enabled-extensions even when gnome-extensions disable
  # fails, so a deleted tree is not left referenced as enabled.
  if declare -F remove_gsettings_list >/dev/null 2>&1; then
    remove_gsettings_list org.gnome.shell enabled-extensions "app-rules@local" \
      || msg "WARN: could not remove app-rules@local from enabled-extensions"
  fi
  run_as_target rm -rf "$(rm_ext_dir app-rules@local)"
  run_as_target rm -f "$TARGET_HOME/.config/taskbar-system-status-monitor/window-rules-skipped-x11" \
    2>/dev/null || true
}
