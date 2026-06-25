#!/usr/bin/env bash
# lifecycle.sh - Detection and uninstall functions for the taskbar system
# monitor repo. detect_* report live state for --detect; uninstall_* reset the
# Resource Monitor feature toggles, remove bundled extensions, reset the
# auto-move list, or remove the generated PWA desktop entries. The mandatory
# Resource Monitor core install is not a component and is never removed here.
# rm_gradient_colors is a pure source-cosmetic patch with no deterministic live
# signal, so it ships no detect/uninstall and uses the runtime's receipt
# fallback.

RM_SCHEMA="org.gnome.shell.extensions.resource-monitor"
rm_ext_dir() { printf '%s\n' "$TARGET_HOME/.local/share/gnome-shell/extensions/$1"; }

# --- Detection ---------------------------------------------------------------
detect_rm_vram()     { [ "$(run_as_target gsettings get "$RM_SCHEMA" gpumemorymonitor 2>/dev/null)" = "true" ]; }
detect_rm_per_disk() { [ "$(run_as_target gsettings get "$RM_SCHEMA" diskstatsstatus 2>/dev/null)" = "true" ]; }
detect_window_rules() { [ -d "$(rm_ext_dir app-rules@local)" ]; }

# --- Uninstall ---------------------------------------------------------------
uninstall_rm_vram()     { msg "Disabling Resource Monitor VRAM display"; run_as_target gsettings reset "$RM_SCHEMA" gpumemorymonitor 2>/dev/null || true; }
uninstall_rm_per_disk() { msg "Disabling Resource Monitor per-disk display"; run_as_target gsettings reset "$RM_SCHEMA" diskstatsstatus 2>/dev/null || true; }
uninstall_window_rules() {
  msg "Removing app window-rules extension"
  run_as_target gnome-extensions disable app-rules@local 2>/dev/null || true
  run_as_target rm -rf "$(rm_ext_dir app-rules@local)"
}
