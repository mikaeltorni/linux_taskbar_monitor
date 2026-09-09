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
# detect_rm_refresh_interval: live check that the component persist file exists
# and the installed schema refreshtime matches the configured millisecond
# interval (converted to seconds). Core no longer writes refreshtime, so a
# deselected component cannot look installed via schema alone. Compare
# numerically so gsettings' `0.5` still matches the helper's `0.500`.
detect_rm_refresh_interval() {
  local ext_dir expected actual
  [ -f "$(resource_monitor_refresh_interval_file)" ] || return 1
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
# Stable mode is present when either stable-width reservation marker is in
# containers.js (disk-space secondary and/or GPU VRAM). Compact mode requires
# the positive compact marker left by patch-stable-width --mode compact, not
# mere absence of stable markers (clean upstream also lacks those).
detect_rm_panel_spacing() {
  local js disk_marker gpu_marker compact_marker mode
  js="$(_rm_containers_js)"
  disk_marker="Space separator between disk-space activity percent and its unit (stable width)"
  gpu_marker="VRAM value (0-99 GB, 2 digits) gets its own tighter reserved"
  compact_marker="Resource Monitor panel spacing: compact (no reserved widths)"
  mode="$(resource_monitor_spacing_mode)" || return 1
  [ -f "$js" ] || return 1
  case "$mode" in
    compact)
      grep -q "$compact_marker" "$js" \
        && ! grep -q "$disk_marker" "$js" \
        && ! grep -q "$gpu_marker" "$js"
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
# detect_rm_process_popup: the popup patcher injects a marker-guarded block
# ("Process popup: top resource users per metric over a rolling window") whose
# sampler carries the configured window as a baked-in constant. Both must match
# the configuration, so a reconfigured window shows as "not installed" until
# the patcher has rewritten extension.js.
detect_rm_process_popup() {
  local js window
  js="$(_rm_extension_js)"
  [ -f "$js" ] || return 1
  grep -q "Process popup: top resource users per metric over a rolling window" "$js" || return 1
  [ -f "$(resource_monitor_top_window_file)" ] || return 1
  window="$(resource_monitor_top_window_minutes)" || return 1
  grep -q "const patchedMinutes = ${window};" "$js"
}
detect_window_rules() {
  # Require a real extension tree (metadata.json), not an empty leftover dir.
  # The X11 skip marker only satisfies detect while still on X11/XOrg —
  # otherwise a leftover marker would hide a missing install after switching
  # to Wayland.
  local ext
  ext="$(rm_ext_dir app-rules@local)"
  if [ -f "$ext/metadata.json" ]; then
    return 0
  fi
  case "${SESSION_TYPE:-}" in
    x11|xorg)
      if declare -F window_rules_skip_marker >/dev/null 2>&1; then
        [ -f "$(window_rules_skip_marker)" ] && return 0
      else
        [ -f "$TARGET_HOME/.config/taskbar-system-status-monitor/window-rules-skipped-x11" ] && return 0
      fi
      ;;
  esac
  return 1
}

# --- Uninstall ---------------------------------------------------------------
# uninstall_rm_panel_spacing - Source patch (stable-width markers). Applying
# stable leaves detect green for the default mode, so fail hard like other
# source patches: re-run install without the component so core re-extract
# reverts containers.js.
uninstall_rm_panel_spacing() { uninstall_rm_source_patch rm_panel_spacing; }

# Source patches (rm_vram, rm_gradient_colors, rm_per_disk, …) cannot be
# uninstalled in place: markers stay until core re-extract. Explicit uninstall
# helpers fail hard so --uninstall does not clear receipts while --detect still
# reports installed.
uninstall_rm_source_patch() {
  local id="$1"
  msg "ERROR: $id is a source patch; re-run install without it so core re-extract reverts the change." >&2
  return 1
}
uninstall_rm_gradient_colors() { uninstall_rm_source_patch rm_gradient_colors; }
uninstall_rm_vram() { uninstall_rm_source_patch rm_vram; }
uninstall_rm_per_disk() { uninstall_rm_source_patch rm_per_disk; }
uninstall_rm_hide_eth_icon() { uninstall_rm_source_patch rm_hide_eth_icon; }
uninstall_rm_process_popup() { uninstall_rm_source_patch rm_process_popup; }

# uninstall_rm_refresh_interval — Revert live refreshtime away from the
# configured value and drop the persist file so detect_rm_refresh_interval
# becomes false (receipt-only uninstall would leave detect green).
uninstall_rm_refresh_interval() {
  local ext_dir
  ext_dir="$(resource_monitor_ext_dir)"
  if [ ! -d "$ext_dir/schemas" ]; then
    msg "ERROR: Resource Monitor is not installed; cannot uninstall refresh interval." >&2
    return 1
  fi
  msg "Reverting Resource Monitor update time to upstream-like 2.0 s"
  # Upstream default was 2 seconds; configured installer default is 0.5 s.
  if ! ext_gsettings "$ext_dir" set org.gnome.shell.extensions.resource-monitor refreshtime 2.0; then
    msg "ERROR: failed to reset refreshtime" >&2
    return 1
  fi
  run_as_target rm -f "$(resource_monitor_refresh_interval_file)"
  return 0
}

uninstall_window_rules() {
  local marker failures=0
  msg "Removing app window-rules extension"
  run_as_target gnome-extensions disable app-rules@local 2>/dev/null || true
  # Drop the UUID from enabled-extensions even when gnome-extensions disable
  # fails, so a deleted tree is not left referenced as enabled.
  if declare -F remove_gsettings_list >/dev/null 2>&1; then
    if ! remove_gsettings_list org.gnome.shell enabled-extensions "app-rules@local"; then
      msg "ERROR: could not remove app-rules@local from enabled-extensions" >&2
      failures=1
    fi
  fi
  run_as_target rm -rf "$(rm_ext_dir app-rules@local)"
  if declare -F window_rules_skip_marker >/dev/null 2>&1; then
    marker="$(window_rules_skip_marker)"
  else
    marker="$TARGET_HOME/.config/taskbar-system-status-monitor/window-rules-skipped-x11"
  fi
  run_as_target rm -f "$marker"
  if [ -f "$marker" ]; then
    msg "ERROR: could not remove window-rules skip marker at $marker" >&2
    failures=1
  fi
  if [ -f "$(rm_ext_dir app-rules@local)/metadata.json" ]; then
    msg "ERROR: app-rules@local tree still present after uninstall" >&2
    failures=1
  fi
  return "$failures"
}
