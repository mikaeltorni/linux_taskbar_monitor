#!/usr/bin/env bash
# components.sh - Component manifest for ubuntu_2404_taskbar_system_status_monitor.
#
# Maps each taskbar/extension component to the function (defined in lib/) that
# installs it. install.sh defines its inline helpers and sources the lib files
# first, then this manifest, then routes execution through component_main from
# component_runtime.sh.
#
# Entry format: "id|label|default(on/off)|function".

ISC_REPO_NAME="ubuntu_2404_taskbar_system_status_monitor"
ISC_REPO_LABEL="Taskbar system status monitor"

ISC_POSTFLIGHT="report_sudo_required"

# The Resource Monitor extension is the program's mandatory core: install.sh
# installs it unconditionally (install_resource_monitor_core) before this
# component selection runs, so the taskbar indicator works no matter which
# components the user picks. The refresh row exposes core configuration; the
# remaining entries are optional tweaks that layer on top of the core.
ISC_COMPONENTS=(
  "rm_refresh_interval|Resource Monitor update time|on|apply_resource_monitor_refresh_interval|||||configure_resource_monitor_refresh_interval|resource_monitor_refresh_interval_status"
  "rm_gradient_colors|Resource Monitor gradient indicator colors|on|patch_resource_monitor_gradient_colors"
  "rm_vram|Resource Monitor GPU VRAM display|on|patch_resource_monitor_vram|detect_rm_vram|uninstall_rm_vram"
  "rm_per_disk|Resource Monitor per-disk display|on|patch_resource_monitor_per_disk"
  "window_rules|App window-rules extension (Wayland)|on|monitor_configure_window_rules|detect_window_rules|uninstall_window_rules"
)
# Note: this repo owns only the Resource Monitor system-status extension and its
# window-rules helper. Features that are desktop-wide behavior were moved to their
# owning repositories to avoid cross-repo duplication: auto-move-windows placement
# -> linux_workspaces_setup, Chrome PWA icons/desktop entries and the Dash-to-Panel
# layout -> linux_configuration_setup.

# monitor_configure_window_rules - install the window-rules extension, skipping
# it on X11 sessions where it does not apply (matches the original guard).
monitor_configure_window_rules() {
  if [[ "${SESSION_TYPE:-}" =~ ^(x11|xorg)$ ]]; then
    msg "X11 session detected; skipping custom window-rules extension."
    return 0
  fi
  configure_window_rules_extension
}
