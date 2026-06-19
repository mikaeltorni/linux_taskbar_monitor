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

ISC_COMPONENTS=(
  "resource_monitor|Resource Monitor (CPU/RAM/disk/GPU) extension|on|configure_resource_monitor_extension"
  "window_rules|App window-rules extension (Wayland)|on|monitor_configure_window_rules"
  "auto_move_windows|Auto-move-windows workspace placement|on|configure_auto_move_windows"
  "dash_to_panel|Dash-to-Panel and Ubuntu Dock configuration|on|configure_dash_and_switchers"
  "pwa_icons|Chrome PWA icons and desktop entries|on|install_pwa_icons"
)

# monitor_configure_window_rules - install the window-rules extension, skipping
# it on X11 sessions where it does not apply (matches the original guard).
monitor_configure_window_rules() {
  if [[ "${SESSION_TYPE:-}" =~ ^(x11|xorg)$ ]]; then
    msg "X11 session detected; skipping custom window-rules extension."
    return 0
  fi
  configure_window_rules_extension
}
