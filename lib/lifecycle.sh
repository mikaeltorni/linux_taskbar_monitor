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

RM_SCHEMA="org.gnome.shell.extensions.resource-monitor"
rm_ext_dir() { printf '%s\n' "$TARGET_HOME/.local/share/gnome-shell/extensions/$1"; }

# Resolve the installed Resource Monitor extension's source files through the
# same helper the patch components use, so detection matches the live core state
# (RESOURCE_MONITOR_EXT_DIR when the core has run, the canonical path otherwise).
_rm_ext_root() { resource_monitor_ext_dir; }
_rm_extension_js() { printf '%s/extension.js\n' "$(_rm_ext_root)"; }
_rm_containers_js() { printf '%s/panel/containers.js\n' "$(_rm_ext_root)"; }
_rm_refreshers_js() { printf '%s/services/refreshers.js\n' "$(_rm_ext_root)"; }

# --- Detection ---------------------------------------------------------------
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
detect_window_rules() { [ -d "$(rm_ext_dir app-rules@local)" ]; }

# --- Uninstall ---------------------------------------------------------------
uninstall_rm_vram()     { msg "Disabling Resource Monitor VRAM display"; run_as_target gsettings reset "$RM_SCHEMA" gpumemorymonitor 2>/dev/null || true; }
uninstall_window_rules() {
  msg "Removing app window-rules extension"
  run_as_target gnome-extensions disable app-rules@local 2>/dev/null || true
  run_as_target rm -rf "$(rm_ext_dir app-rules@local)"
}
